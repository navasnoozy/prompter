use std::{
    ffi::c_void,
    ptr, thread,
    time::{Duration, Instant},
};

use core_foundation::{
    base::{Boolean, CFEqual, CFIndex, CFRange, CFType, CFTypeRef, TCFType},
    string::{kCFStringEncodingUTF8, CFString, CFStringGetBytes, CFStringGetTypeID, CFStringRef},
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

use super::service::{
    BackendFailure, CaptureBackend, ClipboardSnapshot, RestoreDisposition, MAX_CAPTURE_BYTES,
};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const MAX_CLIPBOARD_ITEMS: usize = 128;
const MAX_REPRESENTATIONS_PER_ITEM: usize = 128;
const MAX_CLIPBOARD_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;
const MAX_TYPE_IDENTIFIER_BYTES: usize = 4 * 1024;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StringConversionFailure {
    InvalidUtf8,
    TooLarge,
}

pub(crate) struct PasteboardSnapshot {
    change_count: isize,
    frontmost_pid: i32,
    focused_element: CFType,
    items: Vec<PasteboardItemSnapshot>,
}

type AXUIElementRef = *const c_void;
type AXError = i32;
const AX_SUCCESS: AXError = 0;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
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
        read_general_pasteboard_text(None)
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

    fn read_selected_text(&self) -> Result<Option<String>, BackendFailure> {
        read_accessibility_selected_text()
    }

    fn snapshot_clipboard(&self) -> Result<Self::Snapshot, BackendFailure> {
        snapshot_general_pasteboard()
    }

    fn validate_capture_target(
        &self,
        snapshot: &Self::Snapshot,
        expected_change_count: isize,
    ) -> Result<(), BackendFailure> {
        let pasteboard = NSPasteboard::generalPasteboard();
        let focused_now =
            focused_accessibility_element().ok_or(BackendFailure::ClipboardChanged)?;
        let same_element = unsafe {
            CFEqual(
                snapshot.focused_element.as_CFTypeRef(),
                focused_now.as_CFTypeRef(),
            ) != 0 as Boolean
        };
        if pasteboard.changeCount() != expected_change_count
            || frontmost_process_id()? != snapshot.frontmost_pid
            || accessibility_element_pid(&focused_now) != Some(snapshot.frontmost_pid)
            || !same_element
        {
            return Err(BackendFailure::ClipboardChanged);
        }
        Ok(())
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

    fn read_clipboard_text(&self, expected_change_count: isize) -> Result<String, BackendFailure> {
        read_general_pasteboard_text(Some(expected_change_count))
    }

    fn restore_clipboard_if_unchanged(
        &self,
        snapshot: Self::Snapshot,
        expected_change_count: isize,
    ) -> Result<RestoreDisposition, BackendFailure> {
        restore_general_pasteboard(snapshot, expected_change_count)
    }
}

fn read_general_pasteboard_text(
    expected_change_count: Option<isize>,
) -> Result<String, BackendFailure> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let initial_change_count = pasteboard.changeCount();
    if expected_change_count.is_some_and(|expected| expected != initial_change_count) {
        return Err(BackendFailure::ClipboardChanged);
    }

    let value = pasteboard
        .stringForType(unsafe { NSPasteboardTypeString })
        .ok_or(BackendFailure::NoText)?;
    let text =
        ns_string_to_bounded_utf8(&value, MAX_CAPTURE_BYTES).map_err(|failure| match failure {
            StringConversionFailure::InvalidUtf8 => BackendFailure::NoText,
            StringConversionFailure::TooLarge => BackendFailure::SelectionTooLarge,
        })?;

    if pasteboard.changeCount() != initial_change_count {
        return Err(BackendFailure::ClipboardChanged);
    }
    Ok(text)
}

/// Reads the focused control's selected text directly through macOS
/// Accessibility whenever the target application exposes it. This is both
/// faster and safer than touching the clipboard. Unsupported controls fall
/// back to the guarded clipboard transaction below.
fn read_accessibility_selected_text() -> Result<Option<String>, BackendFailure> {
    let Some(focused) = focused_accessibility_element() else {
        return Ok(None);
    };

    let focused_pid =
        accessibility_element_pid(&focused).ok_or(BackendFailure::ClipboardChanged)?;
    if frontmost_process_id()? != focused_pid {
        return Ok(None);
    }

    let Some(selected) = copy_accessibility_attribute(&focused, "AXSelectedText") else {
        return Ok(None);
    };
    if unsafe { core_foundation::base::CFGetTypeID(selected.as_CFTypeRef()) }
        != unsafe { CFStringGetTypeID() }
    {
        return Ok(None);
    }

    let selected_string = unsafe { CFString::wrap_under_get_rule(selected.as_CFTypeRef().cast()) };
    let text = match cf_string_to_bounded_utf8(&selected_string, MAX_CAPTURE_BYTES) {
        Ok(text) => text,
        // A malformed Accessibility string is unsupported, so use the guarded
        // clipboard fallback instead of calling conversion APIs that assume
        // the external CFString is valid UTF-8.
        Err(StringConversionFailure::InvalidUtf8) => return Ok(None),
        Err(StringConversionFailure::TooLarge) => return Err(BackendFailure::SelectionTooLarge),
    };

    let focused_after = focused_accessibility_element().ok_or(BackendFailure::ClipboardChanged)?;
    let same_element =
        unsafe { CFEqual(focused.as_CFTypeRef(), focused_after.as_CFTypeRef()) != 0 as Boolean };
    if !same_element
        || accessibility_element_pid(&focused_after) != Some(focused_pid)
        || frontmost_process_id()? != focused_pid
    {
        return Err(BackendFailure::ClipboardChanged);
    }

    Ok(Some(text))
}

fn focused_accessibility_element() -> Option<CFType> {
    let system_ref = unsafe { AXUIElementCreateSystemWide() };
    if system_ref.is_null() {
        return None;
    }
    let system = unsafe { CFType::wrap_under_create_rule(system_ref.cast()) };
    copy_accessibility_attribute(&system, "AXFocusedUIElement")
}

fn copy_accessibility_attribute(element: &CFType, attribute: &str) -> Option<CFType> {
    let attribute = CFString::new(attribute);
    let mut value: CFTypeRef = ptr::null();
    let result = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef().cast(),
            attribute.as_concrete_TypeRef(),
            &mut value,
        )
    };
    if result != AX_SUCCESS || value.is_null() {
        return None;
    }
    Some(unsafe { CFType::wrap_under_create_rule(value) })
}

fn accessibility_element_pid(element: &CFType) -> Option<i32> {
    let mut pid = 0i32;
    let result = unsafe { AXUIElementGetPid(element.as_CFTypeRef().cast(), &mut pid) };
    (result == AX_SUCCESS && pid > 0).then_some(pid)
}

fn frontmost_process_id() -> Result<i32, BackendFailure> {
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|application| application.processIdentifier())
        .filter(|pid| *pid > 0)
        .ok_or(BackendFailure::ClipboardUnavailable)
}

fn ns_string_to_bounded_utf8(
    value: &NSString,
    max_bytes: usize,
) -> Result<String, StringConversionFailure> {
    let pointer: *const NSString = value;
    // SAFETY: CFString and NSString are toll-free bridged. The get-rule wrapper
    // retains the object for the duration of the checked conversion.
    let value = unsafe { CFString::wrap_under_get_rule(pointer.cast()) };
    cf_string_to_bounded_utf8(&value, max_bytes)
}

fn cf_string_to_bounded_utf8(
    value: &CFString,
    max_bytes: usize,
) -> Result<String, StringConversionFailure> {
    let character_count = value.char_len();
    if character_count < 0 {
        return Err(StringConversionFailure::InvalidUtf8);
    }
    if character_count == 0 {
        return Ok(String::new());
    }
    // Every valid UTF-8 encoding requires at least one byte per UTF-16 code
    // unit. Reject obviously oversized external strings before asking Core
    // Foundation to scan their entire contents.
    if usize::try_from(character_count).map_err(|_| StringConversionFailure::TooLarge)? > max_bytes
    {
        return Err(StringConversionFailure::TooLarge);
    }

    let mut byte_count: CFIndex = 0;
    let converted_characters = unsafe {
        CFStringGetBytes(
            value.as_concrete_TypeRef(),
            CFRange {
                location: 0,
                length: character_count,
            },
            kCFStringEncodingUTF8,
            0,
            0 as Boolean,
            ptr::null_mut(),
            0,
            &mut byte_count,
        )
    };
    if converted_characters != character_count || byte_count < 0 {
        return Err(StringConversionFailure::InvalidUtf8);
    }

    let byte_count = usize::try_from(byte_count).map_err(|_| StringConversionFailure::TooLarge)?;
    if byte_count > max_bytes {
        return Err(StringConversionFailure::TooLarge);
    }

    let mut bytes = vec![0; byte_count];
    let mut bytes_used: CFIndex = 0;
    let converted_characters = unsafe {
        CFStringGetBytes(
            value.as_concrete_TypeRef(),
            CFRange {
                location: 0,
                length: character_count,
            },
            kCFStringEncodingUTF8,
            0,
            0 as Boolean,
            bytes.as_mut_ptr(),
            CFIndex::try_from(byte_count).map_err(|_| StringConversionFailure::TooLarge)?,
            &mut bytes_used,
        )
    };
    if converted_characters != character_count
        || usize::try_from(bytes_used).ok() != Some(byte_count)
    {
        return Err(StringConversionFailure::InvalidUtf8);
    }
    String::from_utf8(bytes).map_err(|_| StringConversionFailure::InvalidUtf8)
}

fn snapshot_general_pasteboard() -> Result<PasteboardSnapshot, BackendFailure> {
    let pasteboard = NSPasteboard::generalPasteboard();
    snapshot_pasteboard(&pasteboard)
}

fn snapshot_pasteboard(pasteboard: &NSPasteboard) -> Result<PasteboardSnapshot, BackendFailure> {
    let change_count = pasteboard.changeCount();
    let frontmost_pid = frontmost_process_id()?;
    let focused_element =
        focused_accessibility_element().ok_or(BackendFailure::ClipboardUnavailable)?;
    if accessibility_element_pid(&focused_element) != Some(frontmost_pid) {
        return Err(BackendFailure::ClipboardChanged);
    }
    let pasteboard_items = pasteboard.pasteboardItems();
    let item_count = pasteboard_items.as_ref().map_or(0, |items| items.len());

    if item_count > MAX_CLIPBOARD_ITEMS {
        return Err(BackendFailure::ClipboardTooLarge);
    }
    let items = pasteboard_items
        .map(|items| items.to_vec())
        .unwrap_or_default();

    let mut total_bytes = 0usize;
    let mut snapshots = Vec::with_capacity(items.len());

    for item in items {
        let item_types = item.types();
        if item_types.is_empty() {
            return Err(BackendFailure::ClipboardUnavailable);
        }
        if item_types.len() > MAX_REPRESENTATIONS_PER_ITEM {
            return Err(BackendFailure::ClipboardTooLarge);
        }
        let types = item_types.to_vec();

        let mut representations = Vec::with_capacity(types.len());
        for data_type in types {
            let type_identifier = ns_string_to_bounded_utf8(&data_type, MAX_TYPE_IDENTIFIER_BYTES)
                .map_err(|failure| match failure {
                    StringConversionFailure::InvalidUtf8 => BackendFailure::ClipboardUnavailable,
                    StringConversionFailure::TooLarge => BackendFailure::ClipboardTooLarge,
                })?;
            let type_bytes = type_identifier.len();
            let data = item
                .dataForType(&data_type)
                .ok_or(BackendFailure::ClipboardUnavailable)?;
            let data_length = data.length();
            total_bytes = total_bytes
                .checked_add(type_bytes)
                .and_then(|total| total.checked_add(data_length))
                .ok_or(BackendFailure::ClipboardTooLarge)?;
            if total_bytes > MAX_CLIPBOARD_SNAPSHOT_BYTES {
                return Err(BackendFailure::ClipboardTooLarge);
            }

            representations.push(PasteboardRepresentation {
                type_identifier,
                data: data.to_vec(),
            });
        }
        snapshots.push(PasteboardItemSnapshot { representations });
    }

    let focused_after = focused_accessibility_element().ok_or(BackendFailure::ClipboardChanged)?;
    let focus_is_unchanged = unsafe {
        CFEqual(focused_element.as_CFTypeRef(), focused_after.as_CFTypeRef()) != 0 as Boolean
    };
    if pasteboard.changeCount() != change_count
        || frontmost_process_id()? != frontmost_pid
        || accessibility_element_pid(&focused_after) != Some(frontmost_pid)
        || !focus_is_unchanged
    {
        return Err(BackendFailure::ClipboardChanged);
    }

    Ok(PasteboardSnapshot {
        change_count,
        frontmost_pid,
        focused_element,
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
    fn bounded_cf_string_conversion_preserves_unicode_and_enforces_bytes() {
        let value = CFString::new("é🙂");

        assert_eq!(cf_string_to_bounded_utf8(&value, 6), Ok("é🙂".to_string()));
        assert_eq!(
            cf_string_to_bounded_utf8(&value, 5),
            Err(StringConversionFailure::TooLarge)
        );
        assert_eq!(
            cf_string_to_bounded_utf8(&CFString::new(""), 0),
            Ok(String::new())
        );
    }

    #[test]
    fn bounded_cf_string_conversion_rejects_unpaired_surrogates() {
        let code_units = [0xD800_u16, 0x0061];
        let value = unsafe {
            CFString::wrap_under_create_rule(core_foundation::string::CFStringCreateWithCharacters(
                core_foundation::base::kCFAllocatorDefault,
                code_units.as_ptr(),
                code_units.len() as CFIndex,
            ))
        };

        assert_eq!(
            cf_string_to_bounded_utf8(&value, MAX_CAPTURE_BYTES),
            Err(StringConversionFailure::InvalidUtf8)
        );
    }

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
        let expected_items = vec![PasteboardItemSnapshot {
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
        }];
        assert_eq!(snapshot.items, expected_items);

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

        assert_eq!(restored.items, expected_items);
    }
}
