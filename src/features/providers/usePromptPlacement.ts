import { useCallback, useEffect, useRef, useState } from "react";
import { providerGateway } from "./gateway";
import { getProviderLabel, type Provider } from "./model";

const REQUEST_TIMEOUT_MS = 12_000;

type PendingRequest = {
  provider: Provider;
  requestId: string;
  timeout?: number;
};

type UsePromptPlacementOptions = {
  provider: Provider;
  ensureProvider: () => Promise<void>;
  onNotice: (message: string) => void;
};

export function usePromptPlacement({
  provider,
  ensureProvider,
  onNotice,
}: UsePromptPlacementOptions) {
  const [isWorking, setIsWorking] = useState(false);
  const pendingRef = useRef<PendingRequest | null>(null);
  const currentProviderRef = useRef(provider);
  currentProviderRef.current = provider;

  const clearPending = useCallback((requestId?: string) => {
    const pending = pendingRef.current;
    if (!pending || (requestId && pending.requestId !== requestId)) return false;

    if (pending.timeout !== undefined) window.clearTimeout(pending.timeout);
    pendingRef.current = null;
    setIsWorking(false);
    return true;
  }, []);

  useEffect(() => {
    const unlistenFilled = providerGateway.onPromptFilled((event) => {
      const pending = pendingRef.current;
      if (
        !pending ||
        currentProviderRef.current !== event.provider ||
        pending.provider !== event.provider ||
        pending.requestId !== event.requestId
      ) {
        return;
      }

      clearPending(event.requestId);
      onNotice(
        `Prompt ready in ${getProviderLabel(event.provider)} — review it and press Send`,
      );
    });

    const unlistenError = providerGateway.onProviderError((event) => {
      const pending = pendingRef.current;
      if (
        !pending ||
        currentProviderRef.current !== event.provider ||
        pending.provider !== event.provider ||
        pending.requestId !== event.requestId
      ) {
        return;
      }

      clearPending(event.requestId);
      onNotice(event.message);
    });

    return () => {
      void unlistenFilled.then((unlisten) => unlisten());
      void unlistenError.then((unlisten) => unlisten());
      clearPending();
    };
  }, [clearPending, onNotice]);

  useEffect(() => {
    const pending = pendingRef.current;
    if (pending && pending.provider !== provider) clearPending();
  }, [clearPending, provider]);

  const placePrompt = useCallback(
    async (instruction: string, sourceText: string) => {
      if (!sourceText.trim()) {
        onNotice("Add or capture some text first");
        return;
      }

      clearPending();
      setIsWorking(true);
      const requestId = crypto.randomUUID();
      pendingRef.current = { provider, requestId };

      try {
        await ensureProvider();
        if (
          currentProviderRef.current !== provider ||
          pendingRef.current?.requestId !== requestId
        ) {
          return;
        }

        const prompt = await providerGateway.composePrompt(
          instruction,
          sourceText,
        );
        if (
          currentProviderRef.current !== provider ||
          pendingRef.current?.requestId !== requestId
        ) {
          return;
        }

        const timeout = window.setTimeout(() => {
          if (currentProviderRef.current !== provider) return;
          if (!clearPending(requestId)) return;
          onNotice(
            `${getProviderLabel(provider)} did not confirm the prompt was placed. Try again.`,
          );
        }, REQUEST_TIMEOUT_MS);

        pendingRef.current = { provider, requestId, timeout };
        onNotice(`Placing the prompt in ${getProviderLabel(provider)}…`);
        await providerGateway.fillPrompt(provider, prompt, requestId);
      } catch (error) {
        if (!clearPending(requestId)) return;
        onNotice(`Could not prepare the prompt: ${String(error)}`);
      }
    },
    [clearPending, ensureProvider, onNotice, provider],
  );

  return { isWorking, placePrompt };
}
