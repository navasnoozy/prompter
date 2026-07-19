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
