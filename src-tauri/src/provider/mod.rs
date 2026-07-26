//! Embedded provider panes: configuration, geometry, the `prompter://`
//! response bridge, and the Tauri commands that manage the WebViews.

mod bridge;
mod commands;
mod config;
mod error;
mod geometry;
mod navigation;
mod new_chat;

pub(crate) use commands::{
    open_provider_new_chat, place_prompt, resize_provider_webview, set_provider_visibility,
    show_provider_webview, ProviderLifecycle,
};
pub(crate) use navigation::{
    control_provider_navigation, get_provider_navigation_state, ProviderNavigationCoordinator,
};
