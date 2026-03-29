#!/usr/bin/env bash
set -euo pipefail

# Build odd-box release artifacts into release-v<version>/.
#
# Default artifacts mirror current CI/release outputs:
#   odd-box-x86_64-unknown-linux-musl
#   odd-box-aarch64-unknown-linux-musl
#   odd-box-x86_64-pc-windows-msvc.exe
#   odd-box-x86_64-apple-darwin.dmg
#   odd-box-aarch64-apple-darwin.dmg
#
# Optional:
#   odd-box-x86_64-unknown-linux-gnu
#   odd-box-x86_64-pc-windows-gnu.exe
#   odd-box-x86_64-apple-darwin

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}"

CARGO_BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
if [[ -d "${CARGO_BIN_DIR}" && ":$PATH:" != *":${CARGO_BIN_DIR}:"* ]]; then
  export PATH="${CARGO_BIN_DIR}:$PATH"
fi

VERSION="$(
  awk -F'"' '/^version[[:space:]]*=/ {print $2; exit}' Cargo.toml
)"
if [[ -z "${VERSION}" ]]; then
  VERSION="$(
    awk -F"'" '/^version[[:space:]]*=/ {print $2; exit}' Cargo.toml
  )"
fi
if [[ -z "${VERSION}" ]]; then
  echo "error: failed to read version from Cargo.toml" >&2
  exit 1
fi

OUT_DIR="release-v${VERSION}"

ALL_TARGETS=(
  "x86_64-unknown-linux-gnu"
  "x86_64-unknown-linux-musl"
  "aarch64-unknown-linux-musl"
  "x86_64-pc-windows-msvc"
  "x86_64-pc-windows-gnu"
  "x86_64-apple-darwin-dmg"
  "x86_64-apple-darwin"
  "aarch64-apple-darwin-dmg"
)

DEFAULT_TARGETS=(
  "x86_64-unknown-linux-musl"
  "aarch64-unknown-linux-musl"
  "x86_64-pc-windows-msvc"
  "x86_64-apple-darwin-dmg"
  "aarch64-apple-darwin-dmg"
)

SELECTED_TARGETS=()
EXCLUDED_TARGETS=()
SKIP_EXISTING=0

usage() {
  cat <<'USAGE'
Usage: ./build-release-artifacts.sh [--target <name>] [--targets <csv>] [--exclude <name>] [--skip-existing]

Supported target names:
  linux-gnu, linux-x86_64-gnu, x86_64-unknown-linux-gnu
  linux-musl, linux-x86_64-musl, x86_64-unknown-linux-musl
  linux-musl-arm64, linux-aarch64-musl, aarch64-unknown-linux-musl
  windows-msvc, windows-x86_64-msvc, x86_64-pc-windows-msvc
  windows-gnu, windows-x86_64-gnu, x86_64-pc-windows-gnu
  macos-x86_64-dmg, darwin-x86_64-dmg, x86_64-apple-darwin-dmg
  macos-x86_64, darwin-x86_64, x86_64-apple-darwin
  macos-arm64-dmg, darwin-arm64-dmg, aarch64-apple-darwin-dmg
  macos, linux-musl-all, release, all

Examples:
  ./build-release-artifacts.sh
  ./build-release-artifacts.sh --target linux-musl
  ./build-release-artifacts.sh --targets macos,linux-musl-all
  ./build-release-artifacts.sh --exclude linux-gnu --skip-existing
USAGE
}

normalize_target() {
  case "$1" in
    linux-gnu|linux-x86_64-gnu|x86_64-unknown-linux-gnu)
      echo "x86_64-unknown-linux-gnu"
      ;;
    linux-musl|linux-x86_64-musl|x86_64-unknown-linux-musl)
      echo "x86_64-unknown-linux-musl"
      ;;
    linux-musl-arm64|linux-aarch64-musl|aarch64-unknown-linux-musl)
      echo "aarch64-unknown-linux-musl"
      ;;
    windows-msvc|windows-x86_64-msvc|x86_64-pc-windows-msvc)
      echo "x86_64-pc-windows-msvc"
      ;;
    windows-gnu|windows-x86_64-gnu|x86_64-pc-windows-gnu)
      echo "x86_64-pc-windows-gnu"
      ;;
    macos-x86_64-dmg|darwin-x86_64-dmg|x86_64-apple-darwin-dmg)
      echo "x86_64-apple-darwin-dmg"
      ;;
    macos-x86_64|darwin-x86_64|x86_64-apple-darwin)
      echo "x86_64-apple-darwin"
      ;;
    macos-arm64-dmg|darwin-arm64-dmg|aarch64-apple-darwin-dmg)
      echo "aarch64-apple-darwin-dmg"
      ;;
    macos|darwin)
      echo "x86_64-apple-darwin-dmg,aarch64-apple-darwin-dmg"
      ;;
    linux-musl-all)
      echo "x86_64-unknown-linux-musl,aarch64-unknown-linux-musl"
      ;;
    release)
      echo "x86_64-unknown-linux-musl,aarch64-unknown-linux-musl,x86_64-pc-windows-msvc,x86_64-apple-darwin-dmg,aarch64-apple-darwin-dmg"
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
  SELECTED_TARGETS=("${DEFAULT_TARGETS[@]}")
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

dockerignore_excludes_path() {
  local dockerignore_path="$1"
  local path_name="$2"

  if [[ ! -f "${dockerignore_path}" ]]; then
    return 1
  fi

  grep -Eq "^[[:space:]]*(${path_name}|${path_name}/|${path_name}/\\*\\*|\\*\\*/${path_name}|\\*\\*/${path_name}/|\\*\\*/${path_name}/\\*\\*)[[:space:]]*$" "${dockerignore_path}"
}

warn_if_large_cruma_paths_not_ignored() {
  local cruma_sdk_real="$1"
  local cruma_ignore="$2"
  local missing=()
  local path_name size

  for path_name in target target-cross .git; do
    if dockerignore_excludes_path "${cruma_ignore}" "${path_name}"; then
      continue
    fi

    if [[ -e "${cruma_sdk_real}/${path_name}" ]]; then
      size="$(du -sh "${cruma_sdk_real}/${path_name}" 2>/dev/null | cut -f1 || true)"
      missing+=("${path_name}${size:+ (${size})}")
    fi
  done

  if [[ ${#missing[@]} -eq 0 ]]; then
    return 0
  fi

  echo "warning: ${cruma_ignore} does not clearly exclude large Docker context paths: ${missing[*]}" >&2
  echo "warning: add explicit root entries like 'target', 'target/**', 'target-cross', 'target-cross/**', '.git', '.git/**'" >&2
}

release_name_for_target() {
  case "$1" in
    x86_64-unknown-linux-gnu) echo "odd-box-x86_64-unknown-linux-gnu" ;;
    x86_64-unknown-linux-musl) echo "odd-box-x86_64-unknown-linux-musl" ;;
    aarch64-unknown-linux-musl) echo "odd-box-aarch64-unknown-linux-musl" ;;
    x86_64-pc-windows-msvc) echo "odd-box-x86_64-pc-windows-msvc.exe" ;;
    x86_64-pc-windows-gnu) echo "odd-box-x86_64-pc-windows-gnu.exe" ;;
    x86_64-apple-darwin-dmg) echo "odd-box-x86_64-apple-darwin.dmg" ;;
    x86_64-apple-darwin) echo "odd-box-x86_64-apple-darwin" ;;
    aarch64-apple-darwin-dmg) echo "odd-box-aarch64-apple-darwin.dmg" ;;
    *) return 1 ;;
  esac
}

has_cargo_xwin() {
  command -v cargo-xwin >/dev/null 2>&1
}

has_mingw_linker() {
  command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1
}

can_build_target_on_host() {
  local target="$1"
  case "${target}" in
    x86_64-unknown-linux-gnu)
      [[ "${host_os}" == "linux" ]]
      ;;
    x86_64-pc-windows-msvc)
      [[ "${host_os}" == mingw* || "${host_os}" == msys* || "${host_os}" == cygwin* ]] || has_cargo_xwin
      ;;
    x86_64-pc-windows-gnu)
      [[ "${host_os}" == mingw* || "${host_os}" == msys* || "${host_os}" == cygwin* ]] || has_mingw_linker
      ;;
    x86_64-apple-darwin-dmg)
      [[ "${host_os}" == "darwin" && ( "${host_arch}" == "x86_64" || "${host_arch}" == "amd64" ) ]]
      ;;
    x86_64-apple-darwin)
      [[ "${host_os}" == "darwin" ]]
      ;;
    aarch64-apple-darwin-dmg)
      [[ "${host_os}" == "darwin" && ( "${host_arch}" == "arm64" || "${host_arch}" == "aarch64" ) ]]
      ;;
    x86_64-unknown-linux-musl|aarch64-unknown-linux-musl)
      [[ "${host_os}" == "linux" || "${host_os}" == "darwin" ]]
      ;;
    *)
      return 1
      ;;
  esac
}

cleanup_docker_build() {
  local cruma_ignore_path="$1"
  local docker_out_path="$2"
  local cruma_ignore_was_created="$3"

  if [[ "${cruma_ignore_was_created}" -eq 1 && -f "${cruma_ignore_path}" ]]; then
    rm -f "${cruma_ignore_path}"
  fi
  rm -rf "${docker_out_path}"
}

docker_supports_platform() {
  local platform="$1"
  local info

  if ! info="$(docker version --format '{{json .Server.Arch}} {{json .Server.Os}}' 2>/dev/null)" || [[ -z "${info}" ]]; then
    return 1
  fi

  case "${platform}" in
    linux/amd64)
      [[ "${info}" == *'"amd64"'* && "${info}" == *'"linux"'* ]]
      ;;
    linux/arm64)
      if [[ "${info}" == *'"arm64"'* && "${info}" == *'"linux"'* ]]; then
        return 0
      fi
      docker buildx inspect 2>/dev/null | grep -Eq 'Platforms:.*linux/arm64'
      ;;
    *)
      return 1
      ;;
  esac
}

build_linux_musl_with_docker() {
  local target="$1"
  local output_path="$2"

  require_command docker

  local cruma_sdk_real
  cruma_sdk_real="$(realpath cruma-sdk)"
  if [[ ! -d "${cruma_sdk_real}" ]]; then
    echo "error: cruma-sdk symlink resolves to '${cruma_sdk_real}' which does not exist" >&2
    exit 1
  fi

  local cruma_ignore="${cruma_sdk_real}/.dockerignore"
  local cruma_ignore_created=0

  if [[ ! -f "${cruma_ignore}" ]]; then
    cat > "${cruma_ignore}" <<'IGNORE'
**/target
**/.git
**/.DS_Store
**/node_modules
**/.cruma-tunnel-cache
**/.odd-box-cruma-cache
**/.h2t_cache
**/.h2t_db
**/.acme_cache
odd-box
IGNORE
    cruma_ignore_created=1
  fi

  warn_if_large_cruma_paths_not_ignored "${cruma_sdk_real}" "${cruma_ignore}"

  local docker_out="${SCRIPT_DIR}/target/_docker_out_${target}"
  rm -rf "${docker_out}"
  trap 'cleanup_docker_build "$cruma_ignore" "$docker_out" "$cruma_ignore_created"' EXIT

  if [[ "${target}" == "aarch64-unknown-linux-musl" ]]; then
    if ! docker_supports_platform "linux/arm64"; then
      echo "error: Docker on this host is not ready to run linux/arm64 build steps." >&2
      echo "error: building ${target} from $(uname -s)/$(uname -m) requires arm64 support via Docker Desktop or binfmt/QEMU." >&2
      echo "error: verify 'docker buildx inspect --bootstrap' reports linux/arm64 in Platforms, then retry." >&2
      echo "error: on a typical Linux Docker host you can enable it with:" >&2
      echo "error:   docker run --privileged --rm tonistiigi/binfmt --install arm64" >&2
      echo "error:   docker buildx inspect --bootstrap" >&2
      exit 1
    fi

    DOCKER_BUILDKIT=1 docker build \
      --platform linux/arm64 \
      --file Dockerfile.build \
      --build-context "cruma-sdk=${cruma_sdk_real}" \
      --build-arg "RUST_TARGET=${target}" \
      --target export \
      --output "type=local,dest=${docker_out}" \
      .
  else
    DOCKER_BUILDKIT=1 docker build \
      --file Dockerfile.build \
      --build-context "cruma-sdk=${cruma_sdk_real}" \
      --build-arg "RUST_TARGET=${target}" \
      --target export \
      --output "type=local,dest=${docker_out}" \
      .
  fi

  mkdir -p "$(dirname "${output_path}")"
  mv "${docker_out}/odd-box" "${output_path}"
  chmod +x "${output_path}" || true

  if command -v strip >/dev/null 2>&1 && [[ "${target}" != "aarch64-unknown-linux-musl" ]]; then
    strip "${output_path}" || true
  fi

  trap - EXIT
  cleanup_docker_build "${cruma_ignore}" "${docker_out}" "${cruma_ignore_created}"
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
    if [[ "${target}" == "x86_64-pc-windows-msvc" ]]; then
      echo "[skip] ${target} (requires Windows host or cargo-xwin for cross-build on ${host_os}/${host_arch})"
    elif [[ "${target}" == "x86_64-pc-windows-gnu" ]]; then
      echo "[skip] ${target} (requires Windows host or x86_64-w64-mingw32-gcc for cross-build on ${host_os}/${host_arch})"
    else
      echo "[skip] ${target} (unsupported on host ${host_os}/${host_arch})"
    fi
    return
  fi

  case "${target}" in
    x86_64-unknown-linux-musl|aarch64-unknown-linux-musl)
      echo "[build] ${target} via Docker"
      build_linux_musl_with_docker "${target}" "${final_path}"
      ;;
    x86_64-apple-darwin-dmg|aarch64-apple-darwin-dmg)
      echo "[build] ${target} via pack-osx.sh"
      require_command bash
      DMG_OUT="${SCRIPT_DIR}/${final_path}" bash pack-osx.sh
      ;;
    *)
      echo "[build] ${target} via cargo"
      require_command cargo
      require_command rustup
      rustup target add "${target}" >/dev/null
      if [[ "${target}" == "x86_64-pc-windows-msvc" && "${host_os}" != mingw* && "${host_os}" != msys* && "${host_os}" != cygwin* ]]; then
        if has_cargo_xwin; then
          cargo xwin build --release --target "${target}"
        else
          echo "error: cargo-xwin not found; cannot build ${target} on ${host_os}/${host_arch}" >&2
          exit 1
        fi
      else
        cargo build --release --target "${target}"
      fi
      mkdir -p "${OUT_DIR}"
      if [[ "${target}" == "x86_64-pc-windows-msvc" || "${target}" == "x86_64-pc-windows-gnu" ]]; then
        cp "${SCRIPT_DIR}/target/${target}/release/odd-box.exe" "${final_path}"
      else
        cp "${SCRIPT_DIR}/target/${target}/release/odd-box" "${final_path}"
        chmod +x "${final_path}" || true
      fi
      ;;
  esac
}

mkdir -p "${OUT_DIR}"

echo "==> odd-box ${VERSION}"
echo "==> output: ${OUT_DIR}"
echo "==> targets: ${SELECTED_TARGETS[*]}"

for t in "${SELECTED_TARGETS[@]}"; do
  build_target "${t}"
done

echo
echo "==> Done. Artifacts in ${OUT_DIR}:"
if compgen -G "${OUT_DIR}/*" >/dev/null; then
  find "${OUT_DIR}" -maxdepth 1 -type f | sed "s#^${OUT_DIR}/#  #" | sort
else
  echo "  (none)"
fi
