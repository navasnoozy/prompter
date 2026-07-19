import { LazyStore } from "@tauri-apps/plugin-store";
import { publishNotice } from "./notices";

export const SETTINGS_KEYS = {
  presets: "presets",
  selectedInstructionId: "selectedInstructionId",
  theme: "theme",
  provider: "provider",
} as const;

export type SettingsKey = (typeof SETTINGS_KEYS)[keyof typeof SETTINGS_KEYS];

const SETTINGS_FILE = "settings.json";

const store = new LazyStore(SETTINGS_FILE);

// Durable app-data persistence (survives WKWebView storage purges).
// Reads may throw and are handled by the bootstrap; writes surface failures
// on the notice bar instead of losing data silently.
export const settingsGateway = {
  read(key: SettingsKey): Promise<unknown> {
    return store.get(key);
  },

  async write(key: SettingsKey, value: unknown): Promise<void> {
    try {
      await store.set(key, value);
      await store.save();
    } catch {
      publishNotice(
        "error",
        "Prompter could not save your settings. Changes may be lost when it closes.",
      );
    }
  },

  async writeMany(entries: Partial<Record<SettingsKey, unknown>>): Promise<void> {
    try {
      for (const [key, value] of Object.entries(entries)) {
        await store.set(key, value);
      }
      await store.save();
    } catch {
      publishNotice(
        "error",
        "Prompter could not save your settings. Changes may be lost when it closes.",
      );
    }
  },
};
