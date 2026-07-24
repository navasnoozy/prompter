macro_rules! define_app_commands {
    ($( $name:literal => $handler:ident ),+ $(,)?) => {
        // The library build consumes the handler macro, while build.rs and the
        // capability regression test consume the names. Each compilation unit
        // therefore sees one intentionally unused half of this single source.
        #[allow(dead_code)]
        const APP_COMMAND_NAMES: &[&str] = &[$($name),+];

        #[allow(unused_macros)]
        macro_rules! app_command_handler {
            () => {
                tauri::generate_handler![$($handler),+]
            };
        }
    };
}

define_app_commands!(
    "get_provider_navigation_state" => get_provider_navigation_state,
    "control_provider_navigation" => control_provider_navigation,
    "show_provider_webview" => show_provider_webview,
    "resize_provider_webview" => resize_provider_webview,
    "set_provider_visibility" => set_provider_visibility,
    "place_prompt" => place_prompt,
    "get_quick_capture_status" => get_quick_capture_status,
    "request_quick_capture_permission" => request_quick_capture_permission,
    "open_quick_capture_settings" => open_quick_capture_settings,
    "retry_quick_capture_registration" => retry_quick_capture_registration,
    "read_clipboard_text" => read_clipboard_text,
    "list_quick_capture_outcomes" => list_quick_capture_outcomes,
    "acknowledge_quick_capture_outcomes" => acknowledge_quick_capture_outcomes,
    "get_app_lifecycle_status" => get_app_lifecycle_status,
    "set_launch_at_login" => set_launch_at_login,
    "load_settings" => load_settings,
    "save_settings" => save_settings,
);
