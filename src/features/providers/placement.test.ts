import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useNoticeStore } from "../../shared/notices";
import { initializeInstructionStore } from "../instructions/store";
import { useCaptureStore } from "../quickCapture/store";
import { providerGateway } from "./gateway";
import type { PromptFilledEvent, ProviderErrorEvent } from "./model";
import {
  bindPlacementEvents,
  cancelCurrentPlacement,
  placeCurrentPrompt,
  placePrompt,
  registerEnsureProvider,
} from "./placement";
import {
  initializeProviderStore,
  useProviderStore,
} from "./store";

vi.mock("./gateway", async (importOriginal) => {
  const original = await importOriginal<typeof import("./gateway")>();
  return {
    ...original,
    providerGateway: {
      show: vi.fn(),
      resize: vi.fn(),
      setVisibility: vi.fn(),
      placePrompt: vi.fn(),
      openNewChat: vi.fn(),
      onPromptFilled: vi.fn(),
      onProviderError: vi.fn(),
    },
  };
});

const COMPOSITION = {
  beforeText: "Rewrite clearly",
  text: "Original text",
  afterText: "",
};

let filledHandler: (event: PromptFilledEvent) => void;
let errorHandler: (event: ProviderErrorEvent) => void;
let cleanup: (() => void) | undefined;

function currentNotice() {
  return useNoticeStore.getState().notice;
}

function isPlacing() {
  return useProviderStore.getState().isPlacing;
}

function placementCalls() {
  return vi.mocked(providerGateway.placePrompt).mock.calls;
}

function lastRequestId(): string {
  const calls = placementCalls();
  return calls[calls.length - 1][2];
}

function lastNewChatArgument() {
  const calls = placementCalls();
  return calls[calls.length - 1][3];
}

/** Lets the placement chain reach its next native call. */
async function settleMicrotasks(): Promise<void> {
  for (let index = 0; index < 8; index += 1) await Promise.resolve();
}

function confirmFill(): void {
  filledHandler({
    version: 1,
    provider: "chatgpt",
    requestId: lastRequestId(),
  });
}

function failWith(code: ProviderErrorEvent["code"], message: string): void {
  errorHandler({
    version: 1,
    provider: "chatgpt",
    requestId: lastRequestId(),
    code,
    message,
  });
}

function setNavigationLoading(isLoading: boolean): void {
  const previous =
    useProviderStore.getState().navigationByProvider.chatgpt.revision;
  useProviderStore.getState().updateNavigationState({
    version: 1,
    provider: "chatgpt",
    generation: 1,
    revision: previous + 1,
    available: true,
    canGoBack: false,
    canGoForward: false,
    isLoading,
  });
}

describe("placement machine", () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    vi.mocked(providerGateway.onPromptFilled).mockImplementation((handler) => {
      filledHandler = handler;
      return Promise.resolve(() => {});
    });
    vi.mocked(providerGateway.onProviderError).mockImplementation((handler) => {
      errorHandler = handler;
      return Promise.resolve(() => {});
    });
    vi.mocked(providerGateway.placePrompt).mockResolvedValue(undefined);
    vi.mocked(providerGateway.openNewChat).mockResolvedValue(undefined);
    initializeProviderStore("chatgpt", "off", {});
    useProviderStore.setState({
      isPlacing: false,
      placementBridgeReady: false,
    });
    useNoticeStore.setState({
      notice: { id: 0, kind: "info", message: "Ready" },
    });
    registerEnsureProvider(() => Promise.resolve());
    cleanup = bindPlacementEvents();
    for (let index = 0; index < 5; index += 1) await Promise.resolve();
  });

  afterEach(() => {
    cleanup?.();
    registerEnsureProvider(null);
    vi.useRealTimers();
  });

  it("rejects empty text without contacting the provider", async () => {
    await placePrompt({ ...COMPOSITION, text: "   " });

    expect(currentNotice().message).toBe("Add or capture some text first");
    expect(providerGateway.placePrompt).not.toHaveBeenCalled();
    expect(isPlacing()).toBe(false);
  });

  it("rejects a composition whose UTF-8 output exceeds the native cap", async () => {
    await placePrompt({ ...COMPOSITION, text: "x".repeat(1_048_576) });

    expect(currentNotice()).toMatchObject({
      kind: "error",
      message: expect.stringContaining("too large"),
    });
    expect(providerGateway.placePrompt).not.toHaveBeenCalled();
  });

  it("waits for native provider navigation to finish", async () => {
    setNavigationLoading(true);

    await placePrompt(COMPOSITION);

    expect(isPlacing()).toBe(false);
    expect(currentNotice()).toMatchObject({
      kind: "info",
      message: expect.stringContaining("finish loading"),
    });
    expect(providerGateway.placePrompt).not.toHaveBeenCalled();
  });

  it("completes the request when the provider confirms the fill", async () => {
    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();
    expect(isPlacing()).toBe(true);
    expect(currentNotice().kind).toBe("progress");

    confirmFill();
    await placed;

    expect(isPlacing()).toBe(false);
    expect(currentNotice().kind).toBe("success");
    expect(currentNotice().message).toContain("review it and press Send");
  });

  it("ignores confirmations for a different request", async () => {
    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();

    filledHandler({
      version: 1,
      provider: "chatgpt",
      requestId: "stale-request",
    });
    await settleMicrotasks();

    expect(isPlacing()).toBe(true);
    cancelCurrentPlacement();
    await placed;
  });

  it("surfaces provider error events for the active request", async () => {
    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();

    failWith("editor_not_found", "The ChatGPT input box was not found.");
    await placed;

    expect(isPlacing()).toBe(false);
    expect(currentNotice()).toMatchObject({
      kind: "error",
      message: "The ChatGPT input box was not found.",
    });
  });

  it("times out when the provider never confirms", async () => {
    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();

    await vi.advanceTimersByTimeAsync(12_000);
    await placed;

    expect(isPlacing()).toBe(false);
    expect(currentNotice().message).toContain("did not confirm");
  });

  it("reports structured native command failures", async () => {
    vi.mocked(providerGateway.placePrompt).mockRejectedValue({
      version: 1,
      code: "wrong_host",
      message: "ChatGPT is showing a sign-in or external page.",
    });

    await placePrompt(COMPOSITION);

    expect(isPlacing()).toBe(false);
    expect(currentNotice()).toMatchObject({
      kind: "error",
      message: "ChatGPT is showing a sign-in or external page.",
    });
  });

  it("cancels the pending request when the provider switches", async () => {
    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();
    expect(isPlacing()).toBe(true);

    useProviderStore.setState({ provider: "gemini" });
    await placed;

    expect(isPlacing()).toBe(false);
  });

  it("composes the current prompt from the instruction and capture stores", async () => {
    initializeInstructionStore(
      [
        {
          id: "tone",
          name: "Tone",
          beforeText: "Make it warm",
          afterText: "Return only the text",
          color: "violet",
        },
      ],
      "tone",
    );
    useCaptureStore.setState({ sourceText: "Captured text" });

    const placed = placeCurrentPrompt();
    await settleMicrotasks();
    confirmFill();
    await placed;

    expect(providerGateway.placePrompt).toHaveBeenCalledWith(
      "chatgpt",
      {
        beforeText: "Make it warm",
        text: "Captured text",
        afterText: "Return only the text",
      },
      expect.any(String),
      undefined,
    );
  });

  it("disables placement and reports listener registration failures", async () => {
    cleanup?.();
    vi.mocked(providerGateway.onPromptFilled).mockRejectedValueOnce(
      new Error("event bridge unavailable"),
    );
    cleanup = bindPlacementEvents();

    await vi.waitFor(() => {
      expect(useProviderStore.getState().placementBridgeReady).toBe(false);
      expect(currentNotice()).toMatchObject({
        kind: "error",
        message: expect.stringContaining("completion events"),
      });
    });
  });
});

describe("starting a new chat before placing", () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    vi.mocked(providerGateway.onPromptFilled).mockImplementation((handler) => {
      filledHandler = handler;
      return Promise.resolve(() => {});
    });
    vi.mocked(providerGateway.onProviderError).mockImplementation((handler) => {
      errorHandler = handler;
      return Promise.resolve(() => {});
    });
    vi.mocked(providerGateway.placePrompt).mockResolvedValue(undefined);
    vi.mocked(providerGateway.openNewChat).mockResolvedValue(undefined);
    initializeProviderStore("chatgpt", "auto", {});
    useProviderStore.setState({
      isPlacing: false,
      placementBridgeReady: false,
    });
    useNoticeStore.setState({
      notice: { id: 0, kind: "info", message: "Ready" },
    });
    registerEnsureProvider(() => Promise.resolve());
    cleanup = bindPlacementEvents();
    for (let index = 0; index < 5; index += 1) await Promise.resolve();
  });

  afterEach(() => {
    cleanup?.();
    registerEnsureProvider(null);
    vi.useRealTimers();
  });

  it("asks the page to press its own New chat control first", async () => {
    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();

    expect(lastNewChatArgument()).toEqual({});
    expect(providerGateway.openNewChat).not.toHaveBeenCalled();

    confirmFill();
    await placed;
    expect(currentNotice().kind).toBe("success");
  });

  it("passes a saved button description through to the page", async () => {
    useProviderStore.setState({
      newChatOverrides: {
        chatgpt: { matcher: { testId: "create-new-chat-button" } },
      },
    });

    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();

    expect(lastNewChatArgument()).toEqual({
      matcher: { testId: "create-new-chat-button" },
    });

    confirmFill();
    await placed;
  });

  it("loads the new chat address when the page control cannot be found", async () => {
    useProviderStore.setState({
      newChatOverrides: { chatgpt: { url: "https://chatgpt.com/new" } },
    });

    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();
    failWith("new_chat_unavailable", "Prompter could not start a new chat.");
    await settleMicrotasks();

    expect(providerGateway.openNewChat).toHaveBeenCalledWith(
      "chatgpt",
      "https://chatgpt.com/new",
    );

    // The reload has to complete before the prompt is allowed to land.
    expect(placementCalls()).toHaveLength(1);
    setNavigationLoading(true);
    await settleMicrotasks();
    setNavigationLoading(false);
    await settleMicrotasks();
    expect(placementCalls()).toHaveLength(2);

    confirmFill();
    await placed;
    expect(currentNotice().kind).toBe("success");
  });

  it("continues when a reset never reports itself as loading", async () => {
    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();
    failWith("new_chat_unavailable", "Prompter could not start a new chat.");
    await settleMicrotasks();

    expect(placementCalls()).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1_600);
    await settleMicrotasks();
    expect(placementCalls()).toHaveLength(2);

    confirmFill();
    await placed;
  });

  it("places into the open conversation rather than losing the prompt", async () => {
    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();
    failWith("new_chat_unavailable", "Prompter could not start a new chat.");
    await vi.advanceTimersByTimeAsync(1_600);
    await settleMicrotasks();

    failWith("new_chat_unavailable", "Prompter could not start a new chat.");
    await settleMicrotasks();

    expect(currentNotice()).toMatchObject({
      kind: "info",
      message: expect.stringContaining("going into the one that is open"),
    });
    expect(placementCalls()).toHaveLength(3);
    expect(lastNewChatArgument()).toBeUndefined();

    confirmFill();
    await placed;
    expect(currentNotice().kind).toBe("success");
  });

  it("never reloads the page in button-only mode", async () => {
    useProviderStore.setState({ newChatMode: "button" });

    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();
    failWith("new_chat_unavailable", "Prompter could not start a new chat.");
    await settleMicrotasks();

    expect(providerGateway.openNewChat).not.toHaveBeenCalled();
    expect(placementCalls()).toHaveLength(2);
    expect(lastNewChatArgument()).toBeUndefined();

    confirmFill();
    await placed;
  });

  it("loads the address without clicking first in address-only mode", async () => {
    useProviderStore.setState({ newChatMode: "url" });

    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();

    expect(providerGateway.openNewChat).toHaveBeenCalledWith(
      "chatgpt",
      undefined,
    );
    expect(placementCalls()).toHaveLength(0);

    await vi.advanceTimersByTimeAsync(1_600);
    await settleMicrotasks();
    expect(placementCalls()).toHaveLength(1);

    confirmFill();
    await placed;
  });

  it("still places the prompt when the reset command itself fails", async () => {
    useProviderStore.setState({ newChatMode: "url" });
    vi.mocked(providerGateway.openNewChat).mockRejectedValue({
      version: 1,
      code: "invalid_request",
      message: "The New chat address must start with https.",
    });

    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();

    expect(placementCalls()).toHaveLength(1);
    expect(lastNewChatArgument()).toBeUndefined();

    confirmFill();
    await placed;
    expect(currentNotice().kind).toBe("success");
  });

  it("leaves the conversation alone when the reset is switched off", async () => {
    useProviderStore.setState({ newChatMode: "off" });

    const placed = placePrompt(COMPOSITION);
    await settleMicrotasks();

    expect(providerGateway.openNewChat).not.toHaveBeenCalled();
    expect(lastNewChatArgument()).toBeUndefined();

    confirmFill();
    await placed;
  });
});
