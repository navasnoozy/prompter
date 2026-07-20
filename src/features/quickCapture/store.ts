import { create } from "zustand";
import { publishNotice } from "../../shared/notices";
import { useSettingsStore } from "../settings/store";
import { normalizeQuickCaptureError, quickCaptureGateway } from "./gateway";
import type { CaptureOutcome, QuickCaptureStatus } from "./model";

type CaptureState = {
  sourceText: string;
  status: QuickCaptureStatus | null;
  isRefreshingStatus: boolean;
  isCapturingClipboard: boolean;
  isRequestingPermission: boolean;
  isRetryingRegistration: boolean;
  setSourceText: (text: string) => void;
  refreshStatus: (reportFailure?: boolean) => Promise<void>;
  captureClipboard: () => Promise<void>;
  requestPermission: () => Promise<void>;
  retryRegistration: () => Promise<void>;
  openSystemSettings: () => Promise<void>;
};

let statusRequestGeneration = 0;
let sourceTextRevision = 0;

export const useCaptureStore = create<CaptureState>()((set, get) => ({
  sourceText: "",
  status: null,
  isRefreshingStatus: false,
  isCapturingClipboard: false,
  isRequestingPermission: false,
  isRetryingRegistration: false,

  setSourceText: (text) => {
    sourceTextRevision += 1;
    set({ sourceText: text });
  },

  refreshStatus: async (reportFailure = false) => {
    const generation = ++statusRequestGeneration;
    set({ isRefreshingStatus: true });
    try {
      const status = await quickCaptureGateway.getStatus();
      if (generation === statusRequestGeneration) set({ status });
    } catch (error) {
      if (reportFailure && generation === statusRequestGeneration) {
        publishNotice("error", normalizeQuickCaptureError(error).message);
      }
    } finally {
      if (generation === statusRequestGeneration) {
        set({ isRefreshingStatus: false });
      }
    }
  },

  captureClipboard: async () => {
    if (get().isCapturingClipboard) return;
    const revisionAtStart = sourceTextRevision;
    set({ isCapturingClipboard: true });
    try {
      const payload = await quickCaptureGateway.readClipboardText();
      if (sourceTextRevision !== revisionAtStart) return;
      sourceTextRevision += 1;
      set({ sourceText: payload.text });
      publishNotice("success", "Clipboard text captured");
    } catch (error) {
      publishNotice("error", normalizeQuickCaptureError(error).message);
    } finally {
      set({ isCapturingClipboard: false });
    }
  },

  requestPermission: async () => {
    set({ isRequestingPermission: true });
    try {
      const status = await quickCaptureGateway.requestPermission();
      statusRequestGeneration += 1;
      set({ status, isRefreshingStatus: false });
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
      statusRequestGeneration += 1;
      set({ status, isRefreshingStatus: false });
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
    sourceTextRevision += 1;
    useCaptureStore.setState({ sourceText: outcome.text });
    focusPromptInput();
    publishNotice(
      outcome.warnings.length > 0 ? "info" : "success",
      outcome.warnings.length > 0
        ? outcome.warnings.map(({ message }) => message).join(" ")
        : "Selected text captured",
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
const processedOutcomeIds = new Set<string>();

// Durable drain: outcomes queue natively until the frontend collects them,
// so captures that finish while the window is hidden are never lost.
export async function drainPendingOutcomes(): Promise<void> {
  drainRequested = true;
  if (isDraining) return;

  isDraining = true;
  try {
    while (drainRequested) {
      drainRequested = false;
      const outcomes = await quickCaptureGateway.listPendingOutcomes();
      for (const outcome of outcomes) {
        if (!processedOutcomeIds.has(outcome.requestId)) {
          applyOutcome(outcome);
          processedOutcomeIds.add(outcome.requestId);
        }
      }
      if (outcomes.length > 0) {
        await quickCaptureGateway.acknowledgeOutcomes(
          outcomes.map(({ requestId }) => requestId),
        );
        for (const outcome of outcomes) {
          processedOutcomeIds.delete(outcome.requestId);
        }
      }
    }
  } catch (error) {
    publishNotice("error", normalizeQuickCaptureError(error).message);
  } finally {
    isDraining = false;
  }
}
