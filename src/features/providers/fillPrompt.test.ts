// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it, vi } from "vitest";

const FILL_PROMPT_SOURCE = readFileSync(
  join(
    dirname(fileURLToPath(import.meta.url)),
    "../../../src-tauri/src/provider/fill_prompt.js",
  ),
  "utf8",
);

type FillInput = {
  provider: string;
  requestId: string;
  displayName: string;
  selectors: string[];
  prompt: string;
};

function createFill() {
  const factory = new Function(
    "window",
    "document",
    `return (${FILL_PROMPT_SOURCE});`,
  );
  const fakeWindow = {
    location: { href: "" },
    getSelection: () => window.getSelection(),
  };
  const fill = factory(fakeWindow, document) as (
    input: FillInput,
  ) => Promise<void>;
  return { fill, fakeWindow };
}

function fillInput(overrides: Partial<FillInput>): FillInput {
  return {
    provider: "chatgpt",
    requestId: "req-1",
    displayName: "ChatGPT",
    selectors: ["textarea"],
    prompt: "Prompt",
    ...overrides,
  };
}

describe("fill_prompt script", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.useRealTimers();
  });

  it("sets a textarea value exactly and signals filled", async () => {
    const textarea = document.createElement("textarea");
    document.body.appendChild(textarea);
    const events: string[] = [];
    textarea.addEventListener("input", () => events.push("input"));
    const { fill, fakeWindow } = createFill();

    await fill(
      fillInput({ prompt: "Rewrite clearly\n\nOriginal text \"quoted\"" }),
    );

    expect(textarea.value).toBe("Rewrite clearly\n\nOriginal text \"quoted\"");
    expect(events).toContain("input");
    expect(fakeWindow.location.href).toContain("prompter://filled");
    expect(fakeWindow.location.href).toContain("requestId=req-1");
  });

  it("builds one paragraph per line for contenteditable editors", async () => {
    document.body.innerHTML = '<div id="editor" contenteditable="true"></div>';
    const editor = document.getElementById("editor") as HTMLElement;
    const { fill, fakeWindow } = createFill();

    await fill(
      fillInput({
        selectors: ["#editor"],
        prompt: "Rewrite clearly\n\nOriginal text",
      }),
    );

    const paragraphs = Array.from(editor.querySelectorAll("p"));
    expect(paragraphs.map((paragraph) => paragraph.textContent)).toEqual([
      "Rewrite clearly",
      "",
      "Original text",
    ]);
    expect(paragraphs[1].querySelector("br")).not.toBeNull();
    expect(fakeWindow.location.href).toContain("prompter://filled");
  });

  it("keeps the editor's own insertion when it normalizes newlines away", async () => {
    document.body.innerHTML = '<div id="editor" contenteditable="true"></div>';
    const editor = document.getElementById("editor") as HTMLElement;
    // Simulate a rich editor: insertText succeeds but stores the paragraphs
    // as block nodes whose concatenated textContent has no newlines.
    const documentWithExec = document as Document & {
      execCommand: (command: string, ui: boolean, value: string) => boolean;
    };
    documentWithExec.execCommand = () => {
      editor.innerHTML = "<p>Rewrite clearly</p><p>Original text</p>";
      return true;
    };
    const { fill, fakeWindow } = createFill();

    try {
      await fill(
        fillInput({
          selectors: ["#editor"],
          prompt: "Rewrite clearly\n\nOriginal text",
        }),
      );
    } finally {
      delete (documentWithExec as { execCommand?: unknown }).execCommand;
    }

    // The whitespace-insensitive verification must accept this result and
    // never fall back to replacing the editor's DOM (which would add a <br>
    // paragraph for the blank line).
    expect(editor.querySelectorAll("p")).toHaveLength(2);
    expect(editor.querySelector("br")).toBeNull();
    expect(fakeWindow.location.href).toContain("prompter://filled");
  });

  it("signals an error when no editor appears before the deadline", async () => {
    vi.useFakeTimers();
    const { fill, fakeWindow } = createFill();

    const pending = fill(fillInput({ selectors: ["#missing"] }));
    await vi.advanceTimersByTimeAsync(9_000);
    await pending;

    expect(fakeWindow.location.href).toContain("prompter://error");
    expect(fakeWindow.location.href).toContain("requestId=req-1");
  });
});
