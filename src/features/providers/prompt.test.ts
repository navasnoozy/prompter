import { describe, expect, it } from "vitest";
import {
  isPromptTooLarge,
  MAX_PROMPT_BYTES,
  promptByteLength,
} from "./prompt";

describe("prompt byte contract", () => {
  it("matches the native composition including separators and trimming", () => {
    expect(
      promptByteLength({
        beforeText: "  Rewrite  ",
        text: "  Source  ",
        afterText: "  Return only text  ",
      }),
    ).toBe(new TextEncoder().encode("Rewrite\n\nSource\n\nReturn only text").length);
  });

  it("measures UTF-8 bytes rather than JavaScript UTF-16 code units", () => {
    expect(
      promptByteLength({ beforeText: "a", text: "✨", afterText: "" }),
    ).toBe(6);
  });

  it("accepts the exact boundary and rejects one extra byte", () => {
    const exactText = "x".repeat(MAX_PROMPT_BYTES - 3);
    expect(
      isPromptTooLarge({ beforeText: "a", text: exactText, afterText: "" }),
    ).toBe(false);
    expect(
      isPromptTooLarge({
        beforeText: "a",
        text: `${exactText}x`,
        afterText: "",
      }),
    ).toBe(true);
  });
});
