//! Prompter is a macOS-only application: Quick Capture, window lifecycle, and
//! the embedded provider panes all depend on AppKit behavior, so the crate
//! makes no attempt to compile for other platforms.

mod app_lifecycle;
mod platform;
mod prompt;
mod provider;
mod quick_capture;

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";

use app_lifecycle::{
    get_app_lifecycle_status, set_launch_at_login, ActivationSource, AppLifecycleCoordinator,
};
use provider::{
    place_prompt, resize_provider_webview, set_provider_visibility, show_provider_webview,
    ProviderLifecycle,
};
use quick_capture::{
    get_quick_capture_status, open_quick_capture_settings, read_clipboard_text,
    request_quick_capture_permission, retry_quick_capture_registration,
    take_quick_capture_outcomes, QuickCaptureCoordinator,
};

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            app_lifecycle::handle_second_instance(app, &args);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                // Prompter's own targets log at Info+; framework targets are
                // kept only at Warn+ so plugin/webview failures stay visible.
                .filter(|metadata| {
                    metadata.target().starts_with("prompter")
                        || metadata.level() <= log::Level::Warn
                })
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(quick_capture::shortcut_plugin())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(ProviderLifecycle::default())
        .manage(AppLifecycleCoordinator::default())
        .manage(QuickCaptureCoordinator::default())
        .invoke_handler(tauri::generate_handler![
            show_provider_webview,
            resize_provider_webview,
            set_provider_visibility,
            place_prompt,
            get_quick_capture_status,
            request_quick_capture_permission,
            open_quick_capture_settings,
            retry_quick_capture_registration,
            read_clipboard_text,
            take_quick_capture_outcomes,
            get_app_lifecycle_status,
            set_launch_at_login
        ])
        .on_window_event(app_lifecycle::handle_window_event)
        .setup(|app| {
            let autostart_available = app_lifecycle::install_autostart_plugin(app.handle());
            app_lifecycle::initialize(app.handle(), autostart_available);
            app_lifecycle::install_tray(app.handle());
            quick_capture::initialize(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Prompter");

    app.run(|app, event| match event {
        tauri::RunEvent::Reopen { .. } => {
            if let Err(error) = app_lifecycle::request_activation(app, ActivationSource::DockReopen)
            {
                log::warn!(
                    target: "prompter::lifecycle",
                    "event=dock_reopen_failed reason={error}"
                );
            }
        }
        tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } => {
            quick_capture::handle_exit_requested(app, &api);
        }
        _ => {}
    });
}
