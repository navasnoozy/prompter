import { Icon } from "../../shared/Icon";
import { ModalBackdrop } from "../../shared/ModalBackdrop";
import type { QuickCaptureStatus } from "../quickCapture/model";
import type { AppTheme } from "./useTheme";

const TITLE_ID = "settings-title";

type SettingsDialogProps = {
  theme: AppTheme;
  onClose: () => void;
  isRequestingPermission: boolean;
  isRetryingRegistration: boolean;
  onOpenSystemSettings: () => void;
  onRefreshQuickCapture: () => void;
  onRequestPermission: () => void;
  onRetryRegistration: () => void;
  onThemeChange: (theme: AppTheme) => void;
  quickCaptureStatus: QuickCaptureStatus | null;
};

export function SettingsDialog({
  theme,
  onClose,
  isRequestingPermission,
  isRetryingRegistration,
  onOpenSystemSettings,
  onRefreshQuickCapture,
  onRequestPermission,
  onRetryRegistration,
  onThemeChange,
  quickCaptureStatus,
}: SettingsDialogProps) {
  const registrationReady =
    quickCaptureStatus?.registration === "registered";
  const permissionReady = quickCaptureStatus?.permission === "granted";

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

        <div className="settings-divider" />

        <div className="settings-section quick-capture-settings">
          <div className="settings-section-heading">
            <div>
              <strong>Quick Capture</strong>
              <p>
                Select text in any app, then use the shortcut. Prompter opens
                with the text ready to place in ChatGPT or Gemini.
              </p>
            </div>
            <kbd className="settings-shortcut">
              {quickCaptureStatus?.shortcut.display ?? "⌘ ⇧ P"}
            </kbd>
          </div>

          <div aria-live="polite" className="quick-capture-checks">
            <div className="quick-capture-check">
              <div>
                <span
                  aria-hidden="true"
                  className={`settings-status-dot ${registrationReady ? "ready" : "attention"}`}
                />
                <strong>Keyboard shortcut</strong>
                <p>
                  {!quickCaptureStatus
                    ? "Checking shortcut registration…"
                    : registrationReady
                      ? "Registered and available while Prompter is running."
                      : "Unavailable. Another application may be using this shortcut."}
                </p>
              </div>
              {!registrationReady && quickCaptureStatus && (
                <button
                  className="settings-inline-button"
                  disabled={isRetryingRegistration}
                  onClick={onRetryRegistration}
                  type="button"
                >
                  {isRetryingRegistration ? "Retrying…" : "Retry"}
                </button>
              )}
            </div>

            <div className="quick-capture-check">
              <div>
                <span
                  aria-hidden="true"
                  className={`settings-status-dot ${permissionReady ? "ready" : "attention"}`}
                />
                <strong>macOS permission</strong>
                <p>
                  {permissionReady
                    ? "Allowed to press Copy for your selected text."
                    : "Required so Prompter can press Copy for you. Text is never sent automatically."}
                </p>
              </div>
            </div>
          </div>

          <div className="settings-action-row">
            {!permissionReady && (
              <button
                className="primary-button settings-action-button"
                disabled={isRequestingPermission}
                onClick={onRequestPermission}
                type="button"
              >
                {isRequestingPermission ? "Requesting…" : "Enable Quick Capture"}
              </button>
            )}
            {!permissionReady && (
              <button
                className="secondary-button settings-action-button"
                onClick={onOpenSystemSettings}
                type="button"
              >
                Open System Settings
              </button>
            )}
            <button
              className="secondary-button settings-action-button"
              onClick={onRefreshQuickCapture}
              type="button"
            >
              Recheck
            </button>
          </div>
        </div>
      </section>
    </ModalBackdrop>
  );
}
