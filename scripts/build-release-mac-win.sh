#!/usr/bin/env bash
# ==============================================================================
# Bilterm — Build & attach macOS / Windows release archives (run on the right OS)
# ==============================================================================
# The Linux-hosted `build-release-assets.sh` (cargo-zigbuild) cannot link the
# native macOS GUI (needs CoreFoundation/AppKit/QuartzCore from a macOS SDK).
# Run this script ON A MAC to build and attach the macOS archives to an existing
# GitHub release; run it with `--windows` in Git Bash / MSYS2 on Windows to build
# the native MSVC archive.
#
# Usage:
#   ./scripts/build-release-mac-win.sh [vX.Y.Z] [--upload] [--windows]
#
#   vX.Y.Z     Release tag (default: v<version in Cargo.toml>). Checked out clean
#              in a temporary git worktree so local changes never leak in.
#   --upload   Run `gh release upload --clobber` and refresh SHA256SUMS.
#   --windows  Build x86_64-pc-windows-msvc instead of the macOS targets.
#
# Output: target/release-dist/<tag>/ (same names the installers expect:
#   bilterm-<triple>.tar.gz|zip  and  bilterm-<tag>-<triple>.tar.gz|zip)
# ==============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="${BILTERM_GITHUB_REPO:-bsommers/bilterm}"
TAG=""; UPLOAD=0; WINDOWS=0
for a in "$@"; do
    case "$a" in
        --upload) UPLOAD=1 ;;
        --windows) WINDOWS=1 ;;
        v*) TAG="$a" ;;
        *) echo "unknown argument: $a" >&2; exit 2 ;;
    esac
done
TAG="${TAG:-v$(awk -F'"' '/^version = / {print $2; exit}' "$REPO_ROOT/Cargo.toml")}"

if [[ $WINDOWS -eq 0 && "$(uname -s)" != "Darwin" ]]; then
    echo "error: macOS targets must be built on macOS (needs the macOS SDK)." >&2
    exit 1
fi

DIST="$REPO_ROOT/target/release-dist/$TAG"
WT="$(mktemp -d)/src"
trap 'git -C "$REPO_ROOT" worktree remove --force "$WT" 2>/dev/null || true' EXIT
git -C "$REPO_ROOT" fetch --tags --quiet
git -C "$REPO_ROOT" worktree add --detach "$WT" "$TAG" >/dev/null
mkdir -p "$DIST"

pkg_unix() {
    local triple="$1" name="bilterm-$TAG-$1"
    rustup target add "$triple"
    (cd "$WT" && cargo build --release --target "$triple")
    mkdir -p "$DIST/$name"
    cp "$WT/target/$triple/release/bilterm" "$DIST/$name/"
    chmod 0755 "$DIST/$name/bilterm"
    cp "$WT"/{README.md,LICENSE,CHANGELOG.md} "$DIST/$name/"
    tar -czf "$DIST/$name.tar.gz" -C "$DIST" "$name"
    cp "$DIST/$name.tar.gz" "$DIST/bilterm-$triple.tar.gz"
    rm -rf "$DIST/$name"
}

pkg_windows() {
    local triple="x86_64-pc-windows-msvc" name="bilterm-$TAG-x86_64-pc-windows-msvc"
    rustup target add "$triple"
    (cd "$WT" && cargo build --release --target "$triple")
    mkdir -p "$DIST/$name"
    cp "$WT/target/$triple/release/bilterm.exe" "$DIST/$name/"
    cp "$WT"/{README.md,LICENSE,CHANGELOG.md} "$DIST/$name/"
    local d="$DIST"; command -v cygpath >/dev/null && d="$(cygpath -m "$DIST")"   # Git Bash -> native Windows path
    "${PYTHON:-$(command -v python3 || command -v python)}" -c "import shutil; shutil.make_archive('$d/$name','zip','$d','$name')"
    cp "$DIST/$name.zip" "$DIST/bilterm-$triple.zip"
    rm -rf "$DIST/$name"
}

if [[ $WINDOWS -eq 1 ]]; then pkg_windows; else pkg_unix aarch64-apple-darwin; pkg_unix x86_64-apple-darwin; fi

if [[ $UPLOAD -eq 1 ]]; then
    # Merge with the checksums already on the release, then upload new assets.
    gh release download "$TAG" -R "$REPO" -p SHA256SUMS -D "$DIST/prev" 2>/dev/null || true
    # Only this run's platform; nullglob so a missing .zip/.tar.gz isn't fatal.
    new=$(cd "$DIST" && shopt -s nullglob && printf '%s\n' bilterm-*.tar.gz bilterm-*.zip \
        | grep "$([[ $WINDOWS -eq 1 ]] && echo windows || echo apple)")
    (cd "$DIST" && shasum -a 256 $new > SHA256SUMS.new)
    { grep -v -F -f <(awk '{print $2}' "$DIST/SHA256SUMS.new") "$DIST/prev/SHA256SUMS" 2>/dev/null || true
      cat "$DIST/SHA256SUMS.new"; } > "$DIST/SHA256SUMS"
    (cd "$DIST" && gh release upload "$TAG" -R "$REPO" --clobber $new SHA256SUMS)
    echo "Uploaded to https://github.com/$REPO/releases/tag/$TAG"
else
    echo "Built in $DIST (re-run with --upload to attach to the release)"
fi
