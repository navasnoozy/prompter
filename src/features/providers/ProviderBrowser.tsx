import type { RefObject } from "react";
import { Icon } from "../../shared/Icon";
import { getProviderLabel, type Provider } from "./model";

type ProviderBrowserProps = {
  hostRef: RefObject<HTMLDivElement | null>;
  provider: Provider;
};

export function ProviderBrowser({ hostRef, provider }: ProviderBrowserProps) {
  const label = getProviderLabel(provider);

  return (
    <section className="browser-card" aria-label={`${label} browser`}>
      <div className="provider-webview-frame">
        <div className="provider-loading-placeholder">
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
