import sys

with open('src-tauri/src/provider/geometry.rs', 'r') as f:
    content = f.read()

# Revert my previous python script changes
replacement = """    let titlebar_offset = crate::platform::webview_native_offset(main);
    let surface = HostSurface {
        origin_x: position.x,
        origin_y: position.y + titlebar_offset,
        width: size.width,
        height: size.height,
    };"""

target = """    let surface = HostSurface {
        origin_x: position.x,
        origin_y: position.y,
        width: size.width,
        height: size.height,
    };"""

if replacement in content:
    content = content.replace(replacement, target)

# Now inject the new titlebar offset using window inner/outer position
target_measure = """    let size = bounds.size.to_logical::<f64>(scale);

    let surface = HostSurface {"""

replacement_measure = """    let size = bounds.size.to_logical::<f64>(scale);

    let window = main.window();
    let titlebar_offset = match (window.inner_position(), window.outer_position()) {
        (Ok(inner), Ok(outer)) => {
            let diff = inner.y as f64 - outer.y as f64;
            diff.max(0.0) / scale
        }
        _ => 0.0,
    };

    let surface = HostSurface {
        origin_x: position.x,
        origin_y: position.y + titlebar_offset,
        width: size.width,
        height: size.height,
    };"""

# We need to do a regex replace or just targeted replace for the specific block
content = content.replace(target, replacement_measure)

with open('src-tauri/src/provider/geometry.rs', 'w') as f:
    f.write(content)
print("Updated geometry.rs")
