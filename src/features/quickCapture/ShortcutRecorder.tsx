import { useEffect, useState } from "react";
import {
  readShortcutFromEvent,
  RECORDING_REJECTION_MESSAGES,
} from "./accelerator";
import { DEFAULT_ACCELERATOR, DEFAULT_SHORTCUT_DISPLAY } from "./model";
import { useCaptureStore } from "./store";

const RECORDING_PROMPT = "Press the keys you want to use";
const IDLE_HINT = "Click, then press the keys you want to use.";

export function ShortcutRecorder() {
  const status = useCaptureStore((state) => state.status);
  const isSavingShortcut = useCaptureStore((state) => state.isSavingShortcut);
  const setShortcut = useCaptureStore((state) => state.setShortcut);
  const resetShortcut = useCaptureStore((state) => state.resetShortcut);

  const [isRecording, setIsRecording] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [hint, setHint] = useState<string | null>(null);

  useEffect(() => {
    if (!isRecording) return;

    const finish = () => {
      setIsRecording(false);
      setPreview(null);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      // Captured on `window` so nothing else in the app — including the
      // dialog's own Escape and Tab handling — reacts to keys meant for the
      // recorder.
      event.preventDefault();
      event.stopPropagation();
      if (event.repeat) return;

      const result = readShortcutFromEvent(event);
      switch (result.kind) {
        case "pending":
          setPreview(result.display);
          setHint(null);
          return;
        case "rejected":
          setPreview(result.display);
          setHint(RECORDING_REJECTION_MESSAGES[result.reason]);
          return;
        case "cancelled":
          setHint(null);
          finish();
          return;
        case "recorded":
          setHint(null);
          finish();
          void setShortcut(result.accelerator);
      }
    };

    const swallow = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("keyup", swallow, true);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("keyup", swallow, true);
    };
  }, [isRecording, setShortcut]);

  const currentDisplay = status?.shortcut.display ?? DEFAULT_SHORTCUT_DISPLAY;
  const isDefault = status
    ? status.shortcut.accelerator === DEFAULT_ACCELERATOR
    : true;
  const label = isRecording
    ? (preview ?? RECORDING_PROMPT)
    : isSavingShortcut
      ? "Saving…"
      : currentDisplay;

  return (
    <div className="shortcut-recorder">
      <div className="shortcut-recorder-controls">
        <button
          aria-label={`Quick Capture shortcut, currently ${currentDisplay}. Activate to record a new one.`}
          className={`shortcut-recorder-target ${isRecording ? "recording" : ""}`}
          // Blur ends recording so a shortcut is never captured for a control
          // the user has already moved away from.
          onBlur={() => {
            setIsRecording(false);
            setPreview(null);
            setHint(null);
          }}
          onClick={() => {
            setIsRecording((recording) => !recording);
            setPreview(null);
            setHint(null);
          }}
          disabled={isSavingShortcut}
          type="button"
        >
          <kbd className="settings-shortcut">{label}</kbd>
        </button>
        {!isDefault && !isRecording && (
          <button
            className="settings-inline-button"
            disabled={isSavingShortcut}
            onClick={() => void resetShortcut()}
            type="button"
          >
            Use default
          </button>
        )}
      </div>
      <p aria-live="polite" className="shortcut-recorder-hint">
        {hint ?? (isRecording ? "Press Escape to keep the current one." : IDLE_HINT)}
      </p>
    </div>
  );
}
