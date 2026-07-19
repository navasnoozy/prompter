use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use log::warn;
use serde::Serialize;
use tauri::{
    webview::{NewWindowResponse, WebviewBuilder},
    AppHandle, Manager, Rect, State, Url, WebviewUrl,
};

use super::{
    bridge::{self, is_valid_request_id},
    config::Provider,
    error::{ProviderCommandError, ProviderErrorCode},
    geometry::{self, ProviderBounds},
};
use crate::{platform, prompt::PromptInput, MAIN_WINDOW_LABEL};

const FILL_PROMPT_SOURCE: &str = include_str!("fill_prompt.js");

fn operation_failed(message: impl Into<String>) -> ProviderCommandError {
    ProviderCommandError::new(ProviderErrorCode::WebviewOperationFailed, message)
}

#[derive(Default)]
pub(crate) struct ProviderLifecycle {
    creation_lock: Mutex<()>,
    pending_requests: Mutex<HashMap<Provider, String>>,
}

impl ProviderLifecycle {
    fn lock_creation(&self) -> Result<MutexGuard<'_, ()>, ProviderCommandError> {
        self.creation_lock
            .lock()
            .map_err(|_| operation_failed("The provider browser manager is unavailable."))
    }

    fn register_request(
        &self,
        provider: Provider,
        request_id: &str,
    ) -> Result<(), ProviderCommandError> {
        if !is_valid_request_id(request_id) {
            return Err(ProviderCommandError::new(
                ProviderErrorCode::InvalidRequest,
                "The prompt request identifier is invalid.",
            ));
        }
        self.pending_requests
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?
            .insert(provider, request_id.to_string());
        Ok(())
    }

    pub(super) fn complete_request(
        &self,
        provider: Provider,
        request_id: &str,
    ) -> Result<bool, ProviderCommandError> {
        let mut requests = self
            .pending_requests
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?;
        let is_current = requests
            .get(&provider)
            .is_some_and(|current| current == request_id);
        if is_current {
            requests.remove(&provider);
        }
        Ok(is_current)
    }

    fn clear_request(&self, provider: Provider, request_id: &str) {
        if let Ok(mut requests) = self.pending_requests.lock() {
            if requests
                .get(&provider)
                .is_some_and(|current| current == request_id)
            {
                requests.remove(&provider);
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FillScriptInput<'a> {
    provider: &'static str,
    request_id: &'a str,
    display_name: &'static str,
    selectors: &'static [&'static str],
    prompt: &'a str,
}

#[tauri::command]
pub(crate) fn show_provider_webview(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    bounds: ProviderBounds,
) -> Result<(), ProviderCommandError> {
    let _creation_guard = lifecycle.lock_creation()?;
    let config = provider.config();
    let window = app.get_window(MAIN_WINDOW_LABEL).ok_or_else(|| {
        ProviderCommandError::new(
            ProviderErrorCode::WindowMissing,
            "The Prompter window was not found.",
        )
    })?;
    let rect = Rect::from(bounds.validate(geometry::content_offset_y(&window))?);

    for inactive_provider in Provider::ALL
        .into_iter()
        .filter(|candidate| *candidate != provider)
    {
        if let Some(webview) = app.get_webview(inactive_provider.config().webview_label) {
            if let Err(hide_error) = webview.hide() {
                warn!(
                    target: "prompter::provider",
                    "event=inactive_provider_hide_failed reason={hide_error}"
                );
            }
        }
    }

    if let Some(webview) = app.get_webview(config.webview_label) {
        platform::apply_provider_corner_radius(&webview).map_err(operation_failed)?;
        webview.set_bounds(rect).map_err(|error| {
            operation_failed(format!("Could not resize the embedded browser: {error}"))
        })?;
        webview.show().map_err(|error| {
            operation_failed(format!("Could not show the embedded browser: {error}"))
        })?;
        return Ok(());
    }

    let external_url = config
        .url
        .parse()
        .map_err(|error| operation_failed(format!("Invalid provider URL: {error}")))?;
    let bridge_app = app.clone();
    let popup_app = app.clone();
    let popup_label = config.webview_label.to_string();

    let builder = WebviewBuilder::new(config.webview_label, WebviewUrl::External(external_url))
        .focused(false)
        .on_navigation(move |url| {
            if url.scheme() == "prompter" {
                bridge::handle_provider_bridge_url(&bridge_app, provider, url);
                return false;
            }

            if url.scheme() == "about" {
                return true;
            }

            if provider.accepts_navigation_url(url) {
                return true;
            }

            open_url_externally(&bridge_app, url);
            false
        })
        .on_new_window(move |url, _| {
            if provider.accepts_navigation_url(&url) {
                if let Some(webview) = popup_app.get_webview(&popup_label) {
                    if let Err(error) = webview.navigate(url) {
                        warn!(
                            target: "prompter::provider",
                            "event=popup_navigation_failed reason={error}"
                        );
                    }
                }
            } else {
                open_url_externally(&popup_app, &url);
            }
            NewWindowResponse::Deny
        });

    let webview = window
        .add_child(builder, rect.position, rect.size)
        .map_err(|error| {
            operation_failed(format!("Could not embed the provider browser: {error}"))
        })?;
    platform::apply_provider_corner_radius(&webview).map_err(operation_failed)?;

    Ok(())
}

#[tauri::command]
pub(crate) fn resize_provider_webview(
    app: AppHandle,
    provider: Provider,
    bounds: ProviderBounds,
) -> Result<(), ProviderCommandError> {
    let Some(webview) = app.get_webview(provider.config().webview_label) else {
        return Ok(());
    };
    let window = app.get_window(MAIN_WINDOW_LABEL).ok_or_else(|| {
        ProviderCommandError::new(
            ProviderErrorCode::WindowMissing,
            "The Prompter window was not found.",
        )
    })?;

    webview
        .set_bounds(Rect::from(
            bounds.validate(geometry::content_offset_y(&window))?,
        ))
        .map_err(|error| {
            operation_failed(format!("Could not resize the embedded browser: {error}"))
        })
}

#[tauri::command]
pub(crate) fn set_provider_visibility(
    app: AppHandle,
    provider: Provider,
    visible: bool,
) -> Result<(), ProviderCommandError> {
    for candidate in Provider::ALL {
        if let Some(webview) = app.get_webview(candidate.config().webview_label) {
            if visible && candidate == provider {
                webview.show()
            } else {
                webview.hide()
            }
            .map_err(|error| {
                operation_failed(format!("Could not update the embedded browser: {error}"))
            })?;
        }
    }

    Ok(())
}

/// Composes the prompt natively and places it into the provider's editor in a
/// single IPC round trip. Never submits; the user presses Send.
#[tauri::command]
pub(crate) fn place_prompt(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    composition: PromptInput,
    request_id: String,
) -> Result<(), ProviderCommandError> {
    let prompt = composition.compose()?;
    let config = provider.config();
    let webview = app.get_webview(config.webview_label).ok_or_else(|| {
        ProviderCommandError::new(
            ProviderErrorCode::WebviewMissing,
            format!("The {} panel is still loading.", config.display_name),
        )
    })?;
    let current_url = webview.url().map_err(|error| {
        operation_failed(format!(
            "Could not read the {} page: {error}",
            config.display_name
        ))
    })?;

    if !provider.accepts_fill_url(&current_url) {
        return Err(ProviderCommandError::new(
            ProviderErrorCode::WrongHost,
            format!(
                "{} is showing a sign-in or external page. Finish signing in and return to {} before placing the prompt.",
                config.display_name, config.expected_fill_host
            ),
        ));
    }

    let script = provider_fill_script(provider, &request_id, &prompt)?;
    webview
        .show()
        .and_then(|_| webview.set_focus())
        .map_err(|error| {
            operation_failed(format!("Could not focus {}: {error}", config.display_name))
        })?;

    lifecycle.register_request(provider, &request_id)?;
    if let Err(error) = webview.eval(script) {
        lifecycle.clear_request(provider, &request_id);
        return Err(operation_failed(format!(
            "Could not place the prompt in {}: {error}",
            config.display_name
        )));
    }

    Ok(())
}

fn provider_fill_script(
    provider: Provider,
    request_id: &str,
    prompt: &str,
) -> Result<String, ProviderCommandError> {
    if !is_valid_request_id(request_id) {
        return Err(ProviderCommandError::new(
            ProviderErrorCode::InvalidRequest,
            "The prompt request identifier is invalid.",
        ));
    }
    let config = provider.config();
    let input = FillScriptInput {
        provider: config.id,
        request_id,
        display_name: config.display_name,
        selectors: config.editor_selectors,
        prompt,
    };
    let input_json = serde_json::to_string(&input).map_err(|error| {
        operation_failed(format!("Could not prepare the provider prompt: {error}"))
    })?;

    Ok(format!("void ({FILL_PROMPT_SOURCE})({input_json});"))
}

/// Hands a URL the embedded pane may not display to the user's default
/// browser. Content is never logged; only failure reasons are.
fn open_url_externally(app: &AppHandle, url: &Url) {
    if !matches!(url.scheme(), "https" | "http") {
        warn!(
            target: "prompter::provider",
            "event=external_navigation_blocked scheme={}",
            url.scheme()
        );
        return;
    }

    let target = url.to_string();
    let dispatched = app.run_on_main_thread(move || {
        if let Err(open_error) = platform::open_in_default_browser(&target) {
            warn!(
                target: "prompter::provider",
                "event=external_open_failed reason={open_error}"
            );
        }
    });
    if let Err(dispatch_error) = dispatched {
        warn!(
            target: "prompter::provider",
            "event=external_open_dispatch_failed reason={dispatch_error}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{provider_fill_script, Provider};

    #[test]
    fn fill_script_escapes_input_and_never_submits() {
        let script = provider_fill_script(
            Provider::Chatgpt,
            "request-1",
            "A quote: \"hello\"\nA slash: \\",
        )
        .unwrap();

        assert!(script.contains("#prompt-textarea"));
        assert!(script.contains("request-1"));
        assert!(script.contains("\\\"hello\\\""));
        assert!(script.contains("\\nA slash: \\\\"));
        assert!(!script.contains("requestSubmit"));
        assert!(!script.contains(".submit("));
        assert!(!script.contains("KeyboardEvent"));
        assert!(!script.contains("send-button"));
    }

    #[test]
    fn fill_script_rejects_invalid_request_ids() {
        assert!(provider_fill_script(Provider::Chatgpt, "", "prompt").is_err());
        assert!(provider_fill_script(Provider::Chatgpt, "bad\nid", "prompt").is_err());
    }
}
