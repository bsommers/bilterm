#!/usr/bin/env bash
# ==============================================================================
# build-binaries.sh — build, package and (optionally) upload per-platform Rust
# release binaries for one tag, from a clean git worktree of that tag.
# ==============================================================================
# Usage (run from anywhere inside the repo):
#   build-binaries.sh [vX.Y.Z] [--targets "t1 t2"] [--bin NAME] [--repo OWNER/NAME]
#                     [--upload] [--create-release]
#
#   vX.Y.Z            Tag to build (default: v<version> from Cargo.toml). Must exist.
#   --targets         Rust target triples (default: what this host can build natively:
#                     macOS -> aarch64 + x86_64 apple-darwin; Linux -> host gnu
#                     (+ musl when cargo-zigbuild is installed); Windows -> msvc).
#   --bin             Binary name (default: [package] name in Cargo.toml).
#   --repo            GitHub repo for upload (default: gh's current repo).
#   --upload          `gh release upload --clobber` the archives + merged SHA256SUMS.
#   --create-release  With --upload: create the GitHub release if the tag has none.
#
# Output: target/release-dist/<tag>/
#   <bin>-<tag>-<triple>.tar.gz|zip   versioned archive (dir with binary + docs)
#   <bin>-<triple>.tar.gz|zip         unversioned alias for "latest" installers
#   SHA256SUMS                        checksums of everything built here
# ==============================================================================
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
TAG=""; TARGETS=""; BIN=""; REPO=""; UPLOAD=0; CREATE=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --targets) TARGETS="$2"; shift ;;
        --bin) BIN="$2"; shift ;;
        --repo) REPO="$2"; shift ;;
        --upload) UPLOAD=1 ;;
        --create-release) CREATE=1 ;;
        v*) TAG="$1" ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done

cargo_field() { awk -F'"' -v k="$1" '/^\[package\]/{p=1;next} /^\[/{p=0} p && $0 ~ "^"k" *=" {print $2; exit}' "$ROOT/Cargo.toml"; }
TAG="${TAG:-v$(cargo_field version)}"
BIN="${BIN:-$(cargo_field name)}"
[[ -n "$BIN" ]] || { echo "error: could not read package name from Cargo.toml; pass --bin" >&2; exit 2; }

OS="$(uname -s)"
if [[ -z "$TARGETS" ]]; then
    case "$OS" in
        Darwin) TARGETS="aarch64-apple-darwin x86_64-apple-darwin" ;;
        Linux)
            TARGETS="$(uname -m)-unknown-linux-gnu"
            command -v cargo-zigbuild >/dev/null && TARGETS+=" $(uname -m)-unknown-linux-musl" ;;
        MINGW*|MSYS*|CYGWIN*) TARGETS="x86_64-pc-windows-msvc" ;;
        *) echo "error: unknown host $OS; pass --targets" >&2; exit 2 ;;
    esac
fi
# Apple targets need the macOS SDK; refuse early instead of failing at link time.
if [[ "$OS" != Darwin && "$TARGETS" == *apple-darwin* ]]; then
    echo "error: *-apple-darwin targets must be built on macOS" >&2; exit 1
fi

sha256() { if command -v shasum >/dev/null; then shasum -a 256 "$@"; else sha256sum "$@"; fi; }

git -C "$ROOT" fetch --tags --quiet || true
git -C "$ROOT" rev-parse -q --verify "refs/tags/$TAG" >/dev/null \
    || { echo "error: tag $TAG does not exist" >&2; exit 1; }

DIST="$ROOT/target/release-dist/$TAG"
WT="$(mktemp -d)/src"
trap 'git -C "$ROOT" worktree remove --force "$WT" 2>/dev/null || true' EXIT
git -C "$ROOT" worktree add --detach "$WT" "$TAG" >/dev/null
# Share the main target dir's registry/deps cache but keep build output separate.
export CARGO_TARGET_DIR="$ROOT/target/release-build"
mkdir -p "$DIST"

BUILT=()
for triple in $TARGETS; do
    rustup target add "$triple" >/dev/null
    builder=(cargo build)
    [[ "$triple" == *-linux-musl && "$OS" == Linux ]] && builder=(cargo zigbuild)
    # windows-gnu cross-builds from Linux need zig as the MinGW linker (plain cargo lacks one).
    [[ "$triple" == *-windows-gnu && "$OS" == Linux ]] && builder=(cargo zigbuild)
    (cd "$WT" && "${builder[@]}" --locked --release --target "$triple" --bin "$BIN")

    exe="$BIN"; [[ "$triple" == *windows* ]] && exe="$BIN.exe"
    name="$BIN-$TAG-$triple"
    rm -rf "${DIST:?}/$name"; mkdir -p "$DIST/$name"
    cp "$CARGO_TARGET_DIR/$triple/release/$exe" "$DIST/$name/"
    for doc in README.md LICENSE LICENSE-MIT LICENSE-APACHE CHANGELOG.md; do
        [[ -f "$WT/$doc" ]] && cp "$WT/$doc" "$DIST/$name/"
    done
    if [[ "$triple" == *windows* ]]; then
        (cd "$DIST" && rm -f "$name.zip" && python3 -c "import shutil,sys; shutil.make_archive(sys.argv[1],'zip','.',sys.argv[1])" "$name")
        cp "$DIST/$name.zip" "$DIST/$BIN-$triple.zip"
        BUILT+=("$name.zip" "$BIN-$triple.zip")
    else
        chmod 0755 "$DIST/$name/$exe"
        tar -czf "$DIST/$name.tar.gz" -C "$DIST" "$name"
        cp "$DIST/$name.tar.gz" "$DIST/$BIN-$triple.tar.gz"
        BUILT+=("$name.tar.gz" "$BIN-$triple.tar.gz")
    fi
    rm -rf "${DIST:?}/$name"
done

(cd "$DIST" && sha256 "${BUILT[@]}" > SHA256SUMS.new)

if [[ $UPLOAD -eq 0 ]]; then
    mv "$DIST/SHA256SUMS.new" "$DIST/SHA256SUMS"
    echo "Built in $DIST:"; (cd "$DIST" && ls -1 "${BUILT[@]}" SHA256SUMS)
    echo "(re-run with --upload to attach to the GitHub release)"
    exit 0
fi

REPO_ARGS=(); [[ -n "$REPO" ]] && REPO_ARGS=(-R "$REPO")
if ! gh release view "$TAG" ${REPO_ARGS[@]+"${REPO_ARGS[@]}"} >/dev/null 2>&1; then
    [[ $CREATE -eq 1 ]] || { echo "error: no GitHub release for $TAG (pass --create-release)" >&2; exit 1; }
    gh release create "$TAG" ${REPO_ARGS[@]+"${REPO_ARGS[@]}"} --verify-tag --title "$TAG" --generate-notes
fi
# Keep checksums for assets other hosts uploaded; replace lines for ours.
rm -rf "$DIST/prev"
gh release download "$TAG" ${REPO_ARGS[@]+"${REPO_ARGS[@]}"} -p SHA256SUMS -D "$DIST/prev" 2>/dev/null || true
{ grep -v -F -f <(awk '{print $2}' "$DIST/SHA256SUMS.new") "$DIST/prev/SHA256SUMS" 2>/dev/null || true
  cat "$DIST/SHA256SUMS.new"; } > "$DIST/SHA256SUMS"
rm -rf "$DIST/prev" "$DIST/SHA256SUMS.new"
(cd "$DIST" && gh release upload "$TAG" ${REPO_ARGS[@]+"${REPO_ARGS[@]}"} --clobber "${BUILT[@]}" SHA256SUMS)
gh release view "$TAG" ${REPO_ARGS[@]+"${REPO_ARGS[@]}"} --json url -q .url
