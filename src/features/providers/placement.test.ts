import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useNoticeStore } from "../../shared/notices";
import { initializeInstructionStore } from "../instructions/store";
import { useCaptureStore } from "../quickCapture/store";
import { providerGateway } from "./gateway";
import type { PromptFilledEvent, ProviderErrorEvent } from "./model";
import {
  bindPlacementEvents,
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

function lastRequestId(): string {
  const calls = vi.mocked(providerGateway.placePrompt).mock.calls;
  return calls[calls.length - 1][2];
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
    initializeProviderStore("chatgpt");
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
    useProviderStore.getState().updateNavigationState({
      version: 1,
      provider: "chatgpt",
      generation: 1,
      revision: 1,
      available: true,
      canGoBack: false,
      canGoForward: false,
      isLoading: true,
    });

    await placePrompt(COMPOSITION);

    expect(isPlacing()).toBe(false);
    expect(currentNotice()).toMatchObject({
      kind: "info",
      message: expect.stringContaining("finish loading"),
    });
    expect(providerGateway.placePrompt).not.toHaveBeenCalled();
  });

  it("completes the request when the provider confirms the fill", async () => {
    await placePrompt(COMPOSITION);
    expect(isPlacing()).toBe(true);
    expect(currentNotice().kind).toBe("progress");

    filledHandler({
      version: 1,
      provider: "chatgpt",
      requestId: lastRequestId(),
    });

    expect(isPlacing()).toBe(false);
    expect(currentNotice().kind).toBe("success");
    expect(currentNotice().message).toContain("review it and press Send");
  });

  it("ignores confirmations for a different request", async () => {
    await placePrompt(COMPOSITION);

    filledHandler({
      version: 1,
      provider: "chatgpt",
      requestId: "stale-request",
    });

    expect(isPlacing()).toBe(true);
  });

  it("surfaces provider error events for the active request", async () => {
    await placePrompt(COMPOSITION);

    errorHandler({
      version: 1,
      provider: "chatgpt",
      requestId: lastRequestId(),
      code: "editor_not_found",
      message: "The ChatGPT input box was not found.",
    });

    expect(isPlacing()).toBe(false);
    expect(currentNotice()).toMatchObject({
      kind: "error",
      message: "The ChatGPT input box was not found.",
    });
  });

  it("times out when the provider never confirms", async () => {
    await placePrompt(COMPOSITION);

    await vi.advanceTimersByTimeAsync(12_000);

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
    await placePrompt(COMPOSITION);
    expect(isPlacing()).toBe(true);

    useProviderStore.setState({ provider: "gemini" });

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

    await placeCurrentPrompt();

    expect(providerGateway.placePrompt).toHaveBeenCalledWith(
      "chatgpt",
      {
        beforeText: "Make it warm",
        text: "Captured text",
        afterText: "Return only the text",
      },
      expect.any(String),
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
