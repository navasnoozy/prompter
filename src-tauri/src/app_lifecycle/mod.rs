mod tray;

pub(crate) use tray::install_tray;

use std::{fmt, sync::Mutex, time::Instant};

use log::{info, warn};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalSize, Runtime, State, Window, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;

pub(crate) const BACKGROUND_LAUNCH_ARG: &str = "--prompter-background";

use crate::{platform, provider, settings, MAIN_WINDOW_LABEL};

const CONTRACT_VERSION: u8 = 1;
const VISIBILITY_EVENT: &str = "prompter://main-window-visibility";

/// The main window's floor, in *logical* pixels, mirroring `minWidth` and
/// `minHeight` in `tauri.conf.json`. A test below pins the two together.
const MIN_WINDOW_WIDTH: f64 = 1000.0;
const MIN_WINDOW_HEIGHT: f64 = 780.0;

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
    /// The window's size in logical points, as last reported by a resize.
    /// Flushed to disk when the app hides or exits rather than on every
    /// resize, so dragging an edge does not write a file per frame.
    last_size: Option<(f64, f64)>,
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
    TrayOpen,
}

impl ActivationSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::DockReopen => "dock_reopen",
            Self::SecondInstance => "second_instance",
            Self::QuickCapture => "quick_capture",
            Self::TrayOpen => "tray_open",
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
    restore_window_size(app);
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

/// Restores the main window to the size the user last left it at.
///
/// Prompter carries this itself instead of letting `tauri-plugin-window-state`
/// do it. That plugin measures `inner_size()` in physical pixels and replays
/// the number as physical pixels, never recording the scale factor it was
/// taken at, so a window saved on a 2x panel and reopened against a 1x one
/// comes back at half the size — which is how the window was arriving at
/// 676x505 with the sidebar and the prompt dock squeezed out of usefulness.
/// Logical points do not have that failure mode, and `SIZE` is left out of the
/// plugin's flags so nothing overwrites what is set here.
///
/// Restoring during setup is safe precisely because the plugin no longer
/// touches the size: there is no longer anything to race.
fn restore_window_size<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    let coordinator = app.state::<settings::SettingsCoordinator>();
    let Some(stored) = settings::read_backend_string(app, &coordinator, settings::WINDOW_SIZE_KEY)
    else {
        return;
    };
    let Some((width, height)) = parse_window_size(&stored) else {
        warn!(
            target: "prompter::lifecycle",
            "event=window_size_restore outcome=unreadable value={stored}"
        );
        return;
    };

    // A stored size below the floor is still corrected on the way in, so a
    // document written by an older build cannot reintroduce the cramped window.
    let (width, height) = window_size_correction(width, height).unwrap_or((width, height));
    match window.set_size(LogicalSize::new(width, height)) {
        Ok(()) => info!(
            target: "prompter::lifecycle",
            "event=window_size_restore outcome=success size={width:.0}x{height:.0}"
        ),
        Err(error) => warn!(
            target: "prompter::lifecycle",
            "event=window_size_restore outcome=failure reason={error}"
        ),
    }
}

/// Writes the size recorded by the last resize, if there was one.
///
/// Called when the app hides and when it exits, which between them cover every
/// way the user finishes with the window.
pub(crate) fn persist_window_size<R: Runtime>(app: &AppHandle<R>) {
    let lifecycle = app.state::<AppLifecycleCoordinator>();
    let size = lifecycle
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .last_size;
    let Some((width, height)) = size else {
        return;
    };

    let coordinator = app.state::<settings::SettingsCoordinator>();
    let value = format!("{width:.0}x{height:.0}");
    if let Err(error) =
        settings::write_backend_string(app, &coordinator, settings::WINDOW_SIZE_KEY, &value)
    {
        warn!(
            target: "prompter::lifecycle",
            "event=window_size_persist outcome=failure reason={error:?}"
        );
    }
}

/// Reads a `WIDTHxHEIGHT` pair of logical points back.
///
/// Anything unparseable, non-finite, or non-positive is rejected rather than
/// coerced: the configured default is a better window than a guess built from
/// a damaged document.
fn parse_window_size(value: &str) -> Option<(f64, f64)> {
    let (width, height) = value.split_once('x')?;
    let width: f64 = width.trim().parse().ok()?;
    let height: f64 = height.trim().parse().ok()?;
    (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0)
        .then_some((width, height))
}

/// Grows the window back to the size the layout needs, if something shrank it
/// below that.
///
/// The floor is enforced here as well as on the way in, because a scale factor
/// change while the app runs can resize the window without the user asking.
/// AppKit does not cover that case: `contentMinSize` governs the user's own
/// dragging, which is why an edge drag stops dead on the floor, but a
/// programmatic `setContentSize:` passes straight through it.
///
/// Only the floor is enforced, never a ceiling. An oversized window is
/// something the user can drag back; capping one here would fight anybody
/// deliberately spanning two displays.
fn apply_window_size_floor<R: Runtime>(window: &Window<R>, size: PhysicalSize<u32>) {
    let scale = match window.scale_factor() {
        Ok(scale) => scale,
        Err(error) => {
            warn!(
                target: "prompter::lifecycle",
                "event=window_size_floor outcome=failure reason={error}"
            );
            return;
        }
    };
    let current = size.to_logical::<f64>(scale);

    let app = window.app_handle();
    let lifecycle = app.state::<AppLifecycleCoordinator>();
    lifecycle
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .last_size = Some((current.width, current.height));

    let Some((width, height)) = window_size_correction(current.width, current.height) else {
        return;
    };

    match window.set_size(LogicalSize::new(width, height)) {
        Ok(()) => info!(
            target: "prompter::lifecycle",
            "event=window_size_floor outcome=corrected from={:.0}x{:.0} to={width:.0}x{height:.0}",
            current.width, current.height
        ),
        Err(error) => warn!(
            target: "prompter::lifecycle",
            "event=window_size_floor outcome=failure reason={error}"
        ),
    }
}

fn window_size_correction(width: f64, height: f64) -> Option<(f64, f64)> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    if width >= MIN_WINDOW_WIDTH && height >= MIN_WINDOW_HEIGHT {
        return None;
    }
    Some((width.max(MIN_WINDOW_WIDTH), height.max(MIN_WINDOW_HEIGHT)))
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

    // Every native provider pane is positioned against this window. AppKit
    // moves and resizes it without the frontend ever hearing about it — a
    // restored-maximized launch zooms the window asynchronously, a display
    // change restacks it, an occluded window stops running animation frames
    // entirely — so the pane's placement is refreshed from here rather than
    // left to depend on the DOM noticing.
    if matches!(
        event,
        WindowEvent::Resized(_) | WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. }
    ) {
        provider::schedule_placement_refresh(window.app_handle());
    }

    // Ordered before nothing in particular: a correction here raises another
    // resize, which finds the window in range and stops. The placement refresh
    // above is scheduled rather than immediate, so it picks up the corrected
    // size either way.
    if let WindowEvent::Resized(size) = event {
        apply_window_size_floor(window, *size);
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
                persist_window_size(app);
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

    #[test]
    fn the_window_floor_matches_the_bundled_configuration() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        let main = &config["app"]["windows"][0];

        assert_eq!(main["label"], serde_json::json!(MAIN_WINDOW_LABEL));
        assert_eq!(main["minWidth"].as_f64().unwrap(), MIN_WINDOW_WIDTH);
        assert_eq!(main["minHeight"].as_f64().unwrap(), MIN_WINDOW_HEIGHT);
    }

    #[test]
    fn a_window_within_range_is_left_alone() {
        assert_eq!(
            window_size_correction(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT),
            None
        );
        assert_eq!(window_size_correction(1380.0, 850.0), None);
        // Nothing caps an oversized window: spanning two displays is a choice.
        assert_eq!(window_size_correction(2704.0, 2018.0), None);
    }

    #[test]
    fn a_window_halved_by_a_scale_factor_change_is_grown_back() {
        // Exactly what a 1352x1009 state file replays as on a 2x display, and
        // the size that used to drop the sidebar out of the layout entirely.
        assert_eq!(
            window_size_correction(676.0, 505.0),
            Some((MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT))
        );
    }

    #[test]
    fn only_the_axis_below_the_floor_is_corrected() {
        assert_eq!(
            window_size_correction(1380.0, 505.0),
            Some((1380.0, MIN_WINDOW_HEIGHT))
        );
        assert_eq!(
            window_size_correction(676.0, 900.0),
            Some((MIN_WINDOW_WIDTH, 900.0))
        );
    }

    #[test]
    fn a_corrected_size_is_not_corrected_again() {
        // The correction raises another resize event, so the second pass has
        // to settle rather than resize again.
        let (width, height) = window_size_correction(676.0, 505.0).unwrap();
        assert_eq!(window_size_correction(width, height), None);
    }

    #[test]
    fn a_window_with_no_size_is_left_alone() {
        // macOS reports a zero axis for a window that is off screen; resizing
        // one fights the window manager instead of the bug.
        assert_eq!(window_size_correction(0.0, 0.0), None);
        assert_eq!(window_size_correction(1380.0, 0.0), None);
    }

    #[test]
    fn a_remembered_size_survives_the_round_trip() {
        // The format written by `persist_window_size`, read back by
        // `restore_window_size`. Points are whole numbers on the way out.
        assert_eq!(parse_window_size("1380x850"), Some((1380.0, 850.0)));
        assert_eq!(parse_window_size(" 1380 x 850 "), Some((1380.0, 850.0)));
    }

    #[test]
    fn a_damaged_remembered_size_is_refused_rather_than_guessed() {
        // Each of these would open the window somewhere useless if coerced,
        // and the configured default is a better answer than any of them.
        for value in [
            "",
            "1380",
            "1380x",
            "x850",
            "1380x0",
            "0x850",
            "-1380x850",
            "widexhigh",
            "1380x850x2",
            "NaNxNaN",
            "infxinf",
        ] {
            assert_eq!(parse_window_size(value), None, "{value} should be refused");
        }
    }

    #[test]
    fn a_remembered_size_below_the_floor_is_raised_on_the_way_in() {
        // A document written by a build that stored the cramped size cannot
        // bring the cramped window back with it.
        let (width, height) = parse_window_size("676x505").unwrap();
        assert_eq!(
            window_size_correction(width, height),
            Some((MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT))
        );
    }
}
