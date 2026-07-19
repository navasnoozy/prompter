import { initializeInstructionStore } from "../features/instructions/store";
import { decodeStoredInstructions } from "../features/instructions/storage";
import { createDefaultInstructions } from "../features/instructions/defaults";
import { isProvider, type Provider } from "../features/providers/model";
import { initializeProviderStore } from "../features/providers/store";
import {
  initializeSettingsStore,
  type AppTheme,
} from "../features/settings/store";
import type { InstructionPreset } from "../features/instructions/model";
import { settingsGateway, SETTINGS_KEYS } from "./settingsGateway";

// Storage keys used by releases that persisted into WKWebView localStorage.
// Read-only: migration copies them into the durable settings file once and
// leaves the originals untouched as a backup.
const LEGACY_KEYS = {
  presets: "prompter.presets.v1",
  selection: "prompter.selection.v1",
  theme: "prompter.theme.v1",
  provider: "prompter.provider.v1",
} as const;

export type BootState = {
  instructions: InstructionPreset[];
  selectedId: string | undefined;
  theme: AppTheme;
  provider: Provider;
};

type RawSettings = {
  presets: unknown;
  selectedInstructionId: unknown;
  theme: unknown;
  provider: unknown;
};

function readLegacySettings(): Partial<RawSettings> | null {
  let rawPresets: string | null;
  try {
    rawPresets = localStorage.getItem(LEGACY_KEYS.presets);
  } catch {
    return null;
  }
  if (!rawPresets) return null;

  let presets: unknown;
  try {
    presets = JSON.parse(rawPresets);
  } catch {
    return null;
  }
  if (decodeStoredInstructions(presets).length === 0) return null;

  const legacy: Partial<RawSettings> = { presets };
  try {
    const selection = localStorage.getItem(LEGACY_KEYS.selection);
    if (selection) legacy.selectedInstructionId = selection;
    const theme = localStorage.getItem(LEGACY_KEYS.theme);
    if (theme === "light" || theme === "dark") legacy.theme = theme;
    const provider = localStorage.getItem(LEGACY_KEYS.provider);
    if (isProvider(provider)) legacy.provider = provider;
  } catch {
    // Optional keys; the presets alone are worth migrating.
  }
  return legacy;
}

function systemTheme(): AppTheme {
  return typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export async function loadBootState(): Promise<BootState> {
  let raw: Partial<RawSettings> = {};
  try {
    raw = {
      presets: await settingsGateway.read(SETTINGS_KEYS.presets),
      selectedInstructionId: await settingsGateway.read(
        SETTINGS_KEYS.selectedInstructionId,
      ),
      theme: await settingsGateway.read(SETTINGS_KEYS.theme),
      provider: await settingsGateway.read(SETTINGS_KEYS.provider),
    };
  } catch {
    // The durable store is unreachable; run on defaults for this session.
  }

  if (raw.presets === undefined || raw.presets === null) {
    const legacy = readLegacySettings();
    if (legacy) {
      raw = { ...raw, ...legacy };
      const decoded = decodeStoredInstructions(legacy.presets);
      void settingsGateway.writeMany({
        [SETTINGS_KEYS.presets]: { version: 2, instructions: decoded },
        ...(legacy.selectedInstructionId !== undefined && {
          [SETTINGS_KEYS.selectedInstructionId]: legacy.selectedInstructionId,
        }),
        ...(legacy.theme !== undefined && {
          [SETTINGS_KEYS.theme]: legacy.theme,
        }),
        ...(legacy.provider !== undefined && {
          [SETTINGS_KEYS.provider]: legacy.provider,
        }),
      });
    }
  }

  const instructions = decodeStoredInstructions(raw.presets);
  return {
    instructions:
      instructions.length > 0 ? instructions : createDefaultInstructions(),
    selectedId:
      typeof raw.selectedInstructionId === "string"
        ? raw.selectedInstructionId
        : undefined,
    theme:
      raw.theme === "light" || raw.theme === "dark" ? raw.theme : systemTheme(),
    provider: isProvider(raw.provider) ? raw.provider : "chatgpt",
  };
}

export async function bootstrapStores(): Promise<void> {
  const boot = await loadBootState();
  initializeInstructionStore(boot.instructions, boot.selectedId);
  initializeSettingsStore(boot.theme);
  initializeProviderStore(boot.provider);
}
