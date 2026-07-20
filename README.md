# Prompter

Prompter is a macOS-first Tauri 2 companion for rewriting selected text with a user's existing ChatGPT or Gemini account. It does not require an AI API key.

## How it works

- Instruction presets are editable and stored locally. Each preset has a required instruction before the user's text and an optional instruction after it.
- ChatGPT and Gemini run inside an embedded provider WebView.
- Prompter sends exactly the before-text instruction, the user's text, and the optional after-text instruction in that order, then places the result into the provider's real input box.
- Prompter never presses Send. The user reviews and sends the prompt manually.
- Placing text writes it into a third-party website; that website may autosave or process editor contents before Send. See [PRIVACY.md](PRIVACY.md).
- Responses and provider Copy buttons remain inside ChatGPT or Gemini.
- Before inserting user text, the native layer verifies the exact HTTPS provider origin immediately before dispatch, and the in-page fill routine revalidates it at every mutation checkpoint. Hiding or switching closes a provider WebView only when it has an in-flight fill; otherwise it preserves the hidden session. Navigation always invalidates native request correlation, so a stale fill cannot later report success.
- The embedded panes only display exact provider and sign-in hosts. Untrusted same-window navigation is blocked; HTTPS links opened in a new window are handed to the default browser.
- Quick Capture (`Command + Shift + P`) first reads selected text directly through macOS Accessibility. For controls that do not expose selection text, it snapshots the clipboard, checks the focused control, frontmost process, and pasteboard change count, and attempts restoration only while the observed change count still matches its capture transaction. It never sends the text automatically.
- A menu-bar item keeps the background app discoverable: open the window or quit from there at any time.
- Keyboard-first: `Command + Return` places the prompt, `Command + 1` / `Command + 2` switch between ChatGPT and Gemini, and a finished Quick Capture focuses the text box directly.
- Instructions, theme, and selections persist in a settings file in Application Support instead of depending on purgeable WKWebView storage. Existing localStorage data migrates automatically on first launch.

## Requirements

Prompter is macOS-only: Quick Capture, the window lifecycle, and the embedded provider panes all depend on AppKit, and the code makes no attempt to build elsewhere.

- macOS with Xcode Command Line Tools
- Node.js 22.23.1 with npm 10.9.8 (pinned by `.nvmrc` and `packageManager`)
- Rust 1.88.0 (pinned by `rust-toolchain.toml`)

The `tauri` crate is pinned to a tested minor release because the embedded multi-webview API requires Tauri's `unstable` feature, which has no stability guarantee across releases. Re-verify both providers manually after bumping it.

Quick Capture requires macOS 10.15 or newer and Prompter to be allowed under **System Settings → Privacy & Security → Accessibility**. Prompter uses that permission to read selected text exposed by the focused control and, when needed, press Copy for the user. Permission is requested from Settings, never silently at startup.

Closing the main window keeps Prompter running so the shortcut remains available. Use `Command + Q` to quit completely.

**Launch at Login** is optional and disabled by default. When enabled from Prompter Settings, the app starts with its main window hidden, registers Quick Capture, and waits for the user. Provider WebViews are loaded only after the window is shown. For a stable login-item path, move `Prompter.app` to Applications before enabling it.

## Run locally

```bash
nvm use
npm ci
npm run tauri dev
```

If you use another Node version manager, install the exact version in `.nvmrc` before running `npm ci`.

## Architecture

The code is organized by responsibility. Frontend state lives in per-feature zustand stores; components subscribe with narrow selectors, hooks only bind native events to stores, and `App.tsx` is a composition root with no state props. The React Compiler handles memoization at build time.

- `src/App.tsx` mounts the binder hooks and lays out the shell; no data flows through it.
- `src/features/instructions` owns instruction models, the tolerant payload decoder, collection rules, the instruction store, and instruction UI.
- `src/features/lifecycle` owns the versioned native lifecycle contract, the lifecycle store, and window-visibility binding.
- `src/features/providers` owns provider metadata, the typed Tauri gateway with structured `{version, code, message}` errors, the provider store, the placement request machine (`placement.ts`), and WebView lifecycle.
- `src/features/quickCapture` owns versioned native contracts, runtime validation, the capture store with durable event draining, and permission actions.
- `src/features/settings` owns the theme/dialog store and settings UI.
- `src/shared` contains UI primitives, the notice store (severity + auto-expiry), the fixed-path native settings gateway, the boot loader with verified legacy-localStorage migration, and the keyboard shortcut layer.
- `src-tauri/src/prompt.rs` owns prompt validation and composition; `place_prompt` composes and fills in a single IPC round trip.
- `src-tauri/src/app_lifecycle` owns the single permanent native window, close-to-background behavior, activation serialization, Dock/second-launch/tray handling, and Launch at Login integration.
- `src-tauri/src/provider` is split by concern: `config` (providers + navigation allowlists), `geometry` (bounds + derived title-bar offset), `bridge` (`prompter://` response correlation), `commands` (webview commands), `error` (the typed command error contract).
- `src-tauri/src/platform` isolates native window and provider WebView platform behavior.
- `src-tauri/src/quick_capture` separates shortcut coordination, typed outcomes, direct Accessibility selection reads, guarded pasteboard fallback transactions, and macOS permission APIs.
- `src-tauri/src/settings.rs` owns the fixed settings path, key allowlist, size limit, serialization lock, and atomic sync-and-rename writes; the frontend has no generic filesystem-store capability.

Quick Capture logs registration state, outcome codes, and timings to the standard Tauri application log directory. Selected text and clipboard contents are never logged.

The configured `main` window is created once and never reconstructed during normal operation. Red-close hides the application, while Dock reopen, Quick Capture, and a second launch all activate the same native window on the user's currently active macOS Desktop/Space. `Command + Q` is allowed to exit; if a clipboard capture is active, exit waits for the transaction to finish so Prompter never intentionally terminates halfway through restoration.

Provider websites can change their editor DOM. Update each provider's selector list in `src-tauri/src/provider/config.rs`, then run the adapter tests and manually verify both providers.

## Verify

```bash
npm run check
```

This single gate verifies application and toolchain version alignment, then runs the frontend tests, ESLint, the TypeScript build, and the Rust checks (`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`). The GitHub Actions workflow in `.github/workflows/ci.yml` runs these gates for pushes to `main`, every pull request, and manual dispatches; it also builds a universal macOS app and DMG and runs scheduled dependency audits.
