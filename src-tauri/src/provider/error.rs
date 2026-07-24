use serde::Serialize;

pub(crate) const PROVIDER_CONTRACT_VERSION: u8 = 1;

/// Stable machine-readable codes for provider command failures. The frontend
/// branches on `code`; `message` is ready-to-display user copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderErrorCode {
    WindowMissing,
    WebviewMissing,
    WebviewOperationFailed,
    InvalidBounds,
    InvalidRequest,
    NavigationBlocked,
    WrongHost,
    EditorNotFound,
    EditorUpdateFailed,
    MissingInstruction,
    MissingText,
    PromptTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderCommandError {
    pub(crate) version: u8,
    pub(crate) code: ProviderErrorCode,
    pub(crate) message: String,
}

impl ProviderCommandError {
    pub(crate) fn new(code: ProviderErrorCode, message: impl Into<String>) -> Self {
        Self {
            version: PROVIDER_CONTRACT_VERSION,
            code,
            message: message.into(),
        }
    }
}

impl ProviderErrorCode {
    pub(crate) fn from_bridge_value(value: &str) -> Option<Self> {
        match value {
            "wrong_host" => Some(Self::WrongHost),
            "editor_not_found" => Some(Self::EditorNotFound),
            "editor_update_failed" => Some(Self::EditorUpdateFailed),
            "internal" => Some(Self::WebviewOperationFailed),
            _ => None,
        }
    }
}

impl From<crate::prompt::PromptComposeError> for ProviderCommandError {
    fn from(error: crate::prompt::PromptComposeError) -> Self {
        use crate::prompt::PromptComposeError;

        let code = match error {
            PromptComposeError::MissingInstruction => ProviderErrorCode::MissingInstruction,
            PromptComposeError::MissingText => ProviderErrorCode::MissingText,
            PromptComposeError::TooLarge => ProviderErrorCode::PromptTooLarge,
        };
        Self::new(code, error.user_message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_error_serializes_with_stable_contract_fields() {
        let error = ProviderCommandError::new(
            ProviderErrorCode::WrongHost,
            "ChatGPT is showing a sign-in or external page.",
        );
        let value = serde_json::to_value(error).expect("error should serialize");

        assert_eq!(value["version"], PROVIDER_CONTRACT_VERSION);
        assert_eq!(value["code"], "wrong_host");
        assert_eq!(
            value["message"],
            "ChatGPT is showing a sign-in or external page."
        );
    }

    #[test]
    fn compose_errors_map_to_stable_codes_and_messages() {
        use crate::prompt::PromptComposeError;

        let error = ProviderCommandError::from(PromptComposeError::TooLarge);
        assert_eq!(error.code, ProviderErrorCode::PromptTooLarge);
        assert_eq!(error.message, PromptComposeError::TooLarge.user_message());

        assert_eq!(
            ProviderCommandError::from(PromptComposeError::MissingInstruction).code,
            ProviderErrorCode::MissingInstruction
        );
        assert_eq!(
            ProviderCommandError::from(PromptComposeError::MissingText).code,
            ProviderErrorCode::MissingText
        );
    }
}
