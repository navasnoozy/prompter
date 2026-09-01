import sys

with open('src-tauri/src/platform/macos.rs', 'r') as f:
    content = f.read()

target = """                let diff = window_frame.size.height - content_frame.size.height;
                return if diff > 0.0 { diff as f64 } else { 0.0 };"""

repl = """                let diff = window_frame.size.height - content_frame.size.height;
                log::info!(target: "prompter::provider", "event=diagnostic window_height={:.1} content_height={:.1} diff={:.1}", window_frame.size.height, content_frame.size.height, diff);
                return if diff > 0.0 { diff as f64 } else { 0.0 };"""

content = content.replace(target, repl)

with open('src-tauri/src/platform/macos.rs', 'w') as f:
    f.write(content)
