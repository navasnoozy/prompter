import sys

# 1. macos.rs
with open('src-tauri/src/platform/macos.rs', 'r') as f:
    content = f.read()

new_fn = """pub(crate) fn titlebar_offset<R: Runtime>(window: &tauri::Window<R>) -> f64 {
    if let Ok(ns_window) = window.ns_window() {
        unsafe {
            let ns_window = ns_window as *mut objc2::runtime::AnyObject;
            let window_frame: objc2_foundation::NSRect = objc2::msg_send![ns_window, frame];
            let content_view: *mut objc2::runtime::AnyObject =
                objc2::msg_send![ns_window, contentView];
            if !content_view.is_null() {
                let content_frame: objc2_foundation::NSRect =
                    objc2::msg_send![content_view, frame];
                let diff = window_frame.size.height - content_frame.size.height;
                return if diff > 0.0 { diff as f64 } else { 0.0 };
            }
        }
    }
    0.0
}

"""

if 'pub(crate) fn titlebar_offset' not in content:
    insert_point = content.find('pub(crate) fn log_webview_geometry')
    content = content[:insert_point] + new_fn + content[insert_point:]
    with open('src-tauri/src/platform/macos.rs', 'w') as f:
        f.write(content)

# 2. mod.rs
with open('src-tauri/src/platform/mod.rs', 'r') as f:
    content = f.read()

if 'titlebar_offset' not in content:
    content = content.replace(
        'open_in_default_browser, pin_provider_webview_edges,',
        'open_in_default_browser, pin_provider_webview_edges, titlebar_offset,'
    )
    with open('src-tauri/src/platform/mod.rs', 'w') as f:
        f.write(content)

# 3. geometry.rs
with open('src-tauri/src/provider/geometry.rs', 'r') as f:
    content = f.read()

target = """    let window = main.window();
    let titlebar_offset = match (window.inner_position(), window.outer_position()) {
        (Ok(inner), Ok(outer)) => {
            let diff = inner.y as f64 - outer.y as f64;
            diff.max(0.0) / scale
        }
        _ => 0.0,
    };"""

repl = """    let window = main.window();
    let titlebar_offset = crate::platform::titlebar_offset(&window);"""

if target in content:
    content = content.replace(target, repl)
    with open('src-tauri/src/provider/geometry.rs', 'w') as f:
        f.write(content)

print("Patched completely.")
