use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use log::{debug, warn};
use serde::Serialize;
use tauri::{
    webview::{NewWindowResponse, WebviewBuilder},
    AppHandle, Manager, Rect, State, Url, WebviewUrl,
};
use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard};

use super::{
    bridge::{self, is_valid_request_id},
    config::Provider,
    error::{ProviderCommandError, ProviderErrorCode},
    geometry::{self, ProviderBounds},
    navigation,
};
use crate::{platform, prompt::PromptInput, MAIN_WINDOW_LABEL};

const FILL_PROMPT_SOURCE: &str = include_str!("fill_prompt.js");

fn operation_failed(message: impl Into<String>) -> ProviderCommandError {
    ProviderCommandError::new(ProviderErrorCode::WebviewOperationFailed, message)
}

/// Serializes provider pane creation and tracks the small amount of state that
/// outlives a single command: which navigation generation a pane is on, whether
/// its main frame is loading, and which prompt placement is still waiting for a
/// `prompter://` bridge response.
#[derive(Default)]
pub(crate) struct ProviderLifecycle {
    creation_lock: AsyncMutex<()>,
    operation_states: Mutex<HashMap<Provider, ProviderOperationState>>,
}

#[derive(Default)]
struct ProviderOperationState {
    /// Identifier of the placement awaiting its bridge response, if any.
    pending_request: Option<String>,
    navigation_generation: Option<u32>,
    navigation_loading: bool,
}

impl ProviderLifecycle {
    pub(super) async fn lock_creation(&self) -> AsyncMutexGuard<'_, ()> {
        self.creation_lock.lock().await
    }

    fn states(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<Provider, ProviderOperationState>>, ProviderCommandError>
    {
        self.operation_states
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))
    }

    /// Claims the provider for a placement. A newer placement always replaces an
    /// older one: the bridge correlates responses by request id, so an abandoned
    /// request simply stops matching and can never wedge the next placement.
    fn register_request(
        &self,
        provider: Provider,
        generation: u32,
        request_id: &str,
    ) -> Result<(), ProviderCommandError> {
        if !is_valid_request_id(request_id) {
            return Err(ProviderCommandError::new(
                ProviderErrorCode::InvalidRequest,
                "The prompt request identifier is invalid.",
            ));
        }
        let mut states = self.states()?;
        let state = states.entry(provider).or_default();
        if state.navigation_generation != Some(generation) {
            return Err(ProviderCommandError::new(
                ProviderErrorCode::NavigationBlocked,
                "The provider browser is still initializing. Reopen it and try again.",
            ));
        }
        if state.navigation_loading {
            return Err(ProviderCommandError::new(
                ProviderErrorCode::NavigationBlocked,
                "Wait for the provider page to finish loading before placing the prompt.",
            ));
        }
        state.pending_request = Some(request_id.to_string());
        Ok(())
    }

    /// Accepts a bridge response only while it still matches the placement in
    /// flight. A response from a replaced page or a superseded request is stale.
    pub(super) fn complete_request(
        &self,
        provider: Provider,
        request_id: &str,
    ) -> Result<bool, ProviderCommandError> {
        let mut states = self.states()?;
        let state = states.entry(provider).or_default();
        if state.pending_request.as_deref() != Some(request_id) {
            return Ok(false);
        }
        state.pending_request = None;
        Ok(true)
    }

    /// Drops a placement that never reached the page, so the next one is not
    /// mistaken for a duplicate. An older request must not clear a newer one.
    fn cancel_request(&self, provider: Provider, request_id: &str) {
        if let Ok(mut states) = self.operation_states.lock() {
            if let Some(state) = states.get_mut(&provider) {
                if state.pending_request.as_deref() == Some(request_id) {
                    state.pending_request = None;
                }
            }
        }
    }

    pub(super) fn begin_navigation_generation(
        &self,
        provider: Provider,
        generation: u32,
    ) -> Result<(), ProviderCommandError> {
        let mut states = self.states()?;
        let state = states.entry(provider).or_default();
        state.navigation_generation = Some(generation);
        // A newly-created WKWebView begins its initial load immediately. KVO
        // replaces this conservative value with the native one.
        state.navigation_loading = true;
        // Any placement waiting on the replaced page can never be answered.
        state.pending_request = None;
        Ok(())
    }

    pub(super) fn invalidate_navigation_generation(&self, provider: Provider, generation: u32) {
        if let Ok(mut states) = self.operation_states.lock() {
            let state = states.entry(provider).or_default();
            if state.navigation_generation == Some(generation) {
                state.navigation_generation = None;
                state.navigation_loading = false;
                state.pending_request = None;
            }
        }
    }

    /// WebKit is the sole source of truth for main-frame loading.
    pub(super) fn record_navigation_observation(
        &self,
        provider: Provider,
        generation: u32,
        is_loading: bool,
    ) {
        if let Ok(mut states) = self.operation_states.lock() {
            let state = states.entry(provider).or_default();
            if state.navigation_generation == Some(generation) {
                state.navigation_loading = is_loading;
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

/// Tears a provider pane down in the order that keeps state consistent: retire
/// the navigation generation so stale frontend actions cannot target it, remove
/// the KVO observer while the WKWebView is still alive, then close.
async fn close_provider_webview(
    app: &AppHandle,
    provider: Provider,
    webview: &tauri::Webview,
    failure_context: &str,
) -> Result<(), ProviderCommandError> {
    navigation::invalidate_current_provider_navigation(app, provider)?;
    platform::detach_provider_navigation_observer(app, provider.config().webview_label)
        .await
        .map_err(operation_failed)?;
    webview
        .close()
        .map_err(|error| operation_failed(format!("{failure_context}: {error}")))
}

#[tauri::command]
pub(crate) async fn show_provider_webview(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    bounds: ProviderBounds,
) -> Result<(), ProviderCommandError> {
    let _creation_guard = lifecycle.lock_creation().await;
    let config = provider.config();
    let window = app.get_window(MAIN_WINDOW_LABEL).ok_or_else(|| {
        ProviderCommandError::new(
            ProviderErrorCode::WindowMissing,
            "The Prompter window was not found.",
        )
    })?;
    let rect = Rect::from(bounds.validate(geometry::content_offset_y(&window))?);

    for inactive in Provider::ALL
        .into_iter()
        .filter(|candidate| *candidate != provider)
    {
        if let Some(webview) = app.get_webview(inactive.config().webview_label) {
            webview.hide().map_err(|error| {
                operation_failed(format!(
                    "Could not hide the inactive embedded provider: {error}"
                ))
            })?;
        }
    }

    if let Some(webview) = app.get_webview(config.webview_label) {
        if navigation::current_provider_navigation_generation(&app, provider)?.is_some() {
            platform::apply_provider_corner_radius(&webview).map_err(operation_failed)?;
            webview.set_bounds(rect).map_err(|error| {
                operation_failed(format!("Could not resize the embedded browser: {error}"))
            })?;
            webview.show().map_err(|error| {
                operation_failed(format!("Could not show the embedded browser: {error}"))
            })?;
            return Ok(());
        }

        // A pane without an active navigation generation is unobserved: its
        // navigation controls and loading state would be dead. Recreate it.
        close_provider_webview(
            &app,
            provider,
            &webview,
            "Could not close the unobserved provider page",
        )
        .await?;
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

            // WebKit's policy hook fires for every frame, so this also sees
            // iframe loads: sign-in widgets, and the about:blank documents
            // provider pages create for themselves.
            if url.as_str() == "about:blank" || provider.accepts_navigation_url(url) {
                return true;
            }

            // Links the pane can't display (external references in AI
            // responses, Terms of Service, documentation) are handed to the
            // user's default browser instead of being silently swallowed.
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

    if let Err(error) = platform::apply_provider_corner_radius(&webview) {
        discard_failed_provider_webview(&app, provider, &webview).await;
        return Err(operation_failed(error));
    }
    if let Err(error) = navigation::register_provider_navigation(&app, &webview, provider).await {
        discard_failed_provider_webview(&app, provider, &webview).await;
        return Err(error);
    }

    Ok(())
}

/// Removes a pane that failed its own setup. The setup error is what the caller
/// needs to see, so a cleanup failure is logged rather than replacing it.
async fn discard_failed_provider_webview(
    app: &AppHandle,
    provider: Provider,
    webview: &tauri::Webview,
) {
    if let Err(error) = close_provider_webview(
        app,
        provider,
        webview,
        "Could not close the provider after setup failed",
    )
    .await
    {
        warn!(
            target: "prompter::provider",
            "event=provider_setup_cleanup_failed reason={}",
            error.message
        );
    }
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
pub(crate) async fn set_provider_visibility(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    visible: bool,
) -> Result<(), ProviderCommandError> {
    let _creation_guard = lifecycle.lock_creation().await;
    for candidate in Provider::ALL {
        let Some(webview) = app.get_webview(candidate.config().webview_label) else {
            continue;
        };
        let should_show = visible && candidate == provider;
        let result = if should_show {
            webview.show()
        } else {
            webview.hide()
        };
        result.map_err(|error| {
            operation_failed(format!("Could not update the embedded browser: {error}"))
        })?;
    }

    Ok(())
}

/// Composes the prompt natively and places it into the provider's editor in a
/// single IPC round trip. Never submits; the user presses Send.
#[tauri::command]
pub(crate) async fn place_prompt(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    composition: PromptInput,
    request_id: String,
) -> Result<(), ProviderCommandError> {
    let _creation_guard = lifecycle.lock_creation().await;
    let prompt = composition.compose()?;
    let config = provider.config();
    let current_generation = navigation::current_provider_navigation_generation(&app, provider)?
        .ok_or_else(|| {
            ProviderCommandError::new(
                ProviderErrorCode::NavigationBlocked,
                format!(
                    "The {} browser is still initializing. Reopen it and try again.",
                    config.display_name
                ),
            )
        })?;
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

    lifecycle.register_request(provider, current_generation, &request_id)?;
    if let Err(eval_error) = webview.eval(script) {
        lifecycle.cancel_request(provider, &request_id);
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
    // Allow http:// and https:// to open in the user's default browser.
    // Block javascript:, data:, file:, and other dangerous schemes that
    // could execute code or access local files outside the sandbox.
    if !matches!(url.scheme(), "https" | "http") {
        // Routine: provider pages load about:/blob: subframes on every visit.
        debug!(
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
    use super::{provider_fill_script, Provider, ProviderErrorCode, ProviderLifecycle};

    /// Puts a provider in the state a loaded, idle pane has: generation 7 with
    /// its initial load finished.
    fn ready_lifecycle() -> ProviderLifecycle {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_observation(Provider::Chatgpt, 7, false);
        lifecycle
    }

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
    fn placement_requires_the_current_navigation_generation() {
        let lifecycle = ready_lifecycle();

        let stale = lifecycle
            .register_request(Provider::Chatgpt, 6, "request-1")
            .unwrap_err();
        assert_eq!(stale.code, ProviderErrorCode::NavigationBlocked);

        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
    }

    #[test]
    fn placement_waits_while_the_main_frame_is_loading() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();

        let loading = lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap_err();
        assert_eq!(loading.code, ProviderErrorCode::NavigationBlocked);

        lifecycle.record_navigation_observation(Provider::Chatgpt, 7, false);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
    }

    #[test]
    fn a_stale_observation_cannot_unblock_or_block_placement() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();

        // A trailing callback from a replaced pane must not clear the load.
        lifecycle.record_navigation_observation(Provider::Chatgpt, 6, false);
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .is_err());

        lifecycle.record_navigation_observation(Provider::Chatgpt, 7, false);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
        lifecycle.record_navigation_observation(Provider::Chatgpt, 6, true);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-2")
            .unwrap();
    }

    #[test]
    fn a_newer_placement_replaces_an_abandoned_one() {
        let lifecycle = ready_lifecycle();
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-2")
            .unwrap();

        // The superseded request can no longer answer for the current one.
        assert!(!lifecycle
            .complete_request(Provider::Chatgpt, "request-1")
            .unwrap());
        assert!(lifecycle
            .complete_request(Provider::Chatgpt, "request-2")
            .unwrap());
        assert!(!lifecycle
            .complete_request(Provider::Chatgpt, "request-2")
            .unwrap());
    }

    #[test]
    fn a_cancelled_placement_never_clears_a_newer_one() {
        let lifecycle = ready_lifecycle();
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-2")
            .unwrap();

        lifecycle.cancel_request(Provider::Chatgpt, "request-1");
        assert!(lifecycle
            .complete_request(Provider::Chatgpt, "request-2")
            .unwrap());

        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-3")
            .unwrap();
        lifecycle.cancel_request(Provider::Chatgpt, "request-3");
        assert!(!lifecycle
            .complete_request(Provider::Chatgpt, "request-3")
            .unwrap());
    }

    #[test]
    fn replacing_or_retiring_a_pane_drops_its_pending_placement() {
        let lifecycle = ready_lifecycle();
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();

        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 8)
            .unwrap();
        assert!(!lifecycle
            .complete_request(Provider::Chatgpt, "request-1")
            .unwrap());

        lifecycle.record_navigation_observation(Provider::Chatgpt, 8, false);
        lifecycle
            .register_request(Provider::Chatgpt, 8, "request-2")
            .unwrap();

        // A stale invalidation leaves the live generation alone.
        lifecycle.invalidate_navigation_generation(Provider::Chatgpt, 7);
        assert!(lifecycle
            .complete_request(Provider::Chatgpt, "request-2")
            .unwrap());

        lifecycle.invalidate_navigation_generation(Provider::Chatgpt, 8);
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 8, "request-3")
            .is_err());
    }
}
