import {
  useCallback,
  useLayoutEffect,
  useRef,
  type RefObject,
} from "react";
import { providerGateway } from "./gateway";
import {
  getProviderLabel,
  type Provider,
  type ProviderBounds,
} from "./model";

const MIN_PROVIDER_SIZE = 240;

type UseEmbeddedProviderOptions = {
  provider: Provider;
  visible: boolean;
  onNotice: (message: string) => void;
};

type UseEmbeddedProviderResult = {
  hostRef: RefObject<HTMLDivElement | null>;
  ensureProvider: () => Promise<void>;
};

export function useEmbeddedProvider({
  provider,
  visible,
  onNotice,
}: UseEmbeddedProviderOptions): UseEmbeddedProviderResult {
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
    let disposed = false;
    let animationFrame = 0;

    if (!visible) {
      void providerGateway
        .setVisibility(provider, false)
        .catch((error) => onNotice(String(error)));
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
          onNotice(`${getProviderLabel(provider)} is ready inside Prompter`);
        }
      } catch (error) {
        if (!disposed) onNotice(String(error));
      }
    };

    const resizeProvider = () => {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = window.requestAnimationFrame(() => {
        const bounds = readBounds();
        if (bounds) void providerGateway.resize(provider, bounds);
      });
    };

    void showProvider();
    const observer = new ResizeObserver(resizeProvider);
    if (hostRef.current) observer.observe(hostRef.current);
    window.addEventListener("resize", resizeProvider);

    return () => {
      disposed = true;
      observer.disconnect();
      window.removeEventListener("resize", resizeProvider);
      window.cancelAnimationFrame(animationFrame);
    };
  }, [ensureProvider, onNotice, provider, readBounds, visible]);

  return { hostRef, ensureProvider };
}
