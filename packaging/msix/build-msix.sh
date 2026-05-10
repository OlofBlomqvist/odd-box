#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
VERSION="$(grep '^version' "${REPO_ROOT}/Cargo.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')"
OUTPUT_DIR="${REPO_ROOT}/release-v${VERSION}/msix"
NO_BUILD=0
BINARY_PATH=""

usage() {
    cat <<'EOF'
Usage: packaging/msix/build-msix.sh [options]

Options:
  --no-build          Assume the Windows release binary already exists
  --binary <path>     Path to an existing odd-box.exe to package
  --output-dir <dir>  Copy final .msix artifacts into this directory
  --help              Show this help text

Optional environment variables:
  MSIX_SDK_ROOT               Path to a built msix-packaging checkout
  ODDBOX_MSIX_IDENTITY_NAME
  ODDBOX_MSIX_PUBLISHER
  ODDBOX_MSIX_PUBLISHER_DISPLAY_NAME
  ODDBOX_MSIX_DISPLAY_NAME
  ODDBOX_MSIX_DESCRIPTION
  ODDBOX_MSIX_BACKGROUND_COLOR
  ODDBOX_MSIX_APPLICATION_ID
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-build)
            NO_BUILD=1
            shift
            ;;
        --binary)
            BINARY_PATH="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown argument '$1'" >&2
            usage >&2
            exit 1
            ;;
    esac
done

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "error: required command '$1' not found in PATH" >&2
        exit 1
    fi
}

has_cargo_xwin() {
    command -v cargo-xwin >/dev/null 2>&1
}

prepend_path() {
    local dir="$1"
    [[ -d "${dir}" ]] || return 0
    case ":${PATH}:" in
        *":${dir}:"*) ;;
        *) PATH="${dir}:${PATH}"; export PATH ;;
    esac
}

prepend_ld_library_path() {
    local dir="$1"
    [[ -d "${dir}" ]] || return 0
    case ":${LD_LIBRARY_PATH:-}:" in
        *":${dir}:"*) ;;
        *) LD_LIBRARY_PATH="${dir}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"; export LD_LIBRARY_PATH ;;
    esac
}

configure_makemsix_env() {
    local roots=(
        "${MSIX_SDK_ROOT:-}"
        "/home/zdx/repos/msix-packaging"
        "${REPO_ROOT}/../msix-packaging"
    )
    for root in "${roots[@]}"; do
        [[ -n "${root}" ]] || continue
        prepend_path "${root}/.vs/bin"
        prepend_path "${root}/.vs/src"
        prepend_ld_library_path "${root}/.vs/lib"
    done
}

host_os="$(uname -s)"
host_arch="$(uname -m)"
if [[ "${host_os}" != "Linux" || "${host_arch}" != "x86_64" ]]; then
    echo "error: MSIX packaging currently expects a Linux x86_64 host" >&2
    exit 1
fi

configure_makemsix_env

require_command makemsix

semver_to_quad() {
    local raw="${1%%-*}"
    IFS='.' read -r major minor patch _ <<< "${raw}"
    major="${major:-0}"
    minor="${minor:-0}"
    patch="${patch:-0}"
    printf '%s.%s.%s.0\n' "${major}" "${minor}" "${patch}"
}

IDENTITY_NAME="${ODDBOX_MSIX_IDENTITY_NAME:-OlofBlomqvist.OddBox}"
DISPLAY_NAME="${ODDBOX_MSIX_DISPLAY_NAME:-Odd Box}"
PUBLISHER_DISPLAY_NAME="${ODDBOX_MSIX_PUBLISHER_DISPLAY_NAME:-OlofBlomqvist}"
DESCRIPTION="${ODDBOX_MSIX_DESCRIPTION:-A dead simple reverse proxy and web server}"
BACKGROUND_COLOR="${ODDBOX_MSIX_BACKGROUND_COLOR:-transparent}"
APPLICATION_ID="${ODDBOX_MSIX_APPLICATION_ID:-OddBox}"
PACKAGE_VERSION="$(semver_to_quad "${VERSION}")"
PUBLISHER_NAME="${ODDBOX_MSIX_PUBLISHER:-CN=OlofBlomqvist}"

echo "MSIX identity:"
echo "  Name=${IDENTITY_NAME}"
echo "  Publisher=${PUBLISHER_NAME}"
echo "  PublisherDisplayName=${PUBLISHER_DISPLAY_NAME}"

mkdir -p "${OUTPUT_DIR}"

if [[ ${NO_BUILD} -eq 0 ]]; then
    if has_cargo_xwin; then
        cargo xwin build --profile dist --target x86_64-pc-windows-msvc
    else
        echo "error: cargo-xwin is required to cross-build x86_64-pc-windows-msvc for MSIX packaging" >&2
        exit 1
    fi
    BINARY_PATH="${REPO_ROOT}/target/x86_64-pc-windows-msvc/dist/odd-box.exe"
fi

if [[ -z "${BINARY_PATH}" ]]; then
    BINARY_PATH="${REPO_ROOT}/target/x86_64-pc-windows-msvc/dist/odd-box.exe"
fi

if [[ ! -f "${BINARY_PATH}" ]]; then
    echo "error: Windows binary not found at '${BINARY_PATH}'" >&2
    echo "hint: build it first or pass --binary <path> to an existing odd-box.exe" >&2
    exit 1
fi

STAGING_DIR="${OUTPUT_DIR}/_staging"
ASSETS_DIR="${STAGING_DIR}/Assets"
rm -rf "${STAGING_DIR}"
mkdir -p "${ASSETS_DIR}"

cp "${BINARY_PATH}" "${STAGING_DIR}/odd-box.exe"
cp "${REPO_ROOT}/icons/icon.png" "${ASSETS_DIR}/Square150x150Logo.png"
cp "${REPO_ROOT}/icons/icon.png" "${ASSETS_DIR}/Square44x44Logo.png"

cat > "${STAGING_DIR}/AppxManifest.xml" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap10 rescap">
  <Identity
    Name="${IDENTITY_NAME}"
    Version="${PACKAGE_VERSION}"
    Publisher="${PUBLISHER_NAME}"
    ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>${DISPLAY_NAME}</DisplayName>
    <PublisherDisplayName>${PUBLISHER_DISPLAY_NAME}</PublisherDisplayName>
    <Description>${DESCRIPTION}</Description>
    <Logo>Assets\Square44x44Logo.png</Logo>
  </Properties>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.22621.0" />
  </Dependencies>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
  <Applications>
    <Application
      Id="${APPLICATION_ID}"
      Executable="odd-box.exe"
      uap10:RuntimeBehavior="packagedClassicApp"
      uap10:TrustLevel="mediumIL">
      <uap:VisualElements
        BackgroundColor="${BACKGROUND_COLOR}"
        DisplayName="${DISPLAY_NAME}"
        Description="${DESCRIPTION}"
        Square150x150Logo="Assets\Square150x150Logo.png"
        Square44x44Logo="Assets\Square44x44Logo.png" />
    </Application>
  </Applications>
</Package>
EOF

MSIX_NAME="odd-box-${VERSION}-x64.msix"
MSIX_OUTPUT_PATH="${OUTPUT_DIR}/${MSIX_NAME}"
rm -f "${MSIX_OUTPUT_PATH}"

makemsix pack -d "${STAGING_DIR}" -p "${MSIX_OUTPUT_PATH}"

echo "MSIX package ready: ${MSIX_OUTPUT_PATH}"
