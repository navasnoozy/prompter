import { isProvider, type Provider } from "./model";

const PROVIDER_KEY = "prompter.provider.v1";

export function loadStoredProvider(): Provider {
  try {
    const stored = localStorage.getItem(PROVIDER_KEY);
    if (isProvider(stored)) return stored;
  } catch {
    // Fall through to the default provider.
  }
  return "chatgpt";
}

export function saveStoredProvider(provider: Provider): void {
  try {
    localStorage.setItem(PROVIDER_KEY, provider);
  } catch {
    // Provider persistence is optional; the session keeps the choice.
  }
}
