// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { loadBootState } from "./bootstrap";

const storeData = new Map<string, unknown>();

vi.mock("@tauri-apps/plugin-store", () => {
  class LazyStore {
    get(key: string): Promise<unknown> {
      return Promise.resolve(storeData.get(key));
    }
    set(key: string, value: unknown): Promise<void> {
      storeData.set(key, value);
      return Promise.resolve();
    }
    save(): Promise<void> {
      return Promise.resolve();
    }
  }
  return { LazyStore };
});

const LEGACY_PRESETS = {
  version: 2,
  instructions: [
    {
      id: "custom",
      name: "Custom",
      beforeText: "Rewrite warmly",
      afterText: "",
      color: "rose",
    },
  ],
};

describe("bootstrap", () => {
  beforeEach(() => {
    storeData.clear();
    localStorage.clear();
  });

  it("falls back to defaults when nothing is persisted anywhere", async () => {
    const boot = await loadBootState();

    expect(boot.instructions.length).toBeGreaterThan(0);
    expect(boot.instructions[0].id).toBe("clearer");
    expect(boot.provider).toBe("chatgpt");
    expect(["light", "dark"]).toContain(boot.theme);
    expect(boot.selectedId).toBeUndefined();
  });

  it("loads durable settings when present", async () => {
    storeData.set("presets", LEGACY_PRESETS);
    storeData.set("selectedInstructionId", "custom");
    storeData.set("theme", "dark");
    storeData.set("provider", "gemini");

    const boot = await loadBootState();

    expect(boot.instructions).toEqual(LEGACY_PRESETS.instructions);
    expect(boot.selectedId).toBe("custom");
    expect(boot.theme).toBe("dark");
    expect(boot.provider).toBe("gemini");
  });

  it("migrates legacy localStorage settings into the durable store once", async () => {
    localStorage.setItem("prompter.presets.v1", JSON.stringify(LEGACY_PRESETS));
    localStorage.setItem("prompter.selection.v1", "custom");
    localStorage.setItem("prompter.theme.v1", "dark");
    localStorage.setItem("prompter.provider.v1", "gemini");

    const boot = await loadBootState();

    expect(boot.instructions).toEqual(LEGACY_PRESETS.instructions);
    expect(boot.selectedId).toBe("custom");
    expect(boot.theme).toBe("dark");
    expect(boot.provider).toBe("gemini");

    await vi.waitFor(() => {
      expect(storeData.get("presets")).toEqual({
        version: 2,
        instructions: LEGACY_PRESETS.instructions,
      });
      expect(storeData.get("selectedInstructionId")).toBe("custom");
      expect(storeData.get("theme")).toBe("dark");
      expect(storeData.get("provider")).toBe("gemini");
    });
  });

  it("ignores corrupted legacy payloads and invalid durable values", async () => {
    localStorage.setItem("prompter.presets.v1", "{not json");
    storeData.set("theme", "neon");
    storeData.set("provider", "copilot");

    const boot = await loadBootState();

    expect(boot.instructions[0].id).toBe("clearer");
    expect(["light", "dark"]).toContain(boot.theme);
    expect(boot.provider).toBe("chatgpt");
  });
});
