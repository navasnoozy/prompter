// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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
  expectedHost: string;
  prompt: string;
};

function createFill() {
  const factory = new Function(
    "window",
    "document",
    `return (${FILL_PROMPT_SOURCE});`,
  );
  const fakeWindow = {
    location: {
      href: "",
      protocol: "https:",
      hostname: "chatgpt.com",
      port: "",
    },
    getSelection: () => window.getSelection(),
  } as Record<string, unknown> & {
    location: {
      href: string;
      protocol: string;
      hostname: string;
      port: string;
    };
    getSelection: () => Selection | null;
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
    expectedHost: "chatgpt.com",
    prompt: "Prompt",
    ...overrides,
  };
}

describe("fill_prompt script", () => {
  beforeEach(() => {
    vi.spyOn(HTMLElement.prototype, "getClientRects").mockReturnValue([
      {
        width: 320,
        height: 80,
      },
    ] as unknown as DOMRectList);
  });

  afterEach(() => {
    document.body.innerHTML = "";
    vi.useRealTimers();
    vi.restoreAllMocks();
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

  it("replaces rich-editor insertion when it drops a blank line", async () => {
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

    expect(editor.querySelectorAll("p")).toHaveLength(3);
    expect(editor.querySelectorAll("p")[1]?.querySelector("br")).not.toBeNull();
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

  it("refuses to mutate an editor after navigation to another host", async () => {
    const textarea = document.createElement("textarea");
    document.body.appendChild(textarea);
    const { fill, fakeWindow } = createFill();
    fakeWindow.location.hostname = "evil.example";

    await fill(fillInput({ prompt: "Sensitive prompt" }));

    expect(textarea.value).toBe("");
    expect(fakeWindow.location.href).toContain("prompter://error");
    expect(fakeWindow.location.href).toContain("trusted+chat+page");
  });

  it("skips hidden, disabled, and read-only editor decoys", async () => {
    const hidden = document.createElement("textarea");
    hidden.style.display = "none";
    const disabled = document.createElement("textarea");
    disabled.disabled = true;
    const readOnly = document.createElement("textarea");
    readOnly.readOnly = true;
    const usable = document.createElement("textarea");
    document.body.append(hidden, disabled, readOnly, usable);
    const { fill, fakeWindow } = createFill();

    await fill(fillInput({ prompt: "Only the usable editor" }));

    expect(hidden.value).toBe("");
    expect(disabled.value).toBe("");
    expect(readOnly.value).toBe("");
    expect(usable.value).toBe("Only the usable editor");
    expect(fakeWindow.location.href).toContain("prompter://filled");
  });

  it("replaces rich-editor content when insertion leaves extra text", async () => {
    document.body.innerHTML = '<div id="editor" contenteditable="true"></div>';
    const editor = document.getElementById("editor") as HTMLElement;
    const documentWithExec = document as Document & {
      execCommand: (command: string, ui: boolean, value: string) => boolean;
    };
    documentWithExec.execCommand = () => {
      editor.innerHTML = "<p>Prompt</p><p>Unwanted old text</p>";
      return true;
    };
    const { fill, fakeWindow } = createFill();

    try {
      await fill(fillInput({ selectors: ["#editor"], prompt: "Prompt" }));
    } finally {
      delete (documentWithExec as { execCommand?: unknown }).execCommand;
    }

    expect(editor.textContent).toBe("Prompt");
    expect(fakeWindow.location.href).toContain("prompter://filled");
  });

  it("does not treat a deleted space as equivalent rich-editor content", async () => {
    document.body.innerHTML = '<div id="editor" contenteditable="true"></div>';
    const editor = document.getElementById("editor") as HTMLElement;
    const documentWithExec = document as Document & {
      execCommand: (command: string, ui: boolean, value: string) => boolean;
    };
    documentWithExec.execCommand = () => {
      editor.innerHTML = "<p>NewYork</p>";
      return true;
    };
    const { fill, fakeWindow } = createFill();

    try {
      await fill(fillInput({ selectors: ["#editor"], prompt: "New York" }));
    } finally {
      delete (documentWithExec as { execCommand?: unknown }).execCommand;
    }

    expect(editor.textContent).toBe("New York");
    expect(fakeWindow.location.href).toContain("prompter://filled");
  });

  it("does not treat collapsed repeated spaces as equivalent rich-editor content", async () => {
    document.body.innerHTML = '<div id="editor" contenteditable="true"></div>';
    const editor = document.getElementById("editor") as HTMLElement;
    const documentWithExec = document as Document & {
      execCommand: (command: string, ui: boolean, value: string) => boolean;
    };
    documentWithExec.execCommand = () => {
      editor.innerHTML = "<p>New York</p>";
      return true;
    };
    const { fill, fakeWindow } = createFill();

    try {
      await fill(fillInput({ selectors: ["#editor"], prompt: "New  York" }));
    } finally {
      delete (documentWithExec as { execCommand?: unknown }).execCommand;
    }

    expect(editor.textContent).toBe("New  York");
    expect(fakeWindow.location.href).toContain("prompter://filled");
  });

  it("reports an error when a change handler mutates a textarea in place", async () => {
    const textarea = document.createElement("textarea");
    textarea.addEventListener("change", () => {
      textarea.value = "site-normalized";
    });
    document.body.appendChild(textarea);
    const { fill, fakeWindow } = createFill();

    await fill(fillInput({ prompt: "Exact prompt" }));

    expect(textarea.value).toBe("site-normalized");
    expect(fakeWindow.location.href).toContain("prompter://error");
    expect(fakeWindow.location.href).toContain("editor_update_failed");
  });

  it("reports an error when final focus mutates a rich editor in place", async () => {
    document.body.innerHTML = '<div id="editor" contenteditable="true"></div>';
    const editor = document.getElementById("editor") as HTMLElement;
    let focusCount = 0;
    editor.addEventListener("focus", () => {
      focusCount += 1;
      if (focusCount > 1) editor.textContent = "site-normalized";
    });
    editor.addEventListener("change", () => editor.blur());
    const { fill, fakeWindow } = createFill();

    await fill(fillInput({ selectors: ["#editor"], prompt: "Exact prompt" }));

    expect(editor.textContent).toBe("site-normalized");
    expect(fakeWindow.location.href).toContain("prompter://error");
    expect(fakeWindow.location.href).toContain("editor_update_failed");
  });

  it("fails instead of mutating an editor detached by its click handler", async () => {
    const original = document.createElement("textarea");
    document.body.appendChild(original);
    original.addEventListener("click", () => {
      original.replaceWith(document.createElement("textarea"));
    });
    const { fill, fakeWindow } = createFill();

    await fill(fillInput({ prompt: "Sensitive prompt" }));

    expect(original.value).toBe("");
    expect(document.querySelector("textarea")?.value).toBe("");
    expect(fakeWindow.location.href).toContain("prompter://error");
    expect(fakeWindow.location.href).toContain("editor_update_failed");
  });

  it("stops a polling request when its page generation is cancelled", async () => {
    vi.useFakeTimers();
    const { fill, fakeWindow } = createFill();

    const pending = fill(fillInput({ selectors: ["#missing"] }));
    fakeWindow.__PROMPTER_FILL_GENERATION__ = 2;
    await vi.advanceTimersByTimeAsync(500);
    await pending;

    expect(fakeWindow.location.href).toBe("");
  });
});
