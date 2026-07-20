import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { isRecord } from "../../shared/contracts";
import {
  isProvider,
  parseProviderCommandError,
  type PromptComposition,
  type PromptFilledEvent,
  type Provider,
  type ProviderBounds,
  type ProviderCommandError,
  type ProviderErrorEvent,
} from "./model";

export const TAURI_COMMANDS = {
  placePrompt: "place_prompt",
  resizeProviderWebview: "resize_provider_webview",
  setProviderVisibility: "set_provider_visibility",
  showProviderWebview: "show_provider_webview",
} as const;

export const TAURI_EVENTS = {
  promptFilled: "prompter://prompt-filled",
  providerError: "prompter://provider-error",
} as const;

function parsePromptFilled(value: unknown): PromptFilledEvent | null {
  if (
    !isRecord(value) ||
    value.version !== 1 ||
    !isProvider(value.provider) ||
    typeof value.requestId !== "string" ||
    !value.requestId
  ) {
    return null;
  }

  return { version: 1, provider: value.provider, requestId: value.requestId };
}

function parseProviderError(value: unknown): ProviderErrorEvent | null {
  const filled = parsePromptFilled(value);
  const error = parseProviderCommandError(value);
  if (!filled || !error) {
    return null;
  }

  return { ...filled, code: error.code, message: error.message };
}

export function normalizeProviderError(error: unknown): ProviderCommandError {
  return (
    parseProviderCommandError(error) ?? {
      version: 1,
      code: "internal",
      message: "Could not reach the provider. Please try again.",
    }
  );
}

export const providerGateway = {
  show(provider: Provider, bounds: ProviderBounds): Promise<void> {
    return invoke(TAURI_COMMANDS.showProviderWebview, { provider, bounds });
  },

  resize(provider: Provider, bounds: ProviderBounds): Promise<void> {
    return invoke(TAURI_COMMANDS.resizeProviderWebview, { provider, bounds });
  },

  setVisibility(provider: Provider, visible: boolean): Promise<void> {
    return invoke(TAURI_COMMANDS.setProviderVisibility, { provider, visible });
  },

  placePrompt(
    provider: Provider,
    composition: PromptComposition,
    requestId: string,
  ): Promise<void> {
    return invoke(TAURI_COMMANDS.placePrompt, {
      provider,
      composition,
      requestId,
    });
  },

  onPromptFilled(
    handler: (payload: PromptFilledEvent) => void,
  ): Promise<UnlistenFn> {
    return listen<unknown>(TAURI_EVENTS.promptFilled, (event) => {
      const payload = parsePromptFilled(event.payload);
      if (payload) handler(payload);
    });
  },

  onProviderError(
    handler: (payload: ProviderErrorEvent) => void,
  ): Promise<UnlistenFn> {
    return listen<unknown>(TAURI_EVENTS.providerError, (event) => {
      const payload = parseProviderError(event.payload);
      if (payload) handler(payload);
    });
  },
};
