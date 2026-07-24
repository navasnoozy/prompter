import { create } from "zustand";

export type NoticeKind = "info" | "success" | "progress" | "error";

export type Notice = {
  id: number;
  kind: NoticeKind;
  message: string;
};

const AUTO_CLEAR_MS = 6_000;
const READY: Notice = { id: 0, kind: "info", message: "Ready" };

let sequence = 0;
let expiryTimer: ReturnType<typeof setTimeout> | undefined;

type NoticeState = {
  notice: Notice;
  publish: (kind: NoticeKind, message: string) => void;
};

// Single app-wide notice channel. Success and info notices fade back to
// "Ready" after a few seconds; progress and error notices stay until they
// are replaced, so in-flight work is never hidden by a timer.
export const useNoticeStore = create<NoticeState>()((set, get) => ({
  notice: READY,
  publish: (kind, message) => {
    if (expiryTimer !== undefined) {
      clearTimeout(expiryTimer);
      expiryTimer = undefined;
    }

    const id = ++sequence;
    set({ notice: { id, kind, message } });

    if (kind === "success" || kind === "info") {
      expiryTimer = setTimeout(() => {
        expiryTimer = undefined;
        if (get().notice.id === id) set({ notice: READY });
      }, AUTO_CLEAR_MS);
    }
  },
}));

export function publishNotice(kind: NoticeKind, message: string): number {
  useNoticeStore.getState().publish(kind, message);
  return useNoticeStore.getState().notice.id;
}

export function publishNoticeIfCurrent(
  expectedId: number,
  kind: NoticeKind,
  message: string,
): boolean {
  if (useNoticeStore.getState().notice.id !== expectedId) return false;
  publishNotice(kind, message);
  return true;
}
