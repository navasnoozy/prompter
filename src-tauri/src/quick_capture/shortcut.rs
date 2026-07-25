//! Parsing, validation, normalization, and macOS rendering for the
//! user-configurable Quick Capture shortcut.
//!
//! Accelerators cross two trust boundaries: they arrive from the webview and
//! they are persisted between runs. Both paths funnel through [`resolve`], so
//! a value that reaches the shortcut manager has already been parsed, checked
//! for a guarding modifier, and rewritten into one canonical spelling.

use std::str::FromStr;

use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

/// Used until the user chooses their own, and restored by "Use default".
pub(crate) const DEFAULT_ACCELERATOR: &str = "CommandOrControl+Shift+P";

/// Accelerators are attacker-influenced input from the webview, so length is
/// bounded before the parser sees them.
const MAX_ACCELERATOR_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShortcutRejection {
    /// Not a key combination this platform can express.
    Unreadable,
    /// Parsed, but would swallow ordinary typing everywhere on the system.
    MissingModifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedShortcut {
    pub(crate) shortcut: Shortcut,
    /// Canonical spelling. Stable across restarts and safe to persist.
    pub(crate) accelerator: String,
    /// Glyphs ordered the way macOS orders them.
    pub(crate) display: String,
}

/// Shift is deliberately excluded: on its own it still produces ordinary
/// characters, so a Shift-only global shortcut would intercept typing.
fn has_guarding_modifier(mods: Modifiers) -> bool {
    mods.intersects(Modifiers::SUPER | Modifiers::CONTROL | Modifiers::ALT)
}

pub(crate) fn resolve(accelerator: &str) -> Result<ResolvedShortcut, ShortcutRejection> {
    if accelerator.len() > MAX_ACCELERATOR_BYTES {
        return Err(ShortcutRejection::Unreadable);
    }

    let shortcut = Shortcut::from_str(accelerator).map_err(|_| ShortcutRejection::Unreadable)?;
    if !has_guarding_modifier(shortcut.mods) {
        return Err(ShortcutRejection::MissingModifier);
    }

    Ok(ResolvedShortcut {
        // `Shortcut` round-trips through its own `Display`, so this collapses
        // every accepted spelling of a combination onto one stored form.
        accelerator: shortcut.to_string(),
        display: display_for(&shortcut),
        shortcut,
    })
}

pub(crate) fn default_resolved() -> ResolvedShortcut {
    resolve(DEFAULT_ACCELERATOR)
        .expect("the compiled-in default accelerator must always be resolvable")
}

/// Apple orders modifiers Control, Option, Shift, Command regardless of how
/// the user pressed them.
fn display_for(shortcut: &Shortcut) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(5);
    if shortcut.mods.contains(Modifiers::CONTROL) {
        parts.push("⌃".to_string());
    }
    if shortcut.mods.contains(Modifiers::ALT) {
        parts.push("⌥".to_string());
    }
    if shortcut.mods.contains(Modifiers::SHIFT) {
        parts.push("⇧".to_string());
    }
    if shortcut.mods.contains(Modifiers::SUPER) {
        parts.push("⌘".to_string());
    }
    parts.push(key_label(shortcut.key));
    parts.join(" ")
}

fn key_label(code: Code) -> String {
    // `Code` renders as its W3C name, which already reads correctly for
    // function keys and is a sound fallback for anything unlisted below.
    let name = code.to_string();
    if let Some(letter) = name.strip_prefix("Key") {
        return letter.to_string();
    }
    if let Some(digit) = name.strip_prefix("Digit") {
        return digit.to_string();
    }

    match code {
        Code::Space => "Space".to_string(),
        Code::Enter | Code::NumpadEnter => "↩".to_string(),
        Code::Tab => "⇥".to_string(),
        Code::Escape => "⎋".to_string(),
        Code::Backspace => "⌫".to_string(),
        Code::Delete => "⌦".to_string(),
        Code::ArrowUp => "↑".to_string(),
        Code::ArrowDown => "↓".to_string(),
        Code::ArrowLeft => "←".to_string(),
        Code::ArrowRight => "→".to_string(),
        Code::Home => "↖".to_string(),
        Code::End => "↘".to_string(),
        Code::PageUp => "⇞".to_string(),
        Code::PageDown => "⇟".to_string(),
        Code::Minus => "-".to_string(),
        Code::Equal => "=".to_string(),
        Code::BracketLeft => "[".to_string(),
        Code::BracketRight => "]".to_string(),
        Code::Backslash => "\\".to_string(),
        Code::Semicolon => ";".to_string(),
        Code::Quote => "'".to_string(),
        Code::Comma => ",".to_string(),
        Code::Period => ".".to_string(),
        Code::Slash => "/".to_string(),
        Code::Backquote => "`".to_string(),
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compiled_in_default_is_resolvable_and_guarded() {
        let resolved = default_resolved();
        assert!(has_guarding_modifier(resolved.shortcut.mods));
        assert_eq!(resolved.display, "⇧ ⌘ P");
        // The frontend mirrors these two strings to decide whether to offer
        // "Use default", so changing either is a deliberate cross-boundary
        // change rather than an implementation detail.
        assert_eq!(resolved.accelerator, "shift+super+KeyP");
    }

    #[test]
    fn every_spelling_of_one_combination_normalizes_to_the_same_stored_form() {
        let written_by_the_frontend = resolve("super+shift+KeyP").expect("should resolve");
        let written_by_a_human = resolve("CommandOrControl+Shift+P").expect("should resolve");

        assert_eq!(
            written_by_the_frontend.accelerator,
            written_by_a_human.accelerator
        );
        assert_eq!(written_by_the_frontend.shortcut, written_by_a_human.shortcut);
    }

    #[test]
    fn a_persisted_accelerator_reloads_as_the_identical_shortcut() {
        // Storage round-tripping is what keeps a chosen shortcut working
        // across restarts, so it is asserted rather than assumed.
        for spelling in [
            "CommandOrControl+Shift+P",
            "Control+Alt+Shift+Super+KeyQ",
            "Alt+Space",
            "Command+ArrowUp",
            "Control+F5",
            "Cmd+Slash",
        ] {
            let first = resolve(spelling).expect("should resolve");
            let reloaded = resolve(&first.accelerator).expect("stored form should resolve");
            assert_eq!(first, reloaded, "{spelling} did not survive a round trip");
        }
    }

    #[test]
    fn combinations_that_would_intercept_typing_are_refused() {
        for typing in ["P", "Shift+P", "Shift+Space", "shift+KeyA"] {
            assert_eq!(
                resolve(typing),
                Err(ShortcutRejection::MissingModifier),
                "{typing} should not be accepted"
            );
        }
    }

    #[test]
    fn unreadable_accelerators_are_refused_without_panicking() {
        for malformed in [
            "",
            "+",
            "Command+",
            "Command+NotAKey",
            "Command+A+B",
            "Command+Shift",
        ] {
            assert_eq!(
                resolve(malformed),
                Err(ShortcutRejection::Unreadable),
                "{malformed} should not be accepted"
            );
        }
        assert_eq!(
            resolve(&"Command+Shift+".repeat(64)),
            Err(ShortcutRejection::Unreadable)
        );
    }

    #[test]
    fn modifier_glyphs_follow_the_macos_order_not_the_typed_order() {
        let resolved = resolve("Super+Shift+Alt+Control+KeyK").expect("should resolve");
        assert_eq!(resolved.display, "⌃ ⌥ ⇧ ⌘ K");
    }

    #[test]
    fn named_keys_render_as_glyphs_and_unlisted_keys_keep_a_readable_name() {
        let cases = [
            ("Command+Space", "⌘ Space"),
            ("Command+Enter", "⌘ ↩"),
            ("Command+ArrowLeft", "⌘ ←"),
            ("Command+Digit1", "⌘ 1"),
            ("Command+Slash", "⌘ /"),
            ("Command+F7", "⌘ F7"),
            ("Command+Insert", "⌘ Insert"),
        ];
        for (accelerator, expected) in cases {
            assert_eq!(
                resolve(accelerator).expect("should resolve").display,
                expected
            );
        }
    }
}
