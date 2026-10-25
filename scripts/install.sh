#!/usr/bin/env bash
# ==============================================================================
# Bilterm — Plain Binary & Multi-Scope Installer (Linux & macOS)
# ==============================================================================
# Installs the `bilterm` binary, manpage (`bilterm.1`), and shell completions
# for users who just want to run Bilterm without managing a development setup.
#
# Scopes:
#   --local (default)   ~/.local/bin/bilterm, ~/.local/share/man/man1/bilterm.1
#   --cargo             ~/.cargo/bin/bilterm
#   --global            /usr/local/bin/bilterm, /usr/local/share/man/man1/bilterm.1
#   --prefix <DIR>      <DIR>/bin/bilterm, <DIR>/share/man/man1/bilterm.1
#
# Can also be piped directly via curl:
#   curl -fsSL https://raw.githubusercontent.com/bsommers/bilterm/main/scripts/install.sh | bash
# ==============================================================================

set -euo pipefail

GITHUB_REPO="${BILTERM_GITHUB_REPO:-bsommers/bilterm}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-.}")" 2>/dev/null && pwd || pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." 2>/dev/null && pwd || pwd)"

SCOPE="local"
CUSTOM_PREFIX="${PREFIX:-}"
DRY_RUN=0
UNINSTALL=0
FORCE=0
FROM_SOURCE=0
BINARY_ONLY=0
RELEASE_TAG="latest"

usage() {
    cat <<'EOF'
Bilterm Plain Binary Installer (Linux & macOS)

Usage:
  ./scripts/install.sh [OPTIONS]

Install Scopes:
  -l, --local             Install to user home (~/.local/bin & ~/.local/share) [default]
      --cargo             Install to Cargo bin directory (~/.cargo/bin)
  -g, --global            Install system-wide (/usr/local/bin & /usr/local/share)
  -p, --prefix <DIR>      Install into custom prefix (<DIR>/bin & <DIR>/share)

Source / Binary Selection:
  -b, --binary-only       Download prebuilt release binary from GitHub Releases (no Rust needed)
  -s, --from-source       Build from local repo (or cargo install --git) even if prebuilt exists
  -t, --tag <TAG>         Release tag to download when fetching from GitHub (default: latest)

Operations:
  -u, --uninstall         Remove installed bilterm binary, manpage, and completions
  -f, --force             Overwrite existing unmanaged target files
  -n, --dry-run           Print actions without modifying the filesystem
  -h, --help              Print this help message

Examples:
  ./scripts/install.sh                        # Install to ~/.local/bin (uses local build, release, or cargo)
  ./scripts/install.sh --cargo                # Install to ~/.cargo/bin/bilterm
  ./scripts/install.sh --global               # Install system-wide to /usr/local/bin
  ./scripts/install.sh --prefix /opt/bilterm  # Install into /opt/bilterm/bin
  ./scripts/install.sh --uninstall            # Cleanly uninstall bilterm
EOF
}

log_info()  { printf '\033[1;34m[bilterm-install]\033[0m %s\n' "$*" >&2; }
log_ok()    { printf '\033[1;32m  [OK]\033[0m %s\n' "$*" >&2; }
log_warn()  { printf '\033[1;33m[WARN]\033[0m %s\n' "$*" >&2; }
log_err()   { printf '\033[1;31m [ERR]\033[0m %s\n' "$*" >&2; }

run_cmd() {
    if [[ "$DRY_RUN" -eq 1 ]]; then
        printf '  [dry-run] %s\n' "$*" >&2
        return 0
    fi
    "$@"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -l|--local)
            SCOPE="local"
            shift
            ;;
        --cargo)
            SCOPE="cargo"
            shift
            ;;
        -g|--global)
            SCOPE="global"
            shift
            ;;
        -p|--prefix)
            if [[ $# -lt 2 ]]; then
                log_err "--prefix requires a directory argument"
                exit 1
            fi
            SCOPE="prefix"
            CUSTOM_PREFIX="$2"
            shift 2
            ;;
        --prefix=*)
            SCOPE="prefix"
            CUSTOM_PREFIX="${1#*=}"
            shift
            ;;
        -b|--binary-only)
            BINARY_ONLY=1
            shift
            ;;
        -s|--from-source)
            FROM_SOURCE=1
            shift
            ;;
        -t|--tag)
            if [[ $# -lt 2 ]]; then
                log_err "--tag requires a version tag argument"
                exit 1
            fi
            RELEASE_TAG="$2"
            shift 2
            ;;
        -u|--uninstall)
            UNINSTALL=1
            shift
            ;;
        -f|--force)
            FORCE=1
            shift
            ;;
        -n|--dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            log_err "Unknown option: $1 (try --help)"
            exit 1
            ;;
    esac
done

resolve_abs_path() {
    local target="$1"
    if [[ "$target" == /* ]]; then
        echo "$target"
    else
        echo "$(pwd)/${target#./}"
    fi
}

if [[ "$SCOPE" == "local" && -n "$CUSTOM_PREFIX" ]]; then
    SCOPE="prefix"
fi

case "$SCOPE" in
    local)
        BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
        DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/bilterm"
        MAN_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/man/man1"
        ;;
    cargo)
        BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
        DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/bilterm"
        MAN_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/man/man1"
        ;;
    global)
        BIN_DIR="/usr/local/bin"
        DATA_DIR="/usr/local/share/bilterm"
        MAN_DIR="/usr/local/share/man/man1"
        ;;
    prefix)
        ABS_PREFIX="$(resolve_abs_path "$CUSTOM_PREFIX")"
        BIN_DIR="${ABS_PREFIX}/bin"
        DATA_DIR="${ABS_PREFIX}/share/bilterm"
        MAN_DIR="${ABS_PREFIX}/share/man/man1"
        ;;
esac

TARGET_BIN="${BIN_DIR}/bilterm"
TARGET_MAN="${MAN_DIR}/bilterm.1"
COMP_DIR="${DATA_DIR}/completions"

check_write_access() {
    local dir="$1"
    local probe="$dir"
    while [[ ! -d "$probe" && "$probe" != "/" && "$probe" != "." ]]; do
        probe="$(dirname "$probe")"
    done
    if [[ ! -w "$probe" ]]; then
        log_err "No write permission for '${dir}' (parent '${probe}' is not writable)."
        if [[ "$SCOPE" == "global" ]]; then
            log_err "Remedy: re-run with 'sudo ./scripts/install.sh --global' or use '--local' for unprivileged install."
        fi
        exit 1
    fi
}

if [[ "$DRY_RUN" -eq 0 ]]; then
    check_write_access "$BIN_DIR"
fi

# Uninstall mode
if [[ "$UNINSTALL" -eq 1 ]]; then
    log_info "Uninstalling bilterm from ${BIN_DIR}..."
    if [[ -e "$TARGET_BIN" || -L "$TARGET_BIN" ]]; then
        run_cmd rm -f "$TARGET_BIN"
        log_ok "Removed ${TARGET_BIN}"
    else
        log_warn "Binary not found at ${TARGET_BIN}"
    fi
    if [[ -e "$TARGET_MAN" ]]; then
        run_cmd rm -f "$TARGET_MAN"
        log_ok "Removed ${TARGET_MAN}"
    fi
    if [[ -d "$DATA_DIR" ]]; then
        run_cmd rm -rf "$DATA_DIR"
        log_ok "Removed ${DATA_DIR}"
    fi
    log_ok "Uninstall complete."
    exit 0
fi

detect_target_triple() {
    local os arch
    os="$(uname -s 2>/dev/null || echo unknown)"
    arch="$(uname -m 2>/dev/null || echo unknown)"
    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
    esac
    case "$os" in
        Linux*)  echo "${arch}-unknown-linux-gnu" ;;
        Darwin*) echo "${arch}-apple-darwin" ;;
        *)       echo "unknown" ;;
    esac
}

try_download_prebuilt() {
    local dest_bin="$1"
    local triple
    triple="$(detect_target_triple)"
    if [[ "$triple" == "unknown" ]] || ! command -v curl >/dev/null 2>&1; then
        return 1
    fi

    local url
    if [[ "$RELEASE_TAG" == "latest" ]]; then
        url="https://github.com/${GITHUB_REPO}/releases/latest/download/bilterm-${triple}.tar.gz"
    else
        url="https://github.com/${GITHUB_REPO}/releases/download/${RELEASE_TAG}/bilterm-${triple}.tar.gz"
    fi

    log_info "Checking for prebuilt binary at ${url}..."
    if [[ "$DRY_RUN" -eq 1 ]]; then
        printf '  [dry-run] curl -fsSL "%s" | tar -xz -C "%s"\n' "$url" "$(dirname "$dest_bin")" >&2
        return 0
    fi

    local tmp_dir
    tmp_dir="$(mktemp -d)"
    if curl -fsSL "$url" -o "${tmp_dir}/bilterm.tar.gz" 2>/dev/null; then
        tar -xzf "${tmp_dir}/bilterm.tar.gz" -C "$tmp_dir"
        local found_bin=""
        if [[ -f "${tmp_dir}/bilterm" ]]; then
            found_bin="${tmp_dir}/bilterm"
        else
            found_bin="$(find "$tmp_dir" -name "bilterm" -type f | head -n 1 || true)"
        fi
        if [[ -n "$found_bin" && -f "$found_bin" ]]; then
            cp -f "$found_bin" "$dest_bin"
            chmod 0755 "$dest_bin"
            rm -rf "$tmp_dir"
            log_ok "Downloaded prebuilt bilterm (${triple})"
            return 0
        fi
    fi
    rm -rf "$tmp_dir"
    return 1
}

acquire_binary() {
    local dest_bin="$1"

    # 1. If --binary-only is requested, only try prebuilt download
    if [[ "$BINARY_ONLY" -eq 1 ]]; then
        if try_download_prebuilt "$dest_bin"; then
            return 0
        fi
        log_err "Prebuilt release binary could not be downloaded for $(detect_target_triple)."
        log_err "Remedy: omit --binary-only to compile from source, or run './scripts/setup.sh --yes --install'."
        exit 1
    fi

    # 2. If inside a local checkout and target/release/bilterm already exists (and not --from-source)
    if [[ "$FROM_SOURCE" -eq 0 && -f "${REPO_ROOT}/target/release/bilterm" ]]; then
        log_info "Using existing release binary at ${REPO_ROOT}/target/release/bilterm"
        run_cmd cp -f "${REPO_ROOT}/target/release/bilterm" "$dest_bin"
        run_cmd chmod 0755 "$dest_bin"
        return 0
    fi

    # 3. If not --from-source, try downloading a prebuilt binary first if cargo isn't present or outside repo
    if [[ "$FROM_SOURCE" -eq 0 ]] && [[ ! -f "${REPO_ROOT}/Cargo.toml" ]]; then
        if try_download_prebuilt "$dest_bin"; then
            return 0
        fi
    fi

    # 4. Build from local repo Cargo.toml or cargo install --git
    if [[ -f "$HOME/.cargo/env" ]]; then
        # shellcheck disable=SC1091
        source "$HOME/.cargo/env"
    fi

    if ! command -v cargo >/dev/null 2>&1; then
        if [[ "$FROM_SOURCE" -eq 0 ]] && try_download_prebuilt "$dest_bin"; then
            return 0
        fi
        log_err "Cargo is not installed and no prebuilt binary was found."
        log_err "Remedy: run './scripts/setup.sh --yes --install' to install Rust and build Bilterm automatically."
        exit 1
    fi

    if [[ -f "${REPO_ROOT}/Cargo.toml" ]]; then
        log_info "Building optimized release binary from ${REPO_ROOT}/Cargo.toml..."
        (cd "$REPO_ROOT" && run_cmd cargo build --release)
        run_cmd cp -f "${REPO_ROOT}/target/release/bilterm" "$dest_bin"
        run_cmd chmod 0755 "$dest_bin"
    else
        log_info "Building bilterm from https://github.com/${GITHUB_REPO}.git via cargo..."
        local tmp_root
        tmp_root="$(mktemp -d)"
        run_cmd cargo install --git "https://github.com/${GITHUB_REPO}.git" --root "$tmp_root"
        if [[ "$DRY_RUN" -eq 0 ]]; then
            cp -f "${tmp_root}/bin/bilterm" "$dest_bin"
            chmod 0755 "$dest_bin"
            rm -rf "$tmp_root"
        fi
    fi
}

# Prevent accidental overwrite of an unmanaged file unless --force is passed or it's already bilterm
if [[ -e "$TARGET_BIN" && "$FORCE" -eq 0 && "$DRY_RUN" -eq 0 ]]; then
    if ! "$TARGET_BIN" --version 2>/dev/null | grep -q "^bilterm "; then
        log_err "Target '${TARGET_BIN}' already exists and does not appear to be bilterm. Pass --force to overwrite."
        exit 1
    fi
fi

run_cmd mkdir -p "$BIN_DIR" "$MAN_DIR" "$COMP_DIR"

acquire_binary "$TARGET_BIN"
log_ok "Installed binary: ${TARGET_BIN}"

# Install manpage and shell completions
if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '  [dry-run] "%s" --man > "%s"\n' "$TARGET_BIN" "$TARGET_MAN" >&2
    printf '  [dry-run] "%s" --completions bash|zsh|fish > "%s/..."\n' "$TARGET_BIN" "$COMP_DIR" >&2
else
    if [[ -f "${REPO_ROOT}/docs/bilterm.1" ]]; then
        cp -f "${REPO_ROOT}/docs/bilterm.1" "$TARGET_MAN"
    else
        "$TARGET_BIN" --man > "$TARGET_MAN" 2>/dev/null || true
    fi
    "$TARGET_BIN" --completions bash > "${COMP_DIR}/bilterm.bash" 2>/dev/null || true
    "$TARGET_BIN" --completions zsh  > "${COMP_DIR}/_bilterm"     2>/dev/null || true
    "$TARGET_BIN" --completions fish > "${COMP_DIR}/bilterm.fish" 2>/dev/null || true
fi
log_ok "Installed manpage: ${TARGET_MAN}"
log_ok "Installed shell completions: ${COMP_DIR}"

# Check if BIN_DIR is on PATH
case ":${PATH}:" in
    *":${BIN_DIR}:"*)
        PATH_HINT=""
        ;;
    *)
        PATH_HINT="export PATH=\"${BIN_DIR}:\$PATH\""
        ;;
esac

if [[ -n "$PATH_HINT" ]]; then
    log_warn "'${BIN_DIR}' is not currently in your \$PATH."
    printf '\n  Add it to your shell profile (~/.bashrc or ~/.zshrc):\n    %s\n' "$PATH_HINT" >&2
fi

cat >&2 <<EOF

Next steps:
  1. bilterm -v                         # verify installed version
  2. bilterm                            # launch interactive IBM 3270 x Borland TUI
  3. bilterm -h                         # view CLI switches, headless daemon & MCP options
EOF
