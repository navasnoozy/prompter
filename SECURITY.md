# Security policy

## Supported versions

Until Prompter has a stable public release channel, only the latest commit on the default branch is supported with security fixes.

## Reporting a vulnerability

Please use this repository's [private GitHub security advisory form](https://github.com/navasnoozy/prompter/security/advisories/new). Do not include credentials, private prompt content, clipboard data, or provider session material in a public issue.

Useful reports include the affected commit or version, macOS version and architecture, reproducible steps, impact, and a minimal proof of concept. Reports involving ChatGPT or Gemini should clearly distinguish Prompter behavior from behavior owned by the provider website.

## Security boundaries

- The local main WebView may invoke only explicitly allowlisted native commands.
- Remote provider WebViews receive no Tauri IPC capability.
- Provider navigation and prompt placement use exact HTTPS host checks.
- Settings commands use a fixed Application Support path and a fixed key schema.
- Quick Capture bounds and materializes the clipboard snapshot before Copy, then checks the focused Accessibility element, frontmost process, and pasteboard change count at guarded points. An observed mismatch aborts capture or skips restoration instead of overwriting another clipboard update.
- Prompt and clipboard contents must never be written to operational logs.

Changes to any of these boundaries require focused regression tests and a manual macOS smoke test before release.

## Platform limits

The macOS pasteboard API does not provide Prompter with an atomic snapshot/copy/restore transaction. Another application can update the pasteboard between two successful checks, a delayed synthetic Copy can arrive after the timeout, and AppKit may materialize representation data before Prompter can enforce its byte limit. The implementation reduces these risks and refuses to overwrite an observed external update, but it does not claim mathematical clipboard isolation. Release testing must include simultaneous-copy, focus-change, timeout, rich-data, and quit-during-capture cases on supported macOS versions.

Capture outcomes are intentionally held only in a bounded in-memory queue (the newest eight) so selected text is not persisted to disk. A renderer/process crash or a larger undrained burst can therefore lose an outcome. During an active clipboard fallback, quit prioritizes completing the guarded transaction over availability; an indefinitely blocked macOS Accessibility or lazy pasteboard provider can delay exit.

Provider websites and their authentication policies are external security boundaries. Exact host allowlists prevent arbitrary pages from remaining in the address-bar-less pane, but every supported provider and login flow must still be smoke-tested on a clean profile before release.
