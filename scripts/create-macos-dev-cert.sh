#!/usr/bin/env bash
#
# Creates the local, self-signed code signing identity used for macOS
# development builds. Run once per machine; it is safe to re-run.
#
# Why this exists: an ad-hoc signed build's designated requirement is the
# binary's own hash, so macOS treats every rebuild as a different application
# and drops the Accessibility and keystroke grants Quick Capture depends on.
# Signing with a fixed certificate pins the requirement to the bundle
# identifier plus this certificate, and both survive a rebuild.
#
# This identity is for local development only. No other Mac trusts it, and it
# is never used for distribution; releases use the Developer ID identity
# documented in RELEASING.md.
set -euo pipefail

IDENTITY="Prompter Dev"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning | grep -qF "\"$IDENTITY\""; then
  echo "Signing identity \"$IDENTITY\" already exists. Nothing to do."
  security find-identity -v -p codesigning | grep -F "\"$IDENTITY\""
  exit 0
fi

workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT
umask 077

# Apple requires the code signing extended key usage; the certificate is its
# own root, so the designated requirement pins to this certificate's hash.
openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
  -keyout "$workdir/dev.key" -out "$workdir/dev.crt" \
  -subj "/CN=$IDENTITY/O=Prompter Local Development" \
  -addext "basicConstraints=critical,CA:FALSE" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=codeSigning" \
  -addext "subjectKeyIdentifier=hash" >/dev/null 2>&1

# Keychain cannot read PKCS#12 archives that use OpenSSL 3 defaults. LibreSSL
# already writes the older format and rejects the flag that requests it.
legacy_flag=()
if ! openssl version | grep -q LibreSSL; then
  legacy_flag=(-legacy)
fi

password=$(openssl rand -hex 16)
openssl pkcs12 -export "${legacy_flag[@]}" \
  -in "$workdir/dev.crt" -inkey "$workdir/dev.key" \
  -name "$IDENTITY" \
  -out "$workdir/dev.p12" -password "pass:$password"

# -T grants codesign access to the private key without a prompt per build.
security import "$workdir/dev.p12" \
  -k "$KEYCHAIN" \
  -P "$password" \
  -T /usr/bin/codesign \
  -T /usr/bin/security

# A self-signed certificate is not a valid signing identity until it is
# trusted for code signing. macOS asks for the login password here.
echo "macOS will now ask for your login password to trust the certificate."
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" "$workdir/dev.crt"

echo
if security find-identity -v -p codesigning | grep -qF "\"$IDENTITY\""; then
  security find-identity -v -p codesigning | grep -F "\"$IDENTITY\""
  echo
  echo "Created. Build with: npm run macos:build"
  echo "Grant Accessibility to the next build once; later rebuilds keep it."
else
  echo "The certificate was imported but is not a valid signing identity." >&2
  echo "Open Keychain Access, find \"$IDENTITY\", and set Code Signing to Always Trust." >&2
  exit 1
fi
