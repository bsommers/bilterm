#!/usr/bin/env bash
# ==============================================================================
# Bilterm — Build & attach macOS / Windows release archives (run on the right OS)
# ==============================================================================
# The Linux-hosted `build-release-assets.sh` (cargo-zigbuild) cannot link the
# native macOS GUI (needs CoreFoundation/AppKit/QuartzCore from a macOS SDK).
# Run this script ON A MAC to build and attach the macOS archives to an existing
# GitHub release; run it with `--windows` / `--windows-arm64` in Git Bash on
# Windows to build the native MSVC archives (either arch builds on either host
# when the VS ARM64 + x64 toolsets are installed). From a Mac, use
# `build-windows-vm.sh` to drive a Windows 11 ARM VM (win-devvm) instead.
#
# Usage:
#   ./scripts/build-release-mac-win.sh [vX.Y.Z] [--upload] [--windows] [--windows-arm64] [--no-build]
#
#   vX.Y.Z           Release tag (default: v<version in Cargo.toml>). Checked out clean
#                    in a temporary git worktree so local changes never leak in.
#   --upload         Run `gh release upload --clobber` and refresh SHA256SUMS.
#   --windows        Build x86_64-pc-windows-msvc instead of the macOS targets.
#   --windows-arm64  Build aarch64-pc-windows-msvc (combinable with --windows).
#   --no-build       Skip building; upload archives already in the dist dir
#                    (e.g. pulled back from a VM). Implies nothing without --upload.
#
#   CARGO_TARGET_DIR is honoured (keeps the build cache between runs).
#
# Output: target/release-dist/<tag>/ (same names the installers expect:
#   bilterm-<triple>.tar.gz|zip  and  bilterm-<tag>-<triple>.tar.gz|zip)
# ==============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="${BILTERM_GITHUB_REPO:-bsommers/bilterm}"
TAG=""; UPLOAD=0; BUILD=1; WIN_TRIPLES=()
for a in "$@"; do
    case "$a" in
        --upload) UPLOAD=1 ;;
        --no-build) BUILD=0 ;;
        --windows) WIN_TRIPLES+=(x86_64-pc-windows-msvc) ;;
        --windows-arm64) WIN_TRIPLES+=(aarch64-pc-windows-msvc) ;;
        v*) TAG="$a" ;;
        *) echo "unknown argument: $a" >&2; exit 2 ;;
    esac
done
TAG="${TAG:-v$(awk -F'"' '/^version = / {print $2; exit}' "$REPO_ROOT/Cargo.toml")}"
WINDOWS=0; [[ ${#WIN_TRIPLES[@]} -gt 0 ]] && WINDOWS=1

if [[ $BUILD -eq 1 && $WINDOWS -eq 0 && "$(uname -s)" != "Darwin" ]]; then
    echo "error: macOS targets must be built on macOS (needs the macOS SDK)." >&2
    exit 1
fi

DIST="$REPO_ROOT/target/release-dist/$TAG"
mkdir -p "$DIST"
if [[ $BUILD -eq 1 ]]; then
    WT="$(mktemp -d)/src"
    trap 'git -C "$REPO_ROOT" worktree remove --force "$WT" 2>/dev/null || true' EXIT
    git -C "$REPO_ROOT" fetch --tags --quiet
    git -C "$REPO_ROOT" worktree add --detach "$WT" "$TAG" >/dev/null
    TGT="${CARGO_TARGET_DIR:-$WT/target}"
fi

pkg_unix() {
    local triple="$1" name="bilterm-$TAG-$1"
    rustup target add "$triple"
    (cd "$WT" && cargo build --release --target "$triple")
    mkdir -p "$DIST/$name"
    cp "$TGT/$triple/release/bilterm" "$DIST/$name/"
    chmod 0755 "$DIST/$name/bilterm"
    cp "$WT"/{README.md,LICENSE,CHANGELOG.md} "$DIST/$name/"
    tar -czf "$DIST/$name.tar.gz" -C "$DIST" "$name"
    cp "$DIST/$name.tar.gz" "$DIST/bilterm-$triple.tar.gz"
    rm -rf "${DIST:?}/$name"
}

pkg_windows() {
    local triple="$1" name="bilterm-$TAG-$1"
    rustup target add "$triple"
    (cd "$WT" && cargo build --release --target "$triple")
    mkdir -p "$DIST/$name"
    cp "$TGT/$triple/release/bilterm.exe" "$DIST/$name/"
    cp "$WT"/{README.md,LICENSE,CHANGELOG.md} "$DIST/$name/"
    local d="$DIST"; command -v cygpath >/dev/null && d="$(cygpath -m "$DIST")"   # Git Bash -> native Windows path
    local py; py="${PYTHON:-$(command -v python3 || command -v python || true)}"
    if [[ -n "$py" ]] && "$py" -c "import shutil; shutil.make_archive('$d/$name','zip','$d','$name')" 2>/dev/null; then :
    else   # no usable Python (e.g. the Microsoft Store alias): Windows PowerShell's built-in zip
        powershell.exe -NoProfile -Command "Compress-Archive -Force -Path '$d/$name' -DestinationPath '$d/$name.zip'"
    fi
    cp "$DIST/$name.zip" "$DIST/bilterm-$triple.zip"
    rm -rf "${DIST:?}/$name"
}

if [[ $BUILD -eq 1 ]]; then
    if [[ $WINDOWS -eq 1 ]]; then
        for t in "${WIN_TRIPLES[@]}"; do pkg_windows "$t"; done
    else
        pkg_unix aarch64-apple-darwin; pkg_unix x86_64-apple-darwin
    fi
fi

if [[ $UPLOAD -eq 1 ]]; then
    # Only this run's platform(s); nullglob so a missing .zip/.tar.gz isn't fatal.
    if [[ $WINDOWS -eq 1 ]]; then pat="$(IFS='|'; echo "${WIN_TRIPLES[*]}")"; else pat=apple-darwin; fi
    new=$(cd "$DIST" && shopt -s nullglob && printf '%s\n' bilterm-*.tar.gz bilterm-*.zip | grep -E "$pat" || true)
    [[ -n "$new" ]] || { echo "error: no archives matching '$pat' in $DIST" >&2; exit 1; }
    # Merge with the checksums already on the release, then upload new assets.
    rm -rf "$DIST/prev"
    gh release download "$TAG" -R "$REPO" -p SHA256SUMS -D "$DIST/prev" 2>/dev/null || true
    sha() { if command -v shasum >/dev/null; then shasum -a 256 "$@"; else sha256sum "$@"; fi; }
    (cd "$DIST" && sha $new > SHA256SUMS.new)
    { grep -v -F -f <(awk '{print $2}' "$DIST/SHA256SUMS.new") "$DIST/prev/SHA256SUMS" 2>/dev/null || true
      cat "$DIST/SHA256SUMS.new"; } > "$DIST/SHA256SUMS"
    (cd "$DIST" && gh release upload "$TAG" -R "$REPO" --clobber $new SHA256SUMS)
    echo "Uploaded to https://github.com/$REPO/releases/tag/$TAG"
else
    echo "Built in $DIST (re-run with --upload to attach to the release)"
fi
