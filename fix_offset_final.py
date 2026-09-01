import sys

with open('src-tauri/src/platform/macos.rs', 'r') as f:
    content = f.read()

target = """pub(crate) fn titlebar_offset<R: Runtime>(window: &tauri::Window<R>) -> f64 {
    if let Ok(ns_window) = window.ns_window() {
        unsafe {
            let ns_window = ns_window as *mut objc2::runtime::AnyObject;
            let window_frame: objc2_foundation::NSRect = objc2::msg_send![ns_window, frame];
            let content_layout: objc2_foundation::NSRect = objc2::msg_send![ns_window, contentLayoutRect];
            let diff = window_frame.size.height - content_layout.size.height;
            return if diff > 0.0 { diff as f64 } else { 0.0 };
        }
    }
    0.0
}"""

repl = """pub(crate) fn titlebar_offset<R: Runtime>(window: &tauri::Window<R>) -> f64 {
    if let Ok(ns_window) = window.ns_window() {
        unsafe {
            let ns_window = ns_window as *mut objc2::runtime::AnyObject;
            let window_frame: objc2_foundation::NSRect = objc2::msg_send![ns_window, frame];
            let content_rect: objc2_foundation::NSRect = objc2::msg_send![ns_window, contentRectForFrameRect: window_frame];
            let diff = window_frame.size.height - content_rect.size.height;
            log::info!(target: "prompter::provider", "event=diagnostic_offset window={:.1} content={:.1} diff={:.1}", window_frame.size.height, content_rect.size.height, diff);
            return if diff > 0.0 { diff as f64 } else { 0.0 };
        }
    }
    0.0
}"""

if target in content:
    content = content.replace(target, repl)
else:
    # Also handle the older version just in case
    target2 = """pub(crate) fn titlebar_offset<R: Runtime>(window: &tauri::Window<R>) -> f64 {
    if let Ok(ns_window) = window.ns_window() {
        unsafe {
            let ns_window = ns_window as *mut objc2::runtime::AnyObject;
            let window_frame: objc2_foundation::NSRect = objc2::msg_send![ns_window, frame];
            let content_view: *mut objc2::runtime::AnyObject =
                objc2::msg_send![ns_window, contentView];
            if !content_view.is_null() {
                let content_frame: objc2_foundation::NSRect = objc2::msg_send![content_view, frame];
                let diff = window_frame.size.height - content_frame.size.height;
                log::info!(target: "prompter::provider", "event=diagnostic window_height={:.1} content_height={:.1} diff={:.1}", window_frame.size.height, content_frame.size.height, diff);
                return if diff > 0.0 { diff as f64 } else { 0.0 };
            }
        }
    }
    0.0
}"""
    content = content.replace(target2, repl)

with open('src-tauri/src/platform/macos.rs', 'w') as f:
    f.write(content)

