//! Embedded provider panes: configuration, geometry, the `prompter://`
//! response bridge, and the Tauri commands that manage the WebViews.

mod bridge;
mod commands;
mod config;
mod error;
mod geometry;

pub(crate) use commands::{
    place_prompt, resize_provider_webview, set_provider_visibility, show_provider_webview,
    ProviderLifecycle,
};
