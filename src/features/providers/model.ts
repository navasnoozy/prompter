export const PROVIDERS = {
  chatgpt: {
    label: "ChatGPT",
    logo: "◎",
  },
  gemini: {
    label: "Gemini",
    logo: "✦",
  },
} as const;

export type Provider = keyof typeof PROVIDERS;

export const PROVIDER_ORDER = Object.keys(PROVIDERS) as Provider[];

export function isProvider(value: unknown): value is Provider {
  return value === "chatgpt" || value === "gemini";
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

export type PromptFilledEvent = {
  provider: Provider;
  requestId: string;
};

export type ProviderErrorEvent = PromptFilledEvent & {
  message: string;
};
