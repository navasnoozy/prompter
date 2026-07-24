import { create } from "zustand";
import { settingsGateway, SETTINGS_KEYS } from "../../shared/settingsGateway";

export type AppTheme = "light" | "dark";

type SettingsState = {
  theme: AppTheme;
  showSettings: boolean;
  setTheme: (theme: AppTheme) => void;
  openSettings: () => void;
  closeSettings: () => void;
};

function applyColorScheme(theme: AppTheme): void {
  document.documentElement.style.colorScheme = theme;
}

export const useSettingsStore = create<SettingsState>()((set) => ({
  theme: "light",
  showSettings: false,
  // Persist only explicit choices so a fresh install keeps following the
  // system appearance until the user picks a theme.
  setTheme: (theme) => {
    set({ theme });
    applyColorScheme(theme);
    void settingsGateway.write(SETTINGS_KEYS.theme, theme);
  },
  openSettings: () => set({ showSettings: true }),
  closeSettings: () => set({ showSettings: false }),
}));

export function initializeSettingsStore(theme: AppTheme): void {
  useSettingsStore.setState({ theme });
  applyColorScheme(theme);
}
