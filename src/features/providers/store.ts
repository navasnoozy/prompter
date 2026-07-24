import { create } from "zustand";
import { settingsGateway, SETTINGS_KEYS } from "../../shared/settingsGateway";
import {
  unavailableProviderNavigationState,
  type Provider,
  type ProviderNavigationState,
} from "./model";

type ProviderState = {
  provider: Provider;
  isPlacing: boolean;
  navigationByProvider: Record<Provider, ProviderNavigationState>;
  placementBridgeReady: boolean;
  panelOpen: boolean;
  setProvider: (provider: Provider) => void;
  updateNavigationState: (navigation: ProviderNavigationState) => void;
};

function initialNavigationState(): Record<Provider, ProviderNavigationState> {
  return {
    chatgpt: unavailableProviderNavigationState("chatgpt"),
    gemini: unavailableProviderNavigationState("gemini"),
  };
}

export const useProviderStore = create<ProviderState>()((set, get) => ({
  provider: "chatgpt",
  isPlacing: false,
  navigationByProvider: initialNavigationState(),
  placementBridgeReady: false,
  panelOpen: false,
  setProvider: (provider) => {
    if (get().provider === provider) return;
    set({ provider, panelOpen: false });
    void settingsGateway.write(SETTINGS_KEYS.provider, provider);
  },
  updateNavigationState: (navigation) => {
    const current = get().navigationByProvider[navigation.provider];
    const isNewer =
      navigation.generation > current.generation ||
      (navigation.generation === current.generation &&
        navigation.revision > current.revision);
    if (!isNewer) return;

    set((state) => ({
      navigationByProvider: {
        ...state.navigationByProvider,
        [navigation.provider]: navigation,
      },
    }));
  },
}));

export function initializeProviderStore(provider: Provider): void {
  useProviderStore.setState({
    provider,
    navigationByProvider: initialNavigationState(),
  });
}
