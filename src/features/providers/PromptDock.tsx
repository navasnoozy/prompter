import { Icon } from "../../shared/Icon";
import {
  getProviderLabel,
  PROVIDERS,
  PROVIDER_ORDER,
  type PromptComposition,
  type Provider,
} from "./model";

type PromptDockProps = {
  isWorking: boolean;
  composition: PromptComposition;
  provider: Provider;
  onCaptureClipboard: () => void;
  onPlacePrompt: (composition: PromptComposition) => void;
  onProviderChange: (provider: Provider) => void;
  onSourceTextChange: (text: string) => void;
};

export function PromptDock({
  isWorking,
  composition,
  provider,
  onCaptureClipboard,
  onPlacePrompt,
  onProviderChange,
  onSourceTextChange,
}: PromptDockProps) {
  const sourceText = composition.text;

  return (
    <section className="bottom-dock" aria-label="Prepare prompt">
      <div className="dock-provider-options" aria-label="AI provider">
        {PROVIDER_ORDER.map((option) => (
          <button
            aria-pressed={provider === option}
            className={`provider-button ${provider === option ? "selected" : ""}`}
            key={option}
            onClick={() => onProviderChange(option)}
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
          <button onClick={onCaptureClipboard} type="button">
            <Icon name="clipboard" size={14} /> Capture clipboard
          </button>
        </div>
        <textarea
          aria-label="Text to rewrite"
          onChange={(event) => onSourceTextChange(event.target.value)}
          placeholder="Paste or type the text you want to rewrite…"
          value={sourceText}
        />
        <div className="dock-text-meta">
          <span>{sourceText.length} characters</span>
          {sourceText && (
            <button onClick={() => onSourceTextChange("")} type="button">
              Clear
            </button>
          )}
        </div>
      </div>

      <button
        className="dock-place-button"
        disabled={isWorking || !sourceText.trim()}
        onClick={() => onPlacePrompt(composition)}
        type="button"
      >
        <Icon name="sparkle" size={19} />
        <span>
          <strong>
            {isWorking
              ? "Placing…"
              : `Place in ${getProviderLabel(provider)}`}
          </strong>
          <small>You press Send</small>
        </span>
        <Icon name="chevron" size={16} />
      </button>
    </section>
  );
}
