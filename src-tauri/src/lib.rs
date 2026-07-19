mod platform;
mod prompt;
mod provider;
#[cfg(target_os = "macos")]
mod quick_capture;

use prompt::compose_prompt;
use provider::{
    fill_provider_prompt, resize_provider_webview, set_provider_visibility, show_provider_webview,
    ProviderLifecycle,
};
#[cfg(target_os = "macos")]
use quick_capture::{
    get_quick_capture_status, open_quick_capture_settings, read_clipboard_text,
    request_quick_capture_permission, retry_quick_capture_registration,
    take_quick_capture_outcomes, QuickCaptureCoordinator,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Err(error) = quick_capture::show_main_window(app) {
            log::warn!(
                target: "prompter::lifecycle",
                "event=single_instance_focus_failed reason={error}"
            );
        }
    }));

    let builder = builder
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .filter(|metadata| metadata.target().starts_with("prompter"))
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .manage(ProviderLifecycle::default())
        .invoke_handler(tauri::generate_handler![
            compose_prompt,
            show_provider_webview,
            resize_provider_webview,
            set_provider_visibility,
            fill_provider_prompt,
            #[cfg(target_os = "macos")]
            get_quick_capture_status,
            #[cfg(target_os = "macos")]
            request_quick_capture_permission,
            #[cfg(target_os = "macos")]
            open_quick_capture_settings,
            #[cfg(target_os = "macos")]
            retry_quick_capture_registration,
            #[cfg(target_os = "macos")]
            read_clipboard_text,
            #[cfg(target_os = "macos")]
            take_quick_capture_outcomes
        ])
        .on_window_event(|window, event| {
            #[cfg(target_os = "macos")]
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Err(error) = window.hide() {
                        log::warn!(
                            target: "prompter::lifecycle",
                            "event=window_hide_failed reason={error}"
                        );
                    }
                }
            }
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            quick_capture::initialize(app.handle());
            Ok(())
        });

    #[cfg(target_os = "macos")]
    let builder = builder
        .manage(QuickCaptureCoordinator::default())
        .plugin(quick_capture::shortcut_plugin());

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building Prompter");

    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } = event
        {
            if let Err(error) = quick_capture::show_main_window(app) {
                log::warn!(
                    target: "prompter::lifecycle",
                    "event=dock_reopen_failed reason={error}"
                );
            }
        }
    });
}
