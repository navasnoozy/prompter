//! Prompter is a macOS-only application: Quick Capture, window lifecycle, and
//! the embedded provider panes all depend on AppKit behavior, so the crate
//! makes no attempt to compile for other platforms.

mod app_lifecycle;
mod app_shortcuts;
mod platform;
mod prompt;
mod provider;
mod quick_capture;
mod settings;

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";

use app_lifecycle::{
    get_app_lifecycle_status, set_launch_at_login, ActivationSource, AppLifecycleCoordinator,
};
use provider::{
    place_prompt, resize_provider_webview, set_provider_visibility, show_provider_webview,
    ProviderLifecycle,
};
use quick_capture::{
    acknowledge_quick_capture_outcomes, get_quick_capture_status, list_quick_capture_outcomes,
    open_quick_capture_settings, read_clipboard_text, request_quick_capture_permission,
    retry_quick_capture_registration, QuickCaptureCoordinator,
};
use settings::{load_settings, save_settings, SettingsCoordinator};

include!("command_manifest.rs");

pub fn run() {
    let app = tauri::Builder::default()
        .menu(app_shortcuts::build_menu)
        .on_menu_event(app_shortcuts::handle_menu_event)
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
        .manage(ProviderLifecycle::default())
        .manage(AppLifecycleCoordinator::default())
        .manage(QuickCaptureCoordinator::default())
        .manage(SettingsCoordinator::default())
        .invoke_handler(app_command_handler!())
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

#[cfg(test)]
mod command_manifest_tests {
    use std::collections::BTreeSet;

    use super::APP_COMMAND_NAMES;

    #[test]
    fn capability_command_permissions_match_the_native_manifest() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let permissions = capability["permissions"].as_array().unwrap();
        let actual: BTreeSet<String> = permissions
            .iter()
            .map(|permission| permission.as_str().unwrap().to_string())
            .collect();
        let mut expected: BTreeSet<String> = APP_COMMAND_NAMES
            .iter()
            .map(|command| format!("allow-{}", command.replace('_', "-")))
            .collect();
        expected.insert("core:event:allow-listen".into());
        expected.insert("core:event:allow-unlisten".into());

        assert_eq!(actual, expected);
        assert_eq!(permissions.len(), expected.len(), "duplicate permission");
        assert_eq!(capability["webviews"], serde_json::json!(["main"]));
        assert_eq!(capability["platforms"], serde_json::json!(["macOS"]));
    }
}
