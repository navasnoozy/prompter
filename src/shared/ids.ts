// Random identifier for presets and prompt requests. Falls back to
// getRandomValues because crypto.randomUUID needs a newer WebKit than the
// oldest macOS Prompter supports.
export function createId(): string {
  if (typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }

  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
  );
}
