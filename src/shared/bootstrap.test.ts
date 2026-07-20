// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useNoticeStore } from "./notices";
import { loadBootState } from "./bootstrap";

const native = vi.hoisted(() => ({
  storeData: new Map<string, unknown>(),
  failLoad: false,
  failSave: false,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(
    (
      command: string,
      argumentsValue?: { entries?: Record<string, unknown> },
    ): Promise<unknown> => {
      if (command === "load_settings") {
        return native.failLoad
          ? Promise.reject(new Error("load failed"))
          : Promise.resolve({
              version: 1,
              sessionId: 11,
              entries: Object.fromEntries(native.storeData),
            });
      }
      if (command === "save_settings") {
        if (native.failSave) return Promise.reject(new Error("save failed"));
        for (const [key, value] of Object.entries(
          argumentsValue?.entries ?? {},
        )) {
          native.storeData.set(key, value);
        }
        return Promise.resolve();
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    },
  ),
}));

const CUSTOM_PRESETS = {
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

function seedLegacySettings(): void {
  localStorage.setItem("prompter.presets.v1", JSON.stringify(CUSTOM_PRESETS));
  localStorage.setItem("prompter.selection.v1", "custom");
  localStorage.setItem("prompter.theme.v1", "dark");
  localStorage.setItem("prompter.provider.v1", "gemini");
}

describe("bootstrap", () => {
  beforeEach(() => {
    native.storeData.clear();
    native.failLoad = false;
    native.failSave = false;
    localStorage.clear();
    useNoticeStore.setState({
      notice: { id: 0, kind: "info", message: "Ready" },
    });
  });

  it("falls back to defaults when nothing is persisted anywhere", async () => {
    const boot = await loadBootState();

    expect(boot.instructions[0].id).toBe("clearer");
    expect(boot.provider).toBe("chatgpt");
    expect(["light", "dark"]).toContain(boot.theme);
    expect(boot.selectedId).toBeUndefined();
  });

  it("loads durable settings when present", async () => {
    native.storeData.set("presets", CUSTOM_PRESETS);
    native.storeData.set("selectedInstructionId", "custom");
    native.storeData.set("theme", "dark");
    native.storeData.set("provider", "gemini");

    const boot = await loadBootState();

    expect(boot.instructions).toEqual(CUSTOM_PRESETS.instructions);
    expect(boot.selectedId).toBe("custom");
    expect(boot.theme).toBe("dark");
    expect(boot.provider).toBe("gemini");
  });

  it("awaits legacy migration and removes each confirmed legacy value", async () => {
    seedLegacySettings();

    const boot = await loadBootState();

    expect(boot.instructions).toEqual(CUSTOM_PRESETS.instructions);
    expect(native.storeData.get("presets")).toEqual(CUSTOM_PRESETS);
    expect(native.storeData.get("selectedInstructionId")).toBe("custom");
    expect(native.storeData.get("theme")).toBe("dark");
    expect(native.storeData.get("provider")).toBe("gemini");
    expect(localStorage.length).toBe(0);
  });

  it("migrates missing keys without overwriting valid durable values", async () => {
    seedLegacySettings();
    native.storeData.set("theme", "light");
    native.storeData.set("provider", "chatgpt");

    const boot = await loadBootState();

    expect(boot.instructions).toEqual(CUSTOM_PRESETS.instructions);
    expect(boot.theme).toBe("light");
    expect(boot.provider).toBe("chatgpt");
    expect(native.storeData.get("theme")).toBe("light");
    expect(native.storeData.get("provider")).toBe("chatgpt");
    expect(localStorage.length).toBe(0);
  });

  it("recovers from invalid durable presets using valid legacy data", async () => {
    native.storeData.set("presets", { version: 2, instructions: [] });
    localStorage.setItem(
      "prompter.presets.v1",
      JSON.stringify(CUSTOM_PRESETS),
    );

    const boot = await loadBootState();

    expect(boot.instructions).toEqual(CUSTOM_PRESETS.instructions);
    expect(native.storeData.get("presets")).toEqual(CUSTOM_PRESETS);
    expect(localStorage.getItem("prompter.presets.v1")).toBeNull();
  });

  it("replaces a stale durable selection with a valid legacy selection", async () => {
    native.storeData.set("presets", CUSTOM_PRESETS);
    native.storeData.set("selectedInstructionId", "deleted-preset");
    localStorage.setItem("prompter.selection.v1", "custom");

    const boot = await loadBootState();

    expect(boot.selectedId).toBe("custom");
    expect(native.storeData.get("selectedInstructionId")).toBe("custom");
    expect(localStorage.getItem("prompter.selection.v1")).toBeNull();
  });

  it("does not overwrite a future-schema selection during legacy fallback", async () => {
    const futurePresets = {
      version: 3,
      instructions: [{ id: "future", futureField: true }],
    };
    native.storeData.set("presets", futurePresets);
    native.storeData.set("selectedInstructionId", "future");
    localStorage.setItem(
      "prompter.presets.v1",
      JSON.stringify(CUSTOM_PRESETS),
    );
    localStorage.setItem("prompter.selection.v1", "custom");

    const boot = await loadBootState();

    expect(boot.instructions).toEqual(CUSTOM_PRESETS.instructions);
    expect(boot.selectedId).toBe("custom");
    expect(native.storeData.get("presets")).toEqual(futurePresets);
    expect(native.storeData.get("selectedInstructionId")).toBe("future");
    expect(localStorage.getItem("prompter.selection.v1")).toBe("custom");
  });

  it("retains legacy data when its durable save fails", async () => {
    seedLegacySettings();
    native.failSave = true;

    const boot = await loadBootState();

    expect(boot.instructions).toEqual(CUSTOM_PRESETS.instructions);
    expect(localStorage.getItem("prompter.presets.v1")).not.toBeNull();
    expect(useNoticeStore.getState().notice.kind).toBe("error");
  });

  it("surfaces durable load failures without overwriting that file", async () => {
    native.failLoad = true;
    seedLegacySettings();

    const boot = await loadBootState();

    expect(boot.instructions).toEqual(CUSTOM_PRESETS.instructions);
    expect(native.storeData.size).toBe(0);
    expect(localStorage.length).toBe(4);
    expect(useNoticeStore.getState().notice.message).toContain(
      "could not load saved settings",
    );
  });
});
