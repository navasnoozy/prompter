use std::ptr::NonNull;

use log::info;
use objc2::runtime::{AnyObject, Bool};
use objc2::{msg_send, MainThreadMarker};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSWindow, NSWindowCollectionBehavior, NSWorkspace};
use objc2_foundation::{NSEdgeInsets, NSPoint, NSRect, NSSize, NSString, NSURL};
use tauri::{Runtime, Window};

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

pub(crate) fn open_in_default_browser(url: &str) -> Result<(), String> {
    let value = NSString::from_str(url);
    let native_url =
        NSURL::URLWithString(&value).ok_or_else(|| "the URL could not be parsed".to_string())?;

    if NSWorkspace::sharedWorkspace().openURL(&native_url) {
        Ok(())
    } else {
        Err("macOS declined to open the URL".into())
    }
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

/// Pins a provider pane to all four edges of the view that hosts it, so AppKit
/// resizes it in the same frame as the window.
///
/// wry creates child WebViews with `NSViewMinYMargin` — a fixed-size view held
/// against the top of its host. That leaves the pane the wrong size the instant
/// the window changes, and correcting it means a round trip out to the frontend
/// and back, which lags a live drag and stalls entirely while the window is
/// occluded. A fully sizable mask makes the common case free and exact: the
/// pane's insets are already zero-margin on every edge, so tracking the host
/// view reproduces the layout the frontend asked for without any IPC at all.
///
/// Placement still re-derives the pane's rect after the window settles. The
/// mask keeps it right during the change; the re-derivation keeps it right
/// afterwards, when the layout itself — not just the window — may have moved.
pub(crate) fn pin_provider_webview_edges(webview: &tauri::Webview) -> Result<(), String> {
    webview
        .with_webview(|platform_webview| unsafe {
            // SAFETY: Tauri guarantees that PlatformWebview::inner is a valid
            // WKWebView pointer for the duration of this callback. WKWebView
            // inherits from NSView, which declares `setAutoresizingMask:`.
            let view = platform_webview.inner().cast::<objc2::runtime::AnyObject>();
            let mask = NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable;
            let _: () = objc2::msg_send![view, setAutoresizingMask: mask];
        })
        .map_err(|error| format!("Could not pin the embedded browser to the window: {error}"))
}

/// Reports where AppKit has actually put a WebView, in screen coordinates.
///
/// TEMPORARY DIAGNOSTIC. Every other geometry reading in this codebase goes
/// through wry, and wry answers `bounds()` by inverting the exact coordinate
/// flip it applied in `set_bounds`. A pane placed against the wrong height
/// therefore reads back as correctly placed — the error cancels itself out, so
/// the confirmation pass in `provider::placement` cannot see it. This asks
/// AppKit directly, which makes the answer independent of our own arithmetic.
///
/// The reading is logged from inside the callback rather than returned:
/// `with_webview` dispatches to the main thread *without waiting* when called
/// from anywhere else, so a value carried out of the closure would be read
/// before the closure had run.
pub(crate) fn log_webview_geometry(webview: &tauri::Webview, role: &'static str) {
    let dispatched = webview.with_webview(move |platform_webview| unsafe {
        // SAFETY: Tauri guarantees PlatformWebview::inner is a valid WKWebView
        // pointer for the duration of this callback. WKWebView inherits from
        // NSView, and every selector below is declared by NSView or NSWindow.
        let view = platform_webview.inner().cast::<AnyObject>();
        let superview: *mut AnyObject = msg_send![view, superview];
        let window: *mut AnyObject = msg_send![view, window];

        if superview.is_null() || window.is_null() {
            info!(
                target: "prompter::provider",
                "event=geometry_probe role={role} attached=false"
            );
            return;
        }

        let content_view: *mut AnyObject = msg_send![window, contentView];
        let frame: NSRect = msg_send![view, frame];
        let bounds: NSRect = msg_send![view, bounds];
        let superview_bounds: NSRect = msg_send![superview, bounds];
        let flipped: Bool = msg_send![superview, isFlipped];
        let window_frame: NSRect = msg_send![window, frame];
        // `contentLayoutRect` is AppKit's own answer to "which part of the
        // content view is not covered by the title bar". If it differs from the
        // content view's bounds, the web content is running underneath the
        // title bar and the layout has to allow for it.
        let content_layout: NSRect = msg_send![window, contentLayoutRect];
        let style_mask: usize = msg_send![window, styleMask];
        // What AppKit tells *this view* to keep clear. A WebView that spans a
        // full-size content view is told the title bar covers its top edge, and
        // WebKit turns that into a content inset — which moves the DOM's origin
        // away from the view's own origin.
        let safe_area: NSEdgeInsets = msg_send![view, safeAreaInsets];

        let nil = std::ptr::null_mut::<AnyObject>();
        let in_window: NSRect = msg_send![view, convertRect: bounds, toView: nil];
        let screen: NSRect = msg_send![window, convertRectToScreen: in_window];

        let content_screen = if content_view.is_null() {
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0))
        } else {
            let content_bounds: NSRect = msg_send![content_view, bounds];
            let in_window: NSRect =
                msg_send![content_view, convertRect: content_bounds, toView: nil];
            msg_send![window, convertRectToScreen: in_window]
        };

        info!(
            target: "prompter::provider",
            "event=geometry_probe role={role} superview_is_content_view={} superview_flipped={} \
        frame={} superview_bounds={} screen={} screen_top={:.1} window={} window_top={:.1} \
        content_screen={} content_top={:.1} content_layout={} style_mask=0x{:x} \
        safe_area_top={:.1} safe_area_bottom={:.1}",
            superview == content_view,
            flipped.as_bool(),
            describe_rect(frame),
            describe_rect(superview_bounds),
            describe_rect(screen),
            screen.origin.y + screen.size.height,
            describe_rect(window_frame),
            window_frame.origin.y + window_frame.size.height,
            describe_rect(content_screen),
            content_screen.origin.y + content_screen.size.height,
            describe_rect(content_layout),
            style_mask,
            safe_area.top,
            safe_area.bottom,
        );
    });

    if let Err(error) = dispatched {
        info!(
            target: "prompter::provider",
            "event=geometry_probe role={role} outcome=unavailable reason={error}"
        );
    }
}

/// `[x,y,w,h]` exactly as AppKit reported it, unconverted, so the log records
/// what was read rather than what this code believes it means.
fn describe_rect(value: NSRect) -> String {
    format!(
        "[{:.1},{:.1},{:.1},{:.1}]",
        value.origin.x, value.origin.y, value.size.width, value.size.height
    )
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
