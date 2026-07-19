(async function fillPrompt({
  provider,
  requestId,
  displayName,
  selectors,
  prompt,
}) {
  const pause = (milliseconds) =>
    new Promise((resolve) => setTimeout(resolve, milliseconds));

  const findEditor = () => {
    for (const selector of selectors) {
      const element = document.querySelector(selector);
      if (element) return element;
    }
    return null;
  };

  const signal = (kind, message = "") => {
    const params = new URLSearchParams({ provider, requestId, message });
    window.location.href = `prompter://${kind}?${params.toString()}`;
  };

  const startedAt = Date.now();
  let editor = findEditor();
  while (!editor && Date.now() - startedAt < 8000) {
    await pause(200);
    editor = findEditor();
  }

  if (!editor) {
    signal(
      "error",
      `The ${displayName} input box was not found. Finish signing in, then try again.`,
    );
    return;
  }

  editor.focus();
  editor.click();

  if (editor instanceof HTMLTextAreaElement || editor instanceof HTMLInputElement) {
    const prototype =
      editor instanceof HTMLTextAreaElement
        ? HTMLTextAreaElement.prototype
        : HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
    setter?.call(editor, prompt);
    editor.dispatchEvent(
      new InputEvent("input", {
        bubbles: true,
        inputType: "insertText",
        data: prompt,
      }),
    );
  } else {
    const selection = window.getSelection();
    const range = document.createRange();
    range.selectNodeContents(editor);
    selection?.removeAllRanges();
    selection?.addRange(range);

    let inserted = false;
    try {
      inserted = document.execCommand("insertText", false, prompt);
    } catch {
      // execCommand may throw in some engines; treat it as not inserted.
    }

    // Rich editors turn newlines into block nodes whose textContent has no
    // "\n", so the verification must ignore whitespace entirely.
    const withoutWhitespace = (value) => value.replace(/\s+/g, "");
    const editorHasPrompt = () =>
      withoutWhitespace(editor.textContent || "").includes(
        withoutWhitespace(prompt),
      );

    if (!inserted || !editorHasPrompt()) {
      const paragraphs = prompt.split("\n").map((line) => {
        const paragraph = document.createElement("p");
        if (line) {
          paragraph.textContent = line;
        } else {
          paragraph.appendChild(document.createElement("br"));
        }
        return paragraph;
      });
      editor.replaceChildren(...paragraphs);
      editor.dispatchEvent(
        new InputEvent("input", {
          bubbles: true,
          inputType: "insertText",
          data: prompt,
        }),
      );
    }
  }

  editor.dispatchEvent(new Event("change", { bubbles: true }));
  editor.focus();
  signal("filled");
})
