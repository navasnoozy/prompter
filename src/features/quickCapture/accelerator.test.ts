import { describe, expect, it } from "vitest";
import { formatShortcut, readShortcutFromEvent } from "./accelerator";

type PressOptions = {
  code: string;
  shift?: boolean;
  control?: boolean;
  alt?: boolean;
  meta?: boolean;
  repeat?: boolean;
};

function press({
  code,
  shift = false,
  control = false,
  alt = false,
  meta = false,
}: PressOptions): KeyboardEvent {
  return {
    code,
    shiftKey: shift,
    ctrlKey: control,
    altKey: alt,
    metaKey: meta,
  } as KeyboardEvent;
}

describe("shortcut recording", () => {
  it("emits the accelerator spelling the backend canonicalizes to", () => {
    // The recorder and the backend must agree on the exact string, otherwise
    // the saved status would differ from what was just recorded and the UI
    // would visibly correct itself.
    const result = readShortcutFromEvent(
      press({ code: "KeyP", shift: true, meta: true }),
    );

    expect(result).toEqual({
      kind: "recorded",
      accelerator: "shift+super+KeyP",
      display: "⇧ ⌘ P",
    });
  });

  it("orders modifiers the way macOS does, not the way they were pressed", () => {
    const result = readShortcutFromEvent(
      press({ code: "KeyK", meta: true, shift: true, alt: true, control: true }),
    );

    expect(result).toEqual({
      kind: "recorded",
      accelerator: "shift+control+alt+super+KeyK",
      display: "⌃ ⌥ ⇧ ⌘ K",
    });
  });

  it("keeps listening while only modifiers are held", () => {
    expect(readShortcutFromEvent(press({ code: "MetaLeft", meta: true }))).toEqual(
      { kind: "pending", display: "⌘" },
    );
    expect(
      readShortcutFromEvent(
        press({ code: "ShiftRight", meta: true, shift: true }),
      ),
    ).toEqual({ kind: "pending", display: "⇧ ⌘" });
  });

  it("refuses combinations that would intercept ordinary typing", () => {
    for (const event of [
      press({ code: "KeyP" }),
      press({ code: "KeyP", shift: true }),
      press({ code: "Space", shift: true }),
    ]) {
      expect(readShortcutFromEvent(event)).toMatchObject({
        kind: "rejected",
        reason: "needs_modifier",
      });
    }
  });

  it("refuses keys the backend's parser cannot express", () => {
    expect(
      readShortcutFromEvent(press({ code: "ContextMenu", meta: true })),
    ).toMatchObject({ kind: "rejected", reason: "unsupported_key" });
    expect(
      readShortcutFromEvent(press({ code: "F13", meta: true })),
    ).toMatchObject({ kind: "rejected", reason: "unsupported_key" });
  });

  it("treats a bare Escape as cancel but allows it with a modifier", () => {
    expect(readShortcutFromEvent(press({ code: "Escape" }))).toEqual({
      kind: "cancelled",
    });
    expect(readShortcutFromEvent(press({ code: "Escape", shift: true }))).toEqual(
      { kind: "cancelled" },
    );
    expect(readShortcutFromEvent(press({ code: "Escape", meta: true }))).toEqual({
      kind: "recorded",
      accelerator: "super+Escape",
      display: "⌘ ⎋",
    });
  });

  it("renders named keys as the glyphs the backend renders", () => {
    const cases: Array<[string, string]> = [
      ["Space", "⌘ Space"],
      ["Enter", "⌘ ↩"],
      ["ArrowLeft", "⌘ ←"],
      ["Digit1", "⌘ 1"],
      ["Slash", "⌘ /"],
      ["F7", "⌘ F7"],
    ];
    for (const [code, expected] of cases) {
      expect(formatShortcut({ shift: false, control: false, alt: false, meta: true }, code)).toBe(
        expected,
      );
    }
  });
});
