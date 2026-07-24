import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useInstructionStore } from "../features/instructions/store";
import { placeCurrentPrompt } from "../features/providers/placement";
import { useProviderStore } from "../features/providers/store";
import { useSettingsStore } from "../features/settings/store";
import { isRecord } from "./contracts";

const APP_SHORTCUT_EVENT = "prompter://app-shortcut";

type AppShortcutAction =
  | "place_prompt"
  | "select_chatgpt"
  | "select_gemini";

function parseAction(value: unknown): AppShortcutAction | null {
  if (!isRecord(value) || value.version !== 1) return null;
  return value.action === "place_prompt" ||
    value.action === "select_chatgpt" ||
    value.action === "select_gemini"
    ? value.action
    : null;
}

function dialogsAreOpen(): boolean {
  return (
    useSettingsStore.getState().showSettings ||
    useInstructionStore.getState().editorTarget !== null
  );
}

function runAction(action: AppShortcutAction): void {
  if (dialogsAreOpen()) return;

  if (action === "place_prompt") {
    if (!useProviderStore.getState().isPlacing) void placeCurrentPrompt();
  } else {
    useProviderStore
      .getState()
      .setProvider(action === "select_chatgpt" ? "chatgpt" : "gemini");
  }
}

function attachBrowserFallback(): () => void {
  const handleKeydown = (event: KeyboardEvent) => {
    if (!event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) {
      return;
    }

    const action =
      event.key === "Enter"
        ? "place_prompt"
        : event.key === "1"
          ? "select_chatgpt"
          : event.key === "2"
            ? "select_gemini"
            : null;
    if (!action || dialogsAreOpen()) return;
    event.preventDefault();
    runAction(action);
  };

  window.addEventListener("keydown", handleKeydown);
  return () => window.removeEventListener("keydown", handleKeydown);
}

// Native application-menu accelerators work regardless of whether the main
// WebView or an embedded provider WebView owns focus. The DOM handler is kept
// only for ordinary-browser development where Tauri events are unavailable.
export function useAppShortcuts(): void {
  useEffect(() => {
    let disposed = false;
    let cleanup: UnlistenFn | null = null;

    void listen<unknown>(APP_SHORTCUT_EVENT, (event) => {
      const action = parseAction(event.payload);
      if (action) runAction(action);
    }).then(
      (unlisten) => {
        if (disposed) unlisten();
        else cleanup = unlisten;
      },
      () => {
        if (!disposed) cleanup = attachBrowserFallback();
      },
    );

    return () => {
      disposed = true;
      cleanup?.();
    };
  }, []);
}
