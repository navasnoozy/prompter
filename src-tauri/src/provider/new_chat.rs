//! Resetting a provider pane to a blank conversation.
//!
//! Two mechanisms exist, and they fail in different ways, which is why both
//! are kept. Clicking the provider's own "New chat" control is instant and
//! keeps the single-page app mounted, but it depends on that page's DOM.
//! Navigating to a new-chat URL always works but pays a full reload. The
//! frontend chooses between them and falls back; this module owns the two
//! pieces of user-supplied configuration that make either survive a provider
//! redesign — an override URL, and a description of the control to click.
//!
//! Both arrive from the webview and are therefore untrusted. The rules here
//! are about *shape*: bounded sizes, no control characters, attribute names
//! from known-safe families, and a URL that cannot leave the origin placement
//! already requires. Which attributes are worth matching on is a judgement
//! about provider markup rather than about safety, so that filtering lives in
//! the frontend parser where the paste is turned into signals.

use serde::{Deserialize, Serialize};
use tauri::Url;

use super::{
    config::Provider,
    error::{ProviderCommandError, ProviderErrorCode},
};

/// A pasted element carries a handful of short identifiers. These bounds are
/// far above anything real markup needs and far below anything that could
/// bloat the injected script.
const MAX_SIGNAL_BYTES: usize = 256;
const MAX_ATTRIBUTE_NAME_BYTES: usize = 64;
const MAX_ATTRIBUTES: usize = 8;
const MAX_URL_BYTES: usize = 2_048;

/// Attributes outside the `data-`/`aria-` families that identify a control
/// without describing how it currently looks or what state it is in.
const ALLOWED_PLAIN_ATTRIBUTES: [&str; 5] = ["href", "id", "name", "role", "type"];

fn invalid_request(message: impl Into<String>) -> ProviderCommandError {
    ProviderCommandError::new(ProviderErrorCode::InvalidRequest, message)
}

/// One `name="value"` signal extracted from the element the user pasted.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NewChatAttribute {
    pub(crate) name: String,
    pub(crate) value: String,
}

/// A description of the provider's "New chat" control, good enough to find it
/// again after a redesign. Every field is optional because no single one
/// survives forever; the in-page scorer needs any combination that clears its
/// confidence threshold, so losing one signal degrades accuracy instead of
/// breaking the match outright.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NewChatMatcher {
    #[serde(default)]
    pub(crate) test_id: Option<String>,
    #[serde(default)]
    pub(crate) label: Option<String>,
    #[serde(default)]
    pub(crate) href: Option<String>,
    #[serde(default)]
    pub(crate) attributes: Vec<NewChatAttribute>,
}

impl NewChatMatcher {
    /// Returns the matcher in canonical form, or explains why it is unusable.
    ///
    /// A matcher that survives this is still only a *hint*: the page-side
    /// scorer decides whether any element resembles it closely enough to
    /// click, so an honest-but-wrong description costs a fallback, not a
    /// misdirected click.
    pub(crate) fn sanitized(&self) -> Result<Self, ProviderCommandError> {
        let test_id = sanitize_signal(self.test_id.as_deref(), "test id")?;
        let label = sanitize_signal(self.label.as_deref(), "label")?;
        let href = sanitize_signal(self.href.as_deref(), "link target")?;

        if self.attributes.len() > MAX_ATTRIBUTES {
            return Err(invalid_request(
                "The New chat button description carries too many attributes.",
            ));
        }

        let mut attributes: Vec<NewChatAttribute> = Vec::with_capacity(self.attributes.len());
        for attribute in &self.attributes {
            let name = attribute.name.trim().to_ascii_lowercase();
            if !is_allowed_attribute_name(&name) {
                return Err(invalid_request(format!(
                    "The New chat button description contains an unsupported attribute: {name}."
                )));
            }
            let value = sanitize_signal(Some(attribute.value.as_str()), &name)?.unwrap_or_default();
            if attributes
                .iter()
                .any(|existing: &NewChatAttribute| existing.name == name)
            {
                return Err(invalid_request(format!(
                    "The New chat button description repeats the attribute {name}."
                )));
            }
            attributes.push(NewChatAttribute { name, value });
        }

        let sanitized = Self {
            test_id,
            label,
            href,
            attributes,
        };
        if sanitized.is_empty() {
            return Err(invalid_request(
                "The New chat button description has nothing to match on.",
            ));
        }
        Ok(sanitized)
    }

    fn is_empty(&self) -> bool {
        self.test_id.is_none()
            && self.label.is_none()
            && self.href.is_none()
            && self.attributes.is_empty()
    }
}

fn sanitize_signal(
    value: Option<&str>,
    field: &str,
) -> Result<Option<String>, ProviderCommandError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > MAX_SIGNAL_BYTES {
        return Err(invalid_request(format!(
            "The New chat button {field} is too long."
        )));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(invalid_request(format!(
            "The New chat button {field} contains invalid characters."
        )));
    }
    Ok(Some(trimmed.to_string()))
}

/// `data-`/`aria-` attributes are the families providers use for automation
/// and accessibility hooks; the short plain list covers identity attributes.
/// `class` and `style` are deliberately absent — they describe appearance,
/// change on nearly every deploy, and matching them makes an override rot
/// faster than having no override at all.
fn is_allowed_attribute_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_ATTRIBUTE_NAME_BYTES {
        return false;
    }
    if !name.chars().all(|character| {
        character.is_ascii_lowercase() || matches!(character, '-' | '_' | '0'..='9')
    }) {
        return false;
    }
    if name.starts_with("data-") || name.starts_with("aria-") {
        return name.len() > "data-".len();
    }
    ALLOWED_PLAIN_ATTRIBUTES.contains(&name)
}

/// Resolves where a URL-based reset should navigate.
///
/// An override may retarget the path — providers move their new-chat route,
/// and Google's `/u/<n>/` account prefix differs per person — but never the
/// origin. Placement refuses to fill a page that is not on
/// `expected_fill_host`, so an off-origin reset would strand the pane
/// somewhere the prompt can never land; rejecting it here turns that into an
/// error the user sees while editing the setting.
pub(crate) fn resolve_new_chat_url(
    provider: Provider,
    override_url: Option<&str>,
) -> Result<Url, ProviderCommandError> {
    let config = provider.config();
    let Some(candidate) = override_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Url::parse(config.new_chat_url).map_err(|error| {
            ProviderCommandError::new(
                ProviderErrorCode::WebviewOperationFailed,
                format!("Invalid built-in new chat URL: {error}"),
            )
        });
    };

    if candidate.len() > MAX_URL_BYTES {
        return Err(invalid_request("The New chat address is too long."));
    }
    let url = Url::parse(candidate)
        .map_err(|_| invalid_request("The New chat address is not a valid web address."))?;
    if url.scheme() != "https" {
        return Err(invalid_request(
            "The New chat address must start with https.",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid_request(
            "The New chat address must not contain sign-in credentials.",
        ));
    }
    if url.port_or_known_default() != Some(443) {
        return Err(invalid_request(
            "The New chat address must use the standard https port.",
        ));
    }
    if url.host_str() != Some(config.expected_fill_host) {
        return Err(invalid_request(format!(
            "The New chat address must stay on {}.",
            config.expected_fill_host
        )));
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher_with_attribute(name: &str, value: &str) -> NewChatMatcher {
        NewChatMatcher {
            attributes: vec![NewChatAttribute {
                name: name.into(),
                value: value.into(),
            }],
            ..NewChatMatcher::default()
        }
    }

    #[test]
    fn sanitizing_trims_signals_and_lowercases_attribute_names() {
        let matcher = NewChatMatcher {
            test_id: Some("  create-new-chat-button  ".into()),
            label: Some(" New chat ".into()),
            href: Some("/".into()),
            attributes: vec![NewChatAttribute {
                name: "Data-Sidebar-Item".into(),
                value: " true ".into(),
            }],
        }
        .sanitized()
        .expect("a well-formed matcher should sanitize");

        assert_eq!(matcher.test_id.as_deref(), Some("create-new-chat-button"));
        assert_eq!(matcher.label.as_deref(), Some("New chat"));
        assert_eq!(matcher.href.as_deref(), Some("/"));
        assert_eq!(matcher.attributes[0].name, "data-sidebar-item");
        assert_eq!(matcher.attributes[0].value, "true");
    }

    #[test]
    fn sanitizing_rejects_appearance_attributes_and_oversized_signals() {
        assert!(matcher_with_attribute("class", "menu-item")
            .sanitized()
            .is_err());
        assert!(matcher_with_attribute("style", "color:red")
            .sanitized()
            .is_err());
        assert!(matcher_with_attribute("onclick", "alert(1)")
            .sanitized()
            .is_err());
        assert!(matcher_with_attribute("data-", "x").sanitized().is_err());

        let oversized = NewChatMatcher {
            test_id: Some("x".repeat(MAX_SIGNAL_BYTES + 1)),
            ..NewChatMatcher::default()
        };
        assert!(oversized.sanitized().is_err());

        let control_characters = NewChatMatcher {
            label: Some("New\nchat".into()),
            ..NewChatMatcher::default()
        };
        assert!(control_characters.sanitized().is_err());
    }

    #[test]
    fn sanitizing_rejects_matchers_that_cannot_identify_anything() {
        assert!(NewChatMatcher::default().sanitized().is_err());

        let blank = NewChatMatcher {
            test_id: Some("   ".into()),
            label: Some("".into()),
            ..NewChatMatcher::default()
        };
        assert!(blank.sanitized().is_err());
    }

    #[test]
    fn sanitizing_rejects_duplicate_and_excessive_attributes() {
        let duplicated = NewChatMatcher {
            attributes: vec![
                NewChatAttribute {
                    name: "data-x".into(),
                    value: "1".into(),
                },
                NewChatAttribute {
                    name: "DATA-X".into(),
                    value: "2".into(),
                },
            ],
            ..NewChatMatcher::default()
        };
        assert!(duplicated.sanitized().is_err());

        let excessive = NewChatMatcher {
            attributes: (0..=MAX_ATTRIBUTES)
                .map(|index| NewChatAttribute {
                    name: format!("data-{index}"),
                    value: "1".into(),
                })
                .collect(),
            ..NewChatMatcher::default()
        };
        assert!(excessive.sanitized().is_err());
    }

    #[test]
    fn an_absent_override_resolves_to_the_built_in_target() {
        for provider in Provider::ALL {
            let resolved = resolve_new_chat_url(provider, None).unwrap();
            assert_eq!(resolved.as_str(), provider.config().new_chat_url);
            assert_eq!(
                resolve_new_chat_url(provider, Some("   ")).unwrap(),
                resolved
            );
        }
    }

    #[test]
    fn an_override_may_retarget_the_path_but_never_the_origin() {
        let accepted = resolve_new_chat_url(
            Provider::Gemini,
            Some("https://gemini.google.com/u/1/app/new"),
        )
        .expect("a same-origin account path should be accepted");
        assert_eq!(accepted.path(), "/u/1/app/new");

        assert!(resolve_new_chat_url(Provider::Chatgpt, Some("https://chatgpt.com/new")).is_ok());
        assert!(
            resolve_new_chat_url(Provider::Chatgpt, Some("https://chatgpt.com/?model=gpt-5"))
                .is_ok()
        );
    }

    #[test]
    fn an_override_is_held_to_the_same_origin_policy_as_placement() {
        let rejected = [
            (Provider::Chatgpt, "http://chatgpt.com/"),
            (Provider::Chatgpt, "https://chatgpt.com:8443/"),
            (Provider::Chatgpt, "https://evil.chatgpt.com/"),
            (Provider::Chatgpt, "https://chatgpt.com.evil.com/"),
            (Provider::Chatgpt, "https://gemini.google.com/app/new"),
            (Provider::Chatgpt, "javascript:alert(1)"),
            (Provider::Chatgpt, "data:text/html,<b>x</b>"),
            (Provider::Chatgpt, "https://user:pass@chatgpt.com/"),
            (Provider::Chatgpt, "not a url"),
            (Provider::Gemini, "https://chatgpt.com/"),
            (Provider::Gemini, "https://sites.google.com/view/untrusted"),
        ];
        for (provider, candidate) in rejected {
            let error = resolve_new_chat_url(provider, Some(candidate))
                .expect_err("{candidate} must be rejected");
            assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
        }

        let oversized = format!("https://chatgpt.com/{}", "x".repeat(MAX_URL_BYTES));
        assert!(resolve_new_chat_url(Provider::Chatgpt, Some(&oversized)).is_err());
    }
}
