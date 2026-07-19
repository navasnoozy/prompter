use std::{fmt, sync::Mutex, time::Instant};

use log::{info, warn};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State, Window, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

pub(crate) const BACKGROUND_LAUNCH_ARG: &str = "--prompter-background";

use crate::{platform, MAIN_WINDOW_LABEL};

const CONTRACT_VERSION: u8 = 1;
const VISIBILITY_EVENT: &str = "prompter://main-window-visibility";

#[derive(Debug, Default)]
struct LifecycleState {
    ready: bool,
    pending_activation: bool,
    /// True once the window has been presented through an activation and until
    /// the red-close button hides the app. System-level hides (⌘H) and
    /// minimize are intentionally not tracked: native child webviews of a
    /// hidden window do not render, so the frontend contract only needs the
    /// presented/red-closed distinction.
    visible: bool,
    autostart_available: bool,
}

#[derive(Debug, Default)]
pub(crate) struct AppLifecycleCoordinator {
    state: Mutex<LifecycleState>,
    login_item_gate: Mutex<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationSource {
    Startup,
    DockReopen,
    SecondInstance,
    QuickCapture,
}

impl ActivationSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::DockReopen => "dock_reopen",
            Self::SecondInstance => "second_instance",
            Self::QuickCapture => "quick_capture",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationResult {
    Presented,
    Queued,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresentationStage {
    ShowApplication,
    ShowWindow,
    Unminimize,
    Focus,
}

impl PresentationStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::ShowApplication => "show_application",
            Self::ShowWindow => "show_window",
            Self::Unminimize => "unminimize",
            Self::Focus => "focus",
        }
    }
}

#[derive(Debug)]
pub(crate) enum AppLifecycleError {
    MainWindowMissing,
    Presentation {
        stage: PresentationStage,
        reason: String,
    },
}

impl fmt::Display for AppLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MainWindowMissing => formatter.write_str("main window not found"),
            Self::Presentation { stage, reason } => {
                write!(formatter, "{} failed: {reason}", stage.as_str())
            }
        }
    }
}

trait MainWindowActions {
    fn show_application(&mut self) -> Result<(), String>;
    fn show_window(&mut self) -> Result<(), String>;
    fn unminimize(&mut self) -> Result<(), String>;
    fn focus(&mut self) -> Result<(), String>;
}

struct TauriMainWindowActions<'a, R: Runtime> {
    app: &'a AppHandle<R>,
    window: &'a Window<R>,
}

impl<R: Runtime> MainWindowActions for TauriMainWindowActions<'_, R> {
    fn show_application(&mut self) -> Result<(), String> {
        self.app.show().map_err(|error| error.to_string())
    }

    fn show_window(&mut self) -> Result<(), String> {
        self.window.show().map_err(|error| error.to_string())
    }

    fn unminimize(&mut self) -> Result<(), String> {
        self.window.unminimize().map_err(|error| error.to_string())
    }

    fn focus(&mut self) -> Result<(), String> {
        self.window.set_focus().map_err(|error| error.to_string())
    }
}

fn present(actions: &mut impl MainWindowActions) -> Result<(), AppLifecycleError> {
    actions
        .show_application()
        .map_err(|reason| AppLifecycleError::Presentation {
            stage: PresentationStage::ShowApplication,
            reason,
        })?;
    actions
        .show_window()
        .map_err(|reason| AppLifecycleError::Presentation {
            stage: PresentationStage::ShowWindow,
            reason,
        })?;
    actions
        .unminimize()
        .map_err(|reason| AppLifecycleError::Presentation {
            stage: PresentationStage::Unminimize,
            reason,
        })?;
    actions
        .focus()
        .map_err(|reason| AppLifecycleError::Presentation {
            stage: PresentationStage::Focus,
            reason,
        })?;
    Ok(())
}

fn emit_visibility<R: Runtime>(app: &AppHandle<R>, visible: bool) {
    let payload = MainWindowVisibilityPayload {
        version: CONTRACT_VERSION,
        visible,
    };
    if let Err(error) = app.emit_to(MAIN_WINDOW_LABEL, VISIBILITY_EVENT, payload) {
        warn!(
            target: "prompter::lifecycle",
            "event=visibility_notification_failed visible={visible} reason={error}"
        );
    }
}

pub(crate) fn install_autostart_plugin<R: Runtime>(app: &AppHandle<R>) -> bool {
    let plugin = tauri_plugin_autostart::Builder::new()
        .macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent)
        .arg(BACKGROUND_LAUNCH_ARG)
        .build();
    match app.plugin(plugin) {
        Ok(()) => true,
        Err(error) => {
            warn!(
                target: "prompter::lifecycle",
                "event=autostart_plugin_initialization outcome=failure reason={error}"
            );
            false
        }
    }
}

pub(crate) fn initialize<R: Runtime>(app: &AppHandle<R>, autostart_available: bool) {
    let background_launch = is_background_launch(std::env::args());
    configure_active_space_policy(app);
    let coordinator = app.state::<AppLifecycleCoordinator>();
    let pending_activation = {
        let mut state = coordinator
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.ready = true;
        state.autostart_available = autostart_available;
        std::mem::take(&mut state.pending_activation)
    };

    if !background_launch || pending_activation {
        if let Err(error) = request_activation(app, ActivationSource::Startup) {
            warn!(
                target: "prompter::lifecycle",
                "event=startup_activation_failed reason={error}"
            );
        }
    } else {
        info!(
            target: "prompter::lifecycle",
            "event=startup mode=background window=hidden"
        );
    }
}

fn configure_active_space_policy<R: Runtime>(app: &AppHandle<R>) {
    let result = app
        .get_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window not found".to_string())
        .and_then(|window| platform::configure_main_window_active_space(&window));

    match result {
        Ok(()) => info!(
            target: "prompter::lifecycle",
            "event=active_space_policy outcome=configured"
        ),
        Err(error) => warn!(
            target: "prompter::lifecycle",
            "event=active_space_policy outcome=failure reason={error} fallback=system_default"
        ),
    }
}

pub(crate) fn is_background_launch(args: impl IntoIterator<Item = String>) -> bool {
    args.into_iter()
        .any(|argument| argument == BACKGROUND_LAUNCH_ARG)
}

pub(crate) fn handle_second_instance<R: Runtime>(app: &AppHandle<R>, args: &[String]) {
    if args
        .iter()
        .any(|argument| argument == BACKGROUND_LAUNCH_ARG)
    {
        info!(
            target: "prompter::lifecycle",
            "event=second_instance action=ignored_background_launch"
        );
        return;
    }

    if let Err(error) = request_activation(app, ActivationSource::SecondInstance) {
        warn!(
            target: "prompter::lifecycle",
            "event=second_instance_activation_failed reason={error}"
        );
    }
}

pub(crate) fn request_activation<R: Runtime>(
    app: &AppHandle<R>,
    source: ActivationSource,
) -> Result<ActivationResult, AppLifecycleError> {
    let started = Instant::now();
    let coordinator = app.state::<AppLifecycleCoordinator>();
    let mut state = coordinator
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if !state.ready {
        state.pending_activation = true;
        info!(
            target: "prompter::lifecycle",
            "event=activation source={} outcome=queued pid={}",
            source.as_str(),
            std::process::id()
        );
        return Ok(ActivationResult::Queued);
    }

    let window = app
        .get_window(MAIN_WINDOW_LABEL)
        .ok_or(AppLifecycleError::MainWindowMissing)?;
    let mut actions = TauriMainWindowActions {
        app,
        window: &window,
    };
    present(&mut actions)?;
    state.visible = true;
    state.pending_activation = false;
    drop(state);

    emit_visibility(app, true);
    info!(
        target: "prompter::lifecycle",
        "event=activation source={} outcome=presented pid={} duration_ms={}",
        source.as_str(),
        std::process::id(),
        started.elapsed().as_millis()
    );
    Ok(ActivationResult::Presented)
}

pub(crate) fn handle_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }

    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        let app = window.app_handle();
        let coordinator = app.state::<AppLifecycleCoordinator>();
        let mut state = coordinator
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        match app.hide() {
            Ok(()) => {
                state.visible = false;
                drop(state);
                emit_visibility(app, false);
                info!(
                    target: "prompter::lifecycle",
                    "event=red_close action=hide outcome=success pid={}",
                    std::process::id()
                );
            }
            Err(error) => {
                warn!(
                    target: "prompter::lifecycle",
                    "event=red_close action=hide outcome=failure reason={error}"
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct MainWindowVisibilityPayload {
    version: u8,
    visible: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppLifecycleStatus {
    version: u8,
    launch_at_login: LaunchAtLoginState,
    main_window_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum LaunchAtLoginState {
    Enabled,
    Disabled,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum AppLifecycleCommandErrorCode {
    LaunchAtLoginUnavailable,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppLifecycleCommandError {
    version: u8,
    code: AppLifecycleCommandErrorCode,
    message: &'static str,
}

impl AppLifecycleCommandError {
    fn launch_at_login() -> Self {
        Self {
            version: CONTRACT_VERSION,
            code: AppLifecycleCommandErrorCode::LaunchAtLoginUnavailable,
            message: "Prompter could not update Launch at Login. Please try again.",
        }
    }
}

fn lifecycle_status<R: Runtime>(
    app: &AppHandle<R>,
    coordinator: &AppLifecycleCoordinator,
) -> AppLifecycleStatus {
    let (main_window_visible, autostart_available) = {
        let state = coordinator
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (state.visible, state.autostart_available)
    };
    let launch_at_login = if !autostart_available {
        LaunchAtLoginState::Unavailable
    } else {
        match app.autolaunch().is_enabled() {
            Ok(true) => LaunchAtLoginState::Enabled,
            Ok(false) => LaunchAtLoginState::Disabled,
            Err(error) => {
                warn!(
                    target: "prompter::lifecycle",
                    "event=launch_at_login_read outcome=failure reason={error}"
                );
                LaunchAtLoginState::Unavailable
            }
        }
    };

    AppLifecycleStatus {
        version: CONTRACT_VERSION,
        launch_at_login,
        main_window_visible,
    }
}

#[tauri::command]
pub(crate) fn get_app_lifecycle_status<R: Runtime>(
    app: AppHandle<R>,
    coordinator: State<'_, AppLifecycleCoordinator>,
) -> AppLifecycleStatus {
    lifecycle_status(&app, &coordinator)
}

#[tauri::command]
pub(crate) fn set_launch_at_login<R: Runtime>(
    app: AppHandle<R>,
    coordinator: State<'_, AppLifecycleCoordinator>,
    enabled: bool,
) -> Result<AppLifecycleStatus, AppLifecycleCommandError> {
    let _gate = coordinator
        .login_item_gate
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let autostart_available = coordinator
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .autostart_available;
    if !autostart_available {
        return Err(AppLifecycleCommandError::launch_at_login());
    }
    let manager = app.autolaunch();
    let current = manager.is_enabled().map_err(|error| {
        warn!(
            target: "prompter::lifecycle",
            "event=launch_at_login_read outcome=failure reason={error}"
        );
        AppLifecycleCommandError::launch_at_login()
    })?;

    if current != enabled {
        let result = if enabled {
            manager.enable()
        } else {
            manager.disable()
        };
        result.map_err(|error| {
            warn!(
                target: "prompter::lifecycle",
                "event=launch_at_login_update requested={enabled} outcome=failure reason={error}"
            );
            AppLifecycleCommandError::launch_at_login()
        })?;
    }

    let status = lifecycle_status(&app, &coordinator);
    let verified = matches!(
        (enabled, status.launch_at_login),
        (true, LaunchAtLoginState::Enabled) | (false, LaunchAtLoginState::Disabled)
    );
    if !verified {
        warn!(
            target: "prompter::lifecycle",
            "event=launch_at_login_update requested={enabled} outcome=verification_failed"
        );
        return Err(AppLifecycleCommandError::launch_at_login());
    }

    info!(
        target: "prompter::lifecycle",
        "event=launch_at_login_update enabled={enabled} outcome=success"
    );
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct FakeActions {
        calls: Vec<&'static str>,
        fail_at: Option<&'static str>,
    }

    impl FakeActions {
        fn call(&mut self, name: &'static str) -> Result<(), String> {
            self.calls.push(name);
            if self.fail_at == Some(name) {
                Err(format!("{name} failed"))
            } else {
                Ok(())
            }
        }
    }

    impl MainWindowActions for FakeActions {
        fn show_application(&mut self) -> Result<(), String> {
            self.call("show_application")
        }

        fn show_window(&mut self) -> Result<(), String> {
            self.call("show_window")
        }

        fn unminimize(&mut self) -> Result<(), String> {
            self.call("unminimize")
        }

        fn focus(&mut self) -> Result<(), String> {
            self.call("focus")
        }
    }

    #[test]
    fn presentation_unhides_shows_unminimizes_and_focuses_in_order() {
        let mut actions = FakeActions::default();

        present(&mut actions).expect("presentation should succeed");

        assert_eq!(
            actions.calls,
            ["show_application", "show_window", "unminimize", "focus"]
        );
    }

    #[test]
    fn presentation_stops_at_the_first_failed_operation() {
        let mut actions = FakeActions {
            fail_at: Some("unminimize"),
            ..FakeActions::default()
        };

        let error = present(&mut actions).expect_err("presentation should fail");

        assert!(matches!(
            error,
            AppLifecycleError::Presentation {
                stage: PresentationStage::Unminimize,
                ..
            }
        ));
        assert_eq!(
            actions.calls,
            ["show_application", "show_window", "unminimize"]
        );
    }

    #[test]
    fn background_launch_requires_the_exact_marker() {
        assert!(is_background_launch([
            "/Applications/Prompter.app/Contents/MacOS/prompter".into(),
            BACKGROUND_LAUNCH_ARG.into(),
        ]));
        assert!(!is_background_launch([
            "prompter".into(),
            "--prompter-background-other".into(),
        ]));
    }
}
