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
    request_states: Mutex<HashMap<Provider, ProviderRequestState>>,
}

enum ProviderRequestState {
    Pending(String),
    MustClose,
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
        let mut states = self
            .request_states
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?;
        if matches!(states.get(&provider), Some(ProviderRequestState::MustClose)) {
            return Err(operation_failed(
                "The embedded provider page could not be closed safely. Reopen the provider before placing another prompt.",
            ));
        }
        states.insert(
            provider,
            ProviderRequestState::Pending(request_id.to_string()),
        );
        Ok(())
    }

    pub(super) fn complete_request(
        &self,
        provider: Provider,
        request_id: &str,
    ) -> Result<bool, ProviderCommandError> {
        let mut states = self
            .request_states
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?;
        let is_current = states
            .get(&provider)
            .is_some_and(|state| {
                matches!(state, ProviderRequestState::Pending(current) if current == request_id)
            });
        if is_current {
            states.remove(&provider);
        }
        Ok(is_current)
    }

    /// Marks any in-flight fill as requiring page closure. The marker remains
    /// until a close succeeds, so a failed close cannot turn a live routine
    /// into an untracked, merely hidden page.
    fn mark_provider_for_close(&self, provider: Provider) -> bool {
        self.request_states
            .lock()
            .map(|mut states| match states.get(&provider) {
                Some(ProviderRequestState::Pending(_)) => {
                    states.insert(provider, ProviderRequestState::MustClose);
                    true
                }
                Some(ProviderRequestState::MustClose) => true,
                None => false,
            })
            // A poisoned manager is treated as unsafe so callers close the
            // page instead of merely hiding it.
            .unwrap_or(true)
    }

    /// Marks an eval failure for closure only when it still belongs to the
    /// current request. An older command must not cancel a newer fill.
    fn mark_request_for_close(&self, provider: Provider, request_id: &str) -> bool {
        self.request_states
            .lock()
            .map(|mut states| match states.get(&provider) {
                Some(ProviderRequestState::Pending(current)) if current == request_id => {
                    states.insert(provider, ProviderRequestState::MustClose);
                    true
                }
                Some(ProviderRequestState::MustClose) => true,
                _ => false,
            })
            .unwrap_or(true)
    }

    fn must_close(&self, provider: Provider) -> bool {
        self.request_states
            .lock()
            .map(|states| matches!(states.get(&provider), Some(ProviderRequestState::MustClose)))
            .unwrap_or(true)
    }

    /// Clears only the close marker. A concurrently registered newer request
    /// is deliberately preserved.
    fn confirm_closed(&self, provider: Provider) {
        if let Ok(mut states) = self.request_states.lock() {
            if matches!(states.get(&provider), Some(ProviderRequestState::MustClose)) {
                states.remove(&provider);
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
    expected_host: &'static str,
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
        let must_close = lifecycle.mark_provider_for_close(inactive_provider);
        if let Some(webview) = app.get_webview(inactive_provider.config().webview_label) {
            if must_close {
                // EvaluateScript is fire-and-forget on WKWebView, so a
                // JavaScript cancellation flag cannot prove that a hidden page
                // stopped. Close only panes with an in-flight fill.
                webview.close().map_err(|close_error| {
                    operation_failed(format!(
                        "Could not close the active provider fill: {close_error}"
                    ))
                })?;
                lifecycle.confirm_closed(inactive_provider);
            } else {
                webview.hide().map_err(|hide_error| {
                    operation_failed(format!(
                        "Could not hide the inactive embedded provider: {hide_error}"
                    ))
                })?;
            }
        } else if must_close {
            // With no page, there is no JavaScript routine left to contain.
            lifecycle.confirm_closed(inactive_provider);
        }
    }

    if lifecycle.must_close(provider) {
        if let Some(webview) = app.get_webview(config.webview_label) {
            webview.close().map_err(|close_error| {
                operation_failed(format!(
                    "Could not close the unsafe provider page: {close_error}"
                ))
            })?;
        }
        lifecycle.confirm_closed(provider);
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

            if url.as_str() == "about:blank" {
                // An accepted navigation invalidates the old correlation. Keep
                // a close requirement instead of simply forgetting it: hash
                // navigation can preserve the JavaScript context.
                bridge_app
                    .state::<ProviderLifecycle>()
                    .mark_provider_for_close(provider);
                return true;
            }

            if provider.accepts_navigation_url(url) {
                bridge_app
                    .state::<ProviderLifecycle>()
                    .mark_provider_for_close(provider);
                return true;
            }

            warn!(
                target: "prompter::provider",
                "event=embedded_navigation_blocked scheme={} host={}",
                url.scheme(),
                url.host_str().unwrap_or("none")
            );
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
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    visible: bool,
) -> Result<(), ProviderCommandError> {
    for candidate in Provider::ALL {
        let selected = visible && candidate == provider;
        let must_close = if selected {
            lifecycle.must_close(candidate)
        } else {
            lifecycle.mark_provider_for_close(candidate)
        };
        if let Some(webview) = app.get_webview(candidate.config().webview_label) {
            if must_close {
                webview.close().map_err(|error| {
                    operation_failed(format!("Could not close the active provider fill: {error}"))
                })?;
                lifecycle.confirm_closed(candidate);
            } else if selected {
                webview.show().map_err(|error| {
                    operation_failed(format!("Could not update the embedded browser: {error}"))
                })?;
            } else {
                webview.hide().map_err(|error| {
                    operation_failed(format!("Could not hide the embedded browser: {error}"))
                })?;
            }
        } else if must_close {
            lifecycle.confirm_closed(candidate);
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
    if let Err(eval_error) = webview.eval(script) {
        if lifecycle.mark_request_for_close(provider, &request_id) {
            if let Err(close_error) = webview.close() {
                return Err(operation_failed(format!(
                    "Could not place the prompt in {} ({eval_error}) or safely close its page ({close_error}).",
                    config.display_name
                )));
            }
            lifecycle.confirm_closed(provider);
        }
        return Err(operation_failed(format!(
            "Could not place the prompt in {}: {eval_error}",
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
        expected_host: config.expected_fill_host,
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
    if url.scheme() != "https" || url.port_or_known_default() != Some(443) {
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
    use super::{provider_fill_script, Provider, ProviderLifecycle};

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

    #[test]
    fn cancellation_requires_a_confirmed_close_before_reuse() {
        let lifecycle = ProviderLifecycle::default();

        assert!(!lifecycle.mark_provider_for_close(Provider::Chatgpt));
        lifecycle
            .register_request(Provider::Chatgpt, "request-1")
            .unwrap();
        assert!(lifecycle.mark_provider_for_close(Provider::Chatgpt));
        assert!(lifecycle.mark_provider_for_close(Provider::Chatgpt));
        assert!(lifecycle
            .register_request(Provider::Chatgpt, "request-2")
            .is_err());

        lifecycle.confirm_closed(Provider::Chatgpt);
        lifecycle
            .register_request(Provider::Chatgpt, "request-2")
            .unwrap();
    }

    #[test]
    fn an_old_eval_failure_cannot_cancel_a_newer_request() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .register_request(Provider::Chatgpt, "request-1")
            .unwrap();
        lifecycle
            .register_request(Provider::Chatgpt, "request-2")
            .unwrap();

        assert!(!lifecycle.mark_request_for_close(Provider::Chatgpt, "request-1"));
        assert!(lifecycle
            .complete_request(Provider::Chatgpt, "request-2")
            .unwrap());
    }

    #[test]
    fn navigation_style_invalidation_requires_close_and_rejects_stale_completion() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .register_request(Provider::Chatgpt, "request-1")
            .unwrap();
        assert!(lifecycle.mark_provider_for_close(Provider::Chatgpt));
        assert!(!lifecycle
            .complete_request(Provider::Chatgpt, "request-1")
            .unwrap());
        assert!(lifecycle.must_close(Provider::Chatgpt));
    }
}
