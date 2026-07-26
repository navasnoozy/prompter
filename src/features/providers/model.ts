import { z } from "zod";
import {
  ProviderCommandErrorSchema,
  ProviderErrorCodeSchema,
  ProviderNavigationStateSchema,
  ProviderSchema,
} from "../../shared/schemas";

// `host` and `newChatUrl` mirror `expected_fill_host` and `new_chat_url` in
// `src-tauri/src/provider/config.rs`. The native side stays authoritative — it
// re-validates every override — but Settings needs them to explain a rejected
// address while the user is still typing it.
export const PROVIDERS = {
  chatgpt: {
    label: "ChatGPT",
    logo: "◎",
    host: "chatgpt.com",
    newChatUrl: "https://chatgpt.com/",
  },
  gemini: {
    label: "Gemini",
    logo: "✦",
    host: "gemini.google.com",
    newChatUrl: "https://gemini.google.com/app/new",
  },
} as const;

export type Provider = keyof typeof PROVIDERS;

export const PROVIDER_ORDER = Object.keys(PROVIDERS) as Provider[];

export function isProvider(value: unknown): value is Provider {
  return ProviderSchema.safeParse(value).success;
}

export function getProviderLabel(provider: Provider): string {
  return PROVIDERS[provider].label;
}

export type ProviderBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export const PROVIDER_NAVIGATION_ACTIONS = [
  "back",
  "forward",
  "reload",
  "stop",
] as const;

export type ProviderNavigationAction =
  (typeof PROVIDER_NAVIGATION_ACTIONS)[number];

export type ProviderNavigationState = z.infer<
  typeof ProviderNavigationStateSchema
>;

export function unavailableProviderNavigationState(
  provider: Provider,
): ProviderNavigationState {
  return {
    version: 1,
    provider,
    generation: 0,
    revision: 0,
    available: false,
    canGoBack: false,
    canGoForward: false,
    isLoading: false,
  };
}

export type PromptComposition = {
  beforeText: string;
  text: string;
  afterText: string;
};

export type PromptFilledEvent = {
  version: 1;
  provider: Provider;
  requestId: string;
};

export type ProviderErrorCode = z.infer<typeof ProviderErrorCodeSchema>;

export type ProviderErrorEvent = PromptFilledEvent & {
  code: ProviderErrorCode;
  message: string;
};

export const PROVIDER_CONTRACT_VERSION = 1;

export type ProviderCommandError = {
  version: 1;
  code: ProviderErrorCode;
  message: string;
};

export function parseProviderCommandError(
  value: unknown,
): ProviderCommandError | null {
  const result = ProviderCommandErrorSchema.safeParse(value);
  return result.success ? result.data : null;
}
