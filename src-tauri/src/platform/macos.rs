use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode},
    event_source::{CGEventSource, CGEventSourceStateID},
};
use objc2_app_kit::NSPasteboard;

pub(super) const PROVIDER_CONTENT_OFFSET_Y: f64 = 32.0;
const PROVIDER_CORNER_RADIUS: f64 = 16.0;

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

pub(crate) fn copy_current_selection() -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Could not create a macOS keyboard event.".to_string())?;
    let key_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_C, true)
        .map_err(|_| "Could not create the copy key event.".to_string())?;
    let key_up = CGEvent::new_keyboard_event(source, KeyCode::ANSI_C, false)
        .map_err(|_| "Could not create the copy key event.".to_string())?;

    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);
    key_up.post(CGEventTapLocation::HID);
    Ok(())
}

pub(crate) fn clipboard_change_count() -> isize {
    NSPasteboard::generalPasteboard().changeCount()
}
