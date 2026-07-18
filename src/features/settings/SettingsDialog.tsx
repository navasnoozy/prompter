import { Icon } from "../../shared/Icon";
import { ModalBackdrop } from "../../shared/ModalBackdrop";
import type { AppTheme } from "./useTheme";

const TITLE_ID = "settings-title";

type SettingsDialogProps = {
  theme: AppTheme;
  onClose: () => void;
  onThemeChange: (theme: AppTheme) => void;
};

export function SettingsDialog({
  theme,
  onClose,
  onThemeChange,
}: SettingsDialogProps) {
  return (
    <ModalBackdrop onClose={onClose}>
      <section
        aria-labelledby={TITLE_ID}
        aria-modal="true"
        className="preset-modal settings-modal"
        role="dialog"
      >
        <div className="modal-header">
          <div>
            <p className="eyebrow">Prompter</p>
            <h2 id={TITLE_ID}>Settings</h2>
          </div>
          <button
            aria-label="Close settings"
            className="icon-button"
            onClick={onClose}
            type="button"
          >
            <Icon name="close" />
          </button>
        </div>

        <div className="settings-section">
          <div>
            <strong>Appearance</strong>
            <p>Choose how the Prompter interface looks.</p>
          </div>
          <div className="theme-options" role="group" aria-label="Appearance">
            <button
              className={theme === "light" ? "selected" : ""}
              onClick={() => onThemeChange("light")}
              type="button"
            >
              <Icon name="settings" size={16} /> Light
            </button>
            <button
              className={theme === "dark" ? "selected" : ""}
              onClick={() => onThemeChange("dark")}
              type="button"
            >
              <Icon name="moon" size={16} /> Dark
            </button>
          </div>
        </div>
      </section>
    </ModalBackdrop>
  );
}
