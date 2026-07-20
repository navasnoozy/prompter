import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  parseCaptureCommandError,
  parseCaptureOutcome,
  parseCaptureReadyEvent,
  parseClipboardTextPayload,
  parseQuickCaptureStatus,
  type CaptureCommandError,
  type CaptureOutcome,
  type ClipboardTextPayload,
  type QuickCaptureStatus,
} from "./model";

export const QUICK_CAPTURE_COMMANDS = {
  getStatus: "get_quick_capture_status",
  requestPermission: "request_quick_capture_permission",
  openSettings: "open_quick_capture_settings",
  retryRegistration: "retry_quick_capture_registration",
  readClipboardText: "read_clipboard_text",
  listOutcomes: "list_quick_capture_outcomes",
  acknowledgeOutcomes: "acknowledge_quick_capture_outcomes",
} as const;

export const QUICK_CAPTURE_EVENTS = {
  ready: "prompter://quick-capture-ready",
} as const;

export class QuickCaptureProtocolError extends Error {
  constructor(contract: string) {
    super(`Invalid Quick Capture ${contract} response`);
    this.name = "QuickCaptureProtocolError";
  }
}

async function invokeStatus(command: string): Promise<QuickCaptureStatus> {
  const value = await invoke<unknown>(command);
  const status = parseQuickCaptureStatus(value);
  if (!status) throw new QuickCaptureProtocolError("status");
  return status;
}

export function normalizeQuickCaptureError(
  error: unknown,
): CaptureCommandError {
  return (
    parseCaptureCommandError(error) ?? {
      version: 1,
      code: "internal",
      message: "Quick Capture could not finish. Please try again.",
    }
  );
}

export const quickCaptureGateway = {
  getStatus(): Promise<QuickCaptureStatus> {
    return invokeStatus(QUICK_CAPTURE_COMMANDS.getStatus);
  },

  requestPermission(): Promise<QuickCaptureStatus> {
    return invokeStatus(QUICK_CAPTURE_COMMANDS.requestPermission);
  },

  retryRegistration(): Promise<QuickCaptureStatus> {
    return invokeStatus(QUICK_CAPTURE_COMMANDS.retryRegistration);
  },

  openSystemSettings(): Promise<void> {
    return invoke(QUICK_CAPTURE_COMMANDS.openSettings);
  },

  async readClipboardText(): Promise<ClipboardTextPayload> {
    const value = await invoke<unknown>(
      QUICK_CAPTURE_COMMANDS.readClipboardText,
    );
    const payload = parseClipboardTextPayload(value);
    if (!payload) throw new QuickCaptureProtocolError("clipboard");
    return payload;
  },

  async listPendingOutcomes(): Promise<CaptureOutcome[]> {
    const value = await invoke<unknown>(QUICK_CAPTURE_COMMANDS.listOutcomes);
    if (!Array.isArray(value)) {
      throw new QuickCaptureProtocolError("outcomes");
    }
    const outcomes = value.map(parseCaptureOutcome);
    if (outcomes.some((outcome) => outcome === null)) {
      throw new QuickCaptureProtocolError("outcome");
    }
    return outcomes as CaptureOutcome[];
  },

  acknowledgeOutcomes(requestIds: string[]): Promise<void> {
    return invoke(QUICK_CAPTURE_COMMANDS.acknowledgeOutcomes, { requestIds });
  },

  onReady(handler: () => void): Promise<UnlistenFn> {
    return listen<unknown>(QUICK_CAPTURE_EVENTS.ready, (event) => {
      if (parseCaptureReadyEvent(event.payload)) handler();
    });
  },
};
