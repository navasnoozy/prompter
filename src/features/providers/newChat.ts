import { z } from "zod";
import { PROVIDERS, PROVIDER_ORDER, type Provider } from "./model";

/**
 * How a placement should reach a blank conversation.
 *
 * The two mechanisms fail in different ways, which is why the user picks
 * between them instead of getting one hard-coded choice. Clicking the page's
 * own control is instant and keeps the app mounted, but depends on that
 * page's markup. Navigating to a new-chat address always works, but pays a
 * full reload. `auto` tries the click first and navigates only if it fails.
 */
export const NEW_CHAT_MODES = ["auto", "button", "url", "off"] as const;
export type NewChatMode = (typeof NEW_CHAT_MODES)[number];
export const DEFAULT_NEW_CHAT_MODE: NewChatMode = "auto";

export const NEW_CHAT_MODE_LABELS: Record<
  NewChatMode,
  { title: string; detail: string }
> = {
  auto: {
    title: "Automatic (recommended)",
    detail: "Press the page's New chat button, and reload the page if that fails.",
  },
  button: {
    title: "Button only",
    detail: "Press the page's New chat button. Never reloads the page.",
  },
  url: {
    title: "Address only",
    detail: "Always open the new chat address. Slower, but does not depend on the page.",
  },
  off: {
    title: "Off",
    detail: "Keep the conversation that is already open.",
  },
};

/**
 * Mirrors the limits enforced in `src-tauri/src/provider/new_chat.rs`. The
 * native side is authoritative; these exist so a bad paste is explained in the
 * form rather than rejected after the user saves it.
 */
const MAX_SIGNAL_BYTES = 256;
const MAX_ATTRIBUTES = 8;
const MAX_URL_BYTES = 2_048;
const MAX_PASTE_BYTES = 64 * 1024;

const ALLOWED_PLAIN_ATTRIBUTES = ["href", "id", "name", "role", "type"];

function byteLength(value: string): number {
  return new TextEncoder().encode(value).length;
}

const BoundedSignalSchema = z
  .string()
  .trim()
  .min(1)
  .refine((value) => byteLength(value) <= MAX_SIGNAL_BYTES)
  .refine((value) => !/\p{Cc}/u.test(value));

export const NewChatMatcherSchema = z
  .strictObject({
    testId: BoundedSignalSchema.optional(),
    label: BoundedSignalSchema.optional(),
    href: BoundedSignalSchema.optional(),
    attributes: z
      .array(
        z.strictObject({
          name: z
            .string()
            .trim()
            .min(1)
            .refine((name) => isAllowedAttributeName(name.toLowerCase())),
          value: z
            .string()
            .trim()
            .refine((value) => byteLength(value) <= MAX_SIGNAL_BYTES)
            .refine((value) => !/\p{Cc}/u.test(value)),
        }),
      )
      .max(MAX_ATTRIBUTES)
      .optional(),
  })
  .refine(
    (matcher) =>
      matcher.testId !== undefined ||
      matcher.label !== undefined ||
      matcher.href !== undefined ||
      (matcher.attributes?.length ?? 0) > 0,
    { message: "The description has nothing to match on." },
  );

export type NewChatMatcher = z.infer<typeof NewChatMatcherSchema>;

export const NewChatProviderOverrideSchema = z.strictObject({
  url: z.string().trim().min(1).optional(),
  matcher: NewChatMatcherSchema.optional(),
});

export type NewChatProviderOverride = z.infer<
  typeof NewChatProviderOverrideSchema
>;

export type NewChatOverrides = Partial<
  Record<Provider, NewChatProviderOverride>
>;

/**
 * Same shape rule as the native allowlist: the `data-`/`aria-` families plus a
 * short list of identity attributes. `class` and `style` are absent by
 * design — they describe appearance and change on nearly every deploy.
 */
export function isAllowedAttributeName(name: string): boolean {
  if (!/^[a-z0-9_-]+$/.test(name)) return false;
  if (name.startsWith("data-") || name.startsWith("aria-")) {
    return name.length > "data-".length;
  }
  return ALLOWED_PLAIN_ATTRIBUTES.includes(name);
}

// ---------------------------------------------------------------------------
// Reading settings back
// ---------------------------------------------------------------------------

export function decodeStoredNewChatMode(value: unknown): NewChatMode {
  return NEW_CHAT_MODES.includes(value as NewChatMode)
    ? (value as NewChatMode)
    : DEFAULT_NEW_CHAT_MODE;
}

/**
 * A damaged or half-written override must not cost the user the whole
 * setting, so each provider is validated on its own and a bad one is dropped.
 */
export function decodeStoredNewChatOverrides(value: unknown): NewChatOverrides {
  if (typeof value !== "object" || value === null) return {};

  const overrides: NewChatOverrides = {};
  for (const provider of PROVIDER_ORDER) {
    const candidate = (value as Record<string, unknown>)[provider];
    if (candidate === undefined) continue;

    const result = NewChatProviderOverrideSchema.safeParse(candidate);
    if (!result.success) continue;
    if (result.data.url === undefined && result.data.matcher === undefined) {
      continue;
    }
    // An address that no longer passes validation would be rejected natively
    // on every placement, so it is dropped here and the built-in one is used.
    if (
      result.data.url !== undefined &&
      validateNewChatUrl(provider, result.data.url) !== null
    ) {
      if (result.data.matcher === undefined) continue;
      overrides[provider] = { matcher: result.data.matcher };
      continue;
    }
    overrides[provider] = result.data;
  }
  return overrides;
}

export function newChatUrlFor(
  provider: Provider,
  overrides: NewChatOverrides,
): string | undefined {
  return overrides[provider]?.url;
}

export function newChatMatcherFor(
  provider: Provider,
  overrides: NewChatOverrides,
): NewChatMatcher | undefined {
  return overrides[provider]?.matcher;
}

/**
 * Plain-language list of what a saved matcher looks for, so the setting can be
 * checked at a glance long after it was pasted.
 */
export function describeNewChatMatcher(matcher: NewChatMatcher): string[] {
  const signals: string[] = [];
  if (matcher.testId) signals.push(`test id "${matcher.testId}"`);
  if (matcher.label) signals.push(`name "${matcher.label}"`);
  if (matcher.href) signals.push(`link "${matcher.href}"`);
  for (const { name, value } of matcher.attributes ?? []) {
    signals.push(value === "" ? name : `${name}="${value}"`);
  }
  return signals;
}

// ---------------------------------------------------------------------------
// Address validation
// ---------------------------------------------------------------------------

/**
 * Returns a message explaining why the address is unusable, or `null` when it
 * is fine. The rules match `resolve_new_chat_url` on the native side: a
 * placement refuses to fill a page that is not on the provider's own host, so
 * an address that leaves the origin would strand the pane somewhere the prompt
 * can never land.
 */
export function validateNewChatUrl(
  provider: Provider,
  candidate: string,
): string | null {
  const trimmed = candidate.trim();
  if (!trimmed) return "Enter an address, or leave the field empty.";
  if (byteLength(trimmed) > MAX_URL_BYTES) return "That address is too long.";

  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return "That is not a valid web address.";
  }

  if (url.protocol !== "https:") return "The address must start with https://.";
  if (url.username || url.password) {
    return "The address must not contain a username or password.";
  }
  if (url.port !== "" && url.port !== "443") {
    return "The address must use the standard https port.";
  }

  const { host, label } = PROVIDERS[provider];
  if (url.hostname !== host) {
    return `The address must stay on ${host}, so it must look like https://${host}/… for ${label}.`;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Reading a pasted element
// ---------------------------------------------------------------------------

/**
 * Attributes that describe what a control is doing right now rather than
 * which control it is. Matching on them makes an override rot faster than
 * having no override at all, so they are dropped from a paste.
 */
const VOLATILE_ATTRIBUTE_NAMES = new Set([
  "aria-activedescendant",
  "aria-busy",
  "aria-checked",
  "aria-controls",
  "aria-current",
  "aria-describedby",
  "aria-disabled",
  "aria-expanded",
  "aria-haspopup",
  "aria-hidden",
  "aria-labelledby",
  "aria-live",
  "aria-owns",
  "aria-pressed",
  "aria-selected",
  "aria-valuenow",
  "data-active",
  "data-checked",
  "data-disabled",
  "data-focus",
  "data-focused",
  "data-highlighted",
  "data-hover",
  "data-index",
  "data-key",
  "data-loading",
  "data-open",
  "data-pressed",
  "data-reactid",
  "data-reactroot",
  "data-revealed",
  "data-selected",
  "data-state",
  "data-value",
]);

/**
 * Framework bookkeeping. The names are generated per build, so a match today
 * says nothing about tomorrow.
 */
const VOLATILE_ATTRIBUTE_PREFIXES = [
  "data-headlessui-",
  "data-ng-",
  "data-radix-",
  "data-svelte-",
  "data-v-",
];

/**
 * `role` and `type` are structural HTML rather than identity: nearly every
 * clickable row on a provider page carries them, so matching on one would let
 * a sidebar conversation borrow the confidence meant for the real control.
 */
const GENERIC_ATTRIBUTE_NAMES = new Set(["role", "type"]);

/**
 * Values a build tool produced. An id like `:r7:` or `mat-button-14` is
 * reassigned on the next render, so it is worse than no signal at all.
 */
const GENERATED_VALUE_PATTERNS = [
  /^:[a-z0-9]+:$/i,
  /^[0-9a-f]{12,}$/i,
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-/i,
  /^(cdk|ember|headlessui|mat|mui|ng|radix)[-:]/i,
  /\d{4,}/,
];

function looksGenerated(value: string): boolean {
  return GENERATED_VALUE_PATTERNS.some((pattern) => pattern.test(value));
}

export type NewChatParseResult =
  | { ok: true; matcher: NewChatMatcher; signals: string[]; dropped: number }
  | { ok: false; message: string };

/**
 * Text as a person reads it. Keyboard hints and screen-reader-only decoration
 * are skipped so ChatGPT's "New chat ⇧⌘O" row reduces to "New chat" — the
 * same reduction the in-page scorer performs, so the two agree on a match.
 */
function visibleTextOf(element: Element): string {
  let text = "";
  const walk = (node: Node) => {
    if (text.length > MAX_SIGNAL_BYTES) return;
    if (node.nodeType === 3) {
      text += node.nodeValue ?? "";
      return;
    }
    if (node.nodeType !== 1) return;
    const child = node as Element;
    if (child.tagName === "KBD" || child.tagName === "SCRIPT") return;
    if (child.tagName === "STYLE" || child.tagName === "SVG") return;
    if (child.getAttribute("aria-hidden") === "true") return;
    child.childNodes.forEach(walk);
  };
  walk(element);
  return text.replace(/\s+/g, " ").trim();
}

/**
 * Kept as a path so a matcher recorded from an absolute link still matches the
 * relative href the page renders, and vice versa.
 */
function hrefSignalOf(element: Element): string | undefined {
  const raw = element.getAttribute("href")?.trim();
  if (!raw) return undefined;

  let path = raw;
  try {
    path = new URL(raw, "https://provider.invalid/").pathname;
  } catch {
    // A non-URL href is compared verbatim, exactly as the scorer would.
  }
  // A link to one specific conversation is the opposite of a new-chat control;
  // recording it would teach the matcher to reopen an old thread.
  if (path.split("/").some(looksGenerated)) return undefined;
  return path;
}

function attributeRank(name: string): number {
  if (name.startsWith("data-")) return 0;
  if (name === "name") return 2;
  if (name === "id") return 4;
  return 6;
}

/**
 * Turns a DevTools "Copy element" paste into a set of signals.
 *
 * The HTML is parsed into an inert document and never enters the live page:
 * nothing is fetched, no script runs, and only whitelisted attribute values
 * leave this function. What is returned is data that eventually reaches the
 * provider page as a JSON argument, never as script text.
 */
export function parseNewChatElement(html: string): NewChatParseResult {
  const source = html.trim();
  if (!source) {
    return { ok: false, message: "Paste the copied element first." };
  }
  if (byteLength(source) > MAX_PASTE_BYTES) {
    return {
      ok: false,
      message: "That paste is too large. Copy just the button element.",
    };
  }
  if (typeof DOMParser === "undefined") {
    return {
      ok: false,
      message: "Prompter could not read the pasted element on this system.",
    };
  }

  let element: Element | null;
  try {
    const document = new DOMParser().parseFromString(source, "text/html");
    element = document.body.firstElementChild;
  } catch {
    return { ok: false, message: "Prompter could not read that paste." };
  }
  if (!element) {
    return {
      ok: false,
      message:
        "That does not look like an HTML element. In the browser, right-click the New chat button, choose Inspect, then right-click the highlighted line and choose Copy → Copy element.",
    };
  }

  const signals: string[] = [];
  const matcher: {
    testId?: string;
    label?: string;
    href?: string;
    attributes: { name: string; value: string }[];
  } = { attributes: [] };

  const testId = (
    element.getAttribute("data-testid") ??
    element.getAttribute("data-test-id") ??
    ""
  ).trim();
  if (testId && byteLength(testId) <= MAX_SIGNAL_BYTES) {
    matcher.testId = testId;
    signals.push(`test id "${testId}"`);
  }

  const label = (element.getAttribute("aria-label") ?? "").trim() ||
    visibleTextOf(element);
  if (label && byteLength(label) <= MAX_SIGNAL_BYTES) {
    matcher.label = label;
    signals.push(`name "${label}"`);
  }

  const href = hrefSignalOf(element);
  if (href && byteLength(href) <= MAX_SIGNAL_BYTES) {
    matcher.href = href;
    signals.push(`link "${href}"`);
  }

  let dropped = 0;
  const usable: { name: string; value: string; rank: number }[] = [];
  for (const attribute of Array.from(element.attributes)) {
    const name = attribute.name.toLowerCase();
    if (name === "data-testid" || name === "data-test-id") continue;
    if (name === "aria-label" || name === "href") continue;

    const value = attribute.value.trim();
    if (
      !isAllowedAttributeName(name) ||
      GENERIC_ATTRIBUTE_NAMES.has(name) ||
      VOLATILE_ATTRIBUTE_NAMES.has(name) ||
      VOLATILE_ATTRIBUTE_PREFIXES.some((prefix) => name.startsWith(prefix)) ||
      byteLength(value) > MAX_SIGNAL_BYTES ||
      /\p{Cc}/u.test(value) ||
      (value !== "" && looksGenerated(value))
    ) {
      dropped += 1;
      continue;
    }
    // An empty value is still a signal: the scorer treats it as "this
    // attribute is present", which is how flag attributes are written.
    const isFlag = value === "" || value === "true" || value === "false";
    usable.push({ name, value, rank: attributeRank(name) + (isFlag ? 1 : 0) });
  }

  usable.sort((left, right) => left.rank - right.rank);
  for (const { name, value } of usable.slice(0, MAX_ATTRIBUTES)) {
    matcher.attributes.push({ name, value });
    signals.push(value === "" ? name : `${name}="${value}"`);
  }
  dropped += Math.max(0, usable.length - MAX_ATTRIBUTES);

  const parsed = NewChatMatcherSchema.safeParse(
    matcher.attributes.length > 0
      ? matcher
      : { testId: matcher.testId, label: matcher.label, href: matcher.href },
  );
  if (!parsed.success) {
    return {
      ok: false,
      message:
        "Prompter found nothing reliable to match in that element. Copy the New chat button itself rather than a wrapper around it.",
    };
  }

  return { ok: true, matcher: parsed.data, signals, dropped };
}
