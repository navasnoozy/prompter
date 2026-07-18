#[cfg(target_os = "macos")]
mod capture;
mod platform;
mod prompt;
mod provider;

use prompt::compose_prompt;
use provider::{
    fill_provider_prompt, resize_provider_webview, set_provider_visibility, show_provider_webview,
    ProviderLifecycle,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .manage(ProviderLifecycle::default())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            compose_prompt,
            show_provider_webview,
            resize_provider_webview,
            set_provider_visibility,
            fill_provider_prompt
        ]);

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(capture::shortcut_plugin());

    builder
        .run(tauri::generate_context!())
        .expect("error while running Prompter");
}
