import { describe, expect, it } from "vitest";
import {
  parseCaptureCommandError,
  parseCaptureOutcome,
  parseCaptureReadyEvent,
  parseClipboardTextPayload,
  parseQuickCaptureStatus,
} from "./model";

describe("Quick Capture native contracts", () => {
  it("parses a valid status and rejects unknown versions", () => {
    const status = {
      version: 2,
      shortcut: {
        accelerator: "CommandOrControl+Shift+P",
        display: "⌘ ⇧ P",
      },
      registration: "registered",
      permission: "granted",
      accessibility: "granted",
    };

    expect(parseQuickCaptureStatus(status)).toEqual(status);
    expect(parseQuickCaptureStatus({ ...status, version: 3 })).toBeNull();
    // Accessibility trust is reported separately from keystroke permission, so
    // a status that only carries the latter is no longer a valid status.
    expect(
      parseQuickCaptureStatus({
        version: 2,
        shortcut: status.shortcut,
        registration: "registered",
        permission: "granted",
      }),
    ).toBeNull();
  });

  it("preserves exact multiline Unicode text from a success outcome", () => {
    const outcome = {
      kind: "success",
      version: 2,
      requestId: "capture-1",
      text: "First line\n✨ രണ്ടാം വരി",
      warnings: [],
      durationMs: 75,
    };

    expect(parseCaptureOutcome(outcome)).toEqual(outcome);
  });

  it("rejects malformed outcomes and unknown error codes", () => {
    expect(
      parseCaptureOutcome({
        kind: "failure",
        version: 2,
        requestId: "capture-2",
        code: "raw_native_error",
        message: "Do not expose this",
        permission: "required",
        accessibility: "granted",
        durationMs: 20,
      }),
    ).toBeNull();
    expect(parseCaptureOutcome({ kind: "success", version: 2 })).toBeNull();
  });

  it("validates notification, clipboard, and command-error payloads", () => {
    expect(
      parseCaptureReadyEvent({ version: 2, requestId: "capture-3" }),
    ).toEqual({ version: 2, requestId: "capture-3" });
    expect(
      parseClipboardTextPayload({ version: 2, text: "Clipboard text" }),
    ).toEqual({ version: 2, text: "Clipboard text" });
    expect(
      parseCaptureCommandError({
        version: 2,
        code: "accessibility_permission_required",
        message: "Accessibility permission required",
      }),
    ).toEqual({
      version: 2,
      code: "accessibility_permission_required",
      message: "Accessibility permission required",
    });
  });
});
