import { Icon } from "../../shared/Icon";
import {
  DEFAULT_SHORTCUT_DISPLAY,
} from "../quickCapture/model";
import { useCaptureStore } from "../quickCapture/store";
import { useSettingsStore } from "../settings/store";
import { useInstructionStore } from "./store";

export function InstructionSidebar() {
  const instructions = useInstructionStore((state) => state.library.instructions);
  const selectedId = useInstructionStore((state) => state.library.selectedId);
  const select = useInstructionStore((state) => state.select);
  const openEditor = useInstructionStore((state) => state.openEditor);
  const openSettings = useSettingsStore((state) => state.openSettings);
  const quickCaptureStatus = useCaptureStore((state) => state.status);

  const shortcutDisplay =
    quickCaptureStatus?.shortcut.display ?? DEFAULT_SHORTCUT_DISPLAY;
  const quickCaptureMessage = !quickCaptureStatus
    ? "Checking setup…"
    : quickCaptureStatus.registration === "unavailable"
      ? "Shortcut unavailable. Open Settings."
      : quickCaptureStatus.permission === "required"
        ? "Permission required. Open Settings."
        : "Prompter copies the selection and opens here.";

  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark">
          <Icon name="sparkle" size={19} />
        </span>
        <span>Prompter</span>
      </div>

      <section className="sidebar-instructions" aria-label="Instructions">
        <div className="sidebar-section-heading">
          <span>Instructions</span>
          <button
            aria-label="Create instruction"
            onClick={() => openEditor("new")}
            title="Create instruction"
            type="button"
          >
            <Icon name="plus" size={15} />
          </button>
        </div>

        <div className="preset-list sidebar-preset-list">
          {instructions.map((instruction) => (
            <div
              className={`preset-row ${selectedId === instruction.id ? "selected" : ""}`}
              key={instruction.id}
            >
              <button
                aria-pressed={selectedId === instruction.id}
                className="preset-main"
                onClick={() => select(instruction.id)}
                type="button"
              >
                <span className={`preset-icon ${instruction.color}`}>
                  <Icon name="wand" size={16} />
                </span>
                <span>
                  <strong>{instruction.name}</strong>
                </span>
              </button>
              <button
                aria-label={`Edit ${instruction.name}`}
                className="edit-preset"
                onClick={() => openEditor(instruction)}
                type="button"
              >
                <Icon name="edit" size={14} />
              </button>
            </div>
          ))}
        </div>
      </section>

      <button
        aria-label="Open Quick Capture settings"
        className="sidebar-tip quick-capture-tip"
        onClick={openSettings}
        type="button"
      >
        <span className="tip-kicker">Quick capture</span>
        <strong>Select text, then press</strong>
        <div className="shortcut-row">
          {shortcutDisplay.split(" ").map((key) => (
            <kbd key={key}>{key}</kbd>
          ))}
        </div>
        <small>{quickCaptureMessage}</small>
      </button>

      <button
        className="nav-item settings-link"
        onClick={openSettings}
        type="button"
      >
        <Icon name="settings" />
        <span>Settings</span>
      </button>
    </aside>
  );
}
