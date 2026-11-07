#!/usr/bin/env bash
# ==============================================================================
# Bilterm — Linux release binary builder & packager (run on a Linux host)
# ==============================================================================
# Cross-compiles (cargo-zigbuild) and packages the Linux release archives:
#   - x86_64-unknown-linux-gnu      (glibc >= 2.28)
#   - x86_64-unknown-linux-musl     (static Linux x86_64)
#   - aarch64-unknown-linux-musl    (static Linux ARM64)
#
# macOS and Windows archives are NOT built here: macOS needs the Apple SDK and
# Windows ships native MSVC builds. Both come from the `release-binaries`
# workflow (scripts/build-release-mac-win.sh).
#
# Usage:
#   ./scripts/build-release-assets.sh [vX.Y.Z] [--upload] [--no-build]
#
#   vX.Y.Z      Release tag (default: v<version in Cargo.toml>). Checked out clean
#               in a temporary git worktree so local changes never leak in.
#   --upload    Attach the Linux archives to the existing GitHub release and
#               merge their hashes into the release's SHA256SUMS. Run it AFTER
#               the release-binaries workflow finishes: both rewrite SHA256SUMS.
#   --no-build  Skip building; checksum/upload archives already in the dist dir.
#
#   CARGO_TARGET_DIR is honoured (keeps the build cache between runs).
#
# Output: target/release-dist/<tag>/ with versioned (`bilterm-<tag>-<triple>.tar.gz`)
# and unversioned installer aliases (`bilterm-<triple>.tar.gz`) plus SHA256SUMS.
# ==============================================================================

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="${BILTERM_GITHUB_REPO:-bsommers/bilterm}"
TAG=""; UPLOAD=0; BUILD=1
for a in "$@"; do
    case "$a" in
        --upload) UPLOAD=1 ;;
        --no-build) BUILD=0 ;;
        v*) TAG="$a" ;;
        *) echo "unknown argument: $a" >&2; exit 2 ;;
    esac
done
TAG="${TAG:-v$(awk -F'"' '/^version = / {print $2; exit}' "$REPO_ROOT/Cargo.toml")}"
DIST="$REPO_ROOT/target/release-dist/$TAG"

# Rust triple -> cargo-zigbuild target (glibc floor pinned for the gnu build).
TRIPLES=(x86_64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-musl)
zig_target() { if [[ "$1" == x86_64-unknown-linux-gnu ]]; then echo "$1.2.28"; else echo "$1"; fi; }

if [[ $BUILD -eq 1 ]]; then
    [[ "$(uname -s)" == Linux ]] || { echo "error: run this on Linux (macOS/Windows: release-binaries workflow)" >&2; exit 1; }

    # zig is the cross-linker for cargo-zigbuild. Resolution: PATH, then $ZIG_DIR, then ~/.local/bin.
    if ! command -v zig >/dev/null 2>&1; then
        for candidate_zig in "${ZIG_DIR:-}" "$HOME/.local/bin"; do
            if [[ -n "$candidate_zig" && -x "${candidate_zig}/zig" ]]; then
                export PATH="${candidate_zig}:$PATH"
                break
            fi
        done
    fi
    command -v zig >/dev/null 2>&1 || {
        echo "error: zig not found; install it on PATH or set ZIG_DIR=<dir containing zig>" >&2
        exit 1
    }
    command -v cargo-zigbuild >/dev/null 2>&1 || cargo install --locked cargo-zigbuild

    WT="$(mktemp -d)/src"
    trap 'git -C "$REPO_ROOT" worktree remove --force "$WT" 2>/dev/null || true' EXIT
    git -C "$REPO_ROOT" fetch --tags --quiet
    git -C "$REPO_ROOT" worktree add --detach "$WT" "$TAG" >/dev/null
    TGT="${CARGO_TARGET_DIR:-$WT/target}"

    rustup target add "${TRIPLES[@]}"
    rm -rf "$DIST"; mkdir -p "$DIST"
    for triple in "${TRIPLES[@]}"; do
        echo "[bilterm-release] Building ${triple}..."
        (cd "$WT" && cargo zigbuild --release --target "$(zig_target "$triple")")
        name="bilterm-$TAG-$triple"
        mkdir -p "$DIST/$name"
        cp "$TGT/$triple/release/bilterm" "$DIST/$name/bilterm"
        chmod 0755 "$DIST/$name/bilterm"
        cp "$WT"/{README.md,LICENSE,CHANGELOG.md} "$DIST/$name/"
        tar -czf "$DIST/$name.tar.gz" -C "$DIST" "$name"
        cp "$DIST/$name.tar.gz" "$DIST/bilterm-$triple.tar.gz"
        rm -rf "${DIST:?}/$name"
    done
fi

[[ -d "$DIST" ]] || { echo "error: $DIST does not exist (build first)" >&2; exit 1; }
new=$(cd "$DIST" && shopt -s nullglob && printf '%s\n' bilterm-*linux*.tar.gz)
[[ -n "$new" ]] || { echo "error: no Linux archives in $DIST" >&2; exit 1; }
# shellcheck disable=SC2086 # $new is a newline-separated list of plain file names
(cd "$DIST" && sha256sum $new > SHA256SUMS.linux)

if [[ $UPLOAD -eq 1 ]]; then
    # Merge with the checksums already on the release: drop stale lines for our
    # files (exact name match; '*' binary markers normalized), then append ours.
    rm -rf "$DIST/prev"
    gh release download "$TAG" -R "$REPO" -p SHA256SUMS -D "$DIST/prev" 2>/dev/null || true
    {
        if [[ -f "$DIST/prev/SHA256SUMS" ]]; then
            awk 'NR==FNR { mine[$2]=1; next } { f=$2; sub(/^\*/, "", f); if (!(f in mine)) print $1 "  " f }' \
                "$DIST/SHA256SUMS.linux" "$DIST/prev/SHA256SUMS"
        fi
        cat "$DIST/SHA256SUMS.linux"
    } | sort -k2 > "$DIST/SHA256SUMS"
    # shellcheck disable=SC2086
    (cd "$DIST" && gh release upload "$TAG" -R "$REPO" --clobber $new SHA256SUMS)
    echo "Uploaded to https://github.com/$REPO/releases/tag/$TAG"
else
    cp "$DIST/SHA256SUMS.linux" "$DIST/SHA256SUMS"
    echo "Built in $DIST (re-run with --upload --no-build to attach to the release)"
fi
ls -lh "$DIST"
