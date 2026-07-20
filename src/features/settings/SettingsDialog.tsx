import { Icon } from "../../shared/Icon";
import { ModalBackdrop } from "../../shared/ModalBackdrop";
import { useLifecycleStore } from "../lifecycle/store";
import { useCaptureStore } from "../quickCapture/store";
import { useSettingsStore } from "./store";

const TITLE_ID = "settings-title";

export function SettingsDialog() {
  const theme = useSettingsStore((state) => state.theme);
  const setTheme = useSettingsStore((state) => state.setTheme);
  const closeSettings = useSettingsStore((state) => state.closeSettings);

  const launchAtLogin = useLifecycleStore(
    (state) => state.status?.launchAtLogin ?? null,
  );
  const isUpdatingLaunchAtLogin = useLifecycleStore(
    (state) => state.isUpdatingLaunchAtLogin,
  );
  const setLaunchAtLogin = useLifecycleStore((state) => state.setLaunchAtLogin);

  const quickCaptureStatus = useCaptureStore((state) => state.status);
  const isRequestingPermission = useCaptureStore(
    (state) => state.isRequestingPermission,
  );
  const isRefreshingStatus = useCaptureStore(
    (state) => state.isRefreshingStatus,
  );
  const isRetryingRegistration = useCaptureStore(
    (state) => state.isRetryingRegistration,
  );
  const requestPermission = useCaptureStore((state) => state.requestPermission);
  const retryRegistration = useCaptureStore((state) => state.retryRegistration);
  const openSystemSettings = useCaptureStore(
    (state) => state.openSystemSettings,
  );
  const refreshQuickCapture = useCaptureStore((state) => state.refreshStatus);

  const registrationReady = quickCaptureStatus?.registration === "registered";
  const permissionReady = quickCaptureStatus?.permission === "granted";
  const launchAtLoginEnabled = launchAtLogin === "enabled";
  const launchAtLoginUnavailable = launchAtLogin === "unavailable";

  return (
    <ModalBackdrop onClose={closeSettings}>
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
            onClick={closeSettings}
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
              aria-pressed={theme === "light"}
              className={theme === "light" ? "selected" : ""}
              onClick={() => setTheme("light")}
              type="button"
            >
              <Icon name="sun" size={16} />
              <span>Light</span>
            </button>
            <button
              aria-pressed={theme === "dark"}
              className={theme === "dark" ? "selected" : ""}
              onClick={() => setTheme("dark")}
              type="button"
            >
              <Icon name="moon" size={16} />
              <span>Dark</span>
            </button>
          </div>
        </div>

        <div className="settings-divider" />

        <div className="settings-section background-settings">
          <div>
            <strong>Background behaviour</strong>
            <p>
              Closing the window keeps Quick Capture ready. Press ⌘ Q to quit
              Prompter completely.
            </p>
          </div>

          <div className="settings-preference-row">
            <div>
              <strong>Launch at login</strong>
              <p>
                {launchAtLoginUnavailable
                  ? "Launch at Login is currently unavailable. Prompter otherwise works normally."
                  : "Start quietly in the background when you sign in to your Mac. Keep Prompter in Applications for reliable startup."}
              </p>
            </div>
            <button
              aria-checked={launchAtLoginEnabled}
              aria-label="Launch Prompter at login"
              className={`settings-switch ${launchAtLoginEnabled ? "enabled" : ""} ${isUpdatingLaunchAtLogin || launchAtLogin === null ? "updating" : ""}`}
              disabled={
                launchAtLogin === null ||
                launchAtLoginUnavailable ||
                isUpdatingLaunchAtLogin
              }
              onClick={() => void setLaunchAtLogin(!launchAtLoginEnabled)}
              role="switch"
              type="button"
            >
              <span aria-hidden="true" />
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
                  onClick={() => void retryRegistration()}
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
                  {!quickCaptureStatus
                    ? "Checking macOS Accessibility permission…"
                    : permissionReady
                    ? "Allowed to press Copy for your selected text."
                    : "Required so Prompter can press Copy for you. Text is never sent automatically."}
                </p>
              </div>
            </div>
          </div>

          <div className="settings-action-row">
            {quickCaptureStatus && !permissionReady && (
              <button
                className="primary-button settings-action-button"
                disabled={isRequestingPermission}
                onClick={() => void requestPermission()}
                type="button"
              >
                {isRequestingPermission ? "Requesting…" : "Enable Quick Capture"}
              </button>
            )}
            {quickCaptureStatus && !permissionReady && (
              <button
                className="secondary-button settings-action-button"
                onClick={() => void openSystemSettings()}
                type="button"
              >
                Open System Settings
              </button>
            )}
            <button
              className="secondary-button settings-action-button"
              disabled={isRefreshingStatus}
              onClick={() => void refreshQuickCapture(true)}
              type="button"
            >
              {isRefreshingStatus ? "Checking…" : "Recheck"}
            </button>
          </div>
        </div>
      </section>
    </ModalBackdrop>
  );
}
