# Privacy

Prompter is a local-first desktop interface. It has no Prompter-operated account, analytics, advertising, telemetry, or remote logging in this codebase.

## Data stored on the Mac

Prompter stores instruction presets, the selected instruction, the selected provider, theme preference, window state, and Launch at Login preference locally. Settings are written to Prompter's Application Support directory. Captured source text is held in application memory and is not included in the settings file or Prompter logs.

Quick Capture first asks macOS Accessibility for the focused control's selected text. If that control does not expose selection text, Prompter temporarily uses the system clipboard and attempts to restore its prior contents. It stops before Copy when its bounded snapshot or observed focus/process checks fail, and it does not overwrite the clipboard when the observed pasteboard change count no longer matches its capture transaction.

## Third-party providers

ChatGPT and Gemini are third-party websites embedded in native WebViews. Their cookies, account sessions, storage, network requests, and handling of content are governed by their respective policies.

When the user chooses **Place**, Prompter writes the composed prompt into the provider website's editor. Prompter does not press Send, but the provider website can still autosave, process, or transmit editor contents before the user presses Send. Users should therefore treat **Place** as disclosure of that prompt to the selected provider.

Prompter does not read provider responses or account credentials. Authentication is handled directly by the provider website.

## Logs

Local diagnostic logs contain operational event names, safe error codes, durations, and request identifiers. Prompt text, selected text, clipboard contents, cookies, and credentials are not intentionally logged.

## Permissions

Prompter requests macOS Accessibility permission for Quick Capture. It uses that permission to read selected text when an app exposes it and, when necessary, to synthesize Copy. Permission can be revoked at any time in System Settings.

## Removal

Deleting Prompter removes the application but macOS may retain its Application Support data, WebView website data, logs, login-item state, and privacy permission record. Users can remove those through macOS or their Library folder when they no longer want them retained.
