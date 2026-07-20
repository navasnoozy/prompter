import { create } from "zustand";
import { settingsGateway, SETTINGS_KEYS } from "../../shared/settingsGateway";
import type { Provider } from "./model";

type ProviderState = {
  provider: Provider;
  isPlacing: boolean;
  placementBridgeReady: boolean;
  panelOpen: boolean;
  setProvider: (provider: Provider) => void;
};

export const useProviderStore = create<ProviderState>()((set, get) => ({
  provider: "chatgpt",
  isPlacing: false,
  placementBridgeReady: false,
  panelOpen: false,
  setProvider: (provider) => {
    if (get().provider === provider) return;
    set({ provider, panelOpen: false });
    void settingsGateway.write(SETTINGS_KEYS.provider, provider);
  },
}));

export function initializeProviderStore(provider: Provider): void {
  useProviderStore.setState({ provider });
}
