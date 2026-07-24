import { useEffect } from "react";
import { bindPlacementEvents } from "./placement";

// Binder hook: mounts the placement bridge listeners for the app's lifetime.
// All placement state lives in the provider store and placement module.
export function usePromptPlacement(): void {
  useEffect(() => bindPlacementEvents(), []);
}
