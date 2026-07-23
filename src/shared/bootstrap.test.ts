// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useNoticeStore } from "./notices";
import { loadBootState } from "./bootstrap";

const native = vi.hoisted(() => ({
  storeData: new Map<string, unknown>(),
  failLoad: false,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(
    (command: string): Promise<unknown> => {
      if (command === "load_settings") {
        return native.failLoad
          ? Promise.reject(new Error("load failed"))
          : Promise.resolve({
              version: 1,
              sessionId: 11,
              entries: Object.fromEntries(native.storeData),
            });
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

describe("bootstrap", () => {
  beforeEach(() => {
    native.storeData.clear();
    native.failLoad = false;
    useNoticeStore.setState({
      notice: { id: 0, kind: "info", message: "Ready" },
    });
  });

  it("falls back to defaults when nothing is persisted", async () => {
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

  it("surfaces durable load failures gracefully", async () => {
    native.failLoad = true;

    const boot = await loadBootState();

    expect(boot.instructions[0].id).toBe("clearer");
    expect(boot.provider).toBe("chatgpt");
    expect(useNoticeStore.getState().notice.message).toContain(
      "could not load saved settings",
    );
  });

  it("ignores a selected instruction ID that does not match any preset", async () => {
    native.storeData.set("presets", CUSTOM_PRESETS);
    native.storeData.set("selectedInstructionId", "nonexistent-id");

    const boot = await loadBootState();

    expect(boot.instructions).toEqual(CUSTOM_PRESETS.instructions);
    expect(boot.selectedId).toBeUndefined();
  });

  it("uses system theme when no theme is persisted", async () => {
    const boot = await loadBootState();

    expect(["light", "dark"]).toContain(boot.theme);
  });

  it("falls back to chatgpt when an invalid provider is persisted", async () => {
    native.storeData.set("provider", "invalid-provider");

    const boot = await loadBootState();

    expect(boot.provider).toBe("chatgpt");
  });
});
