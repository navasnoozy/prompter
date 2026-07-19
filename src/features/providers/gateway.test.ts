import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { providerGateway, TAURI_COMMANDS } from "./gateway";
import type { PromptComposition } from "./model";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

describe("provider prompt composition contract", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("passes named before, text, and after fields to Tauri", async () => {
    const composition: PromptComposition = {
      beforeText: "Rewrite clearly.",
      text: "Original text",
      afterText: "Return only the result.",
    };
    vi.mocked(invoke).mockResolvedValueOnce("Composed prompt");

    await expect(providerGateway.composePrompt(composition)).resolves.toBe(
      "Composed prompt",
    );
    expect(invoke).toHaveBeenCalledWith(
      TAURI_COMMANDS.composePrompt,
      composition,
    );
  });

  it("keeps an omitted after-text instruction explicit as an empty string", async () => {
    const composition: PromptComposition = {
      beforeText: "Fix grammar.",
      text: "Original text",
      afterText: "",
    };
    vi.mocked(invoke).mockResolvedValueOnce("Composed prompt");

    await providerGateway.composePrompt(composition);

    expect(invoke).toHaveBeenCalledWith(TAURI_COMMANDS.composePrompt, {
      beforeText: "Fix grammar.",
      text: "Original text",
      afterText: "",
    });
  });
});
