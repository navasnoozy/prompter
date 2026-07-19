mod macos;
mod model;
mod service;

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
    thread,
    time::Instant,
};

use log::{error, info, warn};
use tauri::{
    plugin::TauriPlugin, AppHandle, Emitter, Manager, Runtime, State, WebviewWindow,
    WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutEvent, ShortcutState};

use self::{
    macos::MacCaptureBackend,
    model::{
        CaptureCommandError, CaptureErrorCode, CaptureOutcome, CaptureReadyEvent, CaptureWarning,
        CaptureWarningCode, ClipboardTextPayload, PermissionState, QuickCaptureStatus,
        ShortcutRegistrationState, CAPTURE_SHORTCUT, CONTRACT_VERSION,
    },
    service::{capture_selection, validate_text},
};

const READY_EVENT: &str = "prompter://quick-capture-ready";
const MAX_PENDING_OUTCOMES: usize = 8;

pub(crate) struct QuickCaptureCoordinator {
    registration: RwLock<ShortcutRegistrationState>,
    registration_gate: Mutex<()>,
    in_progress: Arc<AtomicBool>,
    request_sequence: AtomicU64,
    pending_outcomes: Mutex<VecDeque<CaptureOutcome>>,
}

impl Default for QuickCaptureCoordinator {
    fn default() -> Self {
        Self {
            registration: RwLock::new(ShortcutRegistrationState::Unavailable),
            registration_gate: Mutex::new(()),
            in_progress: Arc::new(AtomicBool::new(false)),
            request_sequence: AtomicU64::new(1),
            pending_outcomes: Mutex::new(VecDeque::new()),
        }
    }
}

impl QuickCaptureCoordinator {
    fn registration(&self) -> ShortcutRegistrationState {
        self.registration
            .read()
            .map(|value| *value)
            .unwrap_or(ShortcutRegistrationState::Unavailable)
    }

    fn set_registration(&self, value: ShortcutRegistrationState) {
        match self.registration.write() {
            Ok(mut registration) => *registration = value,
            Err(poisoned) => *poisoned.into_inner() = value,
        }
    }

    fn try_begin_capture(&self) -> Option<CaptureLease> {
        self.in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| CaptureLease {
                in_progress: Arc::clone(&self.in_progress),
            })
    }

    fn next_request_id(&self) -> String {
        let sequence = self.request_sequence.fetch_add(1, Ordering::Relaxed);
        format!("capture-{sequence}")
    }

    fn push_outcome(&self, outcome: CaptureOutcome) {
        let mut outcomes = self
            .pending_outcomes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if outcomes.len() == MAX_PENDING_OUTCOMES {
            outcomes.pop_front();
            warn!(
                target: "prompter::quick_capture",
                "event=outcome_queue_overflow action=dropped_oldest"
            );
        }
        outcomes.push_back(outcome);
    }

    fn take_outcomes(&self) -> Vec<CaptureOutcome> {
        let mut outcomes = self
            .pending_outcomes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        outcomes.drain(..).collect()
    }
}

struct CaptureLease {
    in_progress: Arc<AtomicBool>,
}

impl Drop for CaptureLease {
    fn drop(&mut self) {
        self.in_progress.store(false, Ordering::Release);
    }
}

pub(crate) fn shortcut_plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new().build()
}

pub(crate) fn initialize<R: Runtime>(app: &AppHandle<R>) {
    let status = register_shortcut(app);
    info!(
        target: "prompter::quick_capture",
        "event=shortcut_registration state={:?}",
        status.registration
    );
}

pub(crate) fn show_main_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = match app.get_webview_window("main") {
        Some(window) => window,
        None => recreate_main_window(app)?,
    };
    reveal_window(&window)
}

fn recreate_main_window<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>, String> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == "main")
        .ok_or_else(|| "main window configuration not found".to_string())?;

    info!(
        target: "prompter::lifecycle",
        "event=main_window_recovery action=recreate"
    );
    WebviewWindowBuilder::from_config(app, config)
        .and_then(WebviewWindowBuilder::build)
        .map_err(|error| error.to_string())
}

fn reveal_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    window
        .show()
        .and_then(|_| window.unminimize())
        .and_then(|_| window.set_focus())
        .map_err(|error| error.to_string())
}

fn register_shortcut<R: Runtime>(app: &AppHandle<R>) -> QuickCaptureStatus {
    let coordinator = app.state::<QuickCaptureCoordinator>();
    let _registration_guard = coordinator
        .registration_gate
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let shortcut_manager = app.global_shortcut();
    let registration = if shortcut_manager.is_registered(CAPTURE_SHORTCUT) {
        ShortcutRegistrationState::Registered
    } else {
        match shortcut_manager.on_shortcut(CAPTURE_SHORTCUT, handle_shortcut::<R>) {
            Ok(()) => ShortcutRegistrationState::Registered,
            Err(registration_error) => {
                warn!(
                    target: "prompter::quick_capture",
                    "event=shortcut_registration_failed reason={registration_error}"
                );
                ShortcutRegistrationState::Unavailable
            }
        }
    };

    coordinator.set_registration(registration);
    current_status(&coordinator)
}

fn handle_shortcut<R: Runtime>(
    app: &AppHandle<R>,
    _shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: ShortcutEvent,
) {
    if event.state != ShortcutState::Released {
        return;
    }

    let coordinator = app.state::<QuickCaptureCoordinator>();
    let Some(lease) = coordinator.try_begin_capture() else {
        info!(
            target: "prompter::quick_capture",
            "event=capture_coalesced reason=already_in_progress"
        );
        return;
    };

    let app = app.clone();
    if let Err(spawn_error) = thread::Builder::new()
        .name("prompter-quick-capture".into())
        .spawn(move || process_capture(app, lease))
    {
        error!(
            target: "prompter::quick_capture",
            "event=capture_worker_spawn_failed reason={spawn_error}"
        );
    }
}

fn process_capture<R: Runtime>(app: AppHandle<R>, _lease: CaptureLease) {
    let coordinator = app.state::<QuickCaptureCoordinator>();
    let request_id = coordinator.next_request_id();
    let started = Instant::now();
    let result = capture_selection(&MacCaptureBackend);

    let mut window_warning = None;
    if let Err(window_error) = show_main_window(&app) {
        warn!(
            target: "prompter::quick_capture",
            "event=window_reveal_failed request_id={request_id} reason={window_error}"
        );
        window_warning = Some(CaptureWarningCode::WindowUnavailable);
    }

    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let outcome = match result {
        Ok(captured) => {
            let mut warnings = Vec::with_capacity(2);
            if let Some(warning) = captured.warning {
                warnings.push(CaptureWarning::from(warning));
            }
            if let Some(warning) = window_warning {
                warnings.push(CaptureWarning::from(warning));
            }
            info!(
                target: "prompter::quick_capture",
                "event=capture_completed request_id={request_id} outcome=success duration_ms={duration_ms} warning_count={}",
                warnings.len()
            );
            CaptureOutcome::Success {
                version: CONTRACT_VERSION,
                request_id: request_id.clone(),
                text: captured.text,
                warnings,
                duration_ms,
            }
        }
        Err(code) => {
            let permission = permission_state();
            warn!(
                target: "prompter::quick_capture",
                "event=capture_completed request_id={request_id} outcome=failure code={code:?} duration_ms={duration_ms}"
            );
            CaptureOutcome::Failure {
                version: CONTRACT_VERSION,
                request_id: request_id.clone(),
                code,
                message: code.user_message().to_string(),
                permission,
                duration_ms,
            }
        }
    };

    coordinator.push_outcome(outcome);
    let event = CaptureReadyEvent {
        version: CONTRACT_VERSION,
        request_id,
    };
    if let Err(emit_error) = app.emit_to("main", READY_EVENT, event) {
        warn!(
            target: "prompter::quick_capture",
            "event=capture_notification_failed reason={emit_error} delivery=pending_queue"
        );
    }
}

fn permission_state() -> PermissionState {
    if MacCaptureBackend::permission_state() {
        PermissionState::Granted
    } else {
        PermissionState::Required
    }
}

fn current_status(coordinator: &QuickCaptureCoordinator) -> QuickCaptureStatus {
    QuickCaptureStatus::new(coordinator.registration(), permission_state())
}

#[tauri::command]
pub(crate) fn get_quick_capture_status(
    coordinator: State<'_, QuickCaptureCoordinator>,
) -> QuickCaptureStatus {
    current_status(&coordinator)
}

#[tauri::command]
pub(crate) fn request_quick_capture_permission(
    coordinator: State<'_, QuickCaptureCoordinator>,
) -> QuickCaptureStatus {
    let granted = MacCaptureBackend::request_permission();
    info!(
        target: "prompter::quick_capture",
        "event=permission_request granted={granted}"
    );
    current_status(&coordinator)
}

#[tauri::command]
pub(crate) fn open_quick_capture_settings() -> Result<(), CaptureCommandError> {
    MacCaptureBackend::open_accessibility_settings()
        .map_err(|_| CaptureCommandError::new(CaptureErrorCode::Internal))
}

#[tauri::command]
pub(crate) fn retry_quick_capture_registration<R: Runtime>(
    app: AppHandle<R>,
) -> QuickCaptureStatus {
    register_shortcut(&app)
}

#[tauri::command]
pub(crate) fn read_clipboard_text() -> Result<ClipboardTextPayload, CaptureCommandError> {
    let text = MacCaptureBackend::read_current_text()
        .map_err(|_| CaptureCommandError::new(CaptureErrorCode::NoText))?;
    let text = validate_text(text).map_err(CaptureCommandError::new)?;
    Ok(ClipboardTextPayload {
        version: CONTRACT_VERSION,
        text,
    })
}

#[tauri::command]
pub(crate) fn take_quick_capture_outcomes(
    coordinator: State<'_, QuickCaptureCoordinator>,
) -> Vec<CaptureOutcome> {
    coordinator.take_outcomes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_gate_coalesces_and_recovers() {
        let coordinator = QuickCaptureCoordinator::default();
        let lease = coordinator
            .try_begin_capture()
            .expect("first capture should acquire the gate");

        assert!(coordinator.try_begin_capture().is_none());
        drop(lease);
        assert!(coordinator.try_begin_capture().is_some());
    }

    #[test]
    fn pending_outcomes_are_bounded_and_keep_the_latest_results() {
        let coordinator = QuickCaptureCoordinator::default();
        for index in 0..(MAX_PENDING_OUTCOMES + 2) {
            coordinator.push_outcome(CaptureOutcome::Failure {
                version: CONTRACT_VERSION,
                request_id: format!("capture-{index}"),
                code: CaptureErrorCode::Internal,
                message: "failure".into(),
                permission: PermissionState::Granted,
                duration_ms: 0,
            });
        }

        let outcomes = coordinator.take_outcomes();

        assert_eq!(outcomes.len(), MAX_PENDING_OUTCOMES);
        assert_eq!(outcomes[0].request_id(), "capture-2");
        assert_eq!(
            outcomes.last().map(CaptureOutcome::request_id),
            Some("capture-9")
        );
        assert!(coordinator.take_outcomes().is_empty());
    }
}
