/**
 * Turns a physical key press into an accelerator the backend can parse.
 *
 * `KeyboardEvent.code` and the backend's key names are both W3C UI Events
 * code names, so a press maps across the boundary without a translation
 * table. This module only produces candidates and live feedback — the backend
 * remains the authority on whether a combination is acceptable and on how the
 * committed shortcut is finally displayed.
 */

/** Physical keys the backend's accelerator parser understands. */
const SUPPORTED_KEYS = new Set<string>([
  ...Array.from({ length: 26 }, (_, index) =>
    String.fromCharCode(65 + index),
  ).map((letter) => `Key${letter}`),
  ...Array.from({ length: 10 }, (_, digit) => `Digit${digit}`),
  ...Array.from({ length: 12 }, (_, index) => `F${index + 1}`),
  ...Array.from({ length: 10 }, (_, digit) => `Numpad${digit}`),
  "Backquote",
  "Backslash",
  "BracketLeft",
  "BracketRight",
  "Comma",
  "Equal",
  "Minus",
  "Period",
  "Quote",
  "Semicolon",
  "Slash",
  "Backspace",
  "Delete",
  "Enter",
  "Space",
  "Tab",
  "Escape",
  "Home",
  "End",
  "Insert",
  "PageUp",
  "PageDown",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "NumpadAdd",
  "NumpadDecimal",
  "NumpadDivide",
  "NumpadEnter",
  "NumpadEqual",
  "NumpadMultiply",
  "NumpadSubtract",
]);

const MODIFIER_KEYS = new Set<string>([
  "ShiftLeft",
  "ShiftRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
  "CapsLock",
]);

/** Mirrors the backend's renderer so live feedback matches the saved result. */
const KEY_GLYPHS: Record<string, string> = {
  Space: "Space",
  Enter: "↩",
  NumpadEnter: "↩",
  Tab: "⇥",
  Escape: "⎋",
  Backspace: "⌫",
  Delete: "⌦",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Home: "↖",
  End: "↘",
  PageUp: "⇞",
  PageDown: "⇟",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
};

export type ModifierFlags = {
  shift: boolean;
  control: boolean;
  alt: boolean;
  meta: boolean;
};

export type RecordingRejection = "needs_modifier" | "unsupported_key";

export type RecordingResult =
  /** Only modifiers are down; keep listening. */
  | { kind: "pending"; display: string }
  | { kind: "recorded"; accelerator: string; display: string }
  | { kind: "rejected"; reason: RecordingRejection; display: string }
  /** Escape on its own means "leave the shortcut alone". */
  | { kind: "cancelled" };

function modifiersOf(event: KeyboardEvent): ModifierFlags {
  return {
    shift: event.shiftKey,
    control: event.ctrlKey,
    alt: event.altKey,
    meta: event.metaKey,
  };
}

/**
 * ⌘, ⌥, or ⌃ must be present. Shift alone still produces ordinary characters,
 * so a Shift-only global shortcut would intercept typing system-wide — the
 * backend refuses those too.
 */
function hasGuardingModifier(modifiers: ModifierFlags): boolean {
  return modifiers.meta || modifiers.control || modifiers.alt;
}

/** Apple orders modifiers ⌃⌥⇧⌘ regardless of the order they were pressed. */
export function formatShortcut(
  modifiers: ModifierFlags,
  code: string | null,
): string {
  const parts: string[] = [];
  if (modifiers.control) parts.push("⌃");
  if (modifiers.alt) parts.push("⌥");
  if (modifiers.shift) parts.push("⇧");
  if (modifiers.meta) parts.push("⌘");
  if (code) parts.push(formatKey(code));
  return parts.join(" ");
}

function formatKey(code: string): string {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return KEY_GLYPHS[code] ?? code;
}

/**
 * Builds the backend's own canonical spelling, so the accelerator that comes
 * back in the saved status is identical to the one that was sent.
 */
function toAccelerator(modifiers: ModifierFlags, code: string): string {
  const parts: string[] = [];
  if (modifiers.shift) parts.push("shift");
  if (modifiers.control) parts.push("control");
  if (modifiers.alt) parts.push("alt");
  if (modifiers.meta) parts.push("super");
  parts.push(code);
  return parts.join("+");
}

export function readShortcutFromEvent(event: KeyboardEvent): RecordingResult {
  const modifiers = modifiersOf(event);

  if (MODIFIER_KEYS.has(event.code)) {
    return { kind: "pending", display: formatShortcut(modifiers, null) };
  }

  if (event.code === "Escape" && !hasGuardingModifier(modifiers)) {
    return { kind: "cancelled" };
  }

  if (!SUPPORTED_KEYS.has(event.code)) {
    return {
      kind: "rejected",
      reason: "unsupported_key",
      display: formatShortcut(modifiers, null),
    };
  }

  if (!hasGuardingModifier(modifiers)) {
    return {
      kind: "rejected",
      reason: "needs_modifier",
      display: formatShortcut(modifiers, event.code),
    };
  }

  return {
    kind: "recorded",
    accelerator: toAccelerator(modifiers, event.code),
    display: formatShortcut(modifiers, event.code),
  };
}

export const RECORDING_REJECTION_MESSAGES: Record<RecordingRejection, string> =
  {
    needs_modifier: "Hold ⌘, ⌥, or ⌃ together with another key.",
    unsupported_key: "That key cannot be used in a shortcut.",
  };
