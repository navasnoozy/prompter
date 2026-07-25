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
  isSavingShortcut: boolean;
  setSourceText: (text: string) => void;
  refreshStatus: (reportFailure?: boolean) => Promise<void>;
  captureClipboard: () => Promise<void>;
  requestPermission: () => Promise<void>;
  retryRegistration: () => Promise<void>;
  /** Resolves `true` once the backend has accepted and stored the change. */
  setShortcut: (accelerator: string) => Promise<boolean>;
  resetShortcut: () => Promise<boolean>;
  openSystemSettings: () => Promise<void>;
};

let statusRequestGeneration = 0;
let sourceTextRevision = 0;

function permissionNotice(
  status: QuickCaptureStatus,
): ["success" | "info", string] {
  if (status.accessibility !== "granted") {
    return [
      "info",
      "Enable Prompter under Privacy & Security → Accessibility, then relaunch Prompter.",
    ];
  }
  if (status.permission !== "granted") {
    return [
      "info",
      "Permission is still required. Open System Settings to enable Prompter.",
    ];
  }
  return ["success", "Quick Capture is ready"];
}

export const useCaptureStore = create<CaptureState>()((set, get) => ({
  sourceText: "",
  status: null,
  isRefreshingStatus: false,
  isCapturingClipboard: false,
  isRequestingPermission: false,
  isRetryingRegistration: false,
  isSavingShortcut: false,

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
      // macOS records the two grants separately, and Accessibility only takes
      // effect for a process that was already trusted when it launched.
      publishNotice(...permissionNotice(status));
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

  setShortcut: (accelerator) =>
    changeShortcut(set, get, () =>
      quickCaptureGateway.setShortcut(accelerator),
    ),

  resetShortcut: () =>
    changeShortcut(set, get, () => quickCaptureGateway.resetShortcut()),

  openSystemSettings: async () => {
    try {
      await quickCaptureGateway.openSystemSettings();
    } catch (error) {
      publishNotice("error", normalizeQuickCaptureError(error).message);
    }
  },
}));

/**
 * Shared by both shortcut changes. The backend rolls back to the previous
 * combination when a change fails, so a rejection leaves the last known status
 * in place rather than clearing it.
 */
async function changeShortcut(
  set: (partial: Partial<CaptureState>) => void,
  get: () => CaptureState,
  change: () => Promise<QuickCaptureStatus>,
): Promise<boolean> {
  if (get().isSavingShortcut) return false;
  set({ isSavingShortcut: true });
  try {
    const status = await change();
    statusRequestGeneration += 1;
    set({ status, isRefreshingStatus: false });
    publishNotice("success", `Shortcut set to ${status.shortcut.display}`);
    return true;
  } catch (error) {
    publishNotice("error", normalizeQuickCaptureError(error).message);
    return false;
  } finally {
    set({ isSavingShortcut: false });
  }
}

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
      ? {
          ...state.status,
          permission: outcome.permission,
          accessibility: outcome.accessibility,
        }
      : state.status,
  }));
  publishNotice("error", outcome.message);
  if (
    outcome.code === "permission_required" ||
    outcome.code === "accessibility_permission_required"
  ) {
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
