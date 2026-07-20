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

/// Each address-bar-less pane receives only its own provider and observed
/// sign-in hosts. These lists intentionally remain separate so one provider
/// never inherits another provider's credential origins.
const CHATGPT_NAVIGATION_HOSTS: &[&str] = &[
    "chatgpt.com",
    "auth.openai.com",
    "accounts.google.com",
    "appleid.apple.com",
    "login.microsoftonline.com",
    "login.live.com",
];
const GEMINI_NAVIGATION_HOSTS: &[&str] = &["gemini.google.com", "accounts.google.com"];

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

    pub(crate) fn accepts_fill_url(self, url: &Url) -> bool {
        url.scheme() == "https"
            && url.port_or_known_default() == Some(443)
            && url.host_str() == Some(self.config().expected_fill_host)
    }

    fn navigation_hosts(self) -> &'static [&'static str] {
        match self {
            Self::Chatgpt => CHATGPT_NAVIGATION_HOSTS,
            Self::Gemini => GEMINI_NAVIGATION_HOSTS,
        }
    }

    pub(crate) fn accepts_navigation_url(self, url: &Url) -> bool {
        if url.scheme() != "https" || url.port_or_known_default() != Some(443) {
            return false;
        }
        let Some(host) = url.host_str() else {
            return false;
        };

        self.navigation_hosts().contains(&host)
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
    pub(crate) expected_fill_host: &'static str,
    pub(crate) editor_selectors: &'static [&'static str],
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

    #[test]
    fn navigation_policy_limits_the_pane_to_provider_and_auth_hosts() {
        let allowed = [
            (Provider::Chatgpt, "https://chatgpt.com/c/example"),
            (Provider::Chatgpt, "https://auth.openai.com/authorize"),
            (Provider::Chatgpt, "https://accounts.google.com/signin"),
            (Provider::Chatgpt, "https://appleid.apple.com/auth"),
            (Provider::Gemini, "https://gemini.google.com/app"),
            (Provider::Gemini, "https://accounts.google.com/signin"),
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
        for (provider, url) in denied {
            assert!(
                !provider.accepts_navigation_url(&Url::parse(url).unwrap()),
                "{url} must not be allowed in the {provider:?} pane"
            );
        }
    }
}
