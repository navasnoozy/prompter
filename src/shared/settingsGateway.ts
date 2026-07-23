import { invoke } from "@tauri-apps/api/core";
import { publishNotice } from "./notices";
import { SettingsLoadResponseSchema } from "./schemas";

export const SETTINGS_KEYS = {
  presets: "presets",
  selectedInstructionId: "selectedInstructionId",
  theme: "theme",
  provider: "provider",
} as const;

export type SettingsKey = (typeof SETTINGS_KEYS)[keyof typeof SETTINGS_KEYS];
export type SettingsDocument = Partial<Record<SettingsKey, unknown>>;

export const SETTINGS_COMMANDS = {
  load: "load_settings",
  save: "save_settings",
} as const;

let activeSessionId: number | null = null;
let nextRevision = 0;

function parseLoadResponse(value: unknown): SettingsDocument {
  const result = SettingsLoadResponseSchema.safeParse(value);
  if (!result.success) {
    throw new Error("The native settings response was invalid.");
  }

  const document: SettingsDocument = {};
  for (const key of Object.values(SETTINGS_KEYS)) {
    if (Object.prototype.hasOwnProperty.call(result.data.entries, key)) {
      document[key] = result.data.entries[key];
    }
  }
  activeSessionId = result.data.sessionId;
  nextRevision = 0;
  return document;
}

async function dispatchWrite(entries: SettingsDocument): Promise<boolean> {
  if (activeSessionId === null) {
    publishNotice(
      "error",
      "Prompter could not save your settings because the settings session is unavailable.",
    );
    return false;
  }

  const revision = ++nextRevision;
  try {
    // Dispatch immediately. The native layer applies per-key revisions under
    // one mutex, so out-of-order command completion cannot restore stale data.
    await invoke(SETTINGS_COMMANDS.save, {
      entries,
      sessionId: activeSessionId,
      revision,
    });
    return true;
  } catch {
    publishNotice(
      "error",
      "Prompter could not save your settings. Changes may be lost when it closes.",
    );
    return false;
  }
}

export const settingsGateway = {
  async load(): Promise<SettingsDocument> {
    activeSessionId = null;
    return parseLoadResponse(await invoke<unknown>(SETTINGS_COMMANDS.load));
  },

  write(key: SettingsKey, value: unknown): Promise<boolean> {
    return dispatchWrite({ [key]: value });
  },

  writeMany(entries: SettingsDocument): Promise<boolean> {
    return dispatchWrite(entries);
  },
};
