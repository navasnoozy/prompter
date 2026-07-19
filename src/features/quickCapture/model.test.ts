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
      version: 1,
      shortcut: {
        accelerator: "CommandOrControl+Shift+P",
        display: "⌘ ⇧ P",
      },
      registration: "registered",
      permission: "granted",
    };

    expect(parseQuickCaptureStatus(status)).toEqual(status);
    expect(parseQuickCaptureStatus({ ...status, version: 2 })).toBeNull();
  });

  it("preserves exact multiline Unicode text from a success outcome", () => {
    const outcome = {
      kind: "success",
      version: 1,
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
        version: 1,
        requestId: "capture-2",
        code: "raw_native_error",
        message: "Do not expose this",
        permission: "required",
        durationMs: 20,
      }),
    ).toBeNull();
    expect(parseCaptureOutcome({ kind: "success", version: 1 })).toBeNull();
  });

  it("validates notification, clipboard, and command-error payloads", () => {
    expect(
      parseCaptureReadyEvent({ version: 1, requestId: "capture-3" }),
    ).toEqual({ version: 1, requestId: "capture-3" });
    expect(
      parseClipboardTextPayload({ version: 1, text: "Clipboard text" }),
    ).toEqual({ version: 1, text: "Clipboard text" });
    expect(
      parseCaptureCommandError({
        version: 1,
        code: "permission_required",
        message: "Permission required",
      }),
    ).toEqual({
      version: 1,
      code: "permission_required",
      message: "Permission required",
    });
  });
});
