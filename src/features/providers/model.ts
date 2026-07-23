import { z } from "zod";
import { ProviderCommandErrorSchema, ProviderErrorCodeSchema, ProviderSchema } from "../../shared/schemas";

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
