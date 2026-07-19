import { publishNotice } from "../../shared/notices";
import { createId } from "../../shared/ids";
import { useCaptureStore } from "../quickCapture/store";
import { selectedInstructionOf, useInstructionStore } from "../instructions/store";
import { normalizeProviderError, providerGateway } from "./gateway";
import { getProviderLabel, type PromptComposition, type Provider } from "./model";
import { useProviderStore } from "./store";

const REQUEST_TIMEOUT_MS = 12_000;

type PendingRequest = {
  provider: Provider;
  requestId: string;
  timeout?: ReturnType<typeof setTimeout>;
};

// Module-level request machine: exactly one placement can be in flight, and
// bridge events must match its provider and request id to have any effect.
let pending: PendingRequest | null = null;
let ensureProvider: (() => Promise<void>) | null = null;

export function registerEnsureProvider(
  ensure: (() => Promise<void>) | null,
): void {
  ensureProvider = ensure;
}

function setPlacing(isPlacing: boolean): void {
  useProviderStore.setState({ isPlacing });
}

function clearPending(requestId?: string): boolean {
  if (!pending || (requestId && pending.requestId !== requestId)) return false;

  if (pending.timeout !== undefined) clearTimeout(pending.timeout);
  pending = null;
  setPlacing(false);
  return true;
}

export function placementErrorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : normalizeProviderError(error).message;
}

export async function placePrompt(
  composition: PromptComposition,
): Promise<void> {
  const provider = useProviderStore.getState().provider;
  if (!composition.text.trim()) {
    publishNotice("info", "Add or capture some text first");
    return;
  }

  clearPending();
  setPlacing(true);
  const requestId = createId();
  pending = { provider, requestId };

  try {
    if (!ensureProvider) {
      throw new Error("The embedded browser area is not ready yet.");
    }
    await ensureProvider();
    if (
      useProviderStore.getState().provider !== provider ||
      pending?.requestId !== requestId
    ) {
      return;
    }

    const timeout = setTimeout(() => {
      if (useProviderStore.getState().provider !== provider) return;
      if (!clearPending(requestId)) return;
      publishNotice(
        "error",
        `${getProviderLabel(provider)} did not confirm the prompt was placed. Try again.`,
      );
    }, REQUEST_TIMEOUT_MS);
    pending = { provider, requestId, timeout };

    publishNotice(
      "progress",
      `Placing the prompt in ${getProviderLabel(provider)}…`,
    );
    await providerGateway.placePrompt(provider, composition, requestId);
  } catch (error) {
    if (!clearPending(requestId)) return;
    publishNotice("error", placementErrorMessage(error));
  }
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

// Mounts the bridge event listeners and the provider-switch canceller.
// Returns a cleanup function; call once from the composition root.
export function bindPlacementEvents(): () => void {
  const unlistenFilled = providerGateway.onPromptFilled((event) => {
    if (
      !pending ||
      useProviderStore.getState().provider !== event.provider ||
      pending.provider !== event.provider ||
      pending.requestId !== event.requestId
    ) {
      return;
    }
    clearPending(event.requestId);
    publishNotice(
      "success",
      `Prompt ready in ${getProviderLabel(event.provider)} — review it and press Send`,
    );
  });

  const unlistenError = providerGateway.onProviderError((event) => {
    if (
      !pending ||
      useProviderStore.getState().provider !== event.provider ||
      pending.provider !== event.provider ||
      pending.requestId !== event.requestId
    ) {
      return;
    }
    clearPending(event.requestId);
    publishNotice("error", event.message);
  });

  const unsubscribeProvider = useProviderStore.subscribe((state, previous) => {
    if (
      state.provider !== previous.provider &&
      pending &&
      pending.provider !== state.provider
    ) {
      clearPending();
    }
  });

  return () => {
    void unlistenFilled.then((unlisten) => unlisten());
    void unlistenError.then((unlisten) => unlisten());
    unsubscribeProvider();
    clearPending();
  };
}
