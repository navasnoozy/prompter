// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { providerGateway } from "./gateway";
import type { PromptFilledEvent, ProviderErrorEvent } from "./model";
import { usePromptPlacement } from "./usePromptPlacement";

vi.mock("./gateway", () => ({
  providerGateway: {
    composePrompt: vi.fn(),
    fillPrompt: vi.fn(),
    onPromptFilled: vi.fn(),
    onProviderError: vi.fn(),
  },
}));

let filledHandler: (payload: PromptFilledEvent) => void;
let errorHandler: (payload: ProviderErrorEvent) => void;

function renderPlacement(notices: string[]) {
  // Stable references, matching how App passes setNotice; unstable ones
  // would re-run the hook's listener effect and cancel in-flight requests.
  const onNotice = (message: string) => notices.push(message);
  const ensureProvider = () => Promise.resolve();
  return renderHook(() =>
    usePromptPlacement({ provider: "chatgpt", ensureProvider, onNotice }),
  );
}

async function placeDefaultPrompt(
  result: ReturnType<typeof renderPlacement>["result"],
) {
  await act(async () => {
    await result.current.placePrompt({
      beforeText: "Rewrite clearly",
      text: "Original text",
      afterText: "",
    });
  });
}

describe("usePromptPlacement", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(providerGateway.onPromptFilled).mockImplementation((handler) => {
      filledHandler = handler;
      return Promise.resolve(() => {});
    });
    vi.mocked(providerGateway.onProviderError).mockImplementation((handler) => {
      errorHandler = handler;
      return Promise.resolve(() => {});
    });
    vi.mocked(providerGateway.composePrompt).mockResolvedValue("Composed");
    vi.mocked(providerGateway.fillPrompt).mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("rejects empty text without contacting the provider", async () => {
    const notices: string[] = [];
    const { result } = renderPlacement(notices);

    await act(async () => {
      await result.current.placePrompt({
        beforeText: "Rewrite clearly",
        text: "   ",
        afterText: "",
      });
    });

    expect(notices.at(-1)).toBe("Add or capture some text first");
    expect(providerGateway.fillPrompt).not.toHaveBeenCalled();
    expect(result.current.isWorking).toBe(false);
  });

  it("completes the request when the provider confirms the fill", async () => {
    const notices: string[] = [];
    const { result } = renderPlacement(notices);

    await placeDefaultPrompt(result);
    expect(result.current.isWorking).toBe(true);
    const requestId = vi.mocked(providerGateway.fillPrompt).mock
      .calls[0][2] as string;

    await act(async () => {
      filledHandler({ provider: "chatgpt", requestId });
    });

    expect(result.current.isWorking).toBe(false);
    expect(notices.at(-1)).toContain("review it and press Send");
  });

  it("ignores confirmations for a different request", async () => {
    const notices: string[] = [];
    const { result } = renderPlacement(notices);

    await placeDefaultPrompt(result);
    await act(async () => {
      filledHandler({ provider: "chatgpt", requestId: "stale-request" });
    });

    expect(result.current.isWorking).toBe(true);
  });

  it("surfaces provider errors for the active request", async () => {
    const notices: string[] = [];
    const { result } = renderPlacement(notices);

    await placeDefaultPrompt(result);
    const requestId = vi.mocked(providerGateway.fillPrompt).mock
      .calls[0][2] as string;

    await act(async () => {
      errorHandler({
        provider: "chatgpt",
        requestId,
        message: "The ChatGPT input box was not found.",
      });
    });

    expect(result.current.isWorking).toBe(false);
    expect(notices.at(-1)).toBe("The ChatGPT input box was not found.");
  });

  it("times out when the provider never confirms", async () => {
    vi.useFakeTimers();
    const notices: string[] = [];
    const { result } = renderPlacement(notices);

    await placeDefaultPrompt(result);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(12_000);
    });

    expect(result.current.isWorking).toBe(false);
    expect(notices.at(-1)).toContain("did not confirm");
  });

  it("reports failures from the native fill command", async () => {
    vi.mocked(providerGateway.fillPrompt).mockRejectedValue(
      "ChatGPT is showing a sign-in or external page.",
    );
    const notices: string[] = [];
    const { result } = renderPlacement(notices);

    await placeDefaultPrompt(result);

    expect(result.current.isWorking).toBe(false);
    expect(notices.at(-1)).toContain("Could not prepare the prompt");
  });
});
