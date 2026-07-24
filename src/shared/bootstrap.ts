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
import { publishNotice } from "./notices";
import {
  settingsGateway,
  type SettingsDocument,
} from "./settingsGateway";

export type BootState = {
  instructions: InstructionPreset[];
  selectedId: string | undefined;
  theme: AppTheme;
  provider: Provider;
};

function systemTheme(): AppTheme {
  return typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export async function loadBootState(): Promise<BootState> {
  let durable: SettingsDocument = {};
  try {
    durable = await settingsGateway.load();
  } catch {
    publishNotice(
      "error",
      "Prompter could not load saved settings. Defaults are being used for this session.",
    );
  }

  const instructions = decodeStoredInstructions(durable.presets);
  const resolvedInstructions =
    instructions.length > 0 ? instructions : createDefaultInstructions();

  const selectedInstructionId =
    typeof durable.selectedInstructionId === "string" &&
    resolvedInstructions.some(
      ({ id }) => id === durable.selectedInstructionId,
    )
      ? (durable.selectedInstructionId as string)
      : undefined;

  const theme =
    durable.theme === "light" || durable.theme === "dark"
      ? durable.theme
      : systemTheme();

  const provider = isProvider(durable.provider) ? durable.provider : "chatgpt";

  return {
    instructions: resolvedInstructions,
    selectedId: selectedInstructionId,
    theme,
    provider,
  };
}

export async function bootstrapStores(): Promise<void> {
  const boot = await loadBootState();
  initializeInstructionStore(boot.instructions, boot.selectedId);
  initializeSettingsStore(boot.theme);
  initializeProviderStore(boot.provider);
}
