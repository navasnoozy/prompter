import { Icon } from "../../shared/Icon";
import { registerPromptInput, useCaptureStore } from "../quickCapture/store";
import {
  selectedInstructionOf,
  useInstructionStore,
} from "../instructions/store";
import { placeCurrentPrompt } from "./placement";
import { getProviderLabel, PROVIDERS, PROVIDER_ORDER } from "./model";
import { isPromptTooLarge, MAX_PROMPT_BYTES, promptByteLength } from "./prompt";
import { useProviderStore } from "./store";

export function PromptDock() {
  const provider = useProviderStore((state) => state.provider);
  const setProvider = useProviderStore((state) => state.setProvider);
  const isPlacing = useProviderStore((state) => state.isPlacing);
  const sourceText = useCaptureStore((state) => state.sourceText);
  const instruction = useInstructionStore(selectedInstructionOf);
  const setSourceText = useCaptureStore((state) => state.setSourceText);
  const captureClipboard = useCaptureStore((state) => state.captureClipboard);
  const isCapturingClipboard = useCaptureStore(
    (state) => state.isCapturingClipboard,
  );
  const composition = {
    beforeText: instruction.beforeText,
    text: sourceText,
    afterText: instruction.afterText,
  };
  const promptBytes = promptByteLength(composition);
  const isTooLarge = isPromptTooLarge(composition);

  return (
    <section className="bottom-dock" aria-label="Prepare prompt">
      <div
        className="dock-provider-options"
        aria-label="AI provider"
        role="group"
      >
        {PROVIDER_ORDER.map((option, index) => (
          <button
            aria-pressed={provider === option}
            className={`provider-button ${provider === option ? "selected" : ""}`}
            key={option}
            onClick={() => setProvider(option)}
            aria-keyshortcuts={`Meta+${index + 1}`}
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
          <button
            disabled={isCapturingClipboard}
            onClick={() => void captureClipboard()}
            type="button"
          >
            <Icon name="clipboard" size={14} />
            <span>
              {isCapturingClipboard ? "Capturing…" : "Capture clipboard"}
            </span>
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
            {isTooLarge &&
              ` — ${promptBytes.toLocaleString()} / ${MAX_PROMPT_BYTES.toLocaleString()} prompt bytes`}
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
        aria-keyshortcuts="Meta+Enter"
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
