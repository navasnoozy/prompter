# Prompter

Prompter is a macOS-first Tauri 2 desktop companion for rewriting selected text with a user's existing ChatGPT or Gemini account.

## Current milestone

- Tauri 2 + Rust native shell
- React 19 + TypeScript interface
- Editable instruction presets saved locally
- ChatGPT and Gemini provider windows inside Prompter
- Clipboard capture
- Global `Command + Shift + P` shortcut
- Automatic macOS `Command + C` event for the current selection
- Rust prompt composition with validation
- Provider adapters that fill the ChatGPT/Gemini composer, send, observe completion, and return response text to Prompter

The next milestone hardens the provider selectors against real signed-in sessions, adds onboarding for macOS Accessibility permission, and replaces the original selected text from the result panel.

## Run locally

```bash
npm install
npm run tauri dev
```

## Verify

```bash
npm run build
cd src-tauri
cargo test
```
