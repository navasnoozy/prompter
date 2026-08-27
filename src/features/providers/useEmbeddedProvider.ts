import {
  useCallback,
  useLayoutEffect,
  useRef,
  type RefObject,
} from "react";
import { publishNotice } from "../../shared/notices";
import { useInstructionStore } from "../instructions/store";
import { useLifecycleStore } from "../lifecycle/store";
import { useSettingsStore } from "../settings/store";
import { providerGateway } from "./gateway";
import { type Provider, type ProviderBounds } from "./model";
import {
  cancelCurrentPlacement,
  placementErrorMessage,
  registerEnsureProvider,
} from "./placement";
import { useProviderStore } from "./store";

const MIN_PROVIDER_SIZE = 240;
// Collapses the burst of layout notifications a live window drag produces.
const RESIZE_COALESCE_MS = 16;

type UseEmbeddedProviderResult = {
  hostRef: RefObject<HTMLDivElement | null>;
};

// Owns the native child-webview lifecycle: creation, visibility, and layout
// tracking against the host element. The pane is visible only while the window
// is presented and no dialog covers it.
//
// Layout is all this hook reports. Where the pane ends up is decided natively:
// the backend records each report as insets from the window's content edges and
// re-derives the pane's frame whenever the window itself moves or resizes.
// That split matters because the DOM cannot see a window resize that AppKit has
// already applied, and this hook stops running altogether while the window is
// hidden to the tray.
export function useEmbeddedProvider(): UseEmbeddedProviderResult {
  const provider = useProviderStore((state) => state.provider);
  const mainWindowVisible = useLifecycleStore(
    (state) => state.status?.mainWindowVisible === true,
  );
  const editorOpen = useInstructionStore((state) => state.editorTarget !== null);
  const settingsOpen = useSettingsStore((state) => state.showSettings);
  const visible = mainWindowVisible && !editorOpen && !settingsOpen;

  const hostRef = useRef<HTMLDivElement | null>(null);
  const currentProviderRef = useRef(provider);
  const visibleRef = useRef(visible);
  // Declared before the main layout effect so the refs are current by the
  // time it (and its async continuations) read them.
  useLayoutEffect(() => {
    currentProviderRef.current = provider;
    visibleRef.current = visible;
  });
  const pendingShowRef = useRef<{
    provider: Provider;
    promise: Promise<void>;
  } | null>(null);

  const readBounds = useCallback((): ProviderBounds | null => {
    const host = hostRef.current;
    if (!host) return null;

    const rect = host.getBoundingClientRect();
    if (rect.width < MIN_PROVIDER_SIZE || rect.height < MIN_PROVIDER_SIZE) {
      return null;
    }

    return {
      x: rect.left,
      y: rect.top,
      width: rect.width,
      height: rect.height,
    };
  }, []);

  const ensureProvider = useCallback((): Promise<void> => {
    const bounds = readBounds();
    if (!bounds) {
      return Promise.reject(
        new Error("The embedded browser area is not ready yet."),
      );
    }

    const pending = pendingShowRef.current;
    if (pending?.provider === provider) return pending.promise;

    const promise = providerGateway.show(provider, bounds).finally(() => {
      if (pendingShowRef.current?.promise === promise) {
        pendingShowRef.current = null;
      }
    });
    pendingShowRef.current = { provider, promise };
    return promise;
  }, [provider, readBounds]);

  useLayoutEffect(() => {
    registerEnsureProvider(ensureProvider);
    return () => registerEnsureProvider(null);
  }, [ensureProvider]);

  useLayoutEffect(() => {
    let disposed = false;
    let resizeTimer = 0;
    let resizeErrorReported = false;

    if (!visible) {
      cancelCurrentPlacement();
      useProviderStore.setState({ panelOpen: false });
      void providerGateway
        .setVisibility(provider, false)
        .catch((error) => publishNotice("error", placementErrorMessage(error)));
      return () => {
        disposed = true;
      };
    }

    const showProvider = async () => {
      try {
        await ensureProvider();
        const activeProvider = currentProviderRef.current;
        await providerGateway.setVisibility(activeProvider, visibleRef.current);
        if (!disposed && activeProvider === provider) {
          useProviderStore.setState({ panelOpen: visibleRef.current });
        }
      } catch (error) {
        if (!disposed) publishNotice("error", placementErrorMessage(error));
      }
    };

    // Deliberately a timer rather than an animation frame: animation frames
    // stop being delivered while the window is occluded, so a pane whose
    // placement depended on them would be left stale by exactly the transitions
    // most likely to move it.
    const resizeProvider = () => {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        const bounds = readBounds();
        if (!bounds) return;
        void providerGateway.resize(provider, bounds).catch((error) => {
          if (!disposed && !resizeErrorReported) {
            resizeErrorReported = true;
            publishNotice("error", placementErrorMessage(error));
          }
        });
      }, RESIZE_COALESCE_MS);
    };

    void showProvider();
    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(resizeProvider);
    if (hostRef.current) observer?.observe(hostRef.current);
    window.addEventListener("resize", resizeProvider);

    return () => {
      disposed = true;
      observer?.disconnect();
      window.removeEventListener("resize", resizeProvider);
      window.clearTimeout(resizeTimer);
    };
  }, [ensureProvider, provider, readBounds, visible]);

  return { hostRef };
}
