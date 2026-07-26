(async function fillPrompt({
  provider,
  requestId,
  displayName,
  selectors,
  expectedHost,
  prompt,
  newChat,
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

  const isVisibleElement = (element) => {
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

    return Array.from(element.getClientRects()).some(
      (rect) => rect.width > 0 && rect.height > 0,
    );
  };

  const isUsableEditor = (element) => {
    if (!isVisibleElement(element)) return false;

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

  // Confidence weights for identifying the provider's "New chat" control.
  //
  // The ordering encodes how well each signal survives a redesign, not how
  // convenient it is to read. A test id is written for automation and is worth
  // an immediate accept. An explicit `aria-label` is close behind. Incidental
  // text is weaker than both, and deliberately below ACCEPT on its own: an
  // untitled conversation in the sidebar is also called "New chat", and
  // clicking that would silently reopen an old thread — the exact failure this
  // whole feature exists to prevent. Text therefore has to be corroborated by
  // something structural before anything is clicked.
  //
  // ACCEPT sits above text plus a single attribute for that same reason: a
  // sidebar row can easily share one generic attribute with the real control,
  // so a lone attribute is not allowed to be the corroboration that text needs.
  const SCORE = {
    testId: 100,
    ariaLabel: 60,
    text: 40,
    knownSelector: 30,
    href: 20,
    attribute: 10,
  };
  const ATTRIBUTE_SCORE_CAP = 30;
  const ACCEPT_SCORE = 60;
  const MAX_CANDIDATES = 600;
  const MAX_NAME_LENGTH = 120;
  const NEW_CHAT_RESET_TIMEOUT_MS = 4000;
  const NEW_CHAT_SETTLE_TIMEOUT_MS = 1500;
  const CANDIDATE_SELECTOR =
    "a[href], button, [role='button'], [data-testid], [data-test-id], [aria-label]";

  const normalizeName = (value) =>
    String(value || "")
      .replace(/\s+/g, " ")
      .trim()
      .slice(0, MAX_NAME_LENGTH)
      .toLowerCase();

  // Text as a person reads it: keyboard hints, icons, and screen-reader-only
  // decoration are skipped so ChatGPT's "New chat ⇧⌘O" row reduces to the
  // name the user would have pointed at.
  const visibleTextOf = (element) => {
    let text = "";
    const walk = (node) => {
      if (text.length > MAX_NAME_LENGTH) return;
      if (node.nodeType === 3) {
        text += node.nodeValue || "";
        return;
      }
      if (node.nodeType !== 1) return;
      if (node.tagName === "KBD") return;
      if (node.getAttribute("aria-hidden") === "true") return;
      for (let index = 0; index < node.childNodes.length; index += 1) {
        walk(node.childNodes[index]);
      }
    };
    walk(element);
    return normalizeName(text);
  };

  const attributeOf = (element, name) => {
    const value = element.getAttribute(name);
    return value === null ? null : value.trim();
  };

  const testIdOf = (element) =>
    attributeOf(element, "data-testid") ?? attributeOf(element, "data-test-id");

  // Compared as resolved paths so a matcher recorded as `/` still matches an
  // href the page later renders absolutely.
  const hrefPathOf = (element) => {
    const raw = attributeOf(element, "href");
    if (raw === null) return null;
    try {
      return new URL(raw, window.location.href).pathname;
    } catch {
      return raw;
    }
  };

  const expectedHrefPath = (value) => {
    try {
      return new URL(value, window.location.href).pathname;
    } catch {
      return value;
    }
  };

  const runNewChatStep = async () => {
    const matcher = newChat.matcher;
    const expectedLabels = new Set(
      (newChat.labels || []).map(normalizeName).filter(Boolean),
    );
    if (matcher && matcher.label) expectedLabels.add(normalizeName(matcher.label));

    const freshPaths = new Set(newChat.freshPaths || []);
    // Google addresses a multi-account session as `/u/<n>/…`; the account
    // segment says nothing about whether a conversation is open.
    const isFreshPath = () => {
      const path = window.location.pathname.replace(/^\/u\/\d+(?=\/|$)/, "");
      return freshPaths.has(path || "/");
    };

    const scoreOf = (element, knownSelectorMatches) => {
      let score = 0;
      if (matcher && matcher.testId) {
        const testId = testIdOf(element);
        if (testId !== null && testId === matcher.testId) score += SCORE.testId;
      }

      const ariaLabel = normalizeName(attributeOf(element, "aria-label"));
      if (ariaLabel && expectedLabels.has(ariaLabel)) {
        score += SCORE.ariaLabel;
      } else if (expectedLabels.has(visibleTextOf(element))) {
        score += SCORE.text;
      }

      if (knownSelectorMatches.has(element)) score += SCORE.knownSelector;

      if (matcher && matcher.href) {
        const path = hrefPathOf(element);
        if (path !== null && path === expectedHrefPath(matcher.href)) {
          score += SCORE.href;
        }
      }

      if (matcher && matcher.attributes && matcher.attributes.length > 0) {
        let attributeScore = 0;
        for (const attribute of matcher.attributes) {
          const actual = attributeOf(element, attribute.name);
          if (actual !== null && actual === attribute.value) {
            attributeScore += SCORE.attribute;
          }
        }
        score += Math.min(attributeScore, ATTRIBUTE_SCORE_CAP);
      }

      return score;
    };

    const findControl = () => {
      const knownSelectorMatches = new Set();
      for (const selector of newChat.selectors || []) {
        let matches;
        try {
          matches = document.querySelectorAll(selector);
        } catch {
          // A selector that this engine cannot parse simply contributes no
          // candidates; the remaining signals still stand on their own.
          continue;
        }
        for (let index = 0; index < matches.length; index += 1) {
          knownSelectorMatches.add(matches[index]);
        }
      }

      const seen = new Set(knownSelectorMatches);
      const candidates = document.querySelectorAll(CANDIDATE_SELECTOR);
      for (
        let index = 0;
        index < candidates.length && seen.size < MAX_CANDIDATES;
        index += 1
      ) {
        seen.add(candidates[index]);
      }

      let best = null;
      let bestScore = 0;
      for (const candidate of seen) {
        if (!isVisibleElement(candidate)) continue;
        if (candidate.getAttribute("aria-disabled") === "true") continue;
        if (candidate.disabled === true) continue;

        const score = scoreOf(candidate, knownSelectorMatches);
        if (score > bestScore) {
          best = candidate;
          bestScore = score;
        }
      }
      return bestScore >= ACCEPT_SCORE ? best : null;
    };

    const editorIsEmpty = () => {
      const editor = findEditor();
      if (!editor) return false;
      const value =
        editor instanceof HTMLTextAreaElement ||
        editor instanceof HTMLInputElement
          ? editor.value
          : editor.textContent;
      return String(value || "").trim() === "";
    };

    // Already blank: clicking would be a no-op, and skipping it avoids
    // disturbing a page the user may have just opened themselves.
    if (isFreshPath() && editorIsEmpty()) return true;

    const control = findControl();
    if (!control) return false;

    control.click();

    // Some frameworks only commit on a full pointer sequence. That is the
    // exception, so it is tried once, late, rather than on every reset.
    let retried = false;
    const startedAt = Date.now();
    while (Date.now() - startedAt < NEW_CHAT_RESET_TIMEOUT_MS) {
      if (isFreshPath() && editorIsEmpty()) return true;
      if (!retried && Date.now() - startedAt > NEW_CHAT_RESET_TIMEOUT_MS / 2) {
        retried = true;
        if (control.isConnected) {
          for (const type of ["pointerdown", "mousedown", "mouseup", "click"]) {
            control.dispatchEvent(
              new MouseEvent(type, { bubbles: true, cancelable: true }),
            );
          }
        }
      }
      await pause(100);
      if (!isActive() || !isExpectedOrigin()) return false;
    }

    // The path is what proves the conversation was replaced. An editor that is
    // still settling is handled by the wait that follows.
    if (isFreshPath()) {
      const settleStartedAt = Date.now();
      while (Date.now() - settleStartedAt < NEW_CHAT_SETTLE_TIMEOUT_MS) {
        if (editorIsEmpty()) break;
        await pause(100);
        if (!isActive() || !isExpectedOrigin()) return false;
      }
      return true;
    }
    return false;
  };

  try {
    if (!isActive() || rejectWrongOrigin()) return;

    if (newChat) {
      const reset = await runNewChatStep();
      if (!isActive() || rejectWrongOrigin()) return;
      if (!reset) {
        signal(
          "error",
          `Prompter could not start a new ${displayName} chat from the page.`,
          "new_chat_unavailable",
        );
        return;
      }
    }

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
