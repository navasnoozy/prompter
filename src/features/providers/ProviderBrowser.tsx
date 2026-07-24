import type { RefObject } from "react";
import { Icon } from "../../shared/Icon";
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
          aria-hidden={panelOpen}
          aria-live="polite"
          className="provider-loading-placeholder"
          role="status"
        >
          <span className="empty-orbit">
            <Icon name="sparkle" size={25} />
          </span>
          <strong>Opening {label}…</strong>
          <p>Sign in here once, then use the same panel for every rewrite.</p>
        </div>
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
