use serde::Serialize;
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Default)]
struct ResultChunkStore(Mutex<HashMap<String, PendingResult>>);

struct PendingResult {
    provider: String,
    chunks: Vec<Option<String>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderResult {
    provider: String,
    text: String,
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

#[tauri::command]
async fn open_provider_window(app: tauri::AppHandle, provider: String) -> Result<(), String> {
    let (label, title, url) = match provider.as_str() {
        "chatgpt" => (
            "provider-chatgpt",
            "ChatGPT — Prompter",
            "https://chatgpt.com/",
        ),
        "gemini" => (
            "provider-gemini",
            "Gemini — Prompter",
            "https://gemini.google.com/",
        ),
        _ => return Err("Unknown AI provider.".into()),
    };

    if let Some(window) = app.get_webview_window(label) {
        window.show().map_err(|error| error.to_string())?;
        let _ = window.unminimize();
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }

    let external_url = url
        .parse()
        .map_err(|error| format!("Invalid provider URL: {error}"))?;

    let bridge_app = app.clone();

    WebviewWindowBuilder::new(&app, label, WebviewUrl::External(external_url))
        .title(title)
        .inner_size(1040.0, 760.0)
        .min_inner_size(720.0, 560.0)
        .center()
        .on_navigation(move |url| {
            if url.scheme() == "prompter" {
                handle_provider_bridge_url(&bridge_app, url);
                return false;
            }

            url.scheme() == "https"
        })
        .build()
        .map_err(|error| format!("Could not open {title}: {error}"))?;

    Ok(())
}

#[tauri::command]
fn send_prompt_to_provider(
    app: tauri::AppHandle,
    provider: String,
    prompt: String,
) -> Result<(), String> {
    let label = match provider.as_str() {
        "chatgpt" => "provider-chatgpt",
        "gemini" => "provider-gemini",
        _ => return Err("Unknown AI provider.".into()),
    };

    let window = app
        .get_webview_window(label)
        .ok_or_else(|| "Open the AI account window and sign in first.".to_string())?;

    let prompt_json = serde_json::to_string(&prompt).map_err(|error| error.to_string())?;
    let provider_json = serde_json::to_string(&provider).map_err(|error| error.to_string())?;
    let script = provider_adapter_script(&provider_json, &prompt_json);

    window
        .eval(script)
        .map_err(|error| format!("Could not send the prompt: {error}"))
}

fn provider_adapter_script(provider: &str, prompt: &str) -> String {
    format!(
        r##"
(() => {{
  const provider = {provider};
  const prompt = {prompt};
  const runId = `${{Date.now()}}-${{Math.random().toString(36).slice(2)}}`;
  window.__prompterActiveRun = runId;

  const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const queryFirst = (selectors) => {{
    for (const selector of selectors) {{
      const element = document.querySelector(selector);
      if (element) return element;
    }}
    return null;
  }};

  const signalError = (message) => {{
    const params = new URLSearchParams({{ provider, message }});
    window.location.href = `prompter://error?${{params.toString()}}`;
  }};

  const signalResult = async (text) => {{
    const characters = Array.from(text);
    const chunkSize = 700;
    const total = Math.max(1, Math.ceil(characters.length / chunkSize));
    for (let index = 0; index < total; index += 1) {{
      const data = characters.slice(index * chunkSize, (index + 1) * chunkSize).join("");
      const params = new URLSearchParams({{
        provider,
        session: runId,
        index: String(index),
        total: String(total),
        data,
      }});
      window.location.href = `prompter://result?${{params.toString()}}`;
      await pause(90);
    }}
  }};

  const config = provider === "chatgpt"
    ? {{
        editors: ["#prompt-textarea", "div[contenteditable='true'][data-virtualkeyboard]", "textarea"],
        sendButtons: ["button[data-testid='send-button']", "button[aria-label='Send prompt']", "button[aria-label*='Send']"],
        responses: ["[data-message-author-role='assistant']"],
        stopButtons: ["button[data-testid='stop-button']", "button[aria-label*='Stop']"],
      }}
    : {{
        editors: ["rich-textarea .ql-editor[contenteditable='true']", ".ql-editor[contenteditable='true']", "div[contenteditable='true']", "textarea"],
        sendButtons: ["button[aria-label*='Send message']", "button[aria-label*='Send']"],
        responses: ["model-response .model-response-text", "model-response", ".model-response-text", "[data-test-id='model-response']"],
        stopButtons: ["button[aria-label*='Stop response']", "button[aria-label*='Stop']"],
      }};

  const responseElements = () => Array.from(document.querySelectorAll(config.responses.join(",")));
  const initialResponseCount = responseElements().length;
  const editor = queryFirst(config.editors);

  if (!editor) {{
    signalError("The prompt box was not found. Sign in to the AI account, then try again.");
    return;
  }}

  editor.focus();
  if (editor instanceof HTMLTextAreaElement || editor instanceof HTMLInputElement) {{
    const prototype = editor instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
    setter?.call(editor, prompt);
  }} else {{
    editor.innerHTML = "";
    const paragraph = document.createElement("p");
    paragraph.textContent = prompt;
    editor.appendChild(paragraph);
  }}

  editor.dispatchEvent(new InputEvent("input", {{
    bubbles: true,
    inputType: "insertText",
    data: prompt,
  }}));
  editor.dispatchEvent(new Event("change", {{ bubbles: true }}));

  void (async () => {{
    await pause(500);
    if (window.__prompterActiveRun !== runId) return;

    const sendButton = queryFirst(config.sendButtons);
    if (!sendButton) {{
      signalError("Prompter filled the text, but could not find the Send button.");
      return;
    }}
    sendButton.click();

    const startedAt = Date.now();
    let lastText = "";
    let stableChecks = 0;

    const timer = setInterval(() => {{
      if (window.__prompterActiveRun !== runId) {{
        clearInterval(timer);
        return;
      }}

      const responses = responseElements();
      const newest = responses.at(-1);
      const text = newest?.innerText?.trim() || newest?.textContent?.trim() || "";
      const hasNewResponse = responses.length > initialResponseCount || (text && text !== lastText);
      const isGenerating = Boolean(queryFirst(config.stopButtons));

      if (hasNewResponse && text && text === lastText && !isGenerating) stableChecks += 1;
      else stableChecks = 0;
      lastText = text;

      if (stableChecks >= 3) {{
        clearInterval(timer);
        void signalResult(text);
      }} else if (Date.now() - startedAt > 150000) {{
        clearInterval(timer);
        signalError("The AI response took too long. Check the provider window and try again.");
      }}
    }}, 700);
  }})();
}})();
"##
    )
}

fn handle_provider_bridge_url(app: &tauri::AppHandle, url: &tauri::Url) {
    let event_kind = url.host_str().unwrap_or_default();
    let values: HashMap<String, String> = url.query_pairs().into_owned().collect();
    let provider = values.get("provider").cloned().unwrap_or_default();

    if event_kind == "error" {
        let message = values
            .get("message")
            .cloned()
            .unwrap_or_else(|| "The provider connection failed.".into());
        let _ = app.emit("prompter://provider-error", message);
        return;
    }

    if event_kind != "result" {
        return;
    }

    let Some(session) = values.get("session").cloned() else {
        return;
    };
    let Some(index) = values
        .get("index")
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return;
    };
    let Some(total) = values
        .get("total")
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return;
    };
    if total == 0 || total > 512 || index >= total {
        return;
    }
    let data = values.get("data").cloned().unwrap_or_default();

    let completed = {
        let store = app.state::<ResultChunkStore>();
        let Ok(mut pending) = store.0.lock() else {
            return;
        };
        let result = pending
            .entry(session.clone())
            .or_insert_with(|| PendingResult {
                provider: provider.clone(),
                chunks: vec![None; total],
            });

        if result.chunks.len() != total {
            pending.remove(&session);
            return;
        }

        result.chunks[index] = Some(data);
        if result.chunks.iter().all(Option::is_some) {
            pending.remove(&session).map(|finished| ProviderResult {
                provider: finished.provider,
                text: finished.chunks.into_iter().flatten().collect(),
            })
        } else {
            None
        }
    };

    if let Some(payload) = completed {
        let _ = app.emit("prompter://rewrite-result", payload);
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
        .manage(ResultChunkStore::default())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            compose_prompt,
            open_provider_window,
            send_prompt_to_provider
        ]);

    #[cfg(desktop)]
    let builder = builder.plugin(shortcut_plugin);

    builder
        .run(tauri::generate_context!())
        .expect("error while running Prompter");
}

#[cfg(test)]
mod tests {
    use super::compose_prompt;

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
}
