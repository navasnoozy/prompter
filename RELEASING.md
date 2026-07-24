# macOS release checklist

Prompter is not ready for public distribution merely because `tauri build` succeeds. A release owner must complete every item below on a clean checkout.

## One-time product decisions

- Finalize the bundle identifier before the first signed release. Changing it later changes macOS identity, Application Support location, Launch at Login identity, and Accessibility consent continuity.
- Approve a 1024×1024 source icon and regenerate a complete macOS iconset. The current `icon.icns` is incomplete.
- Choose and add the distribution license or EULA, provider non-affiliation wording, and generated third-party license notices after legal review.
- Choose a documented update channel. No in-app updater is currently configured.

## Functional release gate

1. Install Node 22.23.1 with npm 10.9.8 and Rust 1.88.0 as pinned by `.nvmrc`, `packageManager`, and `rust-toolchain.toml`; then run `npm ci` and `npm run check`.
2. Run `npm audit --audit-level=low` and a current RustSec `cargo audit --file src-tauri/Cargo.lock`. Resolve vulnerabilities and document the target relevance and disposition of every informational or unmaintained warning.
3. On clean macOS user profiles, authenticate to both providers and verify sign-in, sign-out, navigation blocking, prompt placement without submission, selector failure handling, and WebView relaunch. Google can reject OAuth in embedded WebViews, so this is a mandatory gate rather than an automated assumption.
4. Exercise direct Accessibility selection and clipboard fallback with plain text, rich text, images, files, multiple pasteboard items, large data, a simultaneous external copy, permission denial/revocation, provider switching, red-close, and Command-Q during capture.
5. Verify VoiceOver, keyboard-only navigation, reduced motion, light/dark appearance, and the oldest supported macOS/WebKit release on real hardware or a VM.

## Universal signed build

Use the same Developer ID Application identity for every release so macOS preserves application identity and privacy grants.

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

Configure Tauri with `APPLE_SIGNING_IDENTITY` and either App Store Connect credentials (`APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH`) or Apple ID notarization credentials (`APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`). Secrets belong in the CI secret store, never in the repository or logs.

## Artifact verification

Run the executable-policy checks against the app bundle, adapting paths to the versioned output:

```bash
lipo -archs src-tauri/target/universal-apple-darwin/release/bundle/macos/Prompter.app/Contents/MacOS/prompter
codesign --verify --deep --strict --verbose=2 src-tauri/target/universal-apple-darwin/release/bundle/macos/Prompter.app
spctl --assess --type execute --verbose=4 src-tauri/target/universal-apple-darwin/release/bundle/macos/Prompter.app
xcrun stapler validate src-tauri/target/universal-apple-darwin/release/bundle/macos/Prompter.app
```

Confirm that `codesign -dv --verbose=4` reports the expected TeamIdentifier, hardened-runtime flag, final bundle identifier, and Developer ID identity.

Assess and validate the disk image with the artifact-policy checks intended for a DMG:

```bash
hdiutil verify src-tauri/target/universal-apple-darwin/release/bundle/dmg/Prompter_0.1.0_universal.dmg
spctl --assess --type open --context context:primary-signature --verbose=4 src-tauri/target/universal-apple-darwin/release/bundle/dmg/Prompter_0.1.0_universal.dmg
xcrun stapler validate src-tauri/target/universal-apple-darwin/release/bundle/dmg/Prompter_0.1.0_universal.dmg
hdiutil attach -readonly -nobrowse src-tauri/target/universal-apple-darwin/release/bundle/dmg/Prompter_0.1.0_universal.dmg
```

From the mounted volume, repeat `lipo`, `codesign`, `spctl --type execute`, and `stapler validate` against the contained app. Then drag it to Applications on a clean Mac, launch it through Finder, and repeat the functional smoke test from the installed path. Detach the mounted image after verification.

Generate and retain SHA-256 checksums, an SBOM, generated third-party license notices, release notes, test evidence, signer identity, notarization result, minimum macOS version, and both supported architectures. Where the publishing platform supports it, attach build provenance/attestation to the exact released artifacts. Never publish an ad-hoc-signed artifact as a production build.
