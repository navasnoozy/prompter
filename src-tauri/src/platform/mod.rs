mod macos;
mod webview_navigation;

pub(crate) use macos::{
    apply_provider_corner_radius, configure_main_window_active_space, open_in_default_browser,
};
pub(crate) use webview_navigation::{
    control_provider_navigation, detach_provider_navigation_observer, observe_provider_navigation,
    NativeNavigationAction, NativeNavigationSnapshot,
};
