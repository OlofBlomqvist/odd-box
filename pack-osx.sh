#!/usr/bin/env bash
set -euo pipefail

# Builds the macOS .app bundle and produces a DMG.
# Requires: cargo-bundle, create-dmg
# Output: target/release/bundle/osx/Odd Box.app
#         odd-box.dmg (in current directory, or $DMG_OUT if set)

DMG_OUT="${DMG_OUT:-odd-box.dmg}"
BUNDLE_DIR="target/release/bundle/osx/Odd Box.app"
ICON_SRC="icons/icon.icns"
PLIST="${BUNDLE_DIR}/Contents/Info.plist"

if ! command -v cargo-bundle &>/dev/null; then
    echo "Installing cargo-bundle..."
    cargo install cargo-bundle
fi

if ! command -v create-dmg &>/dev/null; then
    echo "create-dmg not found. Install with: brew install create-dmg"
    exit 1
fi

# ── Build ─────────────────────────────────────────────────────────────────────
echo "Building app bundle..."
cargo bundle --release

# ── Patch the bundle (cargo-bundle leaves these incomplete) ───────────────────
echo "Patching bundle..."

# PkgInfo: required by mds (Spotlight) to recognise this as a valid app bundle.
# cargo-bundle doesn't generate it, causing Spotlight to skip the app entirely.
printf "APPL????" > "${BUNDLE_DIR}/Contents/PkgInfo"

# Icon: cargo-bundle doesn't copy the .icns or set CFBundleIconFile
mkdir -p "${BUNDLE_DIR}/Contents/Resources"
cp "${ICON_SRC}" "${BUNDLE_DIR}/Contents/Resources/icon.icns"
/usr/libexec/PlistBuddy -c "Delete :CFBundleIconFile" "${PLIST}" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string icon" "${PLIST}"

# Remove the outdated LSRequiresCarbon key cargo-bundle inserts — it has no
# effect on modern macOS but can confuse some tooling
/usr/libexec/PlistBuddy -c "Delete :LSRequiresCarbon" "${PLIST}" 2>/dev/null || true

# Ad-hoc sign the bundle before packaging so the installed app is already in a
# stable, signed state when Homebrew places it in /Applications. Signing in the
# cask postflight modifies the bundle after install and confuses mds/Spotlight.
echo "Ad-hoc signing bundle..."
codesign --force --deep --sign - "${BUNDLE_DIR}"

# ── Create DMG ────────────────────────────────────────────────────────────────

# Remove any existing DMG and create-dmg leftover intermediates to avoid
# "hdiutil: convert failed - File exists" on repeated runs.
rm -f "${DMG_OUT}"
rm -f "$(dirname "${DMG_OUT}")/rw."*".$(basename "${DMG_OUT}")"

echo "Creating DMG -> ${DMG_OUT}..."
create-dmg \
    --app-drop-link 360 150 \
    --filesystem APFS \
    "${DMG_OUT}" \
    "${BUNDLE_DIR}"

echo "Done: ${DMG_OUT}"
