#!/usr/bin/env bash
set -euo pipefail

# Build odd-box release assets locally into release-v<version>/.
#
# Output names match GitHub release assets used by Homebrew generators:
#   odd-box-x86_64-unknown-linux-gnu
#   odd-box-x86_64-unknown-linux-musl
#   odd-box-aarch64-apple-darwin
#   odd-box-x86_64-apple-darwin
#   odd-box-aarch64-apple-darwin.dmg
#   odd-box-x86_64-apple-darwin.dmg
#
# Notes:
# - Windows asset is intentionally not built here.
# - DMG creation requires macOS (hdiutil).
# - Linux musl build uses ./build_static_linux_bin_with_docker.sh when available.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}"

CARGO_BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
if [[ -d "${CARGO_BIN_DIR}" && ":$PATH:" != *":${CARGO_BIN_DIR}:"* ]]; then
  export PATH="${CARGO_BIN_DIR}:$PATH"
fi

VERSION="$(sed -n 's/^version\s*=\s*"\(.*\)"/\1/p' Cargo.toml | head -1)"
if [[ -z "${VERSION}" ]]; then
  echo "error: failed to read version from Cargo.toml" >&2
  exit 1
fi

OUT_DIR="release-v${VERSION}"

ALL_TARGETS=(
  "x86_64-unknown-linux-gnu"
  "x86_64-unknown-linux-musl"
  "aarch64-apple-darwin"
  "x86_64-apple-darwin"
)

SELECTED_TARGETS=()
EXCLUDED_TARGETS=()
SKIP_EXISTING=0

usage() {
  cat <<'EOF'
Usage: ./build-release-artifacts.sh [--target <name>] [--targets <csv>] [--exclude <name>] [--skip-existing]

Supported target names:
  linux-gnu, linux-x86_64-gnu, x86_64-unknown-linux-gnu
  linux-musl, linux-x86_64-musl, x86_64-unknown-linux-musl
  macos-arm64, darwin-arm64, aarch64-apple-darwin
  macos-x86_64, darwin-x86_64, x86_64-apple-darwin
  macos, darwin, all

Examples:
  ./build-release-artifacts.sh
  ./build-release-artifacts.sh --target linux-musl
  ./build-release-artifacts.sh --targets macos,linux-musl
  ./build-release-artifacts.sh --exclude linux-gnu --skip-existing
EOF
}

normalize_target() {
  case "$1" in
    linux-gnu|linux-x86_64-gnu|x86_64-unknown-linux-gnu)
      echo "x86_64-unknown-linux-gnu"
      ;;
    linux-musl|linux-x86_64-musl|x86_64-unknown-linux-musl)
      echo "x86_64-unknown-linux-musl"
      ;;
    macos-arm64|darwin-arm64|aarch64-apple-darwin)
      echo "aarch64-apple-darwin"
      ;;
    macos-x86_64|darwin-x86_64|x86_64-apple-darwin)
      echo "x86_64-apple-darwin"
      ;;
    macos|darwin)
      echo "aarch64-apple-darwin,x86_64-apple-darwin"
      ;;
    all)
      echo "all"
      ;;
    *)
      return 1
      ;;
  esac
}

append_target() {
  local t="$1"
  for existing in "${SELECTED_TARGETS[@]:-}"; do
    [[ "${existing}" == "${t}" ]] && return
  done
  SELECTED_TARGETS+=("${t}")
}

append_excluded_target() {
  local t="$1"
  for existing in "${EXCLUDED_TARGETS[@]:-}"; do
    [[ "${existing}" == "${t}" ]] && return
  done
  EXCLUDED_TARGETS+=("${t}")
}

expand_and_append() {
  local mode="$1"
  local raw="$2"
  local normalized
  normalized="$(normalize_target "${raw}")" || {
    echo "error: unsupported target '${raw}'" >&2
    usage >&2
    exit 1
  }

  if [[ "${normalized}" == "all" ]]; then
    local t
    for t in "${ALL_TARGETS[@]}"; do
      if [[ "${mode}" == "include" ]]; then
        append_target "${t}"
      else
        append_excluded_target "${t}"
      fi
    done
    return
  fi

  IFS=',' read -r -a split_targets <<< "${normalized}"
  local t
  for t in "${split_targets[@]}"; do
    if [[ "${mode}" == "include" ]]; then
      append_target "${t}"
    else
      append_excluded_target "${t}"
    fi
  done
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target|-t)
      [[ $# -ge 2 ]] || { echo "error: --target requires a value" >&2; exit 1; }
      expand_and_append include "$2"
      shift 2
      ;;
    --targets)
      [[ $# -ge 2 ]] || { echo "error: --targets requires a value" >&2; exit 1; }
      IFS=',' read -r -a req <<< "$2"
      for r in "${req[@]}"; do
        r="${r//[[:space:]]/}"
        [[ -n "${r}" ]] && expand_and_append include "${r}"
      done
      shift 2
      ;;
    --exclude|-x)
      [[ $# -ge 2 ]] || { echo "error: --exclude requires a value" >&2; exit 1; }
      expand_and_append exclude "$2"
      shift 2
      ;;
    --skip-existing)
      SKIP_EXISTING=1
      shift
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

host_os="$(uname -s | tr '[:upper:]' '[:lower:]')"
host_arch="$(uname -m)"

if [[ ${#SELECTED_TARGETS[@]} -eq 0 ]]; then
  case "${host_os}:${host_arch}" in
    linux:x86_64)
      SELECTED_TARGETS=("x86_64-unknown-linux-gnu" "x86_64-unknown-linux-musl")
      ;;
    darwin:arm64|darwin:aarch64)
      SELECTED_TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin")
      ;;
    darwin:x86_64)
      SELECTED_TARGETS=("x86_64-apple-darwin")
      ;;
    *)
      SELECTED_TARGETS=("${ALL_TARGETS[@]}")
      ;;
  esac
fi

filtered=()
for t in "${SELECTED_TARGETS[@]}"; do
  skip=0
  for x in "${EXCLUDED_TARGETS[@]:-}"; do
    if [[ "${x}" == "${t}" ]]; then
      skip=1
      break
    fi
  done
  [[ ${skip} -eq 0 ]] && filtered+=("${t}")
done
SELECTED_TARGETS=("${filtered[@]}")

if [[ ${#SELECTED_TARGETS[@]} -eq 0 ]]; then
  echo "error: no targets selected" >&2
  exit 1
fi

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: required command '$1' not found in PATH" >&2
    exit 1
  }
}

release_name_for_target() {
  case "$1" in
    x86_64-unknown-linux-gnu) echo "odd-box-x86_64-unknown-linux-gnu" ;;
    x86_64-unknown-linux-musl) echo "odd-box-x86_64-unknown-linux-musl" ;;
    aarch64-apple-darwin) echo "odd-box-aarch64-apple-darwin" ;;
    x86_64-apple-darwin) echo "odd-box-x86_64-apple-darwin" ;;
    *) return 1 ;;
  esac
}

can_build_target_on_host() {
  local target="$1"
  case "${host_os}:${target}" in
    linux:x86_64-unknown-linux-gnu|linux:x86_64-unknown-linux-musl)
      return 0
      ;;
    darwin:aarch64-apple-darwin|darwin:x86_64-apple-darwin)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

copy_built_binary() {
  local target="$1"
  local src="$2"
  local name
  name="$(release_name_for_target "${target}")"
  local dst="${OUT_DIR}/${name}"
  cp "${src}" "${dst}"
  chmod +x "${dst}" || true
  echo "  -> ${dst}"
}

build_target() {
  local target="$1"
  local asset
  asset="$(release_name_for_target "${target}")"
  local final_path="${OUT_DIR}/${asset}"

  if [[ ${SKIP_EXISTING} -eq 1 && -f "${final_path}" ]]; then
    echo "[skip] ${target} (exists: ${final_path})"
    return
  fi

  if ! can_build_target_on_host "${target}"; then
    echo "[skip] ${target} (unsupported on host ${host_os}/${host_arch})"
    return
  fi

  case "${target}" in
    x86_64-unknown-linux-musl)
      echo "[build] ${target} via Docker helper"
      require_command docker
      ./build_static_linux_bin_with_docker.sh
      copy_built_binary "${target}" "${SCRIPT_DIR}/target/odd-box-x86_64-linux-musl"
      ;;
    *)
      echo "[build] ${target} via cargo"
      require_command cargo
      require_command rustup
      rustup target add "${target}" >/dev/null
      cargo build --release --target "${target}"
      copy_built_binary "${target}" "${SCRIPT_DIR}/target/${target}/release/odd-box"
      ;;
  esac
}

build_dmg_for_target() {
  local target="$1"
  local asset
  asset="$(release_name_for_target "${target}")"
  local bin_path="${OUT_DIR}/${asset}"
  local dmg_path="${OUT_DIR}/${asset}.dmg"

  if [[ ! -f "${bin_path}" ]]; then
    return
  fi

  if [[ ${SKIP_EXISTING} -eq 1 && -f "${dmg_path}" ]]; then
    echo "[skip] ${asset}.dmg (exists)"
    return
  fi

  if [[ "${host_os}" != "darwin" ]]; then
    echo "[skip] ${asset}.dmg (DMG creation requires macOS/hdiutil)"
    return
  fi

  require_command hdiutil

  local tmp_dir
  tmp_dir="$(mktemp -d)"
  cp "${bin_path}" "${tmp_dir}/odd-box"
  chmod +x "${tmp_dir}/odd-box"

  echo "[dmg] ${asset}.dmg"
  hdiutil create -volname "odd-box" -srcfolder "${tmp_dir}" -ov -format UDZO "${dmg_path}" >/dev/null
  rm -rf "${tmp_dir}"
}

mkdir -p "${OUT_DIR}"

echo "==> odd-box ${VERSION}"
echo "==> output: ${OUT_DIR}"
echo "==> targets: ${SELECTED_TARGETS[*]}"

for t in "${SELECTED_TARGETS[@]}"; do
  build_target "${t}"
done

for t in "${SELECTED_TARGETS[@]}"; do
  case "${t}" in
    aarch64-apple-darwin|x86_64-apple-darwin)
      build_dmg_for_target "${t}"
      ;;
  esac
done

echo
echo "==> Done. Artifacts in ${OUT_DIR}:"
find "${OUT_DIR}" -maxdepth 1 -type f -printf "  %f\n" | sort
