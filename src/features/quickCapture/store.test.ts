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
      openSystemSettings: vi.fn(),
      readClipboardText: vi.fn(),
      listPendingOutcomes: vi.fn(),
      acknowledgeOutcomes: vi.fn(),
      onReady: vi.fn(),
    },
  };
});

const status = (permission: "granted" | "required"): QuickCaptureStatus => ({
  version: 1,
  shortcut: {
    accelerator: "CommandOrControl+Shift+P",
    display: "⌘ ⇧ P",
  },
  registration: "registered",
  permission,
});

describe("Quick Capture store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useCaptureStore.setState({
      sourceText: "",
      status: null,
      isRefreshingStatus: false,
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

  it("does not overwrite newer typed text when clipboard capture finishes", async () => {
    let resolveClipboard: (value: { version: 1; text: string }) => void = () => {};
    vi.mocked(quickCaptureGateway.readClipboardText).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveClipboard = resolve;
      }),
    );

    const capture = useCaptureStore.getState().captureClipboard();
    useCaptureStore.getState().setSourceText("Text entered while capturing");
    resolveClipboard({ version: 1, text: "Older clipboard text" });
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
      version: 1 as const,
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
