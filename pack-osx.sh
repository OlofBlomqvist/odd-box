#!/usr/bin/env bash
set -euo pipefail

# Builds the macOS .app bundle and produces a DMG.
# Requires: cargo-bundle, create-dmg
# Output: target/release/bundle/osx/Odd Box.app or Odd Box Preview.app
#         odd-box.dmg or odd-box-preview.dmg (in current directory, or $DMG_OUT if set)
# Set ODD_BOX_MACOS_BRANDING=stable to force stable macOS branding for prerelease builds.

ICON_SRC="icons/icon.icns"
APP_MANIFEST="Cargo.toml"
VERSION="$(grep '^version' "${APP_MANIFEST}" | head -1 | sed 's/.*= *"\(.*\)"/\1/')"
DEFAULT_APP_NAME="Odd Box"
MACOS_BRANDING="${ODD_BOX_MACOS_BRANDING:-auto}"

if [[ "${VERSION}" == *-* && "${MACOS_BRANDING}" != "stable" ]]; then
    APP_NAME="Odd Box Preview"
    DEFAULT_BUNDLE_IDENTIFIER="se.twnet.oddbox-preview"
    DEFAULT_DMG_OUT="odd-box-preview.dmg"
else
    APP_NAME="${DEFAULT_APP_NAME}"
    DEFAULT_BUNDLE_IDENTIFIER="se.twnet.oddbox"
    DEFAULT_DMG_OUT="odd-box.dmg"
fi

DMG_OUT="${DMG_OUT:-${DEFAULT_DMG_OUT}}"
DEFAULT_BUNDLE_DIR="target/release/bundle/osx/${DEFAULT_APP_NAME}.app"
BUNDLE_DIR="target/release/bundle/osx/${APP_NAME}.app"
PLIST="${BUNDLE_DIR}/Contents/Info.plist"
BUNDLE_IDENTIFIER="${ODD_BOX_BUNDLE_IDENTIFIER:-${DEFAULT_BUNDLE_IDENTIFIER}}"

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

if [[ "${APP_NAME}" != "${DEFAULT_APP_NAME}" && -d "${DEFAULT_BUNDLE_DIR}" ]]; then
    rm -rf "${BUNDLE_DIR}"
    mv "${DEFAULT_BUNDLE_DIR}" "${BUNDLE_DIR}"
fi

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
/usr/libexec/PlistBuddy -c "Delete :CFBundleIdentifier" "${PLIST}" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string ${BUNDLE_IDENTIFIER}" "${PLIST}"
/usr/libexec/PlistBuddy -c "Delete :CFBundleName" "${PLIST}" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :CFBundleName string ${APP_NAME}" "${PLIST}"
/usr/libexec/PlistBuddy -c "Delete :CFBundleDisplayName" "${PLIST}" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :CFBundleDisplayName string ${APP_NAME}" "${PLIST}"

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
