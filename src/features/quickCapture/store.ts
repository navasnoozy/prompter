import { create } from "zustand";
import { publishNotice } from "../../shared/notices";
import { useSettingsStore } from "../settings/store";
import { normalizeQuickCaptureError, quickCaptureGateway } from "./gateway";
import type { CaptureOutcome, QuickCaptureStatus } from "./model";

type CaptureState = {
  sourceText: string;
  status: QuickCaptureStatus | null;
  isRequestingPermission: boolean;
  isRetryingRegistration: boolean;
  setSourceText: (text: string) => void;
  refreshStatus: () => Promise<void>;
  captureClipboard: () => Promise<void>;
  requestPermission: () => Promise<void>;
  retryRegistration: () => Promise<void>;
  openSystemSettings: () => Promise<void>;
};

export const useCaptureStore = create<CaptureState>()((set) => ({
  sourceText: "",
  status: null,
  isRequestingPermission: false,
  isRetryingRegistration: false,

  setSourceText: (text) => set({ sourceText: text }),

  refreshStatus: async () => {
    try {
      set({ status: await quickCaptureGateway.getStatus() });
    } catch {
      // The next window focus retries; the UI treats null as "checking".
    }
  },

  captureClipboard: async () => {
    try {
      const payload = await quickCaptureGateway.readClipboardText();
      set({ sourceText: payload.text });
      publishNotice("success", "Clipboard text captured");
    } catch (error) {
      publishNotice("error", normalizeQuickCaptureError(error).message);
    }
  },

  requestPermission: async () => {
    set({ isRequestingPermission: true });
    try {
      const status = await quickCaptureGateway.requestPermission();
      set({ status });
      publishNotice(
        status.permission === "granted" ? "success" : "info",
        status.permission === "granted"
          ? "Quick Capture is ready"
          : "Permission is still required. Open System Settings to enable Prompter.",
      );
    } catch (error) {
      publishNotice("error", normalizeQuickCaptureError(error).message);
    } finally {
      set({ isRequestingPermission: false });
    }
  },

  retryRegistration: async () => {
    set({ isRetryingRegistration: true });
    try {
      const status = await quickCaptureGateway.retryRegistration();
      set({ status });
      publishNotice(
        status.registration === "registered" ? "success" : "info",
        status.registration === "registered"
          ? "Keyboard shortcut is ready"
          : "The keyboard shortcut is still unavailable.",
      );
    } catch (error) {
      publishNotice("error", normalizeQuickCaptureError(error).message);
    } finally {
      set({ isRetryingRegistration: false });
    }
  },

  openSystemSettings: async () => {
    try {
      await quickCaptureGateway.openSystemSettings();
    } catch (error) {
      publishNotice("error", normalizeQuickCaptureError(error).message);
    }
  },
}));

// The prompt textarea registers itself so a completed capture can hand
// keyboard focus straight to it without prop plumbing.
let promptInput: HTMLTextAreaElement | null = null;

export function registerPromptInput(element: HTMLTextAreaElement | null): void {
  promptInput = element;
}

function focusPromptInput(): void {
  const element = promptInput;
  if (!element) return;
  requestAnimationFrame(() => {
    element.focus();
    const end = element.value.length;
    element.setSelectionRange(end, end);
  });
}

function applyOutcome(outcome: CaptureOutcome): void {
  if (outcome.kind === "success") {
    useCaptureStore.setState({ sourceText: outcome.text });
    focusPromptInput();
    const warning = outcome.warnings[0];
    publishNotice(
      warning ? "info" : "success",
      warning?.message ?? "Selected text captured",
    );
    return;
  }

  useCaptureStore.setState((state) => ({
    status: state.status
      ? { ...state.status, permission: outcome.permission }
      : state.status,
  }));
  publishNotice("error", outcome.message);
  if (outcome.code === "permission_required") {
    useSettingsStore.getState().openSettings();
  }
}

let drainRequested = false;
let isDraining = false;

// Durable drain: outcomes queue natively until the frontend collects them,
// so captures that finish while the window is hidden are never lost.
export async function drainPendingOutcomes(): Promise<void> {
  drainRequested = true;
  if (isDraining) return;

  isDraining = true;
  try {
    while (drainRequested) {
      drainRequested = false;
      const outcomes = await quickCaptureGateway.takePendingOutcomes();
      outcomes.forEach(applyOutcome);
    }
  } catch (error) {
    publishNotice("error", normalizeQuickCaptureError(error).message);
  } finally {
    isDraining = false;
  }
}
