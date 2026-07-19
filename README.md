# Prompter

Prompter is a macOS-first Tauri 2 companion for rewriting selected text with a user's existing ChatGPT or Gemini account. It does not require an AI API key.

## How it works

- Instruction presets are editable and stored locally. Each preset has a required instruction before the user's text and an optional instruction after it.
- ChatGPT and Gemini run inside an embedded provider WebView.
- Prompter sends exactly the before-text instruction, the user's text, and the optional after-text instruction in that order, then places the result into the provider's real input box.
- Prompter never presses Send. The user reviews and sends the prompt manually.
- Responses and provider Copy buttons remain inside ChatGPT or Gemini.
- Before inserting user text, the native layer verifies that the WebView is on the expected provider host.
- Quick Capture (`Command + Shift + P`) copies selected text from the active macOS app, restores the previous clipboard exactly, and opens Prompter. It never sends the text automatically.

## Requirements

- macOS with Xcode Command Line Tools
- Node.js 20.19+, 22.12+, or 24+
- Rust 1.77.2 or newer

Quick Capture requires macOS 10.15 or newer and Prompter to be allowed under **System Settings → Privacy & Security → Accessibility**. Prompter requests only event-posting access so it can press Copy for the user. Permission is requested from Settings, never silently at startup.

Closing the main window keeps Prompter running so the shortcut remains available. Use `Command + Q` to quit completely.

**Launch at Login** is optional and disabled by default. When enabled from Prompter Settings, the app starts with its main window hidden, registers Quick Capture, and waits for the user. Provider WebViews are loaded only after the window is shown. For a stable login-item path, move `Prompter.app` to Applications before enabling it.

## Run locally

```bash
npm install
npm run tauri dev
```

## Architecture

The code is organized by responsibility:

- `src/App.tsx` coordinates application state and feature composition only.
- `src/features/instructions` owns instruction models, validated storage, collection rules, and instruction UI.
- `src/features/lifecycle` owns the versioned native lifecycle contract, Launch at Login state, window-visibility events, and frontend lifecycle coordination.
- `src/features/providers` owns provider metadata, the typed Tauri gateway, WebView lifecycle, and prompt placement UI.
- `src/features/quickCapture` owns versioned native contracts, runtime validation, durable event draining, selected-text state, and permission actions.
- `src/features/settings` owns theme persistence and settings UI.
- `src/shared` contains small reusable UI primitives.
- `src-tauri/src/prompt.rs` owns prompt validation and composition.
- `src-tauri/src/app_lifecycle` owns the single permanent native window, close-to-background behavior, activation serialization, Dock/second-launch handling, and Launch at Login integration.
- `src-tauri/src/provider` owns provider configuration, safe WebView integration, request correlation, and the generic fill adapter.
- `src-tauri/src/platform` isolates native window and provider WebView platform behavior.
- `src-tauri/src/quick_capture` separates shortcut coordination, typed outcomes, deterministic capture logic, full-fidelity pasteboard transactions, and macOS permission APIs.

Quick Capture logs registration state, outcome codes, and timings to the standard Tauri application log directory. Selected text and clipboard contents are never logged.

The configured `main` window is created once and never reconstructed during normal operation. Red-close hides the application, while Dock reopen, Quick Capture, and a second launch all activate the same native window on the user's currently active macOS Desktop/Space. `Command + Q` is allowed to exit; if a clipboard capture is active, exit is deferred briefly so the original clipboard can be restored first.

Provider websites can change their editor DOM. Update each provider's selector list in `src-tauri/src/provider/mod.rs`, then run the adapter tests and manually verify both providers.

## Verify

```bash
npm run check
cd src-tauri
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
