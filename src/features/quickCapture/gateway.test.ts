import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  normalizeQuickCaptureError,
  QUICK_CAPTURE_COMMANDS,
  QUICK_CAPTURE_EVENTS,
  quickCaptureGateway,
  QuickCaptureProtocolError,
} from "./gateway";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

const validStatus = {
  version: 1 as const,
  shortcut: {
    accelerator: "CommandOrControl+Shift+P",
    display: "⌘ ⇧ P",
  },
  registration: "registered" as const,
  permission: "granted" as const,
};

describe("Quick Capture gateway", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it("loads and validates backend-owned shortcut status", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(validStatus);

    await expect(quickCaptureGateway.getStatus()).resolves.toEqual(validStatus);
    expect(invoke).toHaveBeenCalledWith(QUICK_CAPTURE_COMMANDS.getStatus);
  });

  it("rejects invalid native responses instead of trusting them", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ version: 99 });

    await expect(quickCaptureGateway.getStatus()).rejects.toBeInstanceOf(
      QuickCaptureProtocolError,
    );
  });

  it("lists every valid pending outcome without destructive acknowledgement", async () => {
    const outcome = {
      kind: "success",
      version: 1,
      requestId: "capture-4",
      text: "Captured",
      warnings: [],
      durationMs: 30,
    };
    vi.mocked(invoke).mockResolvedValueOnce([outcome]);

    await expect(quickCaptureGateway.listPendingOutcomes()).resolves.toEqual([
      outcome,
    ]);
    expect(invoke).toHaveBeenCalledWith(QUICK_CAPTURE_COMMANDS.listOutcomes);
  });

  it("acknowledges only explicitly processed request ids", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined);

    await quickCaptureGateway.acknowledgeOutcomes(["capture-4", "capture-5"]);

    expect(invoke).toHaveBeenCalledWith(
      QUICK_CAPTURE_COMMANDS.acknowledgeOutcomes,
      { requestIds: ["capture-4", "capture-5"] },
    );
  });

  it("subscribes to the exact event and ignores malformed notifications", async () => {
    let nativeHandler: ((event: { payload: unknown }) => void) | undefined;
    const unlisten = vi.fn();
    vi.mocked(listen).mockImplementationOnce(async (_event, handler) => {
      nativeHandler = handler as (event: { payload: unknown }) => void;
      return unlisten;
    });
    const handler = vi.fn();

    await expect(quickCaptureGateway.onReady(handler)).resolves.toBe(unlisten);
    expect(listen).toHaveBeenCalledWith(
      QUICK_CAPTURE_EVENTS.ready,
      expect.any(Function),
    );

    nativeHandler?.({ payload: { version: 2, requestId: "bad" } });
    nativeHandler?.({ payload: { version: 1, requestId: "capture-5" } });
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("shows only a stable safe message for unknown native errors", () => {
    expect(normalizeQuickCaptureError("sensitive internal details")).toEqual({
      version: 1,
      code: "internal",
      message: "Quick Capture could not finish. Please try again.",
    });
  });
});
