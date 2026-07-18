import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  isProvider,
  type PromptFilledEvent,
  type Provider,
  type ProviderBounds,
  type ProviderErrorEvent,
} from "./model";

export const TAURI_COMMANDS = {
  composePrompt: "compose_prompt",
  fillProviderPrompt: "fill_provider_prompt",
  resizeProviderWebview: "resize_provider_webview",
  setProviderVisibility: "set_provider_visibility",
  showProviderWebview: "show_provider_webview",
} as const;

export const TAURI_EVENTS = {
  clipboardCaptured: "prompter://clipboard-captured",
  clipboardError: "prompter://clipboard-error",
  promptFilled: "prompter://prompt-filled",
  providerError: "prompter://provider-error",
} as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function parsePromptFilled(value: unknown): PromptFilledEvent | null {
  if (
    !isRecord(value) ||
    !isProvider(value.provider) ||
    typeof value.requestId !== "string" ||
    !value.requestId
  ) {
    return null;
  }

  return { provider: value.provider, requestId: value.requestId };
}

function parseProviderError(value: unknown): ProviderErrorEvent | null {
  const filled = parsePromptFilled(value);
  if (!filled || !isRecord(value) || typeof value.message !== "string") {
    return null;
  }

  return { ...filled, message: value.message };
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

  composePrompt(instruction: string, text: string): Promise<string> {
    return invoke<string>(TAURI_COMMANDS.composePrompt, { instruction, text });
  },

  fillPrompt(
    provider: Provider,
    prompt: string,
    requestId: string,
  ): Promise<void> {
    return invoke(TAURI_COMMANDS.fillProviderPrompt, {
      provider,
      prompt,
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
