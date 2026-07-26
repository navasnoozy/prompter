// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import {
  decodeStoredNewChatMode,
  decodeStoredNewChatOverrides,
  describeNewChatMatcher,
  parseNewChatElement,
  validateNewChatUrl,
} from "./newChat";

/** The element ChatGPT's sidebar renders for its New chat row. */
const CHATGPT_NEW_CHAT = `
<a class="group __menu-item hoverable" data-fill="" data-sidebar-item="true"
   data-testid="create-new-chat-button" href="/" data-discover="true">
  <div class="flex min-w-0 grow items-center gap-2.5">
    <div class="flex items-center justify-center icon"><svg aria-hidden="true"></svg></div>
    <div class="truncate">New chat</div>
  </div>
  <div class="trailing"><kbd><kbd>⇧</kbd><kbd>⌘</kbd><kbd>O</kbd></kbd></div>
</a>`;

function parsed(html: string) {
  const result = parseNewChatElement(html);
  if (!result.ok) throw new Error(`expected a matcher, got: ${result.message}`);
  return result;
}

describe("reading a pasted New chat element", () => {
  it("keeps the identifiers and drops the styling", () => {
    const { matcher } = parsed(CHATGPT_NEW_CHAT);

    expect(matcher.testId).toBe("create-new-chat-button");
    expect(matcher.href).toBe("/");
    expect(matcher.attributes).toContainEqual({
      name: "data-sidebar-item",
      value: "true",
    });
    const names = matcher.attributes?.map(({ name }) => name) ?? [];
    expect(names).not.toContain("class");
    expect(names).not.toContain("style");
  });

  it("reads the name a person would point at, without the keyboard hint", () => {
    // "New chat ⇧⌘O" must reduce to "New chat", or the label never matches the
    // rendered row and the strongest human-readable signal is wasted.
    expect(parsed(CHATGPT_NEW_CHAT).matcher.label).toBe("New chat");
  });

  it("prefers an explicit accessible name over the visible text", () => {
    const { matcher } = parsed(
      `<button aria-label="New chat" data-test-id="new-chat-button"><span>+</span></button>`,
    );

    expect(matcher.label).toBe("New chat");
    expect(matcher.testId).toBe("new-chat-button");
  });

  it("refuses attributes that describe appearance or current state", () => {
    const { matcher } = parsed(
      `<button data-test-id="new-chat-button" class="btn primary" style="color:red"
               aria-expanded="true" data-state="open" data-radix-collection-item=""
               onclick="alert(1)">New chat</button>`,
    );

    expect(matcher.attributes ?? []).toHaveLength(0);
    expect(matcher.testId).toBe("new-chat-button");
  });

  it("drops identifiers a build tool generated", () => {
    const { matcher } = parsed(
      `<button data-test-id="new-chat-button" id=":r7:" name="mat-button-1428"
               data-scope="new-chat">New chat</button>`,
    );

    const names = matcher.attributes?.map(({ name }) => name) ?? [];
    expect(names).toEqual(["data-scope"]);
  });

  it("ignores a link that points at one particular conversation", () => {
    // A sidebar row for an untitled thread is also called "New chat"; recording
    // its permalink would teach the matcher to reopen an old conversation.
    const { matcher } = parsed(
      `<a href="/c/68f1a2b3-4c5d-6e7f-8a9b-0c1d2e3f4a5b" data-testid="history-item-0">New chat</a>`,
    );

    expect(matcher.href).toBeUndefined();
  });

  it("keeps flag attributes that carry no value", () => {
    const { matcher } = parsed(
      `<button data-new-chat="" data-test-id="new-chat-button">New chat</button>`,
    );

    expect(matcher.attributes).toContainEqual({
      name: "data-new-chat",
      value: "",
    });
  });

  it("explains a paste that is not an element", () => {
    for (const input of ["", "   ", "New chat", "https://chatgpt.com/"]) {
      expect(parseNewChatElement(input).ok).toBe(false);
    }
  });

  it("refuses an element with nothing worth matching on", () => {
    const result = parseNewChatElement(`<div class="wrapper"></div>`);

    expect(result.ok).toBe(false);
  });

  it("never lets pasted markup become live content", () => {
    const result = parseNewChatElement(
      `<button data-test-id="x"><img src="https://example.invalid/pixel.png">New chat</button>`,
    );

    expect(result.ok).toBe(true);
    // Nothing from the paste is retained as markup: only named values survive.
    expect(JSON.stringify(result)).not.toContain("<img");
  });

  it("describes a saved matcher the same way it described the paste", () => {
    const { matcher, signals } = parsed(CHATGPT_NEW_CHAT);

    expect(describeNewChatMatcher(matcher)).toEqual(signals);
  });
});

describe("validating a New chat address", () => {
  it("accepts a same-origin address, including a Google account path", () => {
    expect(
      validateNewChatUrl("gemini", "https://gemini.google.com/u/1/app/new"),
    ).toBeNull();
    expect(validateNewChatUrl("chatgpt", "https://chatgpt.com/new")).toBeNull();
    expect(
      validateNewChatUrl("chatgpt", "https://chatgpt.com/?model=gpt-5"),
    ).toBeNull();
  });

  it("refuses anything that would move the pane off the provider", () => {
    const rejected: [Parameters<typeof validateNewChatUrl>[0], string][] = [
      ["chatgpt", "http://chatgpt.com/"],
      ["chatgpt", "https://chatgpt.com:8443/"],
      ["chatgpt", "https://evil.chatgpt.com/"],
      ["chatgpt", "https://chatgpt.com.evil.invalid/"],
      ["chatgpt", "https://user:pass@chatgpt.com/"],
      ["chatgpt", "javascript:alert(1)"],
      ["chatgpt", "data:text/html,<b>x</b>"],
      ["chatgpt", "not a url"],
      ["chatgpt", "https://gemini.google.com/app/new"],
      ["gemini", "https://chatgpt.com/"],
    ];

    for (const [provider, candidate] of rejected) {
      expect(validateNewChatUrl(provider, candidate)).not.toBeNull();
    }
  });
});

describe("reading the stored settings back", () => {
  it("falls back to the default for an unknown mode", () => {
    expect(decodeStoredNewChatMode("button")).toBe("button");
    expect(decodeStoredNewChatMode("nonsense")).toBe("auto");
    expect(decodeStoredNewChatMode(undefined)).toBe("auto");
  });

  it("keeps the providers it can read and drops the ones it cannot", () => {
    const overrides = decodeStoredNewChatOverrides({
      chatgpt: { url: "https://chatgpt.com/new" },
      gemini: { url: "https://evil.invalid/" },
      unknown: { url: "https://chatgpt.com/" },
    });

    expect(overrides.chatgpt).toEqual({ url: "https://chatgpt.com/new" });
    expect(overrides.gemini).toBeUndefined();
    expect(Object.keys(overrides)).toEqual(["chatgpt"]);
  });

  it("keeps a usable button description when the address has gone bad", () => {
    const overrides = decodeStoredNewChatOverrides({
      chatgpt: {
        url: "https://evil.invalid/",
        matcher: { testId: "create-new-chat-button" },
      },
    });

    expect(overrides.chatgpt).toEqual({
      matcher: { testId: "create-new-chat-button" },
    });
  });

  it("survives a settings value of the wrong shape entirely", () => {
    expect(decodeStoredNewChatOverrides(null)).toEqual({});
    expect(decodeStoredNewChatOverrides("chatgpt")).toEqual({});
    expect(decodeStoredNewChatOverrides([1, 2, 3])).toEqual({});
    expect(
      decodeStoredNewChatOverrides({ chatgpt: { matcher: { label: "" } } }),
    ).toEqual({});
  });
});
