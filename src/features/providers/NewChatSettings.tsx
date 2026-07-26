import { useId, useState } from "react";
import { PROVIDERS, PROVIDER_ORDER, type Provider } from "./model";
import {
  describeNewChatMatcher,
  parseNewChatElement,
  validateNewChatUrl,
  NEW_CHAT_MODES,
  NEW_CHAT_MODE_LABELS,
  type NewChatMode,
} from "./newChat";
import { useProviderStore } from "./store";

export function NewChatSettings() {
  const mode = useProviderStore((state) => state.newChatMode);
  const setNewChatMode = useProviderStore((state) => state.setNewChatMode);
  const modeId = useId();

  return (
    <div className="settings-section new-chat-settings">
      <div>
        <strong>New chat</strong>
        <p>
          Start a new chat each time you place a prompt, instead of adding to
          the conversation that is already open.
        </p>
      </div>

      <label htmlFor={modeId}>
        How Prompter starts it
        <select
          id={modeId}
          onChange={(event) =>
            setNewChatMode(event.target.value as NewChatMode)
          }
          value={mode}
        >
          {NEW_CHAT_MODES.map((option) => (
            <option key={option} value={option}>
              {NEW_CHAT_MODE_LABELS[option].title}
            </option>
          ))}
        </select>
      </label>
      <p className="new-chat-mode-detail">{NEW_CHAT_MODE_LABELS[mode].detail}</p>

      {mode !== "off" && (
        <details className="new-chat-advanced">
          <summary>Advanced — set the address and the button yourself</summary>
          <p>
            ChatGPT and Gemini change their pages from time to time. If Prompter
            stops finding the New chat button, you can point it at the new one
            here without waiting for an update.
          </p>
          {PROVIDER_ORDER.map((provider) => (
            <ProviderOverrideEditor key={provider} provider={provider} />
          ))}
        </details>
      )}
    </div>
  );
}

type Feedback = { tone: "ok" | "error"; message: string } | null;

function ProviderOverrideEditor({ provider }: { provider: Provider }) {
  const override = useProviderStore((state) => state.newChatOverrides[provider]);
  const setNewChatOverride = useProviderStore(
    (state) => state.setNewChatOverride,
  );

  const [address, setAddress] = useState(override?.url ?? "");
  const [element, setElement] = useState("");
  const [addressFeedback, setAddressFeedback] = useState<Feedback>(null);
  const [elementFeedback, setElementFeedback] = useState<Feedback>(null);
  const addressId = useId();
  const elementId = useId();

  const { label, newChatUrl } = PROVIDERS[provider];
  const trimmedAddress = address.trim();
  const addressError = trimmedAddress
    ? validateNewChatUrl(provider, trimmedAddress)
    : null;
  const addressUnchanged = trimmedAddress === (override?.url ?? "");

  const saveAddress = () => {
    if (addressError) {
      setAddressFeedback({ tone: "error", message: addressError });
      return;
    }
    setNewChatOverride(provider, {
      ...(trimmedAddress ? { url: trimmedAddress } : {}),
      ...(override?.matcher ? { matcher: override.matcher } : {}),
    });
    setAddressFeedback({
      tone: "ok",
      message: trimmedAddress
        ? "Address saved."
        : `Using the built-in address, ${newChatUrl}`,
    });
  };

  const clearAddress = () => {
    setAddress("");
    setNewChatOverride(
      provider,
      override?.matcher ? { matcher: override.matcher } : null,
    );
    setAddressFeedback({
      tone: "ok",
      message: `Using the built-in address, ${newChatUrl}`,
    });
  };

  const saveElement = () => {
    const result = parseNewChatElement(element);
    if (!result.ok) {
      setElementFeedback({ tone: "error", message: result.message });
      return;
    }
    setNewChatOverride(provider, {
      ...(override?.url ? { url: override.url } : {}),
      matcher: result.matcher,
    });
    setElement("");
    setElementFeedback({
      tone: "ok",
      message: `Saved. Prompter will look for ${result.signals.join(", ")}.`,
    });
  };

  const clearElement = () => {
    setElement("");
    setNewChatOverride(provider, override?.url ? { url: override.url } : null);
    setElementFeedback({
      tone: "ok",
      message: "Using the built-in button description.",
    });
  };

  const savedSignals = override?.matcher
    ? describeNewChatMatcher(override.matcher)
    : null;

  return (
    <section aria-label={`${label} new chat`} className="new-chat-provider">
      <strong>{label}</strong>

      <label htmlFor={addressId}>
        New chat address
        <input
          id={addressId}
          onChange={(event) => {
            setAddress(event.target.value);
            setAddressFeedback(null);
          }}
          placeholder={newChatUrl}
          spellCheck={false}
          type="url"
          value={address}
        />
      </label>
      <p className="new-chat-help">
        Open {label} in your browser, start a new chat, then copy the address
        from the address bar. Leave this empty to use {newChatUrl}
      </p>
      <div className="new-chat-actions">
        <button
          className="settings-inline-button"
          disabled={addressUnchanged && addressFeedback === null}
          onClick={saveAddress}
          type="button"
        >
          Save address
        </button>
        {override?.url && (
          <button
            className="settings-inline-button"
            onClick={clearAddress}
            type="button"
          >
            Use the built-in address
          </button>
        )}
      </div>
      <Note feedback={addressFeedback ?? errorNote(addressError)} />

      <label htmlFor={elementId}>
        New chat button
        <textarea
          id={elementId}
          onChange={(event) => {
            setElement(event.target.value);
            setElementFeedback(null);
          }}
          placeholder="Paste the copied element here"
          spellCheck={false}
          value={element}
        />
      </label>
      <p className="new-chat-help">
        In your browser, right-click the New chat button and choose Inspect.
        Then right-click the highlighted line and choose Copy → Copy element.
        Paste it above. Prompter reads several details from it, so the button is
        still found when one of them changes.
      </p>
      <div className="new-chat-actions">
        <button
          className="settings-inline-button"
          disabled={element.trim() === ""}
          onClick={saveElement}
          type="button"
        >
          Save button
        </button>
        {override?.matcher && (
          <button
            className="settings-inline-button"
            onClick={clearElement}
            type="button"
          >
            Use the built-in button
          </button>
        )}
      </div>
      <Note feedback={elementFeedback} />
      {savedSignals && elementFeedback === null && (
        <p className="new-chat-help">
          Currently looking for {savedSignals.join(", ")}.
        </p>
      )}
    </section>
  );
}

function errorNote(message: string | null): Feedback {
  return message === null ? null : { tone: "error", message };
}

function Note({ feedback }: { feedback: Feedback }) {
  if (feedback === null) return null;
  return (
    <p
      className={`new-chat-note ${feedback.tone}`}
      role={feedback.tone === "error" ? "alert" : "status"}
    >
      {feedback.message}
    </p>
  );
}
