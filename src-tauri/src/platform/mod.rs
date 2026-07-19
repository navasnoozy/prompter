mod macos;

pub(crate) use macos::{
    apply_provider_corner_radius, configure_main_window_active_space, open_in_default_browser,
};

pub(crate) fn provider_y_position(y: f64) -> f64 {
    y + macos::PROVIDER_CONTENT_OFFSET_Y
}

#[cfg(test)]
mod tests {
    use super::provider_y_position;

    #[test]
    fn provider_y_position_applies_the_platform_offset() {
        assert_eq!(provider_y_position(10.0), 42.0);
    }
}
