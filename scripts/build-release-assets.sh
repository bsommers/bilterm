#!/usr/bin/env bash
# ==============================================================================
# Bilterm — Multi-Platform Release Binary Builder & Packager
# ==============================================================================
# Cross-compiles and packages release archives for:
#   - x86_64-unknown-linux-gnu      (glibc >= 2.28)
#   - x86_64-unknown-linux-musl     (static Linux x86_64)
#   - aarch64-unknown-linux-musl    (static Linux ARM64)
#   - aarch64-apple-darwin          (macOS Apple Silicon)
#   - x86_64-apple-darwin           (macOS Intel)
#   - x86_64-pc-windows-gnu         (Windows x86_64)
#
# Produces both versioned (`bilterm-vX.Y.Z-<triple>.tar.gz` / `.zip`) and
# unversioned latest-installer aliases (`bilterm-<triple>.tar.gz` / `.zip`)
# plus `SHA256SUMS` inside `target/release-dist/`.
# ==============================================================================

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

VERSION="${1:-v$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)}"
DIST_DIR="${REPO_ROOT}/target/release-dist/${VERSION}"

# Include scratchpad zig on PATH if present
for candidate_zig in \
    /tmp/claude-1000/-home-bill-src-ai-bilterm/*/scratchpad/zig \
    "$HOME/.local/bin"; do
    if [[ -x "${candidate_zig}/zig" ]]; then
        export PATH="${candidate_zig}:$PATH"
        break
    fi
done

if ! command -v cargo-zigbuild >/dev/null 2>&1; then
    cargo install --locked cargo-zigbuild
fi

RUST_TARGETS=(
    "x86_64-unknown-linux-gnu"
    "x86_64-unknown-linux-musl"
    "aarch64-unknown-linux-musl"
    "aarch64-apple-darwin"
    "x86_64-apple-darwin"
    "x86_64-pc-windows-gnu"
)

rustup target add "${RUST_TARGETS[@]}"

ZIG_TARGETS=(
    "x86_64-unknown-linux-gnu.2.28"
    "x86_64-unknown-linux-musl"
    "aarch64-unknown-linux-musl"
    "aarch64-apple-darwin"
    "x86_64-apple-darwin"
    "x86_64-pc-windows-gnu"
)

for zt in "${ZIG_TARGETS[@]}"; do
    echo "[bilterm-release] Building ${zt}..."
    cargo zigbuild --release --target "$zt"
done

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

package_unix_target() {
    local triple="$1"
    local bin_path="target/${triple}/release/bilterm"
    local pkg_name="bilterm-${VERSION}-${triple}"
    local stage_dir="${DIST_DIR}/${pkg_name}"

    mkdir -p "$stage_dir"
    cp -f "$bin_path" "${stage_dir}/bilterm"
    chmod 0755 "${stage_dir}/bilterm"
    cp -f README.md LICENSE CHANGELOG.md "$stage_dir/"

    tar -czf "${DIST_DIR}/${pkg_name}.tar.gz" -C "$DIST_DIR" "$pkg_name"
    cp -f "${DIST_DIR}/${pkg_name}.tar.gz" "${DIST_DIR}/bilterm-${triple}.tar.gz"
    rm -rf "$stage_dir"
}

package_windows_target() {
    local triple="$1"
    local bin_path="target/${triple}/release/bilterm.exe"
    local pkg_name="bilterm-${VERSION}-${triple}"
    local stage_dir="${DIST_DIR}/${pkg_name}"

    mkdir -p "$stage_dir"
    cp -f "$bin_path" "${stage_dir}/bilterm.exe"
    cp -f README.md LICENSE CHANGELOG.md "$stage_dir/"

    if command -v zip >/dev/null 2>&1; then
        (cd "$DIST_DIR" && zip -qr "${pkg_name}.zip" "$pkg_name")
    else
        python3 -c "import shutil; shutil.make_archive('${DIST_DIR}/${pkg_name}', 'zip', '${DIST_DIR}', '${pkg_name}')"
    fi
    cp -f "${DIST_DIR}/${pkg_name}.zip" "${DIST_DIR}/bilterm-${triple}.zip"
    cp -f "${DIST_DIR}/${pkg_name}.zip" "${DIST_DIR}/bilterm-x86_64-pc-windows-msvc.zip"
    rm -rf "$stage_dir"
}

package_unix_target "x86_64-unknown-linux-gnu"
package_unix_target "x86_64-unknown-linux-musl"
package_unix_target "aarch64-unknown-linux-musl"
package_unix_target "aarch64-apple-darwin"
package_unix_target "x86_64-apple-darwin"
package_windows_target "x86_64-pc-windows-gnu"

(
    cd "$DIST_DIR"
    sha256sum bilterm-* > SHA256SUMS
)

echo "[bilterm-release] Packaged archives in ${DIST_DIR}:"
ls -lh "$DIST_DIR"
