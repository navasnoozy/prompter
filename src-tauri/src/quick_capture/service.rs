use std::time::Duration;

use super::model::{CaptureErrorCode, CaptureWarningCode};
use crate::prompt::MAX_PROMPT_BYTES;

pub(crate) const MAX_CAPTURE_BYTES: usize = MAX_PROMPT_BYTES;
pub(crate) const CLIPBOARD_CHANGE_TIMEOUT: Duration = Duration::from_millis(1_500);
pub(crate) const SHORTCUT_RELEASE_TIMEOUT: Duration = Duration::from_millis(1_000);

pub(crate) trait ClipboardSnapshot {
    fn change_count(&self) -> isize;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BackendFailure {
    ClipboardUnavailable,
    ClipboardChanged,
    ClipboardTooLarge,
    SelectionTooLarge,
    ShortcutKeysHeld,
    CopyFailed,
    CopyTimedOut,
    NoText,
    RestoreFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestoreDisposition {
    Restored,
    SkippedExternalChange,
}

pub(crate) trait CaptureBackend {
    type Snapshot: ClipboardSnapshot;

    fn has_event_posting_access(&self) -> bool;
    fn wait_for_shortcut_release(&self, timeout: Duration) -> Result<(), BackendFailure>;
    fn read_selected_text(&self) -> Result<Option<String>, BackendFailure>;
    fn snapshot_clipboard(&self) -> Result<Self::Snapshot, BackendFailure>;
    fn validate_capture_target(
        &self,
        snapshot: &Self::Snapshot,
        expected_change_count: isize,
    ) -> Result<(), BackendFailure>;
    fn copy_current_selection(&self) -> Result<(), BackendFailure>;
    fn wait_for_clipboard_change(
        &self,
        initial_change_count: isize,
        timeout: Duration,
    ) -> Result<isize, BackendFailure>;
    fn read_clipboard_text(&self, expected_change_count: isize) -> Result<String, BackendFailure>;
    fn restore_clipboard_if_unchanged(
        &self,
        snapshot: Self::Snapshot,
        expected_change_count: isize,
    ) -> Result<RestoreDisposition, BackendFailure>;
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CapturedSelection {
    pub(crate) text: String,
    pub(crate) warning: Option<CaptureWarningCode>,
}

pub(crate) fn capture_selection<B: CaptureBackend>(
    backend: &B,
) -> Result<CapturedSelection, CaptureErrorCode> {
    if !backend.has_event_posting_access() {
        return Err(CaptureErrorCode::PermissionRequired);
    }

    backend
        .wait_for_shortcut_release(SHORTCUT_RELEASE_TIMEOUT)
        .map_err(map_backend_failure)?;

    if let Some(text) = backend.read_selected_text().map_err(map_backend_failure)? {
        return validate_text(text).map(|text| CapturedSelection {
            text,
            warning: None,
        });
    }

    let snapshot = backend.snapshot_clipboard().map_err(map_backend_failure)?;
    let initial_change_count = snapshot.change_count();

    backend
        .validate_capture_target(&snapshot, initial_change_count)
        .map_err(map_backend_failure)?;
    backend
        .copy_current_selection()
        .map_err(map_backend_failure)?;
    let captured_change_count = backend
        .wait_for_clipboard_change(initial_change_count, CLIPBOARD_CHANGE_TIMEOUT)
        .map_err(map_backend_failure)?;

    let text_result = backend
        .validate_capture_target(&snapshot, captured_change_count)
        .and_then(|_| backend.read_clipboard_text(captured_change_count))
        .map_err(map_backend_failure)
        .and_then(validate_text);
    let restore_result = backend.restore_clipboard_if_unchanged(snapshot, captured_change_count);

    match (text_result, restore_result) {
        (Ok(text), Ok(RestoreDisposition::Restored)) => Ok(CapturedSelection {
            text,
            warning: None,
        }),
        (Ok(_), Ok(RestoreDisposition::SkippedExternalChange)) => {
            Err(CaptureErrorCode::ClipboardChanged)
        }
        (Ok(text), Err(_)) => Ok(CapturedSelection {
            text,
            warning: Some(CaptureWarningCode::ClipboardRestoreFailed),
        }),
        (Err(_), Err(_)) => Err(CaptureErrorCode::ClipboardUnavailable),
        (Err(_), Ok(RestoreDisposition::SkippedExternalChange)) => {
            Err(CaptureErrorCode::ClipboardChanged)
        }
        (Err(error), Ok(RestoreDisposition::Restored)) => Err(error),
    }
}

pub(crate) fn validate_text(text: String) -> Result<String, CaptureErrorCode> {
    if text.trim().is_empty() {
        return Err(CaptureErrorCode::NoText);
    }
    if text.len() > MAX_CAPTURE_BYTES {
        return Err(CaptureErrorCode::SelectionTooLarge);
    }
    Ok(text)
}

pub(crate) fn map_backend_failure(failure: BackendFailure) -> CaptureErrorCode {
    match failure {
        BackendFailure::ClipboardUnavailable | BackendFailure::RestoreFailed => {
            CaptureErrorCode::ClipboardUnavailable
        }
        BackendFailure::ClipboardChanged => CaptureErrorCode::ClipboardChanged,
        BackendFailure::ClipboardTooLarge => CaptureErrorCode::ClipboardTooLarge,
        BackendFailure::SelectionTooLarge => CaptureErrorCode::SelectionTooLarge,
        BackendFailure::ShortcutKeysHeld => CaptureErrorCode::ShortcutKeysHeld,
        BackendFailure::CopyFailed => CaptureErrorCode::CopyFailed,
        BackendFailure::CopyTimedOut => CaptureErrorCode::CopyTimedOut,
        BackendFailure::NoText => CaptureErrorCode::NoText,
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::VecDeque};

    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct FakeSnapshot(isize);

    impl ClipboardSnapshot for FakeSnapshot {
        fn change_count(&self) -> isize {
            self.0
        }
    }

    struct FakeBackend {
        access: bool,
        release_result: Result<(), BackendFailure>,
        snapshot_result: Result<FakeSnapshot, BackendFailure>,
        copy_result: Result<(), BackendFailure>,
        wait_result: Result<isize, BackendFailure>,
        text_result: Result<String, BackendFailure>,
        restore_result: Result<RestoreDisposition, BackendFailure>,
        selected_text_result: Result<Option<String>, BackendFailure>,
        target_result: Result<(), BackendFailure>,
        validated_change_counts: RefCell<Vec<isize>>,
        calls: RefCell<VecDeque<&'static str>>,
    }

    impl FakeBackend {
        fn success(text: &str) -> Self {
            Self {
                access: true,
                release_result: Ok(()),
                snapshot_result: Ok(FakeSnapshot(10)),
                copy_result: Ok(()),
                wait_result: Ok(11),
                text_result: Ok(text.into()),
                restore_result: Ok(RestoreDisposition::Restored),
                selected_text_result: Ok(None),
                target_result: Ok(()),
                validated_change_counts: RefCell::new(Vec::new()),
                calls: RefCell::new(VecDeque::new()),
            }
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.borrow().iter().copied().collect()
        }
    }

    impl CaptureBackend for FakeBackend {
        type Snapshot = FakeSnapshot;

        fn has_event_posting_access(&self) -> bool {
            self.calls.borrow_mut().push_back("permission");
            self.access
        }

        fn wait_for_shortcut_release(&self, _timeout: Duration) -> Result<(), BackendFailure> {
            self.calls.borrow_mut().push_back("release");
            self.release_result
        }

        fn read_selected_text(&self) -> Result<Option<String>, BackendFailure> {
            self.calls.borrow_mut().push_back("selected");
            self.selected_text_result.clone()
        }

        fn snapshot_clipboard(&self) -> Result<Self::Snapshot, BackendFailure> {
            self.calls.borrow_mut().push_back("snapshot");
            self.snapshot_result
        }

        fn validate_capture_target(
            &self,
            _snapshot: &Self::Snapshot,
            expected_change_count: isize,
        ) -> Result<(), BackendFailure> {
            self.calls.borrow_mut().push_back("target");
            self.validated_change_counts
                .borrow_mut()
                .push(expected_change_count);
            self.target_result
        }

        fn copy_current_selection(&self) -> Result<(), BackendFailure> {
            self.calls.borrow_mut().push_back("copy");
            self.copy_result
        }

        fn wait_for_clipboard_change(
            &self,
            _initial_change_count: isize,
            _timeout: Duration,
        ) -> Result<isize, BackendFailure> {
            self.calls.borrow_mut().push_back("wait");
            self.wait_result
        }

        fn read_clipboard_text(
            &self,
            _expected_change_count: isize,
        ) -> Result<String, BackendFailure> {
            self.calls.borrow_mut().push_back("read");
            self.text_result.clone()
        }

        fn restore_clipboard_if_unchanged(
            &self,
            _snapshot: Self::Snapshot,
            _expected_change_count: isize,
        ) -> Result<RestoreDisposition, BackendFailure> {
            self.calls.borrow_mut().push_back("restore");
            self.restore_result
        }
    }

    #[test]
    fn denied_permission_never_touches_the_clipboard() {
        let mut backend = FakeBackend::success("text");
        backend.access = false;

        assert_eq!(
            capture_selection(&backend),
            Err(CaptureErrorCode::PermissionRequired)
        );
        assert_eq!(backend.calls(), vec!["permission"]);
    }

    #[test]
    fn successful_capture_is_exact_and_restores_in_order() {
        let backend = FakeBackend::success("Line one\n✨ Line two");

        let captured = capture_selection(&backend).expect("capture should succeed");

        assert_eq!(captured.text, "Line one\n✨ Line two");
        assert_eq!(captured.warning, None);
        assert_eq!(*backend.validated_change_counts.borrow(), vec![10, 11]);
        assert_eq!(
            backend.calls(),
            vec![
                "permission",
                "release",
                "selected",
                "snapshot",
                "target",
                "copy",
                "wait",
                "target",
                "read",
                "restore"
            ]
        );
    }

    #[test]
    fn blank_capture_still_restores_the_original_clipboard() {
        let backend = FakeBackend::success("   \n");

        assert_eq!(capture_selection(&backend), Err(CaptureErrorCode::NoText));
        assert_eq!(backend.calls().last(), Some(&"restore"));
    }

    #[test]
    fn oversized_capture_still_restores_the_original_clipboard() {
        let backend = FakeBackend::success(&"x".repeat(MAX_CAPTURE_BYTES + 1));

        assert_eq!(
            capture_selection(&backend),
            Err(CaptureErrorCode::SelectionTooLarge)
        );
        assert_eq!(backend.calls().last(), Some(&"restore"));
    }

    #[test]
    fn external_clipboard_change_is_not_overwritten_or_returned_as_capture() {
        let mut backend = FakeBackend::success("Captured");
        backend.restore_result = Ok(RestoreDisposition::SkippedExternalChange);

        assert_eq!(
            capture_selection(&backend),
            Err(CaptureErrorCode::ClipboardChanged)
        );
    }

    #[test]
    fn restore_failure_does_not_discard_captured_text() {
        let mut backend = FakeBackend::success("Captured");
        backend.restore_result = Err(BackendFailure::RestoreFailed);

        let captured = capture_selection(&backend).expect("capture should succeed");

        assert_eq!(captured.text, "Captured");
        assert_eq!(
            captured.warning,
            Some(CaptureWarningCode::ClipboardRestoreFailed)
        );
    }

    #[test]
    fn timeout_never_reads_stale_clipboard_text() {
        let mut backend = FakeBackend::success("Old clipboard text");
        backend.wait_result = Err(BackendFailure::CopyTimedOut);

        assert_eq!(
            capture_selection(&backend),
            Err(CaptureErrorCode::CopyTimedOut)
        );
        assert_eq!(
            backend.calls(),
            vec![
                "permission",
                "release",
                "selected",
                "snapshot",
                "target",
                "copy",
                "wait"
            ]
        );
    }

    #[test]
    fn accessibility_selection_avoids_the_clipboard_transaction() {
        let mut backend = FakeBackend::success("clipboard fallback");
        backend.selected_text_result = Ok(Some("Direct selection".into()));

        let captured = capture_selection(&backend).expect("capture should succeed");

        assert_eq!(captured.text, "Direct selection");
        assert_eq!(backend.calls(), vec!["permission", "release", "selected"]);
    }

    #[test]
    fn unsafe_snapshot_aborts_before_copy() {
        let mut backend = FakeBackend::success("text");
        backend.snapshot_result = Err(BackendFailure::ClipboardUnavailable);

        assert_eq!(
            capture_selection(&backend),
            Err(CaptureErrorCode::ClipboardUnavailable)
        );
        assert_eq!(
            backend.calls(),
            vec!["permission", "release", "selected", "snapshot"]
        );
    }
}
