import { create } from "zustand";
import { settingsGateway, SETTINGS_KEYS } from "../../shared/settingsGateway";
import type { Provider } from "./model";

type ProviderState = {
  provider: Provider;
  isPlacing: boolean;
  setProvider: (provider: Provider) => void;
};

export const useProviderStore = create<ProviderState>()((set) => ({
  provider: "chatgpt",
  isPlacing: false,
  setProvider: (provider) => {
    set({ provider });
    void settingsGateway.write(SETTINGS_KEYS.provider, provider);
  },
}));

export function initializeProviderStore(provider: Provider): void {
  useProviderStore.setState({ provider });
}
