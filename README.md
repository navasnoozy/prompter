# Prompter

Prompter is a macOS-first Tauri 2 desktop companion for rewriting selected text with a user's existing ChatGPT or Gemini account.

## Current milestone

- Tauri 2 + Rust native shell
- React 19 + TypeScript interface
- Editable instruction presets saved locally
- ChatGPT and Gemini embedded directly in the right side of the main Prompter window
- Clipboard capture
- Global `Command + Shift + P` shortcut
- Automatic macOS `Command + C` event for the current selection
- Rust prompt composition with validation
- Provider adapters that place the composed instruction and selected text into the real ChatGPT/Gemini input box
- Manual send: the user reviews the prepared prompt and presses the provider's Send button
- Responses and provider Copy buttons remain in the embedded ChatGPT/Gemini interface

The next milestone hardens the provider selectors against future provider UI changes and adds onboarding for macOS Accessibility permission.

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
