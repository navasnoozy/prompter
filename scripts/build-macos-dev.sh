#!/usr/bin/env bash
#
# Builds the macOS app signed with the local development identity so macOS
# keeps the Quick Capture privacy grants across rebuilds. Arguments are
# forwarded to `tauri build`.
#
# Releases do not use this script; see RELEASING.md.
set -euo pipefail

IDENTITY="${APPLE_SIGNING_IDENTITY:-Prompter Dev}"

if ! security find-identity -v -p codesigning | grep -qF "\"$IDENTITY\""; then
  echo "No valid code signing identity named \"$IDENTITY\"." >&2
  echo "Create it with: npm run macos:cert" >&2
  exit 1
fi

echo "Signing with \"$IDENTITY\"."
APPLE_SIGNING_IDENTITY="$IDENTITY" npm run tauri -- build "$@"
