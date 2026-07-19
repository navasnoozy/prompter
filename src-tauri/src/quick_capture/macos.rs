use std::{
    thread,
    time::{Duration, Instant},
};

use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode},
    event_source::{CGEventSource, CGEventSourceStateID},
};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting, NSWorkspace,
};
use objc2_core_graphics::{
    CGEventFlags as NativeEventFlags, CGEventSource as NativeEventSource,
    CGEventSourceStateID as NativeEventSourceStateId, CGPreflightPostEventAccess,
    CGRequestPostEventAccess,
};
use objc2_foundation::{NSArray, NSData, NSString, NSURL};

use super::service::{BackendFailure, CaptureBackend, ClipboardSnapshot, RestoreDisposition};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const MAX_CLIPBOARD_ITEMS: usize = 128;
const MAX_REPRESENTATIONS_PER_ITEM: usize = 128;
const MAX_CLIPBOARD_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;
const ACCESSIBILITY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

#[derive(Debug, PartialEq, Eq)]
struct PasteboardRepresentation {
    type_identifier: String,
    data: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
struct PasteboardItemSnapshot {
    representations: Vec<PasteboardRepresentation>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PasteboardSnapshot {
    change_count: isize,
    items: Vec<PasteboardItemSnapshot>,
}

impl ClipboardSnapshot for PasteboardSnapshot {
    fn change_count(&self) -> isize {
        self.change_count
    }
}

pub(crate) struct MacCaptureBackend;

impl MacCaptureBackend {
    pub(crate) fn permission_state() -> bool {
        CGPreflightPostEventAccess()
    }

    pub(crate) fn request_permission() -> bool {
        let _ = CGRequestPostEventAccess();
        CGPreflightPostEventAccess()
    }

    pub(crate) fn read_current_text() -> Result<String, BackendFailure> {
        read_general_pasteboard_text()
    }

    pub(crate) fn open_accessibility_settings() -> Result<(), BackendFailure> {
        let value = NSString::from_str(ACCESSIBILITY_SETTINGS_URL);
        let url = NSURL::URLWithString(&value).ok_or(BackendFailure::CopyFailed)?;

        if NSWorkspace::sharedWorkspace().openURL(&url) {
            Ok(())
        } else {
            Err(BackendFailure::CopyFailed)
        }
    }
}

impl CaptureBackend for MacCaptureBackend {
    type Snapshot = PasteboardSnapshot;

    fn has_event_posting_access(&self) -> bool {
        Self::permission_state()
    }

    fn wait_for_shortcut_release(&self, timeout: Duration) -> Result<(), BackendFailure> {
        let deadline = Instant::now() + timeout;
        let modifiers = NativeEventFlags::MaskCommand | NativeEventFlags::MaskShift;

        loop {
            let flags = NativeEventSource::flags_state(NativeEventSourceStateId::HIDSystemState);
            if !flags.intersects(modifiers) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(BackendFailure::ShortcutKeysHeld);
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn snapshot_clipboard(&self) -> Result<Self::Snapshot, BackendFailure> {
        snapshot_general_pasteboard()
    }

    fn copy_current_selection(&self) -> Result<(), BackendFailure> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| BackendFailure::CopyFailed)?;
        let key_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_C, true)
            .map_err(|_| BackendFailure::CopyFailed)?;
        let key_up = CGEvent::new_keyboard_event(source, KeyCode::ANSI_C, false)
            .map_err(|_| BackendFailure::CopyFailed)?;

        key_down.set_flags(CGEventFlags::CGEventFlagCommand);
        key_up.set_flags(CGEventFlags::CGEventFlagCommand);
        key_down.post(CGEventTapLocation::HID);
        key_up.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn wait_for_clipboard_change(
        &self,
        initial_change_count: isize,
        timeout: Duration,
    ) -> Result<isize, BackendFailure> {
        let deadline = Instant::now() + timeout;

        loop {
            let current_change_count = NSPasteboard::generalPasteboard().changeCount();
            if current_change_count != initial_change_count {
                return Ok(current_change_count);
            }
            if Instant::now() >= deadline {
                return Err(BackendFailure::CopyTimedOut);
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn read_clipboard_text(&self) -> Result<String, BackendFailure> {
        read_general_pasteboard_text()
    }

    fn restore_clipboard_if_unchanged(
        &self,
        snapshot: Self::Snapshot,
        expected_change_count: isize,
    ) -> Result<RestoreDisposition, BackendFailure> {
        restore_general_pasteboard(snapshot, expected_change_count)
    }
}

fn read_general_pasteboard_text() -> Result<String, BackendFailure> {
    NSPasteboard::generalPasteboard()
        .stringForType(unsafe { NSPasteboardTypeString })
        .map(|value| value.to_string())
        .ok_or(BackendFailure::NoText)
}

fn snapshot_general_pasteboard() -> Result<PasteboardSnapshot, BackendFailure> {
    let pasteboard = NSPasteboard::generalPasteboard();
    snapshot_pasteboard(&pasteboard)
}

fn snapshot_pasteboard(pasteboard: &NSPasteboard) -> Result<PasteboardSnapshot, BackendFailure> {
    let change_count = pasteboard.changeCount();
    let items = pasteboard
        .pasteboardItems()
        .map(|items| items.to_vec())
        .unwrap_or_default();

    if items.len() > MAX_CLIPBOARD_ITEMS {
        return Err(BackendFailure::ClipboardTooLarge);
    }

    let mut total_bytes = 0usize;
    let mut snapshots = Vec::with_capacity(items.len());

    for item in items {
        let types = item.types().to_vec();
        if types.len() > MAX_REPRESENTATIONS_PER_ITEM {
            return Err(BackendFailure::ClipboardTooLarge);
        }

        let mut representations = Vec::with_capacity(types.len());
        for data_type in types {
            let data = item
                .dataForType(&data_type)
                .ok_or(BackendFailure::ClipboardUnavailable)?
                .to_vec();
            total_bytes = total_bytes
                .checked_add(data.len())
                .ok_or(BackendFailure::ClipboardTooLarge)?;
            if total_bytes > MAX_CLIPBOARD_SNAPSHOT_BYTES {
                return Err(BackendFailure::ClipboardTooLarge);
            }

            representations.push(PasteboardRepresentation {
                type_identifier: data_type.to_string(),
                data,
            });
        }
        snapshots.push(PasteboardItemSnapshot { representations });
    }

    Ok(PasteboardSnapshot {
        change_count,
        items: snapshots,
    })
}

fn restore_general_pasteboard(
    snapshot: PasteboardSnapshot,
    expected_change_count: isize,
) -> Result<RestoreDisposition, BackendFailure> {
    let pasteboard = NSPasteboard::generalPasteboard();
    restore_pasteboard(&pasteboard, snapshot, expected_change_count)
}

fn restore_pasteboard(
    pasteboard: &NSPasteboard,
    snapshot: PasteboardSnapshot,
    expected_change_count: isize,
) -> Result<RestoreDisposition, BackendFailure> {
    if pasteboard.changeCount() != expected_change_count {
        return Ok(RestoreDisposition::SkippedExternalChange);
    }

    if snapshot.items.is_empty() {
        pasteboard.clearContents();
        return Ok(RestoreDisposition::Restored);
    }

    let mut writable_items: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> =
        Vec::with_capacity(snapshot.items.len());

    for item_snapshot in snapshot.items {
        let item = NSPasteboardItem::new();
        for representation in item_snapshot.representations {
            let data_type = NSString::from_str(&representation.type_identifier);
            let data = NSData::with_bytes(&representation.data);
            if !item.setData_forType(&data, &data_type) {
                return Err(BackendFailure::RestoreFailed);
            }
        }
        writable_items.push(ProtocolObject::from_retained(item));
    }

    if pasteboard.changeCount() != expected_change_count {
        return Ok(RestoreDisposition::SkippedExternalChange);
    }

    let objects = NSArray::from_retained_slice(&writable_items);
    pasteboard.clearContents();
    if pasteboard.writeObjects(&objects) {
        Ok(RestoreDisposition::Restored)
    } else {
        Err(BackendFailure::RestoreFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an active macOS pasteboard server"]
    fn private_pasteboard_round_trips_multiple_binary_representations() {
        let pasteboard = NSPasteboard::pasteboardWithName(&NSString::from_str(
            "app.prompter.tests.quick-capture-round-trip",
        ));
        let original_item = NSPasteboardItem::new();
        let plain_text_type = NSString::from_str("public.utf8-plain-text");
        let custom_type = NSString::from_str("app.prompter.test-binary");
        assert!(original_item.setData_forType(
            &NSData::with_bytes("Clipboard ✨".as_bytes()),
            &plain_text_type,
        ));
        assert!(original_item
            .setData_forType(&NSData::with_bytes(&[0, 1, 2, 3, 254, 255]), &custom_type,));
        let writable = ProtocolObject::from_retained(original_item);
        assert!(pasteboard.writeObjects(&NSArray::from_retained_slice(&[writable])));

        let snapshot = snapshot_pasteboard(&pasteboard).expect("snapshot should succeed");
        let expected_snapshot = PasteboardSnapshot {
            change_count: snapshot.change_count,
            items: vec![PasteboardItemSnapshot {
                representations: vec![
                    PasteboardRepresentation {
                        type_identifier: "public.utf8-plain-text".into(),
                        data: "Clipboard ✨".as_bytes().to_vec(),
                    },
                    PasteboardRepresentation {
                        type_identifier: "app.prompter.test-binary".into(),
                        data: vec![0, 1, 2, 3, 254, 255],
                    },
                ],
            }],
        };
        assert_eq!(snapshot, expected_snapshot);

        pasteboard.clearContents();
        assert!(pasteboard
            .setString_forType(&NSString::from_str("temporary captured text"), unsafe {
                NSPasteboardTypeString
            },));
        let captured_change_count = pasteboard.changeCount();

        assert_eq!(
            restore_pasteboard(&pasteboard, snapshot, captured_change_count),
            Ok(RestoreDisposition::Restored)
        );
        let restored = snapshot_pasteboard(&pasteboard).expect("restored snapshot should read");

        assert_eq!(restored.items, expected_snapshot.items);
    }
}
