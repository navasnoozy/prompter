#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub(crate) use macos::{
    apply_provider_corner_radius, clipboard_change_count, copy_current_selection,
};

#[cfg(target_os = "macos")]
pub(crate) fn provider_y_position(y: f64) -> f64 {
    y + macos::PROVIDER_CONTENT_OFFSET_Y
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn provider_y_position(y: f64) -> f64 {
    y
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply_provider_corner_radius(_webview: &tauri::Webview) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::provider_y_position;

    #[test]
    fn provider_y_position_applies_the_platform_offset() {
        #[cfg(target_os = "macos")]
        assert_eq!(provider_y_position(10.0), 42.0);

        #[cfg(not(target_os = "macos"))]
        assert_eq!(provider_y_position(10.0), 10.0);
    }
}
