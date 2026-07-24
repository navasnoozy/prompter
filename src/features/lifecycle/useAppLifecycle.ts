import { useEffect } from "react";
import { appLifecycleGateway } from "./gateway";
import { useLifecycleStore } from "./store";

// Binder hook: connects native window-visibility events to the lifecycle
// store and refreshes the authoritative status on mount and window focus.
export function useAppLifecycle(): void {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void appLifecycleGateway
      .onMainWindowVisibility((visible) => {
        if (!disposed) useLifecycleStore.getState().applyVisibility(visible);
      })
      .then((stopListening) => {
        if (disposed) {
          stopListening();
          return;
        }
        unlisten = stopListening;
        void useLifecycleStore.getState().refreshStatus();
      })
      .catch(() => {
        if (!disposed) void useLifecycleStore.getState().refreshStatus();
      });

    const refresh = () => void useLifecycleStore.getState().refreshStatus();
    window.addEventListener("focus", refresh);
    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("focus", refresh);
    };
  }, []);
}
