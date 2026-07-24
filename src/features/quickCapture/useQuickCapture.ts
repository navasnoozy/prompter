import { useEffect } from "react";
import { publishNotice } from "../../shared/notices";
import { quickCaptureGateway } from "./gateway";
import { drainPendingOutcomes, useCaptureStore } from "./store";

// Binder hook: connects native Quick Capture events to the capture store.
// Outcomes queue natively, so draining on mount catches captures that
// completed before the frontend was listening.
export function useQuickCapture(): void {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void quickCaptureGateway
      .onReady(() => {
        if (!disposed) void drainPendingOutcomes();
      })
      .then((stopListening) => {
        if (disposed) {
          stopListening();
          return;
        }
        unlisten = stopListening;
        void drainPendingOutcomes();
      })
      .catch(() => {
        if (!disposed) publishNotice("error", "Quick Capture is unavailable.");
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const refresh = () => {
      void useCaptureStore.getState().refreshStatus();
      void drainPendingOutcomes();
    };
    refresh();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, []);
}
