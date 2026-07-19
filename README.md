# Prompter

Prompter is a macOS-first Tauri 2 companion for rewriting selected text with a user's existing ChatGPT or Gemini account. It does not require an AI API key.

## How it works

- Instruction presets are editable and stored locally. Each preset has a required instruction before the user's text and an optional instruction after it.
- ChatGPT and Gemini run inside an embedded provider WebView.
- Prompter sends exactly the before-text instruction, the user's text, and the optional after-text instruction in that order, then places the result into the provider's real input box.
- Prompter never presses Send. The user reviews and sends the prompt manually.
- Responses and provider Copy buttons remain inside ChatGPT or Gemini.
- Before inserting user text, the native layer verifies that the WebView is on the expected provider host.
- The embedded panes only display the provider itself and known sign-in hosts. Every other link opens in the default browser, so the address-bar-less pane can never show an arbitrary site.
- Quick Capture (`Command + Shift + P`) copies selected text from the active macOS app, restores the previous clipboard exactly, and opens Prompter. It never sends the text automatically.
- A menu-bar item keeps the background app discoverable: open the window or quit from there at any time.
- Keyboard-first: `Command + Return` places the prompt, `Command + 1` / `Command + 2` switch between ChatGPT and Gemini, and a finished Quick Capture focuses the text box directly.
- Instructions, theme, and selections persist in a durable settings file in Application Support (WKWebView storage can be purged by macOS; the settings file cannot). Existing localStorage data migrates automatically on first launch.

## Requirements

Prompter is macOS-only: Quick Capture, the window lifecycle, and the embedded provider panes all depend on AppKit, and the code makes no attempt to build elsewhere.

- macOS with Xcode Command Line Tools
- Node.js 20.19+, 22.12+, or 24+
- Rust 1.77.2 or newer

The `tauri` crate is pinned to a tested minor release because the embedded multi-webview API requires Tauri's `unstable` feature, which has no stability guarantee across releases. Re-verify both providers manually after bumping it.

Quick Capture requires macOS 10.15 or newer and Prompter to be allowed under **System Settings → Privacy & Security → Accessibility**. Prompter requests only event-posting access so it can press Copy for the user. Permission is requested from Settings, never silently at startup.

Closing the main window keeps Prompter running so the shortcut remains available. Use `Command + Q` to quit completely.

**Launch at Login** is optional and disabled by default. When enabled from Prompter Settings, the app starts with its main window hidden, registers Quick Capture, and waits for the user. Provider WebViews are loaded only after the window is shown. For a stable login-item path, move `Prompter.app` to Applications before enabling it.

## Run locally

```bash
npm install
npm run tauri dev
```

## Architecture

The code is organized by responsibility. Frontend state lives in per-feature zustand stores; components subscribe with narrow selectors, hooks only bind native events to stores, and `App.tsx` is a composition root with no state props. The React Compiler handles memoization at build time.

- `src/App.tsx` mounts the binder hooks and lays out the shell; no data flows through it.
- `src/features/instructions` owns instruction models, the tolerant payload decoder, collection rules, the instruction store, and instruction UI.
- `src/features/lifecycle` owns the versioned native lifecycle contract, the lifecycle store, and window-visibility binding.
- `src/features/providers` owns provider metadata, the typed Tauri gateway with structured `{version, code, message}` errors, the provider store, the placement request machine (`placement.ts`), and WebView lifecycle.
- `src/features/quickCapture` owns versioned native contracts, runtime validation, the capture store with durable event draining, and permission actions.
- `src/features/settings` owns the theme/dialog store and settings UI.
- `src/shared` contains UI primitives, the notice store (severity + auto-expiry), the durable settings gateway (`tauri-plugin-store`), the boot loader with legacy-localStorage migration, and the keyboard shortcut layer.
- `src-tauri/src/prompt.rs` owns prompt validation and composition; `place_prompt` composes and fills in a single IPC round trip.
- `src-tauri/src/app_lifecycle` owns the single permanent native window, close-to-background behavior, activation serialization, Dock/second-launch/tray handling, and Launch at Login integration.
- `src-tauri/src/provider` is split by concern: `config` (providers + navigation allowlists), `geometry` (bounds + derived title-bar offset), `bridge` (`prompter://` response correlation), `commands` (webview commands), `error` (the typed command error contract).
- `src-tauri/src/platform` isolates native window and provider WebView platform behavior.
- `src-tauri/src/quick_capture` separates shortcut coordination, typed outcomes, deterministic capture logic, full-fidelity pasteboard transactions, and macOS permission APIs.

Quick Capture logs registration state, outcome codes, and timings to the standard Tauri application log directory. Selected text and clipboard contents are never logged.

The configured `main` window is created once and never reconstructed during normal operation. Red-close hides the application, while Dock reopen, Quick Capture, and a second launch all activate the same native window on the user's currently active macOS Desktop/Space. `Command + Q` is allowed to exit; if a clipboard capture is active, exit is deferred briefly so the original clipboard can be restored first.

Provider websites can change their editor DOM. Update each provider's selector list in `src-tauri/src/provider/mod.rs`, then run the adapter tests and manually verify both providers.

## Verify

```bash
npm run check
```

This single gate runs the frontend tests, ESLint, the TypeScript build, and the Rust checks (`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`). The GitHub Actions workflow in `.github/workflows/ci.yml` runs the same steps on every push and pull request.
