use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use tauri::{
    webview::{NewWindowResponse, WebviewBuilder},
    AppHandle, Manager, State, Url, WebviewUrl,
};
use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard};

use super::{
    bridge::{self, is_valid_request_id},
    config::Provider,
    error::{ProviderCommandError, ProviderErrorCode},
    geometry::ProviderBounds,
    navigation,
    new_chat::{self, NewChatMatcher},
    placement,
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

    /// Drops whatever placement is in flight, whichever request it is. Used
    /// when the page underneath it is about to be replaced deliberately.
    fn abandon_pending_request(&self, provider: Provider) {
        if let Ok(mut states) = self.operation_states.lock() {
            if let Some(state) = states.get_mut(&provider) {
                state.pending_request = None;
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
    /// Present only when this placement should reset the conversation first.
    new_chat: Option<NewChatScriptInput<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NewChatScriptInput<'a> {
    selectors: &'static [&'static str],
    labels: &'static [&'static str],
    fresh_paths: &'static [&'static str],
    matcher: Option<&'a NewChatMatcher>,
}

/// Asks a placement to open a blank conversation before filling the prompt.
/// The optional matcher describes the provider's "New chat" control for pages
/// whose built-in selectors have aged out.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NewChatRequest {
    #[serde(default)]
    matcher: Option<NewChatMatcher>,
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
    info!(target: "prompter::provider", "event=trace step=show_entered provider={provider:?}");
    let _creation_guard = lifecycle.lock_creation().await;
    info!(target: "prompter::provider", "event=trace step=show_guarded");
    let config = provider.config();
    let window = app.get_window(MAIN_WINDOW_LABEL).ok_or_else(|| {
        ProviderCommandError::new(
            ProviderErrorCode::WindowMissing,
            "The Prompter window was not found.",
        )
    })?;
    let rect = placement::adopt(&app, provider, bounds)?;

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
            platform::pin_provider_webview_edges(&webview).map_err(operation_failed)?;
            placement::apply(&app, provider, &webview, rect)?;
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

            // Everything else loads in the pane, exactly as it would in a
            // browser tab.
            //
            // This hook cannot police navigation, and must not try. WebKit
            // consults it for *every* WKNavigationAction, and wry forwards
            // only the URL string (wry 0.55.1 wkwebview/navigation.rs) — it
            // drops `targetFrame.isMainFrame` and `navigationType`. So a page
            // loading its own iframe (Google's app-switcher widget, sign-in
            // frames, embedded viewers) is indistinguishable here from the
            // user clicking a link. Judging one as the other is what sent
            // Gemini's `ogs.google.com` widget to the default browser on
            // every launch.
            //
            // Placement safety does not rest on this hook and never did:
            // `fill_prompt.js` re-verifies protocol/host/port in-page at
            // every step, placement requires the current navigation
            // generation, and KVO `is_loading` holds it while the main frame
            // loads. User-initiated new windows are still handed off below,
            // where macOS guarantees the gesture we lack here.
            true
        })
        // Unlike `on_navigation`, this hook carries the signal that one
        // lacks. It is backed by `WKUIDelegate` `createWebViewWithConfiguration:`,
        // which fires only for `window.open` and `target="_blank"` — and
        // WebKit suppresses those without a user gesture, because wry leaves
        // `javaScriptCanOpenWindowsAutomatically` at its default of NO. So
        // reaching here means the user clicked something that asked for a new
        // window, which is exactly when handing off is correct.
        .on_new_window(move |url, _| {
            // Sign-in flows open this way. Keep them in the pane so the
            // session lands in the same cookie store the provider page uses.
            if provider.keeps_new_window_in_pane(&url) {
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

    info!(target: "prompter::provider", "event=trace step=add_child_begin");
    let webview = window
        .add_child(builder, rect.position, rect.size)
        .map_err(|error| {
            operation_failed(format!("Could not embed the provider browser: {error}"))
        })?;

    if let Err(error) = platform::apply_provider_corner_radius(&webview)
        .and_then(|()| platform::pin_provider_webview_edges(&webview))
    {
        discard_failed_provider_webview(&app, provider, &webview).await;
        return Err(operation_failed(error));
    }
    // `add_child` seeds the pane's frame through a different code path than
    // every later placement takes. Re-applying the same rect here makes
    // `set_bounds` the single authority for where a pane sits, and confirms the
    // platform agreed before the pane is ever shown.
    if let Err(error) = placement::apply(&app, provider, &webview, rect) {
        discard_failed_provider_webview(&app, provider, &webview).await;
        return Err(error);
    }
    info!(target: "prompter::provider", "event=trace step=add_child_done");
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
    let rect = placement::adopt(&app, provider, bounds)?;
    placement::apply(&app, provider, &webview, rect)?;
    Ok(())
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
    new_chat: Option<NewChatRequest>,
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

    // Sanitize before the pane is touched: a rejected matcher must fail the
    // command outright rather than leave a half-reset conversation behind.
    let sanitized_matcher = new_chat
        .as_ref()
        .and_then(|request| request.matcher.as_ref())
        .map(NewChatMatcher::sanitized)
        .transpose()?;
    let script = provider_fill_script(
        provider,
        &request_id,
        &prompt,
        new_chat.is_some().then_some(NewChatScriptInput {
            selectors: config.new_chat_selectors,
            labels: config.new_chat_labels,
            fresh_paths: config.fresh_chat_paths,
            matcher: sanitized_matcher.as_ref(),
        }),
    )?;
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

/// Resets a pane to a blank conversation by navigating to the provider's
/// new-chat address.
///
/// This is the mechanism that always works and never wins on speed: it
/// reloads the whole single-page app. Callers reach for it when the in-page
/// control cannot be found, and users can reach for it directly. The pane
/// keeps its cookie store, so the session survives the reload.
#[tauri::command]
pub(crate) async fn open_provider_new_chat(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    url: Option<String>,
) -> Result<(), ProviderCommandError> {
    let _creation_guard = lifecycle.lock_creation().await;
    let config = provider.config();
    let target = new_chat::resolve_new_chat_url(provider, url.as_deref())?;

    if navigation::current_provider_navigation_generation(&app, provider)?.is_none() {
        return Err(ProviderCommandError::new(
            ProviderErrorCode::NavigationBlocked,
            format!(
                "The {} browser is still initializing. Reopen it and try again.",
                config.display_name
            ),
        ));
    }
    let webview = app.get_webview(config.webview_label).ok_or_else(|| {
        ProviderCommandError::new(
            ProviderErrorCode::WebviewMissing,
            format!("The {} panel is still loading.", config.display_name),
        )
    })?;

    // The document about to be replaced may still be running a fill script.
    // Its `prompter://` answer can never arrive, so the placement is dropped
    // now rather than left to time out.
    lifecycle.abandon_pending_request(provider);

    webview.navigate(target).map_err(|error| {
        operation_failed(format!(
            "Could not open a new {} chat: {error}",
            config.display_name
        ))
    })?;

    info!(
        target: "prompter::provider",
        "event=new_chat_navigated provider={} custom_url={}",
        config.id,
        url.is_some()
    );
    Ok(())
}

fn provider_fill_script(
    provider: Provider,
    request_id: &str,
    prompt: &str,
    new_chat: Option<NewChatScriptInput<'_>>,
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
        new_chat,
    };
    let input_json = serde_json::to_string(&input).map_err(|error| {
        operation_failed(format!("Could not prepare the provider prompt: {error}"))
    })?;

    Ok(format!("void ({FILL_PROMPT_SOURCE})({input_json});"))
}

/// Hands a user-requested new window to the default browser. Only the scheme
/// and host are ever logged: paths and query strings routinely carry session
/// tokens, so the full URL never reaches the log file.
fn open_url_externally(app: &AppHandle, url: &Url) {
    // Allow http:// and https:// to open in the user's default browser.
    // Block javascript:, data:, file:, and other dangerous schemes that
    // could execute code or access local files outside the sandbox.
    if !matches!(url.scheme(), "https" | "http") {
        debug!(
            target: "prompter::provider",
            "event=external_open_skipped scheme={}",
            url.scheme()
        );
        return;
    }

    // Handing control to another application is a visible, user-affecting
    // hand-off, so it is recorded at info. Its absence is diagnostic too: a
    // browser window that opens with no line here did not come from Prompter.
    info!(
        target: "prompter::provider",
        "event=external_open host={}",
        url.host_str().unwrap_or("unknown")
    );

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
    use super::{
        provider_fill_script, NewChatMatcher, NewChatScriptInput, Provider, ProviderErrorCode,
        ProviderLifecycle,
    };

    fn new_chat_input(provider: Provider, matcher: Option<&NewChatMatcher>) -> NewChatScriptInput {
        let config = provider.config();
        NewChatScriptInput {
            selectors: config.new_chat_selectors,
            labels: config.new_chat_labels,
            fresh_paths: config.fresh_chat_paths,
            matcher,
        }
    }

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
            None,
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
        assert!(provider_fill_script(Provider::Chatgpt, "", "prompt", None).is_err());
        assert!(provider_fill_script(Provider::Chatgpt, "bad\nid", "prompt", None).is_err());
    }

    /// Placement without a reset must not carry any new-chat instructions into
    /// the page: an accidental `newChat` payload would clear a conversation the
    /// user meant to continue.
    #[test]
    fn fill_script_omits_the_reset_step_unless_it_was_requested() {
        let script = provider_fill_script(Provider::Chatgpt, "request-1", "prompt", None).unwrap();
        assert!(script.contains("\"newChat\":null"));
        assert!(!script.contains("create-new-chat-button"));
    }

    #[test]
    fn fill_script_carries_reset_signals_as_data_not_code() {
        let matcher = NewChatMatcher {
            test_id: Some("create-new-chat-button".into()),
            label: Some("New chat".into()),
            href: Some("/".into()),
            attributes: Vec::new(),
        };
        let script = provider_fill_script(
            Provider::Chatgpt,
            "request-1",
            "prompt",
            Some(new_chat_input(Provider::Chatgpt, Some(&matcher))),
        )
        .unwrap();

        // The matcher travels inside the JSON argument, so a value containing
        // markup or quotes can never become script text.
        assert!(script.contains("\"testId\":\"create-new-chat-button\""));
        assert!(script.contains("\"freshPaths\":[\"/\"]"));
        assert!(script.contains("\"labels\":[\"new chat\"]"));

        // A matcher that tries to break out of its own string literal must
        // survive as an inert value. The script is `void (SOURCE)(JSON);`, and
        // JSON never contains `)(`, so the last one is the argument boundary.
        let hostile_label = "\");alert(1);//";
        let hostile = NewChatMatcher {
            label: Some(hostile_label.into()),
            ..NewChatMatcher::default()
        };
        let escaped = provider_fill_script(
            Provider::Gemini,
            "request-2",
            "prompt",
            Some(new_chat_input(Provider::Gemini, Some(&hostile))),
        )
        .unwrap();

        let argument = escaped
            .rsplit_once(")(")
            .expect("the script passes its input as one argument")
            .1
            .strip_suffix(");")
            .expect("the script call is terminated");
        let parsed: serde_json::Value =
            serde_json::from_str(argument).expect("the argument must be valid JSON");
        assert_eq!(parsed["newChat"]["matcher"]["label"], hostile_label);
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
