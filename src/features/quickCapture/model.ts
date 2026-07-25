import { isNonEmptyString, isRecord } from "../../shared/contracts";

export const QUICK_CAPTURE_CONTRACT_VERSION = 3;
/**
 * Shown only until the real status arrives. Modifiers follow the macOS order
 * (⌃⌥⇧⌘) the backend renders, so the placeholder does not visibly reshuffle.
 */
export const DEFAULT_SHORTCUT_DISPLAY = "⇧ ⌘ P";
/**
 * The backend's canonical spelling of the same default. Used only to decide
 * whether "Use default" is worth offering; the backend owns the actual reset.
 */
export const DEFAULT_ACCELERATOR = "shift+super+KeyP";

export type PermissionState = "granted" | "required";
export type ShortcutRegistrationState = "registered" | "unavailable";
export type CaptureErrorCode =
  | "permission_required"
  | "accessibility_permission_required"
  | "invalid_request"
  | "clipboard_unavailable"
  | "clipboard_changed"
  | "clipboard_too_large"
  | "shortcut_keys_held"
  | "shortcut_invalid"
  | "shortcut_unavailable"
  | "copy_failed"
  | "copy_timed_out"
  | "no_text"
  | "selection_too_large"
  | "internal";
export type CaptureWarningCode =
  | "clipboard_restore_failed"
  | "window_unavailable";

export type ShortcutDescriptor = {
  /** Canonical accelerator, as normalized by the backend. */
  accelerator: string;
  /** Ready to render: glyphs in macOS order. */
  display: string;
};

export type QuickCaptureStatus = {
  version: 3;
  shortcut: ShortcutDescriptor;
  registration: ShortcutRegistrationState;
  /** Permission to synthesize the ⌘C keystroke. */
  permission: PermissionState;
  /** Accessibility trust, granted separately from `permission`. */
  accessibility: PermissionState;
};

export type CaptureWarning = {
  code: CaptureWarningCode;
  message: string;
};

export type CaptureSuccess = {
  kind: "success";
  version: 3;
  requestId: string;
  text: string;
  warnings: CaptureWarning[];
  durationMs: number;
};

export type CaptureFailure = {
  kind: "failure";
  version: 3;
  requestId: string;
  code: CaptureErrorCode;
  message: string;
  permission: PermissionState;
  accessibility: PermissionState;
  durationMs: number;
};

export type CaptureOutcome = CaptureSuccess | CaptureFailure;

export type CaptureReadyEvent = {
  version: 3;
  requestId: string;
};

export type ClipboardTextPayload = {
  version: 3;
  text: string;
};

export type CaptureCommandError = {
  version: 3;
  code: CaptureErrorCode;
  message: string;
};

const ERROR_CODES = new Set<CaptureErrorCode>([
  "permission_required",
  "accessibility_permission_required",
  "invalid_request",
  "clipboard_unavailable",
  "clipboard_changed",
  "clipboard_too_large",
  "shortcut_keys_held",
  "shortcut_invalid",
  "shortcut_unavailable",
  "copy_failed",
  "copy_timed_out",
  "no_text",
  "selection_too_large",
  "internal",
]);
const WARNING_CODES = new Set<CaptureWarningCode>([
  "clipboard_restore_failed",
  "window_unavailable",
]);

function isContractVersion(value: unknown): value is 3 {
  return value === QUICK_CAPTURE_CONTRACT_VERSION;
}

function isPermissionState(value: unknown): value is PermissionState {
  return value === "granted" || value === "required";
}

function isRegistrationState(
  value: unknown,
): value is ShortcutRegistrationState {
  return value === "registered" || value === "unavailable";
}

function isDuration(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0;
}

function parseWarning(value: unknown): CaptureWarning | null {
  if (
    !isRecord(value) ||
    !WARNING_CODES.has(value.code as CaptureWarningCode) ||
    !isNonEmptyString(value.message)
  ) {
    return null;
  }
  return {
    code: value.code as CaptureWarningCode,
    message: value.message,
  };
}

export function parseQuickCaptureStatus(
  value: unknown,
): QuickCaptureStatus | null {
  if (
    !isRecord(value) ||
    !isContractVersion(value.version) ||
    !isRecord(value.shortcut) ||
    !isNonEmptyString(value.shortcut.accelerator) ||
    !isNonEmptyString(value.shortcut.display) ||
    !isRegistrationState(value.registration) ||
    !isPermissionState(value.permission) ||
    !isPermissionState(value.accessibility)
  ) {
    return null;
  }

  return {
    version: value.version,
    shortcut: {
      accelerator: value.shortcut.accelerator,
      display: value.shortcut.display,
    },
    registration: value.registration,
    permission: value.permission,
    accessibility: value.accessibility,
  };
}

export function parseCaptureOutcome(value: unknown): CaptureOutcome | null {
  if (
    !isRecord(value) ||
    !isContractVersion(value.version) ||
    !isNonEmptyString(value.requestId) ||
    !isDuration(value.durationMs)
  ) {
    return null;
  }

  if (value.kind === "success") {
    if (
      typeof value.text !== "string" ||
      !value.text.trim() ||
      !Array.isArray(value.warnings)
    ) {
      return null;
    }
    const warnings = value.warnings.map(parseWarning);
    if (warnings.some((warning) => warning === null)) return null;
    return {
      kind: "success",
      version: value.version,
      requestId: value.requestId,
      text: value.text,
      warnings: warnings as CaptureWarning[],
      durationMs: value.durationMs,
    };
  }

  if (
    value.kind === "failure" &&
    ERROR_CODES.has(value.code as CaptureErrorCode) &&
    isNonEmptyString(value.message) &&
    isPermissionState(value.permission) &&
    isPermissionState(value.accessibility)
  ) {
    return {
      kind: "failure",
      version: value.version,
      requestId: value.requestId,
      code: value.code as CaptureErrorCode,
      message: value.message,
      permission: value.permission,
      accessibility: value.accessibility,
      durationMs: value.durationMs,
    };
  }

  return null;
}

export function parseCaptureReadyEvent(
  value: unknown,
): CaptureReadyEvent | null {
  if (
    !isRecord(value) ||
    !isContractVersion(value.version) ||
    !isNonEmptyString(value.requestId)
  ) {
    return null;
  }
  return { version: value.version, requestId: value.requestId };
}

export function parseClipboardTextPayload(
  value: unknown,
): ClipboardTextPayload | null {
  if (
    !isRecord(value) ||
    !isContractVersion(value.version) ||
    typeof value.text !== "string" ||
    !value.text.trim()
  ) {
    return null;
  }
  return { version: value.version, text: value.text };
}

export function parseCaptureCommandError(
  value: unknown,
): CaptureCommandError | null {
  if (
    !isRecord(value) ||
    !isContractVersion(value.version) ||
    !ERROR_CODES.has(value.code as CaptureErrorCode) ||
    !isNonEmptyString(value.message)
  ) {
    return null;
  }
  return {
    version: value.version,
    code: value.code as CaptureErrorCode,
    message: value.message,
  };
}
