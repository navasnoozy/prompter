use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tauri::Url;

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

/// Candidate selectors for the provider's own "New chat" control. Only
/// automation-stable hooks belong here — test ids, ARIA labels, and routes.
/// Utility classes and hashed asset names churn on every deploy, so matching
/// them would age worse than matching nothing. Anything found here is still
/// scored against `new_chat_labels` before it is clicked, so a selector that
/// starts matching the wrong element degrades to "not found" rather than to a
/// wrong click.
const CHATGPT_NEW_CHAT_SELECTORS: &[&str] = &[
    "[data-testid='create-new-chat-button']",
    "a[data-testid='create-new-chat-button']",
    "[data-sidebar-item='true'][href='/']",
    "nav a[href='/']",
];

const GEMINI_NEW_CHAT_SELECTORS: &[&str] = &[
    "[data-test-id='new-chat-button']",
    "[data-test-id='expandable-new-chat-button']",
    "expanded-button[data-test-id='new-chat-button']",
    "side-nav-action-button[data-test-id='new-chat-button']",
    "button[aria-label='New chat']",
    "[role='button'][aria-label='New chat']",
];

/// Accessible names that identify the control when every selector above has
/// aged out. Compared case-insensitively against trimmed text and ARIA labels.
const CHATGPT_NEW_CHAT_LABELS: &[&str] = &["new chat"];
const GEMINI_NEW_CHAT_LABELS: &[&str] = &["new chat", "new conversation"];

/// URL paths that mean "no conversation is open". Used to confirm a reset
/// actually happened, and to recognize a pane that is already on a blank chat.
/// A leading `/u/<digits>` account segment is stripped before comparison, so a
/// multi-account Google session matches the same way a single-account one does.
const CHATGPT_FRESH_CHAT_PATHS: &[&str] = &["/"];
const GEMINI_FRESH_CHAT_PATHS: &[&str] = &["/app", "/app/new", "/"];

/// Hosts whose new windows belong inside the pane rather than in the user's
/// browser: the provider itself and its observed sign-in origins, which must
/// land in the same cookie store as the page that opened them. The lists stay
/// separate so one provider never inherits another's credential origins.
///
/// This does not restrict what a pane may load — see `on_navigation` in
/// `commands.rs` for why no such restriction is possible.
const CHATGPT_IN_PANE_WINDOW_HOSTS: &[&str] = &[
    "chatgpt.com",
    "auth.openai.com",
    "accounts.google.com",
    "appleid.apple.com",
    "login.microsoftonline.com",
    "login.live.com",
];
const GEMINI_IN_PANE_WINDOW_HOSTS: &[&str] = &["gemini.google.com", "accounts.google.com"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Provider {
    Chatgpt,
    Gemini,
}

impl Provider {
    pub(crate) const ALL: [Self; 2] = [Self::Chatgpt, Self::Gemini];

    pub(crate) fn config(self) -> ProviderConfig {
        match self {
            Self::Chatgpt => ProviderConfig {
                id: "chatgpt",
                webview_label: "provider-chatgpt",
                display_name: "ChatGPT",
                url: "https://chatgpt.com/",
                new_chat_url: "https://chatgpt.com/",
                expected_fill_host: "chatgpt.com",
                editor_selectors: CHATGPT_EDITOR_SELECTORS,
                new_chat_selectors: CHATGPT_NEW_CHAT_SELECTORS,
                new_chat_labels: CHATGPT_NEW_CHAT_LABELS,
                fresh_chat_paths: CHATGPT_FRESH_CHAT_PATHS,
            },
            Self::Gemini => ProviderConfig {
                id: "gemini",
                webview_label: "provider-gemini",
                display_name: "Gemini",
                url: "https://gemini.google.com/",
                new_chat_url: "https://gemini.google.com/app/new",
                expected_fill_host: "gemini.google.com",
                editor_selectors: GEMINI_EDITOR_SELECTORS,
                new_chat_selectors: GEMINI_NEW_CHAT_SELECTORS,
                new_chat_labels: GEMINI_NEW_CHAT_LABELS,
                fresh_chat_paths: GEMINI_FRESH_CHAT_PATHS,
            },
        }
    }

    pub(crate) fn accepts_fill_url(self, url: &Url) -> bool {
        url.scheme() == "https"
            && url.port_or_known_default() == Some(443)
            && url.host_str() == Some(self.config().expected_fill_host)
    }

    fn in_pane_window_hosts(self) -> &'static [&'static str] {
        match self {
            Self::Chatgpt => CHATGPT_IN_PANE_WINDOW_HOSTS,
            Self::Gemini => GEMINI_IN_PANE_WINDOW_HOSTS,
        }
    }

    pub(crate) fn keeps_new_window_in_pane(self, url: &Url) -> bool {
        if url.scheme() != "https" || url.port_or_known_default() != Some(443) {
            return false;
        }
        let Some(host) = url.host_str() else {
            return false;
        };

        self.in_pane_window_hosts().contains(&host)
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
pub(crate) struct ProviderConfig {
    pub(crate) id: &'static str,
    pub(crate) webview_label: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) url: &'static str,
    /// Where a reset navigates when the in-page control cannot be used. Always
    /// on `expected_fill_host`, so a reset can never move the pane off the
    /// origin placement requires.
    pub(crate) new_chat_url: &'static str,
    pub(crate) expected_fill_host: &'static str,
    pub(crate) editor_selectors: &'static [&'static str],
    pub(crate) new_chat_selectors: &'static [&'static str],
    pub(crate) new_chat_labels: &'static [&'static str],
    pub(crate) fresh_chat_paths: &'static [&'static str],
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::Provider;
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
            assert!(!config.new_chat_selectors.is_empty());
            assert!(!config.new_chat_labels.is_empty());
            assert!(!config.fresh_chat_paths.is_empty());
            assert!(config.url.starts_with("https://"));
            assert!(config
                .new_chat_labels
                .iter()
                .all(|label| label.trim() == *label && label.to_lowercase() == *label));
            assert!(config
                .fresh_chat_paths
                .iter()
                .all(|path| path.starts_with('/')));
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

    /// A reset must never be able to move a pane somewhere placement will then
    /// refuse to fill, so the built-in reset target has to satisfy the same
    /// origin policy as placement itself.
    #[test]
    fn the_built_in_new_chat_target_satisfies_the_fill_policy() {
        for provider in Provider::ALL {
            let url = Url::parse(provider.config().new_chat_url)
                .expect("the built-in new chat URL must parse");
            assert!(
                provider.accepts_fill_url(&url),
                "{provider:?} must reset onto its own fill host"
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
        assert!(!Provider::Gemini
            .accepts_fill_url(&Url::parse("https://gemini.google.com:8443/app").unwrap()));
    }

    /// Decides which user-opened new windows stay in the pane (provider and
    /// sign-in hosts, so the session lands in the pane's cookie store) and
    /// which are handed to the default browser. It does not gate what the
    /// pane may load — see the `on_navigation` hook for why it cannot.
    #[test]
    fn new_window_policy_keeps_provider_and_auth_hosts_in_the_pane() {
        let kept_in_pane = [
            (Provider::Chatgpt, "https://chatgpt.com/c/example"),
            (Provider::Chatgpt, "https://auth.openai.com/authorize"),
            (Provider::Chatgpt, "https://accounts.google.com/signin"),
            (Provider::Chatgpt, "https://appleid.apple.com/auth"),
            (Provider::Gemini, "https://gemini.google.com/app"),
            (Provider::Gemini, "https://accounts.google.com/signin"),
        ];
        for (provider, url) in kept_in_pane {
            assert!(
                provider.keeps_new_window_in_pane(&Url::parse(url).unwrap()),
                "a {provider:?} new window for {url} should stay in the pane"
            );
        }

        let handed_to_browser = [
            (Provider::Chatgpt, "https://example.com/"),
            (Provider::Chatgpt, "https://evil-chatgpt.com/"),
            (Provider::Chatgpt, "https://chatgpt.com.evil.com/"),
            (Provider::Chatgpt, "https://community.openai.com/"),
            (Provider::Chatgpt, "https://accounts.youtube.com/accounts"),
            (Provider::Chatgpt, "https://chatgpt.com:8443/"),
            (Provider::Chatgpt, "http://chatgpt.com/"),
            (Provider::Gemini, "https://chatgpt.com/"),
            (Provider::Gemini, "https://sites.google.com/view/untrusted"),
            (Provider::Gemini, "https://docs.google.com/document/example"),
            (Provider::Gemini, "https://mail.google.com/"),
            (Provider::Gemini, "https://accounts.youtube.com/accounts"),
            (Provider::Gemini, "https://appleid.apple.com/auth"),
            (
                Provider::Gemini,
                "https://login.microsoftonline.com/common/oauth2",
            ),
            (Provider::Gemini, "https://notgoogle.com/"),
        ];
        for (provider, url) in handed_to_browser {
            assert!(
                !provider.keeps_new_window_in_pane(&Url::parse(url).unwrap()),
                "a {provider:?} new window for {url} must go to the default browser"
            );
        }
    }
}
