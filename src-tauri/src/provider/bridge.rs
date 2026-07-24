use std::collections::HashMap;

use log::{error, warn};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Url};

use super::{
    commands::ProviderLifecycle,
    config::Provider,
    error::{ProviderErrorCode, PROVIDER_CONTRACT_VERSION},
};
use crate::MAIN_WINDOW_LABEL;

pub(super) const MAX_REQUEST_ID_LENGTH: usize = 128;
const MAX_BRIDGE_MESSAGE_LENGTH: usize = 600;

/// Request identifiers correlate fill requests with `prompter://` responses
/// navigated by the provider page; they must stay URL- and log-safe.
pub(super) fn is_valid_request_id(request_id: &str) -> bool {
    !request_id.is_empty()
        && request_id.len() <= MAX_REQUEST_ID_LENGTH
        && !request_id.chars().any(char::is_control)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderFilledPayload {
    version: u8,
    provider: Provider,
    request_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderErrorPayload {
    version: u8,
    provider: Provider,
    request_id: String,
    code: ProviderErrorCode,
    message: String,
}

#[derive(Debug, PartialEq)]
pub(super) enum BridgeEventKind {
    Filled,
    Error {
        code: ProviderErrorCode,
        message: String,
    },
}

#[derive(Debug, PartialEq)]
pub(super) struct BridgeEvent {
    provider: Provider,
    request_id: String,
    kind: BridgeEventKind,
}

pub(super) fn parse_bridge_event(
    url: &Url,
    expected_provider: Provider,
) -> Result<BridgeEvent, String> {
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
    if !is_valid_request_id(request_id) {
        return Err("The prompt request identifier is invalid.".into());
    }

    let kind = match event_name {
        "filled" => BridgeEventKind::Filled,
        "error" => {
            let code = values
                .get("code")
                .and_then(|code| ProviderErrorCode::from_bridge_value(code))
                .ok_or_else(|| "The provider bridge error code is invalid.".to_string())?;
            let message = values
                .get("message")
                .filter(|message| !message.trim().is_empty())
                .map(|message| message.chars().take(MAX_BRIDGE_MESSAGE_LENGTH).collect())
                .unwrap_or_else(|| "The provider connection failed.".into());
            BridgeEventKind::Error { code, message }
        }
        _ => return Err("Unknown provider bridge event.".into()),
    };

    Ok(BridgeEvent {
        provider,
        request_id: request_id.to_string(),
        kind,
    })
}

pub(super) fn handle_provider_bridge_url(app: &AppHandle, expected_provider: Provider, url: &Url) {
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
                "event=bridge_response_validation_failed reason={}",
                error.message
            );
            return;
        }
    }

    let result = match event.kind {
        BridgeEventKind::Filled => app.emit_to(
            MAIN_WINDOW_LABEL,
            "prompter://prompt-filled",
            ProviderFilledPayload {
                version: PROVIDER_CONTRACT_VERSION,
                provider: event.provider,
                request_id: event.request_id,
            },
        ),
        BridgeEventKind::Error { code, message } => app.emit_to(
            MAIN_WINDOW_LABEL,
            "prompter://provider-error",
            ProviderErrorPayload {
                version: PROVIDER_CONTRACT_VERSION,
                provider: event.provider,
                request_id: event.request_id,
                code,
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
    use super::{parse_bridge_event, BridgeEvent, BridgeEventKind, Provider};
    use tauri::Url;

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

    #[test]
    fn request_id_validation_rejects_empty_oversized_and_control_ids() {
        use super::{is_valid_request_id, MAX_REQUEST_ID_LENGTH};

        assert!(is_valid_request_id("request-1"));
        assert!(!is_valid_request_id(""));
        assert!(!is_valid_request_id(&"x".repeat(MAX_REQUEST_ID_LENGTH + 1)));
        assert!(!is_valid_request_id("bad\nid"));
    }

    #[test]
    fn bridge_errors_require_a_stable_allowlisted_code() {
        let valid = Url::parse(
            "prompter://error?provider=chatgpt&requestId=request-1&code=editor_not_found&message=Missing",
        )
        .unwrap();
        assert!(matches!(
            parse_bridge_event(&valid, Provider::Chatgpt).unwrap().kind,
            BridgeEventKind::Error {
                code: super::ProviderErrorCode::EditorNotFound,
                ..
            }
        ));

        let unknown = Url::parse(
            "prompter://error?provider=chatgpt&requestId=request-1&code=unknown&message=Missing",
        )
        .unwrap();
        assert!(parse_bridge_event(&unknown, Provider::Chatgpt).is_err());
    }
}
