import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  normalizeProviderError,
  providerGateway,
  TAURI_COMMANDS,
  TAURI_EVENTS,
} from "./gateway";
import type { PromptComposition } from "./model";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

const validNavigation = {
  version: 1 as const,
  provider: "chatgpt" as const,
  generation: 2,
  revision: 4,
  available: true,
  canGoBack: true,
  canGoForward: false,
  isLoading: false,
};

describe("provider gateway", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it("places a prompt through the single merged command", async () => {
    const composition: PromptComposition = {
      beforeText: "Rewrite clearly.",
      text: "Original text",
      afterText: "Return only the result.",
    };
    vi.mocked(invoke).mockResolvedValueOnce(undefined);

    await providerGateway.placePrompt("chatgpt", composition, "request-1");

    expect(invoke).toHaveBeenCalledWith(TAURI_COMMANDS.placePrompt, {
      provider: "chatgpt",
      composition,
      requestId: "request-1",
    });
  });

  it.each(["back", "forward", "reload", "stop"] as const)(
    "sends the %s action with the provider generation",
    async (action) => {
      const acknowledged = {
        ...validNavigation,
        provider: "gemini" as const,
        generation: 17,
      };
      vi.mocked(invoke).mockResolvedValueOnce(acknowledged);

      await expect(
        providerGateway.controlNavigation("gemini", 17, action),
      ).resolves.toEqual(acknowledged);

      expect(invoke).toHaveBeenCalledWith(
        TAURI_COMMANDS.controlProviderNavigation,
        {
          provider: "gemini",
          generation: 17,
          action,
        },
      );
    },
  );

  it("validates navigation snapshots returned by the native layer", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(validNavigation);

    await expect(
      providerGateway.getNavigationState("chatgpt"),
    ).resolves.toEqual(validNavigation);
    expect(invoke).toHaveBeenCalledWith(
      TAURI_COMMANDS.getProviderNavigationState,
      { provider: "chatgpt" },
    );

    vi.mocked(invoke).mockResolvedValueOnce({
      ...validNavigation,
      revision: -1,
    });
    await expect(
      providerGateway.getNavigationState("chatgpt"),
    ).rejects.toThrow("invalid state");
  });

  it("rejects wrong-provider and semantically impossible navigation states", async () => {
    for (const invalid of [
      { ...validNavigation, provider: "gemini" },
      {
        ...validNavigation,
        available: false,
        canGoBack: true,
      },
      {
        ...validNavigation,
        generation: 0,
        revision: 0,
        available: true,
      },
      {
        ...validNavigation,
        generation: 0,
        revision: 2,
      },
    ]) {
      vi.mocked(invoke).mockResolvedValueOnce(invalid);
      await expect(
        providerGateway.getNavigationState("chatgpt"),
      ).rejects.toThrow("invalid state");
    }
  });

  it("keeps an omitted after-text instruction explicit as an empty string", async () => {
    const composition: PromptComposition = {
      beforeText: "Fix grammar.",
      text: "Original text",
      afterText: "",
    };
    vi.mocked(invoke).mockResolvedValueOnce(undefined);

    await providerGateway.placePrompt("gemini", composition, "request-2");

    expect(invoke).toHaveBeenCalledWith(TAURI_COMMANDS.placePrompt, {
      provider: "gemini",
      composition: {
        beforeText: "Fix grammar.",
        text: "Original text",
        afterText: "",
      },
      requestId: "request-2",
    });
  });

  it("passes through structured provider command errors", () => {
    const structured = {
      version: 1,
      code: "wrong_host",
      message: "ChatGPT is showing a sign-in or external page.",
    };

    expect(normalizeProviderError(structured)).toEqual(structured);
  });

  it("normalizes unknown errors to a safe internal fallback", () => {
    for (const malformed of [
      null,
      "raw string error",
      { version: 99, code: "wrong_host", message: "x" },
      { version: 1, code: "unknown_code", message: "x" },
      { version: 1, code: "wrong_host", message: "   " },
    ]) {
      const normalized = normalizeProviderError(malformed);
      expect(normalized.version).toBe(1);
      expect(normalized.code).toBe("internal");
      expect(normalized.message.length).toBeGreaterThan(0);
    }
  });

  it("validates versioned provider completion events", async () => {
    const nativeHandlers: Array<(event: { payload: unknown }) => void> = [];
    vi.mocked(listen).mockImplementation(async (_event, handler) => {
      nativeHandlers.push(handler as (event: { payload: unknown }) => void);
      return () => {};
    });
    const filled = vi.fn();
    const failed = vi.fn();
    await providerGateway.onPromptFilled(filled);
    await providerGateway.onProviderError(failed);

    nativeHandlers[0]({
      payload: { version: 99, provider: "chatgpt", requestId: "request-1" },
    });
    nativeHandlers[0]({
      payload: { version: 1, provider: "chatgpt", requestId: "request-1" },
    });
    nativeHandlers[1]({
      payload: {
        version: 1,
        provider: "chatgpt",
        requestId: "request-1",
        code: "not-a-code",
        message: "bad",
      },
    });
    nativeHandlers[1]({
      payload: {
        version: 1,
        provider: "chatgpt",
        requestId: "request-1",
        code: "editor_not_found",
        message: "Editor not found",
      },
    });

    expect(filled).toHaveBeenCalledTimes(1);
    expect(failed).toHaveBeenCalledWith(
      expect.objectContaining({ code: "editor_not_found" }),
    );
  });

  it("accepts only strict, versioned navigation-state events", async () => {
    let nativeHandler:
      | ((event: { payload: unknown }) => void)
      | undefined;
    const update = vi.fn();
    const unlisten = vi.fn();
    vi.mocked(listen).mockImplementationOnce(
      async (eventName, handler) => {
        expect(eventName).toBe(TAURI_EVENTS.providerNavigationState);
        nativeHandler = handler as (event: {
          payload: unknown;
        }) => void;
        return unlisten;
      },
    );

    const stop = await providerGateway.onNavigationState(update);
    const valid = {
      version: 1,
      provider: "gemini",
      generation: 9,
      revision: 3,
      available: true,
      canGoBack: true,
      canGoForward: false,
      isLoading: true,
    };
    nativeHandler?.({ payload: { ...valid, version: 2 } });
    nativeHandler?.({ payload: { ...valid, provider: "other" } });
    nativeHandler?.({ payload: { ...valid, canGoBack: "yes" } });
    nativeHandler?.({ payload: { ...valid, url: "https://example.com" } });
    nativeHandler?.({ payload: valid });

    expect(update).toHaveBeenCalledOnce();
    expect(update).toHaveBeenCalledWith(valid);
    stop();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
