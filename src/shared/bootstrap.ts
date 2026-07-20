import { createDefaultInstructions } from "../features/instructions/defaults";
import type { InstructionPreset } from "../features/instructions/model";
import { decodeStoredInstructions } from "../features/instructions/storage";
import { initializeInstructionStore } from "../features/instructions/store";
import { isProvider, type Provider } from "../features/providers/model";
import { initializeProviderStore } from "../features/providers/store";
import {
  initializeSettingsStore,
  type AppTheme,
} from "../features/settings/store";
import { isRecord } from "./contracts";
import { publishNotice } from "./notices";
import {
  settingsGateway,
  SETTINGS_KEYS,
  type SettingsDocument,
  type SettingsKey,
} from "./settingsGateway";

// Storage keys used by releases that persisted into WKWebView localStorage.
// Each valid legacy value is migrated independently and deleted only after a
// confirmed durable save (or after a confirmed durable value supersedes it).
const LEGACY_KEYS = {
  presets: "prompter.presets.v1",
  selectedInstructionId: "prompter.selection.v1",
  theme: "prompter.theme.v1",
  provider: "prompter.provider.v1",
} as const satisfies Record<SettingsKey, string>;

export type BootState = {
  instructions: InstructionPreset[];
  selectedId: string | undefined;
  theme: AppTheme;
  provider: Provider;
};

type LegacySettings = {
  values: SettingsDocument;
  presentKeys: Set<SettingsKey>;
};

function readLegacySettings(): LegacySettings {
  const values: SettingsDocument = {};
  const presentKeys = new Set<SettingsKey>();

  function read(key: SettingsKey): string | null {
    try {
      return localStorage.getItem(LEGACY_KEYS[key]);
    } catch {
      return null;
    }
  }

  const rawPresets = read(SETTINGS_KEYS.presets);
  if (rawPresets) {
    try {
      const presets: unknown = JSON.parse(rawPresets);
      const decoded = decodeStoredInstructions(presets);
      if (decoded.length > 0) {
        values.presets = { version: 2, instructions: decoded };
        presentKeys.add(SETTINGS_KEYS.presets);
      }
    } catch {
      // A damaged legacy value is ignored without blocking other valid keys.
    }
  }

  const selection = read(SETTINGS_KEYS.selectedInstructionId)?.trim();
  if (selection) {
    values.selectedInstructionId = selection;
    presentKeys.add(SETTINGS_KEYS.selectedInstructionId);
  }

  const theme = read(SETTINGS_KEYS.theme);
  if (theme === "light" || theme === "dark") {
    values.theme = theme;
    presentKeys.add(SETTINGS_KEYS.theme);
  }

  const provider = read(SETTINGS_KEYS.provider);
  if (isProvider(provider)) {
    values.provider = provider;
    presentKeys.add(SETTINGS_KEYS.provider);
  }

  return { values, presentKeys };
}

function isFutureInstructionDocument(value: unknown): boolean {
  return (
    isRecord(value) &&
    typeof value.version === "number" &&
    value.version > 2
  );
}

function removeLegacyKeys(keys: Iterable<SettingsKey>): void {
  for (const key of keys) {
    try {
      localStorage.removeItem(LEGACY_KEYS[key]);
    } catch {
      // The durable copy is already confirmed; inaccessible legacy storage
      // does not make the current settings unsafe.
    }
  }
}

function systemTheme(): AppTheme {
  return typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export async function loadBootState(): Promise<BootState> {
  let durable: SettingsDocument = {};
  let durableLoaded = false;
  try {
    durable = await settingsGateway.load();
    durableLoaded = true;
  } catch {
    publishNotice(
      "error",
      "Prompter could not load saved settings. Defaults are being used for this session.",
    );
  }

  const legacy = readLegacySettings();
  const migration: SettingsDocument = {};
  const cleanupAfterSave = new Set<SettingsKey>();
  const cleanupNow = new Set<SettingsKey>();

  const durableInstructions = decodeStoredInstructions(durable.presets);
  const legacyInstructions = decodeStoredInstructions(legacy.values.presets);
  const futureInstructions = isFutureInstructionDocument(durable.presets);

  let instructions = durableInstructions;
  if (instructions.length > 0) {
    if (legacy.presentKeys.has(SETTINGS_KEYS.presets)) {
      cleanupNow.add(SETTINGS_KEYS.presets);
    }
  } else if (legacyInstructions.length > 0) {
    instructions = legacyInstructions;
    if (durableLoaded && !futureInstructions) {
      migration.presets = { version: 2, instructions };
      cleanupAfterSave.add(SETTINGS_KEYS.presets);
    }
  }
  const resolvedInstructions =
    instructions.length > 0 ? instructions : createDefaultInstructions();

  function chooseValue(
    key: Exclude<SettingsKey, "presets">,
    durableIsValid: (value: unknown) => boolean,
  ): unknown {
    if (durableIsValid(durable[key])) {
      if (legacy.presentKeys.has(key)) cleanupNow.add(key);
      return durable[key];
    }
    if (durableIsValid(legacy.values[key])) {
      if (durableLoaded) {
        migration[key] = legacy.values[key];
        cleanupAfterSave.add(key);
      }
      return legacy.values[key];
    }
    return undefined;
  }

  const validSelection = (value: unknown): value is string =>
    typeof value === "string" &&
    resolvedInstructions.some(({ id }) => id === value.trim());
  let selectedInstructionId: string | undefined;
  if (validSelection(durable.selectedInstructionId)) {
    selectedInstructionId = durable.selectedInstructionId.trim();
    if (legacy.presentKeys.has(SETTINGS_KEYS.selectedInstructionId)) {
      cleanupNow.add(SETTINGS_KEYS.selectedInstructionId);
    }
  } else if (validSelection(legacy.values.selectedInstructionId)) {
    selectedInstructionId = legacy.values.selectedInstructionId.trim();
    if (durableLoaded && !futureInstructions) {
      migration.selectedInstructionId = selectedInstructionId;
      cleanupAfterSave.add(SETTINGS_KEYS.selectedInstructionId);
    }
  } else if (
    durableLoaded &&
    typeof durable.selectedInstructionId === "string" &&
    durable.selectedInstructionId.length > 0 &&
    !futureInstructions
  ) {
    selectedInstructionId = resolvedInstructions[0].id;
    migration.selectedInstructionId = selectedInstructionId;
  }
  const theme = chooseValue(
    SETTINGS_KEYS.theme,
    (value) => value === "light" || value === "dark",
  );
  const provider = chooseValue(SETTINGS_KEYS.provider, isProvider);

  if (durableLoaded) removeLegacyKeys(cleanupNow);

  if (Object.keys(migration).length > 0) {
    const saved = await settingsGateway.writeMany(migration);
    if (saved) removeLegacyKeys(cleanupAfterSave);
  }

  return {
    instructions: resolvedInstructions,
    selectedId: selectedInstructionId,
    theme: theme === "light" || theme === "dark" ? theme : systemTheme(),
    provider: isProvider(provider) ? provider : "chatgpt",
  };
}

export async function bootstrapStores(): Promise<void> {
  const boot = await loadBootState();
  initializeInstructionStore(boot.instructions, boot.selectedId);
  initializeSettingsStore(boot.theme);
  initializeProviderStore(boot.provider);
}
