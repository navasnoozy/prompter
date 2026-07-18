use serde::Deserialize;
use std::time::Duration;
use tauri::{
    webview::{NewWindowResponse, WebviewBuilder},
    Emitter, LogicalPosition, LogicalSize, Manager, Rect, WebviewUrl,
};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[tauri::command]
fn compose_prompt(instruction: String, text: String) -> Result<String, String> {
    let instruction = instruction.trim();
    let text = text.trim();

    if instruction.is_empty() {
        return Err("Choose an instruction first.".into());
    }

    if text.is_empty() {
        return Err("Add some text to rewrite first.".into());
    }

    Ok(format!(
        "{instruction}\n\nText to rewrite:\n---\n{text}\n---\n\nReturn only the rewritten text unless the instruction asks for something else."
    ))
}

fn provider_details(provider: &str) -> Result<(&'static str, &'static str, &'static str), String> {
    match provider {
        "chatgpt" => Ok(("provider-chatgpt", "ChatGPT", "https://chatgpt.com/")),
        "gemini" => Ok(("provider-gemini", "Gemini", "https://gemini.google.com/")),
        _ => Err("Unknown AI provider.".into()),
    }
}

fn logical_rect(bounds: ProviderBounds) -> Result<Rect, String> {
    if !bounds.x.is_finite()
        || !bounds.y.is_finite()
        || !bounds.width.is_finite()
        || !bounds.height.is_finite()
        || bounds.width < 240.0
        || bounds.height < 240.0
    {
        return Err("The embedded browser area is not ready yet.".into());
    }

    #[cfg(target_os = "macos")]
    let y = bounds.y.max(0.0) + 32.0;
    #[cfg(not(target_os = "macos"))]
    let y = bounds.y.max(0.0);

    Ok(Rect {
        position: LogicalPosition::new(bounds.x.max(0.0), y).into(),
        size: LogicalSize::new(bounds.width, bounds.height).into(),
    })
}

#[tauri::command]
async fn show_provider_webview(
    app: tauri::AppHandle,
    provider: String,
    bounds: ProviderBounds,
) -> Result<(), String> {
    let (label, _, url) = provider_details(&provider)?;
    let rect = logical_rect(bounds)?;

    for other_label in ["provider-chatgpt", "provider-gemini"] {
        if other_label != label {
            if let Some(other) = app.get_webview(other_label) {
                let _ = other.hide();
            }
        }
    }

    if let Some(webview) = app.get_webview(label) {
        webview
            .set_bounds(rect)
            .map_err(|error| format!("Could not resize the embedded browser: {error}"))?;
        webview
            .show()
            .map_err(|error| format!("Could not show the embedded browser: {error}"))?;
        return Ok(());
    }

    let window = app
        .get_window("main")
        .ok_or_else(|| "The Prompter window was not found.".to_string())?;
    let external_url = url
        .parse()
        .map_err(|error| format!("Invalid provider URL: {error}"))?;
    let bridge_app = app.clone();
    let popup_app = app.clone();
    let popup_label = label.to_string();

    let builder = WebviewBuilder::new(label, WebviewUrl::External(external_url))
        .focused(false)
        .on_navigation(move |url| {
            if url.scheme() == "prompter" {
                handle_provider_bridge_url(&bridge_app, url);
                return false;
            }

            matches!(url.scheme(), "https" | "about")
        })
        .on_new_window(move |url, _| {
            if url.scheme() == "https" {
                if let Some(webview) = popup_app.get_webview(&popup_label) {
                    let _ = webview.navigate(url);
                }
            }
            NewWindowResponse::Deny
        });

    let rect = logical_rect(bounds)?;
    window
        .add_child(builder, rect.position, rect.size)
        .map_err(|error| format!("Could not embed the provider browser: {error}"))?;

    Ok(())
}

#[tauri::command]
fn resize_provider_webview(
    app: tauri::AppHandle,
    provider: String,
    bounds: ProviderBounds,
) -> Result<(), String> {
    let (label, _, _) = provider_details(&provider)?;
    let Some(webview) = app.get_webview(label) else {
        return Ok(());
    };

    webview
        .set_bounds(logical_rect(bounds)?)
        .map_err(|error| format!("Could not resize the embedded browser: {error}"))
}

#[tauri::command]
fn set_provider_visibility(
    app: tauri::AppHandle,
    provider: String,
    visible: bool,
) -> Result<(), String> {
    let (active_label, _, _) = provider_details(&provider)?;

    for label in ["provider-chatgpt", "provider-gemini"] {
        if let Some(webview) = app.get_webview(label) {
            if visible && label == active_label {
                webview.show()
            } else {
                webview.hide()
            }
            .map_err(|error| format!("Could not update the embedded browser: {error}"))?;
        }
    }

    Ok(())
}

#[tauri::command]
fn fill_provider_prompt(
    app: tauri::AppHandle,
    provider: String,
    prompt: String,
) -> Result<(), String> {
    let (label, name, _) = provider_details(&provider)?;
    let webview = app
        .get_webview(label)
        .ok_or_else(|| format!("The {name} panel is still loading."))?;

    let prompt_json = serde_json::to_string(&prompt).map_err(|error| error.to_string())?;
    let provider_json = serde_json::to_string(&provider).map_err(|error| error.to_string())?;
    let script = provider_fill_script(&provider_json, &prompt_json);

    webview
        .show()
        .and_then(|_| webview.set_focus())
        .map_err(|error| format!("Could not focus {name}: {error}"))?;
    webview
        .eval(script)
        .map_err(|error| format!("Could not place the prompt in {name}: {error}"))
}

fn provider_fill_script(provider: &str, prompt: &str) -> String {
    format!(
        r##"
(() => {{
  const provider = {provider};
  const prompt = {prompt};
  const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  const selectors = provider === "chatgpt"
    ? [
        "#prompt-textarea",
        "div.ProseMirror[contenteditable='true']",
        "div[contenteditable='true'][data-virtualkeyboard]",
        "main div[contenteditable='true']",
        "textarea",
      ]
    : [
        "rich-textarea .ql-editor[contenteditable='true']",
        ".ql-editor[contenteditable='true']",
        "div[contenteditable='true']",
        "textarea",
      ];

  const findEditor = () => {{
    for (const selector of selectors) {{
      const element = document.querySelector(selector);
      if (element) return element;
    }}
    return null;
  }};

  const signal = (kind, message = "") => {{
    const params = new URLSearchParams({{ provider, message }});
    window.location.href = `prompter://${{kind}}?${{params.toString()}}`;
  }};

  void (async () => {{
    const startedAt = Date.now();
    let editor = findEditor();
    while (!editor && Date.now() - startedAt < 8000) {{
      await pause(200);
      editor = findEditor();
    }}

    if (!editor) {{
      signal("error", `The ${{provider === "chatgpt" ? "ChatGPT" : "Gemini"}} input box was not found. Finish signing in, then try again.`);
      return;
    }}

    editor.focus();
    editor.click();

    if (editor instanceof HTMLTextAreaElement || editor instanceof HTMLInputElement) {{
      const prototype = editor instanceof HTMLTextAreaElement
        ? HTMLTextAreaElement.prototype
        : HTMLInputElement.prototype;
      const setter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
      setter?.call(editor, prompt);
      editor.dispatchEvent(new InputEvent("input", {{
        bubbles: true,
        inputType: "insertText",
        data: prompt,
      }}));
    }} else {{
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(editor);
      selection?.removeAllRanges();
      selection?.addRange(range);

      let inserted = false;
      try {{
        inserted = document.execCommand("insertText", false, prompt);
      }} catch {{
        inserted = false;
      }}

      if (!inserted || !(editor.textContent || "").includes(prompt)) {{
        const paragraph = document.createElement("p");
        paragraph.textContent = prompt;
        editor.replaceChildren(paragraph);
        editor.dispatchEvent(new InputEvent("input", {{
          bubbles: true,
          inputType: "insertText",
          data: prompt,
        }}));
      }}
    }}

    editor.dispatchEvent(new Event("change", {{ bubbles: true }}));
    editor.focus();
    signal("filled");
  }})();
}})();
"##
    )
}

fn handle_provider_bridge_url(app: &tauri::AppHandle, url: &tauri::Url) {
    let event_kind = url.host_str().unwrap_or_default();
    let values: std::collections::HashMap<String, String> =
        url.query_pairs().into_owned().collect();
    let provider = values.get("provider").cloned().unwrap_or_default();

    match event_kind {
        "filled" => {
            let _ = app.emit("prompter://prompt-filled", provider);
        }
        "error" => {
            let message = values
                .get("message")
                .cloned()
                .unwrap_or_else(|| "The provider connection failed.".into());
            let _ = app.emit("prompter://provider-error", message);
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn copy_current_selection() -> Result<(), String> {
    use core_graphics::{
        event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode},
        event_source::{CGEventSource, CGEventSourceStateID},
    };

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

#[cfg(not(target_os = "macos"))]
fn copy_current_selection() -> Result<(), String> {
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(desktop)]
    let shortcut_plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_shortcut("CommandOrControl+Shift+P")
        .expect("failed to configure the Prompter shortcut")
        .with_handler(|app, _shortcut, event| {
            use tauri_plugin_global_shortcut::ShortcutState;

            if event.state != ShortcutState::Pressed {
                return;
            }

            let app_handle = app.clone();
            std::thread::spawn(move || {
                let previous_clipboard = app_handle.clipboard().read_text().ok();
                let _ = copy_current_selection();
                std::thread::sleep(Duration::from_millis(180));
                let clipboard_text = app_handle.clipboard().read_text().unwrap_or_default();

                if let Some(previous) = previous_clipboard {
                    let _ = app_handle.clipboard().write_text(previous);
                }

                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }

                let _ = app_handle.emit("prompter://clipboard-captured", clipboard_text);
            });
        })
        .build();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            compose_prompt,
            show_provider_webview,
            resize_provider_webview,
            set_provider_visibility,
            fill_provider_prompt
        ]);

    #[cfg(desktop)]
    let builder = builder.plugin(shortcut_plugin);

    builder
        .run(tauri::generate_context!())
        .expect("error while running Prompter");
}

#[cfg(test)]
mod tests {
    use super::{compose_prompt, provider_fill_script};

    #[test]
    fn compose_prompt_keeps_instruction_and_source_text() {
        let prompt = compose_prompt(
            "Make this clearer".into(),
            "  This sentence needs help.  ".into(),
        )
        .expect("prompt should be composed");

        assert!(prompt.starts_with("Make this clearer"));
        assert!(prompt.contains("This sentence needs help."));
        assert!(prompt.ends_with(
            "Return only the rewritten text unless the instruction asks for something else."
        ));
    }

    #[test]
    fn compose_prompt_rejects_empty_text() {
        assert!(compose_prompt("Make this clearer".into(), "  ".into()).is_err());
    }

    #[test]
    fn provider_adapter_fills_without_sending() {
        let script = provider_fill_script("\"chatgpt\"", "\"Prepared prompt\"");

        assert!(script.contains("#prompt-textarea"));
        assert!(script.contains("Prepared prompt"));
        assert!(!script.contains("send-button"));
        assert!(!script.contains("Send prompt"));
    }
}
