#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

OUT_DIR="$SCRIPT_DIR/target"
OUT_FILE="$OUT_DIR/odd-box-x86_64-linux-musl"

# cruma-sdk is a symlink in the repo (points to an absolute path on the
# developer's machine).  Docker BuildKit does not follow symlinks that
# resolve outside the build context root, so we resolve it here and pass
# the real directory in as a named build context.
CRUMA_SDK_REAL="$(realpath cruma-sdk)"

if [ ! -d "$CRUMA_SDK_REAL" ]; then
    echo "ERROR: cruma-sdk symlink resolves to '$CRUMA_SDK_REAL' which does not exist."
    exit 1
fi

echo "==> cruma-sdk resolved to: $CRUMA_SDK_REAL"

# --build-context doesn't accept a .dockerignore directly, so we drop a
# temporary one at the root of the real cruma-sdk directory and remove it
# when we're done.  This stops the 140+ GB target/ tree from being sucked
# into the Docker build context.
CRUMA_IGNORE="$CRUMA_SDK_REAL/.dockerignore"
CRUMA_IGNORE_CREATED=0

if [ ! -f "$CRUMA_IGNORE" ]; then
    cat > "$CRUMA_IGNORE" <<'EOF'
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
EOF
    CRUMA_IGNORE_CREATED=1
fi

cleanup() {
    if [ "$CRUMA_IGNORE_CREATED" -eq 1 ] && [ -f "$CRUMA_IGNORE" ]; then
        rm -f "$CRUMA_IGNORE"
    fi
    rm -rf "$OUT_DIR/_docker_out"
}
trap cleanup EXIT

echo "==> Building odd-box (x86_64-unknown-linux-musl) via Docker + Alpine..."

# BuildKit lets us:
#   1. Export a single file straight from the build stage — no docker create/cp/rm.
#   2. Inject the real cruma-sdk directory via --build-context so the Dockerfile
#      can COPY --from=cruma-sdk without ever following the symlink itself.
DOCKER_BUILDKIT=1 docker build \
    --file Dockerfile.build \
    --build-context "cruma-sdk=$CRUMA_SDK_REAL" \
    --target export \
    --output "type=local,dest=$OUT_DIR/_docker_out" \
    .

mkdir -p "$OUT_DIR"
mv "$OUT_DIR/_docker_out/odd-box" "$OUT_FILE"

# Strip debug symbols to shrink the binary.
if command -v strip &>/dev/null; then
    echo "==> Stripping debug symbols..."
    strip "$OUT_FILE"
fi

SIZE=$(du -sh "$OUT_FILE" | cut -f1)
echo ""
echo "==> Done!"
echo "    Binary : $OUT_FILE"
echo "    Size   : $SIZE"
