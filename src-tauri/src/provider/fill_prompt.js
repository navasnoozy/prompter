(async function fillPrompt({
  provider,
  requestId,
  displayName,
  selectors,
  expectedHost,
  prompt,
}) {
  const generationKey = "__PROMPTER_FILL_GENERATION__";
  const generation = (Number(window[generationKey]) || 0) + 1;
  window[generationKey] = generation;

  const isActive = () => window[generationKey] === generation;
  const isExpectedOrigin = () =>
    window.location.protocol === "https:" &&
    window.location.hostname === expectedHost &&
    (window.location.port === "" || window.location.port === "443");
  const pause = (milliseconds) =>
    new Promise((resolve) => setTimeout(resolve, milliseconds));

  const isUsableEditor = (element) => {
    if (!(element instanceof HTMLElement) || !element.isConnected) return false;
    if (element.closest("[hidden], [aria-hidden='true']")) return false;

    for (let current = element; current; current = current.parentElement) {
      const view = current.ownerDocument.defaultView;
      const style = view ? view.getComputedStyle(current) : null;
      if (
        style &&
        (style.display === "none" ||
          style.visibility === "hidden" ||
          style.visibility === "collapse" ||
          style.pointerEvents === "none")
      ) {
        return false;
      }
    }

    const hasVisibleRect = Array.from(element.getClientRects()).some(
      (rect) => rect.width > 0 && rect.height > 0,
    );
    if (!hasVisibleRect) return false;

    if (
      element instanceof HTMLTextAreaElement ||
      element instanceof HTMLInputElement
    ) {
      return (
        !element.disabled &&
        !element.readOnly &&
        element.getAttribute("aria-disabled") !== "true" &&
        (!(element instanceof HTMLInputElement) || element.type !== "hidden")
      );
    }
    return (
      (element.isContentEditable ||
        element.getAttribute("contenteditable") === "true") &&
      element.getAttribute("aria-disabled") !== "true"
    );
  };

  const findEditor = () => {
    for (const selector of selectors) {
      const candidates = document.querySelectorAll(selector);
      for (let index = 0; index < candidates.length; index += 1) {
        if (isUsableEditor(candidates[index])) return candidates[index];
      }
    }
    return null;
  };

  const signal = (kind, message = "", code = "internal") => {
    if (!isActive()) return;
    const params = new URLSearchParams({ provider, requestId, code, message });
    window.location.href = `prompter://${kind}?${params.toString()}`;
  };

  const rejectWrongOrigin = () => {
    if (isExpectedOrigin()) return false;
    signal(
      "error",
      `${displayName} navigated away from its trusted chat page. Return to ${expectedHost}, then try again.`,
      "wrong_host",
    );
    return true;
  };

  try {
    if (!isActive() || rejectWrongOrigin()) return;

    const startedAt = Date.now();
    let editor = findEditor();
    while (!editor && Date.now() - startedAt < 8000) {
      await pause(200);
      if (!isActive() || rejectWrongOrigin()) return;
      editor = findEditor();
    }

    if (!isActive() || rejectWrongOrigin()) return;
    if (!(editor instanceof HTMLElement)) {
      signal(
        "error",
        `The ${displayName} input box was not found. Finish signing in, then try again.`,
        "editor_not_found",
      );
      return;
    }

    const rejectStaleEditor = () => {
      if (editor.isConnected && findEditor() === editor) return false;
      signal(
        "error",
        `The ${displayName} input box changed while Prompter was updating it. Try again.`,
        "editor_update_failed",
      );
      return true;
    };
    let editorMatchesPrompt = () => false;

    editor.focus();
    editor.click();
    if (!isActive() || rejectWrongOrigin() || rejectStaleEditor()) return;

    if (
      editor instanceof HTMLTextAreaElement ||
      editor instanceof HTMLInputElement
    ) {
      const prototype =
        editor instanceof HTMLTextAreaElement
          ? HTMLTextAreaElement.prototype
          : HTMLInputElement.prototype;
      const descriptor = Object.getOwnPropertyDescriptor(prototype, "value");
      if (!descriptor || typeof descriptor.set !== "function") {
        signal(
          "error",
          `The ${displayName} input box could not be updated.`,
          "editor_update_failed",
        );
        return;
      }
      if (rejectStaleEditor()) return;
      descriptor.set.call(editor, prompt);
      editorMatchesPrompt = () => editor.value === prompt;
      editor.dispatchEvent(
        new InputEvent("input", {
          bubbles: true,
          inputType: "insertText",
          data: prompt,
        }),
      );
      if (rejectStaleEditor() || !editorMatchesPrompt()) {
        signal(
          "error",
          `The ${displayName} input box rejected the prompt.`,
          "editor_update_failed",
        );
        return;
      }
    } else {
      if (
        !editor.isContentEditable &&
        editor.getAttribute("contenteditable") !== "true"
      ) {
        signal(
          "error",
          `The ${displayName} input box could not be edited.`,
          "editor_update_failed",
        );
        return;
      }

      const selection = window.getSelection();
      if (!selection) {
        signal(
          "error",
          `The ${displayName} text selection is unavailable.`,
          "editor_update_failed",
        );
        return;
      }
      if (rejectStaleEditor()) return;
      const range = document.createRange();
      range.selectNodeContents(editor);
      selection.removeAllRanges();
      selection.addRange(range);

      let inserted = false;
      try {
        inserted = document.execCommand("insertText", false, prompt);
      } catch {
        // Some rich editors reject execCommand; the DOM fallback below keeps
        // text insertion deterministic without ever pressing Send.
      }

      const blockTags = [
        "ADDRESS",
        "ARTICLE",
        "ASIDE",
        "BLOCKQUOTE",
        "DIV",
        "FOOTER",
        "H1",
        "H2",
        "H3",
        "H4",
        "H5",
        "H6",
        "HEADER",
        "LI",
        "MAIN",
        "NAV",
        "P",
        "PRE",
        "SECTION",
      ];
      const isBlock = (node) =>
        node.nodeType === 1 && blockTags.indexOf(node.tagName) >= 0;
      const renderedText = (node) => {
        if (node.nodeType === 3) return node.nodeValue || "";
        if (node.nodeType !== 1) return "";
        if (node.tagName === "BR") return "\n";

        const parts = [];
        let inlineText = "";
        const flushInlineText = () => {
          if (inlineText) {
            parts.push(inlineText);
            inlineText = "";
          }
        };
        for (let index = 0; index < node.childNodes.length; index += 1) {
          const child = node.childNodes[index];
          let value = renderedText(child);
          if (isBlock(child)) {
            flushInlineText();
            // A lone BR is how rich editors commonly represent an empty block.
            if (value === "\n") value = "";
            parts.push(value);
          } else {
            inlineText += value;
          }
        }
        flushInlineText();
        return parts.join("\n");
      };
      const normalizeLineEndings = (value) => value.replace(/\r\n?/g, "\n");
      editorMatchesPrompt = () =>
        normalizeLineEndings(renderedText(editor)) ===
        normalizeLineEndings(prompt);

      if (rejectStaleEditor()) return;
      if (!inserted || !editorMatchesPrompt()) {
        if (rejectStaleEditor()) return;
        while (editor.firstChild) editor.removeChild(editor.firstChild);
        for (const line of prompt.split("\n")) {
          const paragraph = document.createElement("p");
          if (line) {
            paragraph.textContent = line;
          } else {
            paragraph.appendChild(document.createElement("br"));
          }
          editor.appendChild(paragraph);
        }
        editor.dispatchEvent(
          new InputEvent("input", {
            bubbles: true,
            inputType: "insertText",
            data: prompt,
          }),
        );
      }

      if (rejectStaleEditor() || !editorMatchesPrompt()) {
        signal(
          "error",
          `The ${displayName} input box rejected the prompt.`,
          "editor_update_failed",
        );
        return;
      }
    }

    if (!isActive() || rejectWrongOrigin() || rejectStaleEditor()) return;
    editor.dispatchEvent(new Event("change", { bubbles: true }));
    if (!isActive() || rejectWrongOrigin() || rejectStaleEditor()) return;
    if (!editorMatchesPrompt()) {
      signal(
        "error",
        `The ${displayName} input box changed the prompt after it was inserted. Try again.`,
        "editor_update_failed",
      );
      return;
    }
    editor.focus();
    if (!isActive() || rejectWrongOrigin() || rejectStaleEditor()) return;
    if (!editorMatchesPrompt()) {
      signal(
        "error",
        `The ${displayName} input box changed the prompt after it was inserted. Try again.`,
        "editor_update_failed",
      );
      return;
    }
    signal("filled");
  } catch {
    signal(
      "error",
      `Prompter could not update the ${displayName} input box. Reload the page and try again.`,
    );
  }
})
