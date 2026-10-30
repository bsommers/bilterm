#!/usr/bin/env bash
# ==============================================================================
# Bilterm — Build Windows MSVC release archives inside a win-devvm VM
# ==============================================================================
# Drives a Windows 11 VM provisioned by https://github.com/bsommers/win-devvm
# (Parallels on Apple Silicon -> Windows 11 ARM64; libvirt on Linux -> x64) over
# SSH: ships the tag as a git bundle, runs build-release-mac-win.sh in the
# guest's Git Bash, pulls the archives back to target/release-dist/<tag>/, and
# optionally smoke-tests them in the guest and uploads them from this host.
#
# Usage:
#   ./scripts/build-windows-vm.sh [vX.Y.Z] [--arch arm64|x64|all] [--test] [--cargo-test] [--upload]
#
#   vX.Y.Z        Tag to build (default: v<version in Cargo.toml>). Must exist locally.
#   --arch        arm64 (default) | x64 | all — Windows targets to build in the guest.
#   --test        Install + smoke-test each archive in the guest (windows-smoke-test.ps1).
#   --cargo-test  Also run `cargo test --release` natively in the guest (slow).
#   --upload      Attach the archives to the GitHub release (merged SHA256SUMS), from this host.
#
#   WIN_DEVVM_DIR  win-devvm checkout (default: ../win-devvm next to this repo)
#
# The guest runs the scripts from this repo's committed HEAD (like CI uses the
# default branch), and builds the tag in a clean worktree.
# ==============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VMDIR="${WIN_DEVVM_DIR:-$ROOT/../win-devvm}"
VM="$VMDIR/vm.sh"
TAG=""; ARCH=arm64; TEST=0; CARGO_TEST=0; UPLOAD=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch) ARCH="$2"; shift ;;
        --test) TEST=1 ;;
        --cargo-test) CARGO_TEST=1 ;;
        --upload) UPLOAD=1 ;;
        v*) TAG="$1" ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done
TAG="${TAG:-v$(awk -F'"' '/^version = / {print $2; exit}' "$ROOT/Cargo.toml")}"
case "$ARCH" in
    arm64) FLAGS=(--windows-arm64); TRIPLES=(aarch64-pc-windows-msvc) ;;
    x64)   FLAGS=(--windows);       TRIPLES=(x86_64-pc-windows-msvc) ;;
    # x64 first so the native arm64 build is the one left installed by --test
    all)   FLAGS=(--windows --windows-arm64); TRIPLES=(x86_64-pc-windows-msvc aarch64-pc-windows-msvc) ;;
    *) echo "error: --arch must be arm64, x64 or all" >&2; exit 2 ;;
esac

[[ -x "$VM" ]] || { echo "error: win-devvm not found at $VMDIR (set WIN_DEVVM_DIR)" >&2; exit 1; }
git -C "$ROOT" rev-parse -q --verify "refs/tags/$TAG" >/dev/null || { echo "error: tag $TAG does not exist" >&2; exit 1; }
[[ -z "$(git -C "$ROOT" status --porcelain -- scripts)" ]] \
    || echo "warning: uncommitted changes in scripts/ are NOT used by the guest (it runs HEAD)" >&2
"$VM" status | grep -q 'tools: DONE' || { echo "error: VM not ready ('$VM status' must report tools: DONE)" >&2; exit 1; }

DIST="$ROOT/target/release-dist/$TAG"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
mkdir -p "$DIST"

echo "==> shipping $TAG + HEAD to the guest"
git -C "$ROOT" bundle create "$TMP/bilterm.bundle" HEAD "refs/tags/$TAG" 2>/dev/null
cat > "$TMP/run.sh" <<'EOS'
# Runs in the guest's Git Bash: $1 = action (build|test|cargo-test), rest = args.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR="$HOME/bt/target"
cd "$HOME/bt"
case "$1" in
  build)
    rm -rf src && git clone -q bilterm.bundle src   # origin = the bundle, so `git fetch --tags` works
    shift; cd src && ./scripts/build-release-mac-win.sh "$@" ;;
  test)
    shift; cd src && powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows-smoke-test.ps1 "$@" ;;
  cargo-test)
    shift; cd src && git -c advice.detachedHead=false checkout -q "$1" && cargo test --release --locked ;;
esac
EOS
"$VM" ssh 'New-Item -ItemType Directory -Force bt | Out-Null'
"$VM" push "$TMP/bilterm.bundle" bt/bilterm.bundle
"$VM" push "$TMP/run.sh" bt/run.sh
gbash() { "$VM" ssh "& 'C:\\Program Files\\Git\\bin\\bash.exe' -l bt/run.sh $*"; }

echo "==> building ${TRIPLES[*]} in the guest"
gbash build "$TAG" "${FLAGS[@]}"

echo "==> pulling archives into $DIST"
for t in "${TRIPLES[@]}"; do
    for f in "bilterm-$TAG-$t.zip" "bilterm-$t.zip"; do
        "$VM" pull "bt/src/target/release-dist/$TAG/$f" "$DIST/$f"
    done
    command -v file >/dev/null && unzip -p "$DIST/bilterm-$TAG-$t.zip" "*/bilterm.exe" > "$TMP/b.exe" \
        && echo "    $t: $(file -b "$TMP/b.exe")"
done

rc=0
if [[ $TEST -eq 1 ]]; then
    for t in "${TRIPLES[@]}"; do
        echo "==> smoke test $t in the guest"
        gbash test -Archive "target/release-dist/$TAG/bilterm-$TAG-$t.zip" -ExpectArch "${t%%-*}" || rc=1
    done
fi
if [[ $CARGO_TEST -eq 1 ]]; then
    echo "==> cargo test --release in the guest ($TAG)"
    gbash cargo-test "$TAG" || rc=1
fi
[[ $rc -eq 0 ]] || { echo "error: guest tests failed; not uploading" >&2; exit 1; }

if [[ $UPLOAD -eq 1 ]]; then
    "$ROOT/scripts/build-release-mac-win.sh" "$TAG" "${FLAGS[@]}" --no-build --upload
else
    echo "Built in $DIST (re-run with --upload to attach to the release)"
fi
