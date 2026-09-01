import sys

with open('src-tauri/src/platform/macos.rs', 'r') as f:
    content = f.read()

target = """pub(crate) fn webview_native_offset(webview: &tauri::Webview) -> f64 {
    webview
        .with_webview(|platform_webview| unsafe {
            let view = platform_webview.inner().cast::<objc2::runtime::AnyObject>();
            let window: *mut objc2::runtime::AnyObject = objc2::msg_send![view, window];
            if window.is_null() {
                return 0.0;
            }
            let window_frame: objc2_foundation::NSRect = objc2::msg_send![window, frame];
            let content_view: *mut objc2::runtime::AnyObject =
                objc2::msg_send![window, contentView];
            if content_view.is_null() {
                return 0.0;
            }
            let content_frame: objc2_foundation::NSRect = objc2::msg_send![content_view, frame];
            (window_frame.size.height - content_frame.size.height) as f64
        })
        .unwrap_or(0.0)
}
"""
if target in content:
    content = content.replace(target, '')
else:
    print("could not find target in macos.rs")

with open('src-tauri/src/platform/macos.rs', 'w') as f:
    f.write(content)

with open('src-tauri/src/platform/mod.rs', 'r') as f:
    mod_content = f.read()
mod_content = mod_content.replace('open_in_default_browser, pin_provider_webview_edges, webview_native_offset,', 'open_in_default_browser, pin_provider_webview_edges,')

with open('src-tauri/src/platform/mod.rs', 'w') as f:
    f.write(mod_content)

print("done")
