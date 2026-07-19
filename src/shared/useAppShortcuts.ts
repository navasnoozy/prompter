import { useEffect } from "react";
import { useInstructionStore } from "../features/instructions/store";
import { placeCurrentPrompt } from "../features/providers/placement";
import { useProviderStore } from "../features/providers/store";
import { useSettingsStore } from "../features/settings/store";

// Window-level keyboard layer: ⌘⏎ places the prompt, ⌘1/⌘2 switch the
// provider. Inactive while a dialog is open so dialogs keep their own keys.
export function useAppShortcuts(): void {
  useEffect(() => {
    const handleKeydown = (event: KeyboardEvent) => {
      if (!event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) {
        return;
      }

      const dialogOpen =
        useSettingsStore.getState().showSettings ||
        useInstructionStore.getState().editorTarget !== null;
      if (dialogOpen) return;

      if (event.key === "Enter") {
        if (useProviderStore.getState().isPlacing) return;
        event.preventDefault();
        void placeCurrentPrompt();
      } else if (event.key === "1" || event.key === "2") {
        event.preventDefault();
        useProviderStore
          .getState()
          .setProvider(event.key === "1" ? "chatgpt" : "gemini");
      }
    };

    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  }, []);
}
