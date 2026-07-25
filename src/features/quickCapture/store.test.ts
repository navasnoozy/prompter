// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useNoticeStore } from "../../shared/notices";
import { quickCaptureGateway } from "./gateway";
import type { QuickCaptureStatus } from "./model";
import { drainPendingOutcomes, useCaptureStore } from "./store";

vi.mock("./gateway", async (importOriginal) => {
  const original = await importOriginal<typeof import("./gateway")>();
  return {
    ...original,
    quickCaptureGateway: {
      getStatus: vi.fn(),
      requestPermission: vi.fn(),
      retryRegistration: vi.fn(),
      setShortcut: vi.fn(),
      resetShortcut: vi.fn(),
      openSystemSettings: vi.fn(),
      readClipboardText: vi.fn(),
      listPendingOutcomes: vi.fn(),
      acknowledgeOutcomes: vi.fn(),
      onReady: vi.fn(),
    },
  };
});

const status = (permission: "granted" | "required"): QuickCaptureStatus => ({
  version: 3,
  shortcut: {
    accelerator: "shift+super+KeyP",
    display: "⇧ ⌘ P",
  },
  registration: "registered",
  permission,
  accessibility: "granted",
});

describe("Quick Capture store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useCaptureStore.setState({
      sourceText: "",
      status: null,
      isRefreshingStatus: false,
      isSavingShortcut: false,
    });
    useNoticeStore.setState({
      notice: { id: 0, kind: "info", message: "Ready" },
    });
  });

  it("ignores a stale status response that finishes last", async () => {
    let resolveFirst: (value: QuickCaptureStatus) => void = () => {};
    let resolveSecond: (value: QuickCaptureStatus) => void = () => {};
    vi.mocked(quickCaptureGateway.getStatus)
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveSecond = resolve;
        }),
      );

    const first = useCaptureStore.getState().refreshStatus();
    const second = useCaptureStore.getState().refreshStatus();
    resolveSecond(status("granted"));
    await second;
    resolveFirst(status("required"));
    await first;

    expect(useCaptureStore.getState()).toMatchObject({
      status: status("granted"),
      isRefreshingStatus: false,
    });
  });

  it("adopts the shortcut the backend confirms, not the one that was requested", async () => {
    const saved: QuickCaptureStatus = {
      ...status("granted"),
      shortcut: { accelerator: "alt+control+KeyJ", display: "⌃ ⌥ J" },
    };
    vi.mocked(quickCaptureGateway.setShortcut).mockResolvedValueOnce(saved);

    await expect(
      useCaptureStore.getState().setShortcut("control+alt+KeyJ"),
    ).resolves.toBe(true);

    expect(useCaptureStore.getState().status).toEqual(saved);
    expect(useNoticeStore.getState().notice).toMatchObject({
      kind: "success",
      message: "Shortcut set to ⌃ ⌥ J",
    });
  });

  it("keeps the previous shortcut visible when the backend rejects a change", async () => {
    // The backend rolls its own registration back, so the UI must not show a
    // combination that is not actually bound.
    const current = status("granted");
    useCaptureStore.setState({ status: current });
    vi.mocked(quickCaptureGateway.setShortcut).mockRejectedValueOnce({
      version: 3,
      code: "shortcut_unavailable",
      message: "macOS or another app is already using that shortcut.",
    });

    await expect(
      useCaptureStore.getState().setShortcut("super+Space"),
    ).resolves.toBe(false);

    expect(useCaptureStore.getState()).toMatchObject({
      status: current,
      isSavingShortcut: false,
    });
    expect(useNoticeStore.getState().notice).toMatchObject({
      kind: "error",
      message: "macOS or another app is already using that shortcut.",
    });
  });

  it("ignores a second shortcut change while one is still in flight", async () => {
    let resolveFirst: (value: QuickCaptureStatus) => void = () => {};
    vi.mocked(quickCaptureGateway.setShortcut).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveFirst = resolve;
      }),
    );

    const first = useCaptureStore.getState().setShortcut("super+KeyJ");
    await expect(
      useCaptureStore.getState().setShortcut("super+KeyK"),
    ).resolves.toBe(false);
    resolveFirst(status("granted"));
    await first;

    expect(quickCaptureGateway.setShortcut).toHaveBeenCalledTimes(1);
  });

  it("does not overwrite newer typed text when clipboard capture finishes", async () => {
    let resolveClipboard: (value: { version: 3; text: string }) => void = () => {};
    vi.mocked(quickCaptureGateway.readClipboardText).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveClipboard = resolve;
      }),
    );

    const capture = useCaptureStore.getState().captureClipboard();
    useCaptureStore.getState().setSourceText("Text entered while capturing");
    resolveClipboard({ version: 3, text: "Older clipboard text" });
    await capture;

    expect(useCaptureStore.getState()).toMatchObject({
      sourceText: "Text entered while capturing",
      isCapturingClipboard: false,
    });
    expect(useNoticeStore.getState().notice.message).toBe("Ready");
  });

  it("retries failed acknowledgements without applying an outcome twice", async () => {
    const outcome = {
      kind: "success" as const,
      version: 3 as const,
      requestId: "capture-81",
      text: "Recovered selection",
      warnings: [],
      durationMs: 12,
    };
    vi.mocked(quickCaptureGateway.listPendingOutcomes).mockResolvedValue([
      outcome,
    ]);
    vi.mocked(quickCaptureGateway.acknowledgeOutcomes)
      .mockRejectedValueOnce(new Error("temporary acknowledgement failure"))
      .mockResolvedValueOnce(undefined);

    await drainPendingOutcomes();
    const failureNoticeId = useNoticeStore.getState().notice.id;
    expect(useCaptureStore.getState().sourceText).toBe("Recovered selection");
    expect(useNoticeStore.getState().notice.kind).toBe("error");

    await drainPendingOutcomes();

    expect(quickCaptureGateway.acknowledgeOutcomes).toHaveBeenCalledTimes(2);
    expect(useNoticeStore.getState().notice.id).toBe(failureNoticeId);
  });
});
