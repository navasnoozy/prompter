use serde::Serialize;

pub(crate) const CONTRACT_VERSION: u8 = 1;
pub(crate) const CAPTURE_SHORTCUT: &str = "CommandOrControl+Shift+P";
pub(crate) const CAPTURE_SHORTCUT_DISPLAY: &str = "⌘ ⇧ P";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PermissionState {
    Granted,
    Required,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ShortcutRegistrationState {
    Registered,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CaptureErrorCode {
    PermissionRequired,
    InvalidRequest,
    ClipboardUnavailable,
    ClipboardChanged,
    ClipboardTooLarge,
    ShortcutKeysHeld,
    CopyFailed,
    CopyTimedOut,
    NoText,
    SelectionTooLarge,
    Internal,
}

impl CaptureErrorCode {
    pub(crate) fn user_message(self) -> &'static str {
        match self {
            Self::PermissionRequired => {
                "Quick Capture needs macOS permission before it can copy selected text."
            }
            Self::InvalidRequest => "The Quick Capture request was invalid.",
            Self::ClipboardUnavailable => {
                "Prompter could not safely access the clipboard. Please try again."
            }
            Self::ClipboardChanged => {
                "The clipboard changed during capture, so Prompter stopped rather than use the wrong text. Please try again."
            }
            Self::ClipboardTooLarge => {
                "Quick Capture cannot safely preserve the current clipboard because it is too large."
            }
            Self::ShortcutKeysHeld => {
                "Release the shortcut keys, then press ⌘ ⇧ P again."
            }
            Self::CopyFailed => "Prompter could not copy the selected text.",
            Self::CopyTimedOut => {
                "Nothing was copied. Select some text, then press ⌘ ⇧ P again."
            }
            Self::NoText => "The selected content was not readable as text.",
            Self::SelectionTooLarge => {
                "The selected text is too large. Select less text and try again."
            }
            Self::Internal => "Quick Capture could not finish. Please try again.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CaptureWarningCode {
    ClipboardRestoreFailed,
    WindowUnavailable,
}

impl CaptureWarningCode {
    pub(crate) fn user_message(self) -> &'static str {
        match self {
            Self::ClipboardRestoreFailed => {
                "Text captured, but Prompter could not restore the previous clipboard."
            }
            Self::WindowUnavailable => {
                "Text captured, but Prompter could not bring its window forward."
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShortcutDescriptor {
    pub(crate) accelerator: &'static str,
    pub(crate) display: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuickCaptureStatus {
    pub(crate) version: u8,
    pub(crate) shortcut: ShortcutDescriptor,
    pub(crate) registration: ShortcutRegistrationState,
    pub(crate) permission: PermissionState,
}

impl QuickCaptureStatus {
    pub(crate) fn new(
        registration: ShortcutRegistrationState,
        permission: PermissionState,
    ) -> Self {
        Self {
            version: CONTRACT_VERSION,
            shortcut: ShortcutDescriptor {
                accelerator: CAPTURE_SHORTCUT,
                display: CAPTURE_SHORTCUT_DISPLAY,
            },
            registration,
            permission,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureWarning {
    pub(crate) code: CaptureWarningCode,
    pub(crate) message: String,
}

impl From<CaptureWarningCode> for CaptureWarning {
    fn from(code: CaptureWarningCode) -> Self {
        Self {
            code,
            message: code.user_message().to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum CaptureOutcome {
    Success {
        version: u8,
        request_id: String,
        text: String,
        warnings: Vec<CaptureWarning>,
        duration_ms: u64,
    },
    Failure {
        version: u8,
        request_id: String,
        code: CaptureErrorCode,
        message: String,
        permission: PermissionState,
        duration_ms: u64,
    },
}

impl CaptureOutcome {
    pub(crate) fn request_id(&self) -> &str {
        match self {
            Self::Success { request_id, .. } | Self::Failure { request_id, .. } => request_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureReadyEvent {
    pub(crate) version: u8,
    pub(crate) request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClipboardTextPayload {
    pub(crate) version: u8,
    pub(crate) text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureCommandError {
    pub(crate) version: u8,
    pub(crate) code: CaptureErrorCode,
    pub(crate) message: String,
}

impl CaptureCommandError {
    pub(crate) fn new(code: CaptureErrorCode) -> Self {
        Self {
            version: CONTRACT_VERSION,
            code,
            message: code.user_message().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_contract_uses_stable_camel_case_fields() {
        let outcome = CaptureOutcome::Success {
            version: CONTRACT_VERSION,
            request_id: "capture-7".into(),
            text: "Selected text".into(),
            warnings: vec![CaptureWarningCode::ClipboardRestoreFailed.into()],
            duration_ms: 42,
        };

        let value = serde_json::to_value(outcome).expect("outcome should serialize");

        assert_eq!(value["kind"], "success");
        assert_eq!(value["requestId"], "capture-7");
        assert_eq!(value["durationMs"], 42);
        assert_eq!(value["warnings"][0]["code"], "clipboard_restore_failed");
        assert_eq!(value["text"], "Selected text");
    }

    #[test]
    fn status_contract_reports_the_backend_owned_shortcut() {
        let status = QuickCaptureStatus::new(
            ShortcutRegistrationState::Registered,
            PermissionState::Granted,
        );
        let value = serde_json::to_value(status).expect("status should serialize");

        assert_eq!(value["version"], CONTRACT_VERSION);
        assert_eq!(value["shortcut"]["accelerator"], CAPTURE_SHORTCUT);
        assert_eq!(value["shortcut"]["display"], CAPTURE_SHORTCUT_DISPLAY);
        assert_eq!(value["registration"], "registered");
        assert_eq!(value["permission"], "granted");
    }
}
