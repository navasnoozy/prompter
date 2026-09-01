import sys

with open('src-tauri/src/platform/macos.rs', 'r') as f:
    content = f.read()

insert_point = content.find('pub(crate) fn log_webview_geometry')
if insert_point == -1:
    print("Could not find insert point")
    sys.exit(1)

new_fn = """
pub(crate) fn webview_native_offset(webview: &tauri::Webview) -> f64 {
    webview
        .with_webview(|platform_webview| unsafe {
            let view = platform_webview.inner().cast::<objc2::runtime::AnyObject>();
            let window: *mut objc2::runtime::AnyObject = objc2::msg_send![view, window];
            if window.is_null() {
                return 0.0;
            }
            let window_frame: objc2_foundation::NSRect = objc2::msg_send![window, frame];
            let content_view: *mut objc2::runtime::AnyObject = objc2::msg_send![window, contentView];
            if content_view.is_null() {
                return 0.0;
            }
            let content_frame: objc2_foundation::NSRect = objc2::msg_send![content_view, frame];
            (window_frame.size.height - content_frame.size.height) as f64
        })
        .unwrap_or(0.0)
}

"""

new_content = content[:insert_point] + new_fn + content[insert_point:]

with open('src-tauri/src/platform/macos.rs', 'w') as f:
    f.write(new_content)
print("Updated macos.rs")
