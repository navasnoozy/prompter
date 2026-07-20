import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  normalizeProviderError,
  providerGateway,
  TAURI_COMMANDS,
} from "./gateway";
import type { PromptComposition } from "./model";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

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
});
