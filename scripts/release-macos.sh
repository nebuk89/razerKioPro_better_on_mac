#!/bin/bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IDENTITY="${KIYO_SIGNING_IDENTITY:-Developer ID Application: Benjamin De St Paer-Gotch (89N7ZG42ZM)}"
NOTARY_PROFILE="${KIYO_NOTARY_PROFILE:-kiyo-notary}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"

case "$(uname -m)" in
  arm64) ARCH="arm64" ;;
  x86_64) ARCH="x86_64" ;;
  *) echo "Unsupported build architecture: $(uname -m)" >&2; exit 1 ;;
esac

DIST="$ROOT/dist"
STAGE="$ROOT/target/release-dmg"
DMG="$DIST/kiyo-control-v${VERSION}-macos-${ARCH}.dmg"

mkdir -p "$DIST"
rm -rf "$STAGE"
rm -f "$DMG"

cd "$ROOT"
cargo build --release --locked

codesign \
  --force \
  --options runtime \
  --timestamp \
  --identifier com.github.nebuk89.kiyo-control \
  --sign "$IDENTITY" \
  "$ROOT/target/release/kiyo"
codesign --verify --strict --verbose=2 "$ROOT/target/release/kiyo"

mkdir -p "$STAGE"
cp "$ROOT/target/release/kiyo" "$STAGE/kiyo"
cat > "$STAGE/INSTALL.txt" <<'EOF'
Kiyo Control for macOS
======================

Install:
  mkdir -p ~/.local/bin
  cp /Volumes/Kiyo\ Control/kiyo ~/.local/bin/kiyo

Configure and enable reconnect persistence:
  ~/.local/bin/kiyo show
  ~/.local/bin/kiyo save
  ~/.local/bin/kiyo install
EOF

hdiutil create \
  -volname "Kiyo Control" \
  -srcfolder "$STAGE" \
  -format UDZO \
  "$DMG"

codesign --force --timestamp --sign "$IDENTITY" "$DMG"
codesign --verify --strict --verbose=2 "$DMG"

xcrun notarytool submit "$DMG" \
  --keychain-profile "$NOTARY_PROFILE" \
  --wait
xcrun stapler staple "$DMG"
xcrun stapler validate "$DMG"
spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"

shasum -a 256 "$DMG"
echo "Release ready: $DMG"
