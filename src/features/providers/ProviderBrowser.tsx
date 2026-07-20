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
  const label = getProviderLabel(provider);

  return (
    <section className="browser-card" aria-label={`${label} browser`}>
      <div className="provider-webview-frame">
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
        <div className="provider-webview-host" ref={hostRef} />
      </div>
    </section>
  );
}
