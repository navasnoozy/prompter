# Prompter

Prompter is a macOS-first Tauri 2 companion for rewriting selected text with a user's existing ChatGPT or Gemini account. It does not require an AI API key.

## How it works

- Instruction presets are editable and stored locally. Each preset has a required instruction before the user's text and an optional instruction after it.
- ChatGPT and Gemini run inside an embedded provider WebView.
- Prompter sends exactly the before-text instruction, the user's text, and the optional after-text instruction in that order, then places the result into the provider's real input box.
- Prompter never presses Send. The user reviews and sends the prompt manually.
- Responses and provider Copy buttons remain inside ChatGPT or Gemini.
- Before inserting user text, the native layer verifies that the WebView is on the expected provider host.

## Requirements

- macOS with Xcode Command Line Tools
- Node.js 20.19+, 22.12+, or 24+
- Rust 1.77.2 or newer

Quick Capture (`Command + Shift + P`) requires Prompter to be allowed under **System Settings → Privacy & Security → Accessibility**. Without that permission, Prompter reports an error instead of using stale clipboard content.

## Run locally

```bash
npm install
npm run tauri dev
```

## Architecture

The code is organized by responsibility:

- `src/App.tsx` coordinates application state and feature composition only.
- `src/features/instructions` owns instruction models, validated storage, collection rules, and instruction UI.
- `src/features/providers` owns provider metadata, the typed Tauri gateway, WebView lifecycle, clipboard capture, and prompt placement UI.
- `src/features/settings` owns theme persistence and settings UI.
- `src/shared` contains small reusable UI primitives.
- `src-tauri/src/prompt.rs` owns prompt validation and composition.
- `src-tauri/src/provider` owns provider configuration, safe WebView integration, request correlation, and the generic fill adapter.
- `src-tauri/src/platform` isolates macOS-specific APIs.
- `src-tauri/src/capture.rs` owns global shortcut and selected-text capture orchestration.

Provider websites can change their editor DOM. Update each provider's selector list in `src-tauri/src/provider/mod.rs`, then run the adapter tests and manually verify both providers.

## Verify

```bash
npm run check
cd src-tauri
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
