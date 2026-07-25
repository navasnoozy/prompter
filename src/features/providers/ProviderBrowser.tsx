import type { RefObject } from "react";
import { getProviderLabel } from "./model";
import { useProviderStore } from "./store";

type ProviderBrowserProps = {
  hostRef: RefObject<HTMLDivElement | null>;
};

export function ProviderBrowser({ hostRef }: ProviderBrowserProps) {
  const provider = useProviderStore((state) => state.provider);
  const panelOpen = useProviderStore((state) => state.panelOpen);
  const navigation = useProviderStore(
    (state) => state.navigationByProvider[provider],
  );
  const label = getProviderLabel(provider);

  return (
    <section aria-label={`${label} browser`} className="browser-card">
      <div className="provider-webview-frame">
        <span
          aria-atomic="true"
          aria-live="polite"
          className="provider-navigation-status"
          role="status"
        >
          {panelOpen && navigation.isLoading
            ? `${label} page is loading.`
            : ""}
        </span>
        <div
          aria-busy={panelOpen && navigation.isLoading}
          className="provider-webview-host"
          id="provider-browser-content"
          ref={hostRef}
        />
      </div>
    </section>
  );
}
