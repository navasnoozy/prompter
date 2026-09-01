import sys

with open('src-tauri/src/platform/mod.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'open_in_default_browser, pin_provider_webview_edges,',
    'open_in_default_browser, pin_provider_webview_edges, webview_native_offset,'
)

with open('src-tauri/src/platform/mod.rs', 'w') as f:
    f.write(content)
print("Updated mod.rs")
