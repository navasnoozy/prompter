use log::{error, warn};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Mutex, MutexGuard},
};
use tauri::{
    webview::{NewWindowResponse, WebviewBuilder},
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Rect, State, Url, WebviewUrl,
};

use crate::{platform, prompt::MAX_PROMPT_BYTES, MAIN_WINDOW_LABEL};

const MIN_PROVIDER_SIZE: f64 = 240.0;
const MAX_PROVIDER_SIZE: f64 = 20_000.0;
const MAX_REQUEST_ID_LENGTH: usize = 128;
const MAX_BRIDGE_MESSAGE_LENGTH: usize = 600;
const FILL_PROMPT_SOURCE: &str = include_str!("fill_prompt.js");

/// Sign-in providers the embedded panes may navigate to in addition to their
/// own domain. Everything else opens in the user's default browser so an
/// address-bar-less pane can never present an arbitrary site.
const SHARED_AUTH_DOMAINS: &[&str] = &[
    "accounts.google.com",
    "accounts.youtube.com",
    "appleid.apple.com",
    "login.microsoftonline.com",
    "login.live.com",
];

const CHATGPT_NAVIGATION_DOMAINS: &[&str] = &["chatgpt.com", "openai.com"];
const GEMINI_NAVIGATION_DOMAINS: &[&str] = &["google.com"];

const CHATGPT_EDITOR_SELECTORS: &[&str] = &[
    "#prompt-textarea",
    "div.ProseMirror[contenteditable='true']",
    "div[contenteditable='true'][data-virtualkeyboard]",
    "main div[contenteditable='true']",
    "textarea",
];

const GEMINI_EDITOR_SELECTORS: &[&str] = &[
    "rich-textarea .ql-editor[contenteditable='true']",
    ".ql-editor[contenteditable='true']",
    "div[contenteditable='true']",
    "textarea",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Provider {
    Chatgpt,
    Gemini,
}

impl Provider {
    pub(crate) const ALL: [Self; 2] = [Self::Chatgpt, Self::Gemini];

    fn config(self) -> ProviderConfig {
        match self {
            Self::Chatgpt => ProviderConfig {
                id: "chatgpt",
                webview_label: "provider-chatgpt",
                display_name: "ChatGPT",
                url: "https://chatgpt.com/",
                expected_fill_host: "chatgpt.com",
                editor_selectors: CHATGPT_EDITOR_SELECTORS,
            },
            Self::Gemini => ProviderConfig {
                id: "gemini",
                webview_label: "provider-gemini",
                display_name: "Gemini",
                url: "https://gemini.google.com/",
                expected_fill_host: "gemini.google.com",
                editor_selectors: GEMINI_EDITOR_SELECTORS,
            },
        }
    }

    fn accepts_fill_url(self, url: &Url) -> bool {
        url.scheme() == "https" && url.host_str() == Some(self.config().expected_fill_host)
    }

    fn navigation_domains(self) -> &'static [&'static str] {
        match self {
            Self::Chatgpt => CHATGPT_NAVIGATION_DOMAINS,
            Self::Gemini => GEMINI_NAVIGATION_DOMAINS,
        }
    }

    fn accepts_navigation_url(self, url: &Url) -> bool {
        if url.scheme() != "https" {
            return false;
        }
        let Some(host) = url.host_str() else {
            return false;
        };

        self.navigation_domains()
            .iter()
            .chain(SHARED_AUTH_DOMAINS)
            .any(|domain| {
                host == *domain
                    || (host.len() > domain.len() + 1
                        && host.ends_with(domain)
                        && host.as_bytes()[host.len() - domain.len() - 1] == b'.')
            })
    }
}

impl FromStr for Provider {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "chatgpt" => Ok(Self::Chatgpt),
            "gemini" => Ok(Self::Gemini),
            _ => Err("Unknown AI provider.".into()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ProviderConfig {
    id: &'static str,
    webview_label: &'static str,
    display_name: &'static str,
    url: &'static str,
    expected_fill_host: &'static str,
    editor_selectors: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl ProviderBounds {
    fn validate(self) -> Result<ValidatedBounds, String> {
        if !self.x.is_finite()
            || !self.y.is_finite()
            || !self.width.is_finite()
            || !self.height.is_finite()
            || self.width < MIN_PROVIDER_SIZE
            || self.height < MIN_PROVIDER_SIZE
            || self.width > MAX_PROVIDER_SIZE
            || self.height > MAX_PROVIDER_SIZE
        {
            return Err("The embedded browser area is not ready yet.".into());
        }

        Ok(ValidatedBounds {
            x: self.x.max(0.0),
            y: platform::provider_y_position(self.y.max(0.0)),
            width: self.width,
            height: self.height,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ValidatedBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<ValidatedBounds> for Rect {
    fn from(bounds: ValidatedBounds) -> Self {
        Self {
            position: LogicalPosition::new(bounds.x, bounds.y).into(),
            size: LogicalSize::new(bounds.width, bounds.height).into(),
        }
    }
}

#[derive(Default)]
pub(crate) struct ProviderLifecycle {
    creation_lock: Mutex<()>,
    pending_requests: Mutex<HashMap<Provider, String>>,
}

impl ProviderLifecycle {
    fn lock_creation(&self) -> Result<MutexGuard<'_, ()>, String> {
        self.creation_lock
            .lock()
            .map_err(|_| "The provider browser manager is unavailable.".into())
    }

    fn register_request(&self, provider: Provider, request_id: &str) -> Result<(), String> {
        validate_request_id(request_id)?;
        self.pending_requests
            .lock()
            .map_err(|_| "The provider request manager is unavailable.".to_string())?
            .insert(provider, request_id.to_string());
        Ok(())
    }

    fn complete_request(&self, provider: Provider, request_id: &str) -> Result<bool, String> {
        let mut requests = self
            .pending_requests
            .lock()
            .map_err(|_| "The provider request manager is unavailable.".to_string())?;
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderFilledPayload {
    provider: Provider,
    request_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderErrorPayload {
    provider: Provider,
    request_id: String,
    message: String,
}

#[derive(Debug, PartialEq)]
enum BridgeEventKind {
    Filled,
    Error(String),
}

#[derive(Debug, PartialEq)]
struct BridgeEvent {
    provider: Provider,
    request_id: String,
    kind: BridgeEventKind,
}

#[tauri::command]
pub(crate) fn show_provider_webview(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    bounds: ProviderBounds,
) -> Result<(), String> {
    let _creation_guard = lifecycle.lock_creation()?;
    let config = provider.config();
    let rect = Rect::from(bounds.validate()?);

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
        platform::apply_provider_corner_radius(&webview)?;
        webview
            .set_bounds(rect)
            .map_err(|error| format!("Could not resize the embedded browser: {error}"))?;
        webview
            .show()
            .map_err(|error| format!("Could not show the embedded browser: {error}"))?;
        return Ok(());
    }

    let window = app
        .get_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "The Prompter window was not found.".to_string())?;
    let external_url = config
        .url
        .parse()
        .map_err(|error| format!("Invalid provider URL: {error}"))?;
    let bridge_app = app.clone();
    let popup_app = app.clone();
    let popup_label = config.webview_label.to_string();

    let builder = WebviewBuilder::new(config.webview_label, WebviewUrl::External(external_url))
        .focused(false)
        .on_navigation(move |url| {
            if url.scheme() == "prompter" {
                handle_provider_bridge_url(&bridge_app, provider, url);
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
        .map_err(|error| format!("Could not embed the provider browser: {error}"))?;
    platform::apply_provider_corner_radius(&webview)?;

    Ok(())
}

#[tauri::command]
pub(crate) fn resize_provider_webview(
    app: AppHandle,
    provider: Provider,
    bounds: ProviderBounds,
) -> Result<(), String> {
    let Some(webview) = app.get_webview(provider.config().webview_label) else {
        return Ok(());
    };

    webview
        .set_bounds(Rect::from(bounds.validate()?))
        .map_err(|error| format!("Could not resize the embedded browser: {error}"))
}

#[tauri::command]
pub(crate) fn set_provider_visibility(
    app: AppHandle,
    provider: Provider,
    visible: bool,
) -> Result<(), String> {
    for candidate in Provider::ALL {
        if let Some(webview) = app.get_webview(candidate.config().webview_label) {
            if visible && candidate == provider {
                webview.show()
            } else {
                webview.hide()
            }
            .map_err(|error| format!("Could not update the embedded browser: {error}"))?;
        }
    }

    Ok(())
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

#[tauri::command]
pub(crate) fn fill_provider_prompt(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    prompt: String,
    request_id: String,
) -> Result<(), String> {
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err("The prompt is too large to place. Shorten the text and try again.".into());
    }

    let config = provider.config();
    let webview = app
        .get_webview(config.webview_label)
        .ok_or_else(|| format!("The {} panel is still loading.", config.display_name))?;
    let current_url = webview
        .url()
        .map_err(|error| format!("Could not read the {} page: {error}", config.display_name))?;

    if !provider.accepts_fill_url(&current_url) {
        return Err(format!(
            "{} is showing a sign-in or external page. Finish signing in and return to {} before placing the prompt.",
            config.display_name, config.expected_fill_host
        ));
    }

    let script = provider_fill_script(provider, &request_id, &prompt)?;
    webview
        .show()
        .and_then(|_| webview.set_focus())
        .map_err(|error| format!("Could not focus {}: {error}", config.display_name))?;

    lifecycle.register_request(provider, &request_id)?;
    if let Err(error) = webview.eval(script) {
        lifecycle.clear_request(provider, &request_id);
        return Err(format!(
            "Could not place the prompt in {}: {error}",
            config.display_name
        ));
    }

    Ok(())
}

fn provider_fill_script(
    provider: Provider,
    request_id: &str,
    prompt: &str,
) -> Result<String, String> {
    validate_request_id(request_id)?;
    let config = provider.config();
    let input = FillScriptInput {
        provider: config.id,
        request_id,
        display_name: config.display_name,
        selectors: config.editor_selectors,
        prompt,
    };
    let input_json = serde_json::to_string(&input)
        .map_err(|error| format!("Could not prepare the provider prompt: {error}"))?;

    Ok(format!("void ({FILL_PROMPT_SOURCE})({input_json});"))
}

fn validate_request_id(request_id: &str) -> Result<(), String> {
    if request_id.is_empty()
        || request_id.len() > MAX_REQUEST_ID_LENGTH
        || request_id.chars().any(char::is_control)
    {
        return Err("The prompt request identifier is invalid.".into());
    }
    Ok(())
}

fn parse_bridge_event(url: &Url, expected_provider: Provider) -> Result<BridgeEvent, String> {
    if url.scheme() != "prompter" {
        return Err("Unexpected provider bridge scheme.".into());
    }

    let event_name = url
        .host_str()
        .ok_or_else(|| "The provider bridge event is missing.".to_string())?;
    let values: HashMap<String, String> = url.query_pairs().into_owned().collect();
    let provider = values
        .get("provider")
        .ok_or_else(|| "The provider bridge response is missing its provider.".to_string())?
        .parse::<Provider>()?;
    if provider != expected_provider {
        return Err("The provider bridge response does not match the active provider.".into());
    }

    let request_id = values
        .get("requestId")
        .ok_or_else(|| "The provider bridge response is missing its request.".to_string())?;
    validate_request_id(request_id)?;

    let kind = match event_name {
        "filled" => BridgeEventKind::Filled,
        "error" => {
            let message = values
                .get("message")
                .filter(|message| !message.trim().is_empty())
                .map(|message| message.chars().take(MAX_BRIDGE_MESSAGE_LENGTH).collect())
                .unwrap_or_else(|| "The provider connection failed.".into());
            BridgeEventKind::Error(message)
        }
        _ => return Err("Unknown provider bridge event.".into()),
    };

    Ok(BridgeEvent {
        provider,
        request_id: request_id.to_string(),
        kind,
    })
}

fn handle_provider_bridge_url(app: &AppHandle, expected_provider: Provider, url: &Url) {
    let event = match parse_bridge_event(url, expected_provider) {
        Ok(event) => event,
        Err(error) => {
            warn!(
                target: "prompter::provider",
                "event=bridge_response_invalid reason={error}"
            );
            return;
        }
    };

    let lifecycle = app.state::<ProviderLifecycle>();
    match lifecycle.complete_request(event.provider, &event.request_id) {
        Ok(true) => {}
        Ok(false) => {
            warn!(
                target: "prompter::provider",
                "event=bridge_response_stale request_id={}",
                event.request_id
            );
            return;
        }
        Err(error) => {
            error!(
                target: "prompter::provider",
                "event=bridge_response_validation_failed reason={error}"
            );
            return;
        }
    }

    let result = match event.kind {
        BridgeEventKind::Filled => app.emit_to(
            MAIN_WINDOW_LABEL,
            "prompter://prompt-filled",
            ProviderFilledPayload {
                provider: event.provider,
                request_id: event.request_id,
            },
        ),
        BridgeEventKind::Error(message) => app.emit_to(
            MAIN_WINDOW_LABEL,
            "prompter://provider-error",
            ProviderErrorPayload {
                provider: event.provider,
                request_id: event.request_id,
                message,
            },
        ),
    };

    if let Err(error) = result {
        error!(
            target: "prompter::provider",
            "event=bridge_response_delivery_failed reason={error}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_bridge_event, provider_fill_script, BridgeEvent, BridgeEventKind, Provider,
        ProviderBounds, MAX_PROVIDER_SIZE, MIN_PROVIDER_SIZE,
    };
    use std::collections::HashSet;
    use tauri::Url;

    #[test]
    fn provider_deserializes_from_the_frontend_contract() {
        assert_eq!(
            serde_json::from_str::<Provider>("\"chatgpt\"").unwrap(),
            Provider::Chatgpt
        );
        assert_eq!(
            serde_json::from_str::<Provider>("\"gemini\"").unwrap(),
            Provider::Gemini
        );
        assert!(serde_json::from_str::<Provider>("\"other\"").is_err());
    }

    #[test]
    fn provider_configuration_is_unique_and_complete() {
        let mut labels = HashSet::new();
        let mut hosts = HashSet::new();

        for provider in Provider::ALL {
            let config = provider.config();
            assert!(labels.insert(config.webview_label));
            assert!(hosts.insert(config.expected_fill_host));
            assert!(!config.editor_selectors.is_empty());
            assert!(config.url.starts_with("https://"));
        }

        assert!(Provider::Chatgpt
            .config()
            .editor_selectors
            .contains(&"#prompt-textarea"));
        assert!(Provider::Gemini
            .config()
            .editor_selectors
            .contains(&"rich-textarea .ql-editor[contenteditable='true']"));
    }

    #[test]
    fn bounds_reject_non_finite_and_too_small_values() {
        let valid = ProviderBounds {
            x: 0.0,
            y: 0.0,
            width: MIN_PROVIDER_SIZE,
            height: MIN_PROVIDER_SIZE,
        };
        assert!(valid.validate().is_ok());

        for invalid in [
            ProviderBounds {
                x: f64::NAN,
                ..valid
            },
            ProviderBounds {
                y: f64::INFINITY,
                ..valid
            },
            ProviderBounds {
                width: MIN_PROVIDER_SIZE - 1.0,
                ..valid
            },
            ProviderBounds {
                height: 0.0,
                ..valid
            },
            ProviderBounds {
                width: MAX_PROVIDER_SIZE + 1.0,
                ..valid
            },
            ProviderBounds {
                height: f64::MAX,
                ..valid
            },
        ] {
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn navigation_policy_limits_the_pane_to_provider_and_auth_hosts() {
        let allowed = [
            (Provider::Chatgpt, "https://chatgpt.com/c/example"),
            (Provider::Chatgpt, "https://auth.openai.com/authorize"),
            (Provider::Chatgpt, "https://accounts.google.com/signin"),
            (Provider::Chatgpt, "https://appleid.apple.com/auth"),
            (Provider::Gemini, "https://gemini.google.com/app"),
            (Provider::Gemini, "https://accounts.google.com/signin"),
            (Provider::Gemini, "https://accounts.youtube.com/accounts"),
        ];
        for (provider, url) in allowed {
            assert!(
                provider.accepts_navigation_url(&Url::parse(url).unwrap()),
                "{url} should be allowed in the {provider:?} pane"
            );
        }

        let denied = [
            (Provider::Chatgpt, "https://example.com/"),
            (Provider::Chatgpt, "https://evil-chatgpt.com/"),
            (Provider::Chatgpt, "https://chatgpt.com.evil.com/"),
            (Provider::Chatgpt, "http://chatgpt.com/"),
            (Provider::Gemini, "https://chatgpt.com/"),
            (Provider::Gemini, "https://notgoogle.com/"),
        ];
        for (provider, url) in denied {
            assert!(
                !provider.accepts_navigation_url(&Url::parse(url).unwrap()),
                "{url} must not be allowed in the {provider:?} pane"
            );
        }
    }

    #[test]
    fn fill_policy_requires_the_exact_provider_chat_host() {
        assert!(Provider::Chatgpt
            .accepts_fill_url(&Url::parse("https://chatgpt.com/c/example").unwrap()));
        assert!(!Provider::Chatgpt
            .accepts_fill_url(&Url::parse("https://accounts.google.com/").unwrap()));
        assert!(
            !Provider::Chatgpt.accepts_fill_url(&Url::parse("https://evil.chatgpt.com/").unwrap())
        );
        assert!(Provider::Gemini
            .accepts_fill_url(&Url::parse("https://gemini.google.com/app").unwrap()));
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
    fn bridge_validation_checks_provider_request_and_event() {
        let url =
            Url::parse("prompter://filled?provider=chatgpt&requestId=request-1&message=ignored")
                .unwrap();
        assert_eq!(
            parse_bridge_event(&url, Provider::Chatgpt).unwrap(),
            BridgeEvent {
                provider: Provider::Chatgpt,
                request_id: "request-1".into(),
                kind: BridgeEventKind::Filled,
            }
        );
        assert!(parse_bridge_event(&url, Provider::Gemini).is_err());

        let missing_request =
            Url::parse("prompter://filled?provider=chatgpt").expect("URL should parse");
        assert!(parse_bridge_event(&missing_request, Provider::Chatgpt).is_err());

        let unknown =
            Url::parse("prompter://unknown?provider=chatgpt&requestId=request-1").unwrap();
        assert!(parse_bridge_event(&unknown, Provider::Chatgpt).is_err());
    }
}
