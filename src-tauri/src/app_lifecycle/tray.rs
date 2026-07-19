use log::{info, warn};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Runtime,
};

use super::{request_activation, ActivationSource};
use crate::quick_capture;

const TRAY_ID: &str = "prompter-tray";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";

/// Installs the menu-bar status item that keeps the background app
/// discoverable after the window is closed. Failure is non-fatal: the app
/// still works through the Dock and the global shortcut.
pub(crate) fn install_tray<R: Runtime>(app: &AppHandle<R>) {
    match build_tray(app) {
        Ok(()) => info!(
            target: "prompter::lifecycle",
            "event=tray_install outcome=success"
        ),
        Err(error) => warn!(
            target: "prompter::lifecycle",
            "event=tray_install outcome=failure reason={error}"
        ),
    }
}

fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItemBuilder::with_id(MENU_OPEN, "Open Prompter").build(app)?;
    let shortcut_hint = MenuItemBuilder::with_id("shortcut-hint", "Quick Capture: ⌘ ⇧ P")
        .enabled(false)
        .build(app)?;
    let quit = MenuItemBuilder::with_id(MENU_QUIT, "Quit Prompter").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&open)
        .separator()
        .item(&shortcut_hint)
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::with_id(TRAY_ID)
        .title("✦")
        .tooltip("Prompter")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN => {
                if let Err(error) = request_activation(app, ActivationSource::TrayOpen) {
                    warn!(
                        target: "prompter::lifecycle",
                        "event=tray_open_failed reason={error}"
                    );
                }
            }
            // Quitting during an active Quick Capture defers until the
            // clipboard is restored; the deferral waiter exits on its own.
            MENU_QUIT if !quick_capture::defer_exit_if_capturing(app) => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}
