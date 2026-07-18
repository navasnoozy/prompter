import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { TAURI_EVENTS } from "./gateway";

type UseClipboardCaptureOptions = {
  onNotice: (message: string) => void;
};

function readErrorMessage(payload: unknown): string {
  if (typeof payload === "string" && payload.trim()) return payload;
  if (
    typeof payload === "object" &&
    payload !== null &&
    "message" in payload &&
    typeof payload.message === "string" &&
    payload.message.trim()
  ) {
    return payload.message;
  }
  return "Could not capture the selected text";
}

export function useClipboardCapture({
  onNotice,
}: UseClipboardCaptureOptions) {
  const [sourceText, setSourceText] = useState("");

  useEffect(() => {
    const unlistenCaptured = listen<string>(
      TAURI_EVENTS.clipboardCaptured,
      (event) => {
        if (!event.payload.trim()) return;
        setSourceText(event.payload);
        onNotice("Copied text captured");
      },
    );

    const unlistenError = listen<unknown>(
      TAURI_EVENTS.clipboardError,
      (event) => onNotice(readErrorMessage(event.payload)),
    );

    return () => {
      void unlistenCaptured.then((unlisten) => unlisten());
      void unlistenError.then((unlisten) => unlisten());
    };
  }, [onNotice]);

  async function captureClipboard() {
    try {
      const text = await readText();
      if (!text.trim()) {
        onNotice("Clipboard is empty");
        return;
      }
      setSourceText(text);
      onNotice("Clipboard text captured");
    } catch {
      onNotice("Copy text first, then try again");
    }
  }

  return { sourceText, setSourceText, captureClipboard };
}
