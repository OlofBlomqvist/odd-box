#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
VERSION="$(grep '^version' "${REPO_ROOT}/Cargo.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')"
OUTPUT_DIR="${REPO_ROOT}/release-v${VERSION}/msi"
NO_BUILD=0
BINARY_PATH=""

usage() {
    cat <<'EOF'
Usage: packaging/windows/build-msi.sh [options]

Options:
  --no-build          Assume the Windows release binary already exists
  --binary <path>     Path to an existing odd-box.exe to package
  --output-dir <dir>  Copy final .msi artifacts into this directory
  --help              Show this help text
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

host_os="$(uname -s)"

is_windows_host() {
    case "${host_os}" in
        MINGW*|MSYS*|CYGWIN*)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

is_linux_host() {
    [[ "${host_os}" == "Linux" ]]
}

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "error: required command '$1' not found in PATH" >&2
        exit 1
    fi
}

msi_version_from_semver() {
    local raw="${1%%-*}"
    IFS='.' read -r major minor patch _ <<< "${raw}"
    major="${major:-0}"
    minor="${minor:-0}"
    patch="${patch:-0}"
    printf '%s.%s.%s\n' "${major}" "${minor}" "${patch}"
}

mkdir -p "${OUTPUT_DIR}"

if [[ ${NO_BUILD} -eq 0 ]]; then
    require_command cargo
    if is_windows_host; then
        cargo build --profile dist --target x86_64-pc-windows-msvc
        BINARY_PATH="${REPO_ROOT}/target/x86_64-pc-windows-msvc/dist/odd-box.exe"
    elif is_linux_host; then
        require_command cross
        cross build --profile dist --target x86_64-pc-windows-msvc
        BINARY_PATH="${REPO_ROOT}/target/x86_64-pc-windows-msvc/dist/odd-box.exe"
    else
        echo "error: MSI packaging is supported on Windows or Linux hosts" >&2
        exit 1
    fi
fi

if [[ -z "${BINARY_PATH}" ]]; then
    BINARY_PATH="${REPO_ROOT}/target/x86_64-pc-windows-msvc/dist/odd-box.exe"
fi

if [[ ! -f "${BINARY_PATH}" ]]; then
    echo "error: Windows binary not found at '${BINARY_PATH}'" >&2
    echo "hint: build it first or pass --binary <path> to an existing odd-box.exe" >&2
    exit 1
fi

require_command wixl

MSI_VERSION="$(msi_version_from_semver "${VERSION}")"
MSI_NAME="odd-box-${VERSION}-x64.msi"
WIXL_SOURCE_DIR="$(dirname "${BINARY_PATH}")"
WIXL_OUTPUT_PATH="${OUTPUT_DIR}/${MSI_NAME}"

wixl \
    -D SourceDir="${WIXL_SOURCE_DIR}" \
    -D Version="${MSI_VERSION}" \
    -o "${WIXL_OUTPUT_PATH}" \
    "${SCRIPT_DIR}/wixl/main.wxs"

echo "MSI package ready: ${WIXL_OUTPUT_PATH}"
