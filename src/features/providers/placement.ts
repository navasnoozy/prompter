import { publishNotice } from "../../shared/notices";
import { createId } from "../../shared/ids";
import { useCaptureStore } from "../quickCapture/store";
import { selectedInstructionOf, useInstructionStore } from "../instructions/store";
import { normalizeProviderError, providerGateway } from "./gateway";
import {
  getProviderLabel,
  type PromptComposition,
  type Provider,
  type ProviderErrorCode,
} from "./model";
import {
  newChatMatcherFor,
  newChatUrlFor,
  type NewChatMatcher,
  type NewChatMode,
} from "./newChat";
import { useProviderStore } from "./store";
import { isPromptTooLarge } from "./prompt";

const REQUEST_TIMEOUT_MS = 12_000;
/**
 * A placement that first resets the conversation waits on the page twice: for
 * the New chat control to take effect, and then for the editor to remount. The
 * in-page budgets for those add up to roughly thirteen seconds, so the plain
 * timeout would fire on a slow-but-working reset.
 */
const RESET_REQUEST_TIMEOUT_MS = 22_000;
/**
 * How long a navigation is given to begin before it is assumed to have been a
 * no-op — asking for an address the pane already shows does not always reload.
 */
const NAVIGATION_START_GRACE_MS = 1_500;
const NAVIGATION_IDLE_TIMEOUT_MS = 25_000;

type PlacementOutcome =
  | { kind: "filled" }
  | { kind: "error"; code: ProviderErrorCode; message: string }
  | { kind: "cancelled" };

type PendingRequest = {
  provider: Provider;
  requestId: string;
  timeout?: ReturnType<typeof setTimeout>;
  settle: (outcome: PlacementOutcome) => void;
};

// Module-level request machine: exactly one placement can be in flight, and
// bridge events must match its provider and request id to have any effect.
let pending: PendingRequest | null = null;
let ensureProvider: (() => Promise<void>) | null = null;
let bridgeBindingGeneration = 0;
/**
 * Bumped by every placement request. A chain that has been superseded keeps
 * unwinding through its own awaits, so each step re-checks that it still owns
 * the machine before starting native work or touching shared UI state.
 */
let placementGeneration = 0;

export function registerEnsureProvider(
  ensure: (() => Promise<void>) | null,
): void {
  ensureProvider = ensure;
}

function setPlacing(isPlacing: boolean): void {
  useProviderStore.setState({ isPlacing });
}

function resolvePending(
  requestId: string | undefined,
  outcome: PlacementOutcome,
): boolean {
  if (!pending || (requestId && pending.requestId !== requestId)) return false;

  const { settle } = pending;
  if (pending.timeout !== undefined) clearTimeout(pending.timeout);
  pending = null;
  settle(outcome);
  return true;
}

function clearPending(requestId?: string): boolean {
  const resolved = resolvePending(requestId, { kind: "cancelled" });
  if (resolved) setPlacing(false);
  return resolved;
}

export function cancelCurrentPlacement(): void {
  clearPending();
}

export function placementErrorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : normalizeProviderError(error).message;
}

/**
 * Waits until the pane has finished whatever navigation the caller just asked
 * for. Placement is refused natively while a load is in flight, so this is what
 * keeps the address-based reset from racing the fill that follows it.
 */
function waitForNavigationIdle(provider: Provider): Promise<void> {
  return new Promise((resolve) => {
    let started =
      useProviderStore.getState().navigationByProvider[provider].isLoading;
    let settled = false;

    const finish = () => {
      if (settled) return;
      settled = true;
      clearTimeout(graceTimer);
      clearTimeout(hardTimer);
      unsubscribe();
      resolve();
    };

    const unsubscribe = useProviderStore.subscribe((state) => {
      if (state.navigationByProvider[provider].isLoading) {
        started = true;
        return;
      }
      if (started) finish();
    });

    const graceTimer = setTimeout(() => {
      if (!started) finish();
    }, NAVIGATION_START_GRACE_MS);
    // A page that never reports itself idle must not hold the prompt hostage;
    // the placement that follows reports the real reason if it is still busy.
    const hardTimer = setTimeout(finish, NAVIGATION_IDLE_TIMEOUT_MS);
  });
}

/**
 * Runs one placement and resolves once the page has answered. Every attempt
 * gets its own request id, so a late signal from an abandoned attempt can never
 * be mistaken for the answer to the current one.
 */
async function runPlacement(
  provider: Provider,
  composition: PromptComposition,
  reset: { matcher: NewChatMatcher | undefined } | null,
): Promise<PlacementOutcome> {
  const requestId = createId();
  const answer = new Promise<PlacementOutcome>((resolve) => {
    const timeout = setTimeout(
      () => {
        resolvePending(requestId, {
          kind: "error",
          code: "internal",
          message: `${getProviderLabel(provider)} did not confirm the prompt was placed. Try again.`,
        });
      },
      reset ? RESET_REQUEST_TIMEOUT_MS : REQUEST_TIMEOUT_MS,
    );
    pending = { provider, requestId, timeout, settle: resolve };
  });

  try {
    await providerGateway.placePrompt(
      provider,
      composition,
      requestId,
      reset ? { ...(reset.matcher ? { matcher: reset.matcher } : {}) } : undefined,
    );
  } catch (error) {
    const normalized = normalizeProviderError(error);
    resolvePending(requestId, {
      kind: "error",
      code: normalized.code,
      message: placementErrorMessage(error),
    });
  }

  return answer;
}

type ResetPlan = {
  /** Whether the injected script should press the page's New chat control. */
  clickInPage: boolean;
  /** Whether a failed click may fall back to loading the new chat address. */
  mayNavigate: boolean;
};

function resetPlanFor(mode: NewChatMode): ResetPlan | null {
  switch (mode) {
    case "auto":
      return { clickInPage: true, mayNavigate: true };
    case "button":
      return { clickInPage: true, mayNavigate: false };
    case "url":
      return { clickInPage: false, mayNavigate: true };
    case "off":
      return null;
  }
}

export async function placePrompt(
  composition: PromptComposition,
): Promise<void> {
  const provider = useProviderStore.getState().provider;
  if (!composition.text.trim()) {
    publishNotice("info", "Add or capture some text first");
    return;
  }
  if (isPromptTooLarge(composition)) {
    publishNotice(
      "error",
      "The prompt is too large to place. Shorten the text or instruction and try again.",
    );
    return;
  }
  if (!useProviderStore.getState().placementBridgeReady) {
    publishNotice(
      "error",
      "The provider connection is still starting. Wait a moment, then try again.",
    );
    return;
  }
  if (useProviderStore.getState().navigationByProvider[provider].isLoading) {
    publishNotice(
      "info",
      `Wait for ${getProviderLabel(provider)} to finish loading`,
    );
    return;
  }

  clearPending();
  const generation = ++placementGeneration;
  const isActive = () =>
    placementGeneration === generation &&
    useProviderStore.getState().provider === provider;
  setPlacing(true);

  try {
    if (!ensureProvider) {
      throw new Error("The embedded browser area is not ready yet.");
    }
    await ensureProvider();
    if (!isActive()) return;

    await runPlacementPlan(provider, composition, isActive);
  } catch (error) {
    if (isActive()) publishNotice("error", placementErrorMessage(error));
  } finally {
    // A superseded chain must not clear the spinner the newer one just raised.
    if (placementGeneration === generation) setPlacing(false);
  }
}

/**
 * Drives the reset mechanisms in order of cost and stops at the first one that
 * works. Whatever happens, the last step places the prompt into the
 * conversation that is open: a reset that cannot be performed is a worse
 * outcome than an unreset chat, but losing the user's prompt is worse than
 * both.
 */
async function runPlacementPlan(
  provider: Provider,
  composition: PromptComposition,
  isActive: () => boolean,
): Promise<void> {
  const label = getProviderLabel(provider);
  const { newChatMode, newChatOverrides } = useProviderStore.getState();
  const plan = resetPlanFor(newChatMode);
  const matcher = newChatMatcherFor(provider, newChatOverrides);

  if (plan === null) {
    publishNotice("progress", `Placing the prompt in ${label}…`);
    report(await runPlacement(provider, composition, null), label);
    return;
  }

  if (plan.clickInPage) {
    publishNotice("progress", `Starting a new ${label} chat…`);
    const outcome = await runPlacement(provider, composition, { matcher });
    if (!isActive()) return;
    if (
      outcome.kind !== "error" ||
      outcome.code !== "new_chat_unavailable"
    ) {
      report(outcome, label);
      return;
    }
    if (!plan.mayNavigate) {
      await placeIntoCurrentChat(provider, composition, label, isActive);
      return;
    }
  }

  publishNotice("progress", `Opening a new ${label} chat…`);
  try {
    await providerGateway.openNewChat(
      provider,
      newChatUrlFor(provider, newChatOverrides),
    );
  } catch (error) {
    if (!isActive()) return;
    publishNotice("info", placementErrorMessage(error));
    await placeIntoCurrentChat(provider, composition, label, isActive);
    return;
  }

  await waitForNavigationIdle(provider);
  if (!isActive()) return;

  // The reset step is kept on this attempt too. A reloaded page normally
  // reports itself blank and the script returns immediately, but an address
  // that has quietly gone stale lands on an old conversation — and there the
  // in-page control is the only thing left that can still start a new one.
  const outcome = await runPlacement(provider, composition, { matcher });
  if (!isActive()) return;
  if (outcome.kind === "error" && outcome.code === "new_chat_unavailable") {
    await placeIntoCurrentChat(provider, composition, label, isActive);
    return;
  }
  report(outcome, label);
}

async function placeIntoCurrentChat(
  provider: Provider,
  composition: PromptComposition,
  label: string,
  isActive: () => boolean,
): Promise<void> {
  publishNotice(
    "info",
    `Prompter could not start a new ${label} chat, so the prompt is going into the one that is open. You can set a New chat address in Settings.`,
  );
  const outcome = await runPlacement(provider, composition, null);
  if (isActive()) report(outcome, label);
}

function report(outcome: PlacementOutcome, label: string): void {
  if (outcome.kind === "filled") {
    publishNotice(
      "success",
      `Prompt ready in ${label} — review it and press Send`,
    );
    return;
  }
  if (outcome.kind === "error") publishNotice("error", outcome.message);
}

// Assembles the current composition from the instruction and capture stores.
// Shared by the dock button and the ⌘⏎ shortcut.
export function placeCurrentPrompt(): Promise<void> {
  const instruction = selectedInstructionOf(useInstructionStore.getState());
  return placePrompt({
    beforeText: instruction.beforeText,
    text: useCaptureStore.getState().sourceText,
    afterText: instruction.afterText,
  });
}

/**
 * Opens a blank conversation without placing anything, for the toolbar button.
 */
export async function openNewChat(): Promise<void> {
  const { provider, newChatOverrides } = useProviderStore.getState();
  try {
    await providerGateway.openNewChat(
      provider,
      newChatUrlFor(provider, newChatOverrides),
    );
  } catch (error) {
    publishNotice("error", placementErrorMessage(error));
  }
}

// Mounts the bridge event listeners and the provider-switch canceller.
// Returns a cleanup function; call once from the composition root.
export function bindPlacementEvents(): () => void {
  const bindingGeneration = ++bridgeBindingGeneration;
  let disposed = false;
  useProviderStore.setState({ placementBridgeReady: false });

  const unlistenFilled = Promise.resolve().then(() =>
    providerGateway.onPromptFilled((event) => {
      if (
        !pending ||
        useProviderStore.getState().provider !== event.provider ||
        pending.provider !== event.provider
      ) {
        return;
      }
      resolvePending(event.requestId, { kind: "filled" });
    }),
  );

  const unlistenError = Promise.resolve().then(() =>
    providerGateway.onProviderError((event) => {
      if (
        !pending ||
        useProviderStore.getState().provider !== event.provider ||
        pending.provider !== event.provider
      ) {
        return;
      }
      resolvePending(event.requestId, {
        kind: "error",
        code: event.code,
        message: event.message,
      });
    }),
  );

  const unsubscribeProvider = useProviderStore.subscribe((state, previous) => {
    if (
      state.provider !== previous.provider &&
      pending &&
      pending.provider !== state.provider
    ) {
      clearPending();
    }
  });

  const subscriptions = Promise.all([
    unlistenFilled.then(
      (unlisten) => ({ unlisten, error: null }),
      (error: unknown) => ({ unlisten: null, error }),
    ),
    unlistenError.then(
      (unlisten) => ({ unlisten, error: null }),
      (error: unknown) => ({ unlisten: null, error }),
    ),
  ]);

  const stopSubscriptions = (
    results: Array<{ unlisten: (() => void) | null }>,
  ) => {
    for (const result of results) {
      try {
        result.unlisten?.();
      } catch {
        // Listener cleanup is best-effort during WebView teardown.
      }
    }
  };

  void subscriptions.then((results) => {
    if (disposed) {
      stopSubscriptions(results);
      return;
    }
    if (results.some((result) => result.error !== null)) {
      stopSubscriptions(results);
      publishNotice(
        "error",
        "Prompter could not connect to provider completion events. Reload the app and try again.",
      );
      return;
    }
    if (bridgeBindingGeneration === bindingGeneration) {
      useProviderStore.setState({ placementBridgeReady: true });
    }
  });

  return () => {
    disposed = true;
    void subscriptions.then((results) => {
      stopSubscriptions(results);
    });
    unsubscribeProvider();
    clearPending();
    if (bridgeBindingGeneration === bindingGeneration) {
      useProviderStore.setState({ placementBridgeReady: false });
    }
  };
}
