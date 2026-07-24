import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { publishNotice, useNoticeStore } from "./notices";

function currentNotice() {
  return useNoticeStore.getState().notice;
}

describe("notice store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useNoticeStore.setState({
      notice: { id: 0, kind: "info", message: "Ready" },
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("auto-clears success notices back to Ready", () => {
    publishNotice("success", "Instruction saved");
    expect(currentNotice()).toMatchObject({
      kind: "success",
      message: "Instruction saved",
    });

    vi.advanceTimersByTime(6_000);

    expect(currentNotice()).toMatchObject({ kind: "info", message: "Ready" });
  });

  it("keeps progress and error notices until they are replaced", () => {
    publishNotice("progress", "Placing the prompt…");
    vi.advanceTimersByTime(60_000);
    expect(currentNotice().message).toBe("Placing the prompt…");

    publishNotice("error", "Placement failed");
    vi.advanceTimersByTime(60_000);
    expect(currentNotice().message).toBe("Placement failed");
  });

  it("a newer notice cancels the pending expiry of the previous one", () => {
    publishNotice("success", "First");
    vi.advanceTimersByTime(3_000);

    publishNotice("progress", "Working…");
    vi.advanceTimersByTime(30_000);

    expect(currentNotice().message).toBe("Working…");
  });

  it("an expired timer never clobbers a notice published after it", () => {
    publishNotice("info", "Transient");
    vi.advanceTimersByTime(5_999);
    publishNotice("error", "Something broke");

    vi.advanceTimersByTime(30_000);

    expect(currentNotice()).toMatchObject({
      kind: "error",
      message: "Something broke",
    });
  });
});
