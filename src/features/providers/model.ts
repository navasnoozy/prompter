import { isNonEmptyString, isRecord } from "../../shared/contracts";

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

export type ProviderErrorEvent = PromptFilledEvent & {
  code: ProviderErrorCode;
  message: string;
};

export const PROVIDER_CONTRACT_VERSION = 1;

const PROVIDER_ERROR_CODES = [
  "window_missing",
  "webview_missing",
  "webview_operation_failed",
  "invalid_bounds",
  "invalid_request",
  "wrong_host",
  "editor_not_found",
  "editor_update_failed",
  "missing_instruction",
  "missing_text",
  "prompt_too_large",
  "internal",
] as const;

export type ProviderErrorCode = (typeof PROVIDER_ERROR_CODES)[number];

const ERROR_CODES = new Set<string>(PROVIDER_ERROR_CODES);

export type ProviderCommandError = {
  version: 1;
  code: ProviderErrorCode;
  message: string;
};

export function parseProviderCommandError(
  value: unknown,
): ProviderCommandError | null {
  if (
    !isRecord(value) ||
    value.version !== PROVIDER_CONTRACT_VERSION ||
    typeof value.code !== "string" ||
    !ERROR_CODES.has(value.code) ||
    !isNonEmptyString(value.message)
  ) {
    return null;
  }

  return {
    version: PROVIDER_CONTRACT_VERSION,
    code: value.code as ProviderErrorCode,
    message: value.message,
  };
}
