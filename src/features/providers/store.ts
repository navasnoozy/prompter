import { create } from "zustand";
import { settingsGateway, SETTINGS_KEYS } from "../../shared/settingsGateway";
import {
  unavailableProviderNavigationState,
  type Provider,
  type ProviderNavigationState,
} from "./model";
import {
  DEFAULT_NEW_CHAT_MODE,
  type NewChatMode,
  type NewChatOverrides,
  type NewChatProviderOverride,
} from "./newChat";

type ProviderState = {
  provider: Provider;
  isPlacing: boolean;
  navigationByProvider: Record<Provider, ProviderNavigationState>;
  placementBridgeReady: boolean;
  panelOpen: boolean;
  newChatMode: NewChatMode;
  newChatOverrides: NewChatOverrides;
  setProvider: (provider: Provider) => void;
  setNewChatMode: (mode: NewChatMode) => void;
  setNewChatOverride: (
    provider: Provider,
    override: NewChatProviderOverride | null,
  ) => void;
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
  newChatMode: DEFAULT_NEW_CHAT_MODE,
  newChatOverrides: {},
  setProvider: (provider) => {
    if (get().provider === provider) return;
    set({ provider, panelOpen: false });
    void settingsGateway.write(SETTINGS_KEYS.provider, provider);
  },
  setNewChatMode: (newChatMode) => {
    if (get().newChatMode === newChatMode) return;
    set({ newChatMode });
    void settingsGateway.write(SETTINGS_KEYS.newChatMode, newChatMode);
  },
  setNewChatOverride: (provider, override) => {
    // Written whole rather than per-field: one settings key holds every
    // provider's overrides, so a partial write would be a lost update.
    const next: NewChatOverrides = { ...get().newChatOverrides };
    if (override === null || (!override.url && !override.matcher)) {
      delete next[provider];
    } else {
      next[provider] = override;
    }
    set({ newChatOverrides: next });
    void settingsGateway.write(SETTINGS_KEYS.newChatOverrides, next);
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

export function initializeProviderStore(
  provider: Provider,
  newChatMode: NewChatMode = DEFAULT_NEW_CHAT_MODE,
  newChatOverrides: NewChatOverrides = {},
): void {
  useProviderStore.setState({
    provider,
    newChatMode,
    newChatOverrides,
    navigationByProvider: initialNavigationState(),
  });
}
