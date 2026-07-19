import { Icon } from "../../shared/Icon";
import { registerPromptInput, useCaptureStore } from "../quickCapture/store";
import { placeCurrentPrompt } from "./placement";
import { getProviderLabel, PROVIDERS, PROVIDER_ORDER } from "./model";
import { useProviderStore } from "./store";

// Mirrors the native byte cap conservatively; the backend stays authoritative.
const MAX_PROMPT_CHARS = 1_000_000;

export function PromptDock() {
  const provider = useProviderStore((state) => state.provider);
  const setProvider = useProviderStore((state) => state.setProvider);
  const isPlacing = useProviderStore((state) => state.isPlacing);
  const sourceText = useCaptureStore((state) => state.sourceText);
  const setSourceText = useCaptureStore((state) => state.setSourceText);
  const captureClipboard = useCaptureStore((state) => state.captureClipboard);
  const isTooLarge = sourceText.length > MAX_PROMPT_CHARS;

  return (
    <section className="bottom-dock" aria-label="Prepare prompt">
      <div className="dock-provider-options" aria-label="AI provider">
        {PROVIDER_ORDER.map((option, index) => (
          <button
            aria-pressed={provider === option}
            className={`provider-button ${provider === option ? "selected" : ""}`}
            key={option}
            onClick={() => setProvider(option)}
            title={`Switch with ⌘${index + 1}`}
            type="button"
          >
            <span className={`provider-logo ${option}`}>
              {PROVIDERS[option].logo}
            </span>
            <span>{PROVIDERS[option].label}</span>
            {provider === option && <Icon name="check" size={14} />}
          </button>
        ))}
      </div>

      <div className="dock-text-box">
        <div className="dock-text-heading">
          <span>Your text</span>
          <button onClick={() => void captureClipboard()} type="button">
            <Icon name="clipboard" size={14} /> Capture clipboard
          </button>
        </div>
        <textarea
          aria-label="Text to rewrite"
          onChange={(event) => setSourceText(event.target.value)}
          placeholder="Paste or type the text you want to rewrite…"
          ref={registerPromptInput}
          value={sourceText}
        />
        <div className="dock-text-meta">
          <span>
            {sourceText.length.toLocaleString()} characters
            {isTooLarge && " — too large to place"}
          </span>
          {sourceText && (
            <button onClick={() => setSourceText("")} type="button">
              Clear
            </button>
          )}
        </div>
      </div>

      <button
        className="dock-place-button"
        disabled={isPlacing || !sourceText.trim() || isTooLarge}
        onClick={() => void placeCurrentPrompt()}
        type="button"
      >
        <Icon name="sparkle" size={19} />
        <span>
          <strong>
            {isPlacing ? "Placing…" : `Place in ${getProviderLabel(provider)}`}
          </strong>
          <small>⌘ ⏎ · You press Send</small>
        </span>
        <Icon name="chevron" size={16} />
      </button>
    </section>
  );
}
