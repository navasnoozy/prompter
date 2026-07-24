import { beforeEach, describe, expect, it, vi } from "vitest";
import { settingsGateway, SETTINGS_KEYS } from "./settingsGateway";

const native = vi.hoisted(() => ({
  loadValue: { version: 1, sessionId: 7, entries: {} } as unknown,
  saveCalls: [] as Array<{
    entries: Record<string, unknown>;
    sessionId: number;
    revision: number;
  }>,
  resolvers: [] as Array<() => void>,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(
    (
      command: string,
      argumentsValue?: {
        entries?: Record<string, unknown>;
        sessionId?: number;
        revision?: number;
      },
    ) => {
      if (command === "load_settings") return Promise.resolve(native.loadValue);
      if (command === "save_settings") {
        native.saveCalls.push({
          entries: argumentsValue?.entries ?? {},
          sessionId: argumentsValue?.sessionId ?? 0,
          revision: argumentsValue?.revision ?? 0,
        });
        return new Promise<void>((resolve) => native.resolvers.push(resolve));
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    },
  ),
}));

describe("settings gateway", () => {
  beforeEach(() => {
    native.loadValue = { version: 1, sessionId: 7, entries: {} };
    native.saveCalls.length = 0;
    native.resolvers.length = 0;
  });

  it("returns only the fixed settings schema", async () => {
    native.loadValue = {
      version: 1,
      sessionId: 7,
      entries: {
        theme: "dark",
        provider: "gemini",
        unexpected: "not exposed",
      },
    };

    await expect(settingsGateway.load()).resolves.toEqual({
      theme: "dark",
      provider: "gemini",
    });
  });

  it("dispatches writes immediately with ordered native revisions", async () => {
    await settingsGateway.load();
    const first = settingsGateway.writeMany({
      [SETTINGS_KEYS.presets]: { version: 2, instructions: [] },
      [SETTINGS_KEYS.selectedInstructionId]: "old",
    });
    expect(native.saveCalls).toHaveLength(1);

    const second = settingsGateway.write(
      SETTINGS_KEYS.selectedInstructionId,
      "new",
    );
    expect(native.saveCalls).toHaveLength(2);
    expect(native.saveCalls[0]).toMatchObject({ sessionId: 7, revision: 1 });
    expect(native.saveCalls[1]).toMatchObject({
      entries: { selectedInstructionId: "new" },
      sessionId: 7,
      revision: 2,
    });

    native.resolvers.pop()?.();
    await expect(second).resolves.toBe(true);
    native.resolvers.pop()?.();
    await expect(first).resolves.toBe(true);
  });
});
