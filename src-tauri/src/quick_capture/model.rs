use serde::Serialize;

pub(crate) const CONTRACT_VERSION: u8 = 3;

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
    AccessibilityPermissionRequired,
    InvalidRequest,
    ClipboardUnavailable,
    ClipboardChanged,
    ClipboardTooLarge,
    ShortcutKeysHeld,
    ShortcutInvalid,
    ShortcutUnavailable,
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
            Self::AccessibilityPermissionRequired => {
                "Quick Capture needs Accessibility permission to read your selection. Enable Prompter under Privacy & Security → Accessibility, then relaunch Prompter."
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
            // The shortcut is user-configurable, so these read without naming
            // a specific key combination.
            Self::ShortcutKeysHeld => {
                "Release the shortcut keys, then press the Quick Capture shortcut again."
            }
            Self::ShortcutInvalid => {
                "That key combination cannot be used. Hold ⌘, ⌥, or ⌃ together with another key."
            }
            Self::ShortcutUnavailable => {
                "macOS or another app is already using that shortcut. Choose a different combination."
            }
            Self::CopyFailed => "Prompter could not copy the selected text.",
            Self::CopyTimedOut => {
                "Nothing was copied. Select some text, then use the Quick Capture shortcut again."
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
    /// Canonical accelerator, as normalized by the shortcut module.
    pub(crate) accelerator: String,
    pub(crate) display: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuickCaptureStatus {
    pub(crate) version: u8,
    pub(crate) shortcut: ShortcutDescriptor,
    pub(crate) registration: ShortcutRegistrationState,
    /// Permission to synthesize the ⌘C keystroke.
    pub(crate) permission: PermissionState,
    /// Accessibility trust, granted separately from `permission`.
    pub(crate) accessibility: PermissionState,
}

impl QuickCaptureStatus {
    pub(crate) fn new(
        shortcut: ShortcutDescriptor,
        registration: ShortcutRegistrationState,
        permission: PermissionState,
        accessibility: PermissionState,
    ) -> Self {
        Self {
            version: CONTRACT_VERSION,
            shortcut,
            registration,
            permission,
            accessibility,
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
        accessibility: PermissionState,
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
    fn status_contract_reports_both_macos_grants_separately() {
        let status = QuickCaptureStatus::new(
            ShortcutDescriptor {
                accelerator: "shift+super+KeyP".into(),
                display: "⇧ ⌘ P".into(),
            },
            ShortcutRegistrationState::Registered,
            PermissionState::Granted,
            PermissionState::Required,
        );
        let value = serde_json::to_value(status).expect("status should serialize");

        assert_eq!(value["version"], CONTRACT_VERSION);
        assert_eq!(value["shortcut"]["accelerator"], "shift+super+KeyP");
        assert_eq!(value["shortcut"]["display"], "⇧ ⌘ P");
        assert_eq!(value["registration"], "registered");
        assert_eq!(value["permission"], "granted");
        assert_eq!(value["accessibility"], "required");
    }

    #[test]
    fn shortcut_rejection_messages_never_name_a_fixed_key_combination() {
        // The shortcut is configurable, so guidance that hardcodes one would
        // be wrong for every user who changed it.
        for code in [
            CaptureErrorCode::ShortcutKeysHeld,
            CaptureErrorCode::CopyTimedOut,
        ] {
            let message = code.user_message();
            for glyph in ["⌘", "⇧", "⌃", "⌥"] {
                assert!(
                    !message.contains(glyph),
                    "{code:?} names {glyph}, which is wrong once the user rebinds the shortcut"
                );
            }
        }
    }
}
