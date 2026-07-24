import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  PromptFilledEventSchema,
  ProviderErrorEventSchema,
  ProviderNavigationStateSchema,
} from "../../shared/schemas";
import {
  parseProviderCommandError,
  type PromptComposition,
  type PromptFilledEvent,
  type Provider,
  type ProviderBounds,
  type ProviderCommandError,
  type ProviderErrorEvent,
  type ProviderNavigationAction,
  type ProviderNavigationState,
} from "./model";

export const TAURI_COMMANDS = {
  controlProviderNavigation: "control_provider_navigation",
  getProviderNavigationState: "get_provider_navigation_state",
  placePrompt: "place_prompt",
  resizeProviderWebview: "resize_provider_webview",
  setProviderVisibility: "set_provider_visibility",
  showProviderWebview: "show_provider_webview",
} as const;

export const TAURI_EVENTS = {
  promptFilled: "prompter://prompt-filled",
  providerError: "prompter://provider-error",
  providerNavigationState: "prompter://provider-navigation-state",
} as const;

function parsePromptFilled(value: unknown): PromptFilledEvent | null {
  const result = PromptFilledEventSchema.safeParse(value);
  return result.success ? result.data : null;
}

function parseProviderError(value: unknown): ProviderErrorEvent | null {
  const result = ProviderErrorEventSchema.safeParse(value);
  return result.success ? result.data : null;
}

function parseProviderNavigationState(
  value: unknown,
): ProviderNavigationState | null {
  const result = ProviderNavigationStateSchema.safeParse(value);
  return result.success ? result.data : null;
}

function requireProviderNavigationState(
  value: unknown,
  expectedProvider: Provider,
): ProviderNavigationState {
  const navigation = parseProviderNavigationState(value);
  if (!navigation || navigation.provider !== expectedProvider) {
    throw new Error("The provider browser returned an invalid state.");
  }
  return navigation;
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
  async getNavigationState(
    provider: Provider,
  ): Promise<ProviderNavigationState> {
    const value = await invoke<unknown>(
      TAURI_COMMANDS.getProviderNavigationState,
      { provider },
    );
    return requireProviderNavigationState(value, provider);
  },

  async controlNavigation(
    provider: Provider,
    generation: number,
    action: ProviderNavigationAction,
  ): Promise<ProviderNavigationState> {
    const value = await invoke<unknown>(
      TAURI_COMMANDS.controlProviderNavigation,
      {
        provider,
        generation,
        action,
      },
    );
    return requireProviderNavigationState(value, provider);
  },

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

  onNavigationState(
    handler: (payload: ProviderNavigationState) => void,
  ): Promise<UnlistenFn> {
    return listen<unknown>(TAURI_EVENTS.providerNavigationState, (event) => {
      const payload = parseProviderNavigationState(event.payload);
      if (payload) handler(payload);
    });
  },
};
