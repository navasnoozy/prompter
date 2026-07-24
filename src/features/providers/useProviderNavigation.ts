import { useEffect } from "react";
import { publishNotice } from "../../shared/notices";
import { providerGateway } from "./gateway";
import { useProviderStore } from "./store";

const LISTENER_RETRY_DELAYS_MS = [0, 250, 1_000] as const;
const SNAPSHOT_RETRY_DELAYS_MS = [0, 250, 1_000, 3_000] as const;

function waitForRetry(delayMs: number, signal: AbortSignal): Promise<void> {
  if (delayMs === 0 || signal.aborted) return Promise.resolve();

  return new Promise((resolve) => {
    const finish = () => {
      window.clearTimeout(timeout);
      signal.removeEventListener("abort", finish);
      resolve();
    };
    const timeout = window.setTimeout(finish, delayMs);
    signal.addEventListener("abort", finish, { once: true });
  });
}

// Binds the versioned native navigation-state stream to the provider store.
// The listener is established before the snapshot is requested, and the
// store's generation/revision ordering makes either arrival order safe.
export function useProviderNavigation(): void {
  const provider = useProviderStore((state) => state.provider);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const retries = new AbortController();
    const updateNavigationState =
      useProviderStore.getState().updateNavigationState;

    const requestSnapshot = async (): Promise<boolean> => {
      for (const delayMs of SNAPSHOT_RETRY_DELAYS_MS) {
        await waitForRetry(delayMs, retries.signal);
        if (disposed) return false;

        try {
          const navigation =
            await providerGateway.getNavigationState(provider);
          if (!disposed) updateNavigationState(navigation);
          return !disposed;
        } catch {
          // A newly-created child WebView can briefly be unavailable. The
          // bounded backoff closes that startup gap without polling forever.
        }
      }
      return false;
    };

    const bind = async (): Promise<void> => {
      for (const delayMs of LISTENER_RETRY_DELAYS_MS) {
        await waitForRetry(delayMs, retries.signal);
        if (disposed) return;

        try {
          const stopListening =
            await providerGateway.onNavigationState(
              updateNavigationState,
            );
          if (disposed) {
            stopListening();
            return;
          }

          unlisten = stopListening;
          if (!(await requestSnapshot()) && !disposed) {
            publishNotice(
              "error",
              "Prompter could not synchronize the provider browser controls. Reopen the provider and try again.",
            );
          }
          return;
        } catch {
          // Retry listener setup with a bounded backoff. No listener from a
          // failed attempt is retained by the Tauri API.
        }
      }

      if (!disposed) {
        publishNotice(
          "error",
          "Prompter could not connect to the provider browser controls. Reload the app and try again.",
        );
      }
    };

    void bind();

    return () => {
      disposed = true;
      retries.abort();
      try {
        unlisten?.();
      } catch {
        // Listener cleanup is best-effort during WebView teardown.
      }
    };
  }, [provider]);
}
