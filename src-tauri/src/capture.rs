use serde::Serialize;
use std::{
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use tauri::{plugin::TauriPlugin, AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{ShortcutEvent, ShortcutState};

use crate::platform;

const CAPTURE_SHORTCUT: &str = "CommandOrControl+Shift+P";
const CLIPBOARD_POLL_INTERVAL: Duration = Duration::from_millis(25);
const CLIPBOARD_CHANGE_TIMEOUT: Duration = Duration::from_millis(800);
static CAPTURE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Serialize)]
struct ClipboardErrorPayload {
    message: String,
}

pub(crate) fn shortcut_plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_shortcut(CAPTURE_SHORTCUT)
        .expect("failed to configure the Prompter shortcut")
        .with_handler(handle_shortcut)
        .build()
}

fn handle_shortcut<R: Runtime>(
    app: &AppHandle<R>,
    _shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: ShortcutEvent,
) {
    if event.state != ShortcutState::Pressed {
        return;
    }

    let app = app.clone();
    thread::spawn(move || {
        if let Err(message) = capture_selected_text(&app) {
            let payload = ClipboardErrorPayload { message };
            if let Err(error) = app.emit_to("main", "prompter://clipboard-error", payload) {
                eprintln!("Could not report the clipboard capture error: {error}");
            }
        }
    });
}

fn capture_selected_text<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let _capture_guard = CAPTURE_LOCK
        .try_lock()
        .map_err(|_| "A text capture is already in progress.".to_string())?;

    let clipboard = app.clipboard();
    let previous_text = clipboard.read_text().ok();
    let initial_change_count = platform::clipboard_change_count();

    platform::copy_current_selection()?;
    wait_for_clipboard_change(initial_change_count)?;

    let captured_text = clipboard
        .read_text()
        .map_err(|_| "The selected content was not readable as text.".to_string())?;
    if captured_text.trim().is_empty() {
        return Err("Select some text, then try the shortcut again.".into());
    }

    if let Some(previous_text) = previous_text {
        clipboard.write_text(previous_text).map_err(|error| {
            format!("The text was captured, but the clipboard could not be restored: {error}")
        })?;
    }

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "The Prompter window was not found.".to_string())?;
    window
        .show()
        .and_then(|_| window.unminimize())
        .and_then(|_| window.set_focus())
        .map_err(|error| format!("Could not show Prompter: {error}"))?;

    app.emit_to("main", "prompter://clipboard-captured", captured_text)
        .map_err(|error| format!("Could not deliver the captured text: {error}"))
}

fn wait_for_clipboard_change(initial_change_count: isize) -> Result<(), String> {
    let deadline = Instant::now() + CLIPBOARD_CHANGE_TIMEOUT;

    while Instant::now() < deadline {
        if platform::clipboard_change_count() != initial_change_count {
            return Ok(());
        }
        thread::sleep(CLIPBOARD_POLL_INTERVAL);
    }

    Err(
        "The selected text was not copied. Allow Prompter in macOS Accessibility settings, then try again."
            .into(),
    )
}
