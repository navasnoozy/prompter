use std::ptr::NonNull;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
use tauri::{Runtime, Window};

pub(super) const PROVIDER_CONTENT_OFFSET_Y: f64 = 32.0;
const PROVIDER_CORNER_RADIUS: f64 = 16.0;

/// Makes the permanent main window follow the user to their active macOS Space.
///
/// This is a one-time AppKit policy. It preserves one real window (and its child
/// provider WebViews) instead of cloning the window or showing it on every Space.
pub(crate) fn configure_main_window_active_space<R: Runtime>(
    window: &Window<R>,
) -> Result<(), String> {
    let _main_thread = MainThreadMarker::new()
        .ok_or_else(|| "active Space policy must be configured on the main thread".to_string())?;
    let native_window = NonNull::new(
        window
            .ns_window()
            .map_err(|error| format!("could not access the native window: {error}"))?,
    )
    .ok_or_else(|| "Tauri returned a null native window".to_string())?;

    // SAFETY: Tauri returned the NSWindow pointer for this live Window. Setup
    // calls this adapter on AppKit's main thread before lifecycle activation is
    // enabled, and the reference does not escape this function.
    let native_window = unsafe { native_window.cast::<NSWindow>().as_ref() };
    let behavior = behavior_for_active_space(native_window.collectionBehavior());
    native_window.setCollectionBehavior(behavior);
    Ok(())
}

fn behavior_for_active_space(
    mut behavior: NSWindowCollectionBehavior,
) -> NSWindowCollectionBehavior {
    behavior.remove(NSWindowCollectionBehavior::CanJoinAllSpaces);
    behavior.insert(NSWindowCollectionBehavior::MoveToActiveSpace);
    behavior
}

pub(crate) fn apply_provider_corner_radius(webview: &tauri::Webview) -> Result<(), String> {
    webview
        .with_webview(|platform_webview| unsafe {
            // SAFETY: Tauri guarantees that PlatformWebview::inner is a valid WKWebView
            // pointer for the duration of this callback. WKWebView inherits from NSView,
            // which implements these layer-related selectors.
            let view = platform_webview.inner().cast::<objc2::runtime::AnyObject>();

            let _: () = objc2::msg_send![view, setWantsLayer: true];
            let layer: *mut objc2::runtime::AnyObject = objc2::msg_send![view, layer];

            if let Some(layer) = layer.as_ref() {
                let _: () = objc2::msg_send![layer, setCornerRadius: PROVIDER_CORNER_RADIUS];
                let _: () = objc2::msg_send![layer, setMasksToBounds: true];
            }
        })
        .map_err(|error| format!("Could not round the embedded browser: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_space_policy_replaces_all_spaces_and_preserves_other_behaviors() {
        let original = NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Managed
            | NSWindowCollectionBehavior::FullScreenPrimary;

        let configured = behavior_for_active_space(original);

        assert!(configured.contains(NSWindowCollectionBehavior::MoveToActiveSpace));
        assert!(!configured.contains(NSWindowCollectionBehavior::CanJoinAllSpaces));
        assert!(configured.contains(NSWindowCollectionBehavior::Managed));
        assert!(configured.contains(NSWindowCollectionBehavior::FullScreenPrimary));
    }

    #[test]
    fn active_space_policy_is_idempotent() {
        let original = NSWindowCollectionBehavior::MoveToActiveSpace
            | NSWindowCollectionBehavior::ParticipatesInCycle;

        assert_eq!(
            behavior_for_active_space(behavior_for_active_space(original)),
            original
        );
    }
}
