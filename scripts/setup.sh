#!/usr/bin/env bash
# ==============================================================================
# Bilterm — Cross-Platform Developer & Builder Setup Script (Linux & macOS)
# ==============================================================================
# Sets up a fresh clone of Bilterm with:
#   1. System C toolchain & build prerequisites (cc/gcc/clang, pkg-config, git, curl, tmux)
#   2. Rustup & Rust toolchain (channel 1.90 from rust-toolchain.toml, MSRV 1.88+, rustfmt, clippy)
#   3. Optional developer tools (--with-dev-tools: cargo-nextest, cargo-insta, cargo-llvm-cov)
#   4. Optional release build (--build) and binary installation (--install)
#
# Usage:
#   ./scripts/setup.sh [OPTIONS]
# ==============================================================================

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUIRED_RUST_CHANNEL="1.90"
MIN_RUST_VERSION="1.88.0"

MODE_CHECK=0
DRY_RUN=0
NON_INTERACTIVE=0
WITH_DEV_TOOLS=0
DO_BUILD=0
DO_INSTALL=0
INSTALL_ARGS=()

usage() {
    cat <<'EOF'
Bilterm Developer & Build Environment Setup (Linux / macOS)

Usage:
  ./scripts/setup.sh [OPTIONS]

Options:
  -c, --check             Verify installed tools without modifying the system
  -n, --dry-run           Print commands that would be executed without running them
  -y, --yes, --non-interactive
                          Run non-interactively (auto-confirm package/rustup prompts)
  -d, --with-dev-tools    Also install cargo-nextest, cargo-insta, and cargo-llvm-cov
  -b, --build             Build release binary (cargo build --release) after setup
  -i, --install           Install bilterm binary after setup (via scripts/install.sh)
      --local             Pass --local to scripts/install.sh (default: ~/.local/bin)
      --cargo             Pass --cargo to scripts/install.sh (~/.cargo/bin)
      --global            Pass --global to scripts/install.sh (/usr/local/bin)
      --prefix <DIR>      Pass --prefix <DIR> to scripts/install.sh
  -h, --help              Print this help message

Examples:
  ./scripts/setup.sh --check
  ./scripts/setup.sh --yes --build
  ./scripts/setup.sh --yes --with-dev-tools --install
EOF
}

log_info()  { printf '\033[1;34m[bilterm-setup]\033[0m %s\n' "$*" >&2; }
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
        -c|--check)
            MODE_CHECK=1
            shift
            ;;
        -n|--dry-run)
            DRY_RUN=1
            shift
            ;;
        -y|--yes|--non-interactive)
            NON_INTERACTIVE=1
            shift
            ;;
        -d|--with-dev-tools)
            WITH_DEV_TOOLS=1
            shift
            ;;
        -b|--build)
            DO_BUILD=1
            shift
            ;;
        -i|--install)
            DO_INSTALL=1
            shift
            ;;
        --local|--cargo|--global)
            INSTALL_ARGS+=("$1")
            shift
            ;;
        --prefix)
            if [[ $# -lt 2 ]]; then
                log_err "--prefix requires a directory argument"
                exit 1
            fi
            INSTALL_ARGS+=("--prefix" "$2")
            shift 2
            ;;
        --prefix=*)
            INSTALL_ARGS+=("$1")
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

detect_os() {
    local uname_s
    uname_s="$(uname -s 2>/dev/null || echo Unknown)"
    case "$uname_s" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "macos" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows-msys" ;;
        *)       echo "unknown" ;;
    esac
}

version_ge() {
    # Returns 0 if $1 >= $2 (semver major.minor.patch)
    local v1="${1%%-*}" v2="${2%%-*}"
    if [[ "$v1" == "$v2" ]]; then
        return 0
    fi
    local IFS=.
    local i ver1=($v1) ver2=($v2)
    for ((i=${#ver1[@]}; i<${#ver2[@]}; i++)); do ver1[i]=0; done
    for ((i=0; i<${#ver1[@]}; i++)); do
        if [[ -z "${ver2[i]:-}" ]]; then ver2[i]=0; fi
        if ((10#${ver1[i]} > 10#${ver2[i]})); then return 0; fi
        if ((10#${ver1[i]} < 10#${ver2[i]})); then return 1; fi
    done
    return 0
}

OS="$(detect_os)"
log_info "Detected platform: ${OS} ($(uname -m 2>/dev/null || echo unknown))"

MISSING_ITEMS=0

# 1. Check / Install System C Compiler & Prerequisites
check_or_install_system_deps() {
    local need_cc=0 need_curl=0 need_git=0
    if command -v cc >/dev/null 2>&1 || command -v gcc >/dev/null 2>&1 || command -v clang >/dev/null 2>&1; then
        log_ok "C compiler found ($(command -v cc || command -v gcc || command -v clang))"
    else
        log_warn "C compiler (cc/gcc/clang) not found — required by Rust linker and native crates"
        need_cc=1
    fi

    if command -v curl >/dev/null 2>&1; then
        log_ok "curl found ($(command -v curl))"
    else
        log_warn "curl not found"
        need_curl=1
    fi

    if command -v git >/dev/null 2>&1; then
        log_ok "git found ($(command -v git))"
    else
        log_warn "git not found"
        need_git=1
    fi

    if command -v tmux >/dev/null 2>&1; then
        log_ok "tmux found ($(tmux -V 2>/dev/null || echo tmux)) — optional tmux integration ready"
    else
        log_warn "tmux not found (optional: install tmux for Mode A/B tmux integration)"
    fi

    if [[ "$need_cc" -eq 0 && "$need_curl" -eq 0 && "$need_git" -eq 0 ]]; then
        return 0
    fi

    if [[ "$MODE_CHECK" -eq 1 ]]; then
        MISSING_ITEMS=$((MISSING_ITEMS + 1))
        log_warn "Remedy: install build-essential/base-devel, curl, and git for your OS"
        return 0
    fi

    local sudo_cmd=""
    if [[ "${EUID:-$(id -u)}" -ne 0 ]] && command -v sudo >/dev/null 2>&1; then
        sudo_cmd="sudo"
    fi

    case "$OS" in
        linux)
            if command -v apt-get >/dev/null 2>&1; then
                log_info "Installing build prerequisites via apt-get..."
                run_cmd $sudo_cmd apt-get update -qq
                run_cmd $sudo_cmd apt-get install -y build-essential pkg-config curl git
            elif command -v dnf >/dev/null 2>&1; then
                log_info "Installing build prerequisites via dnf..."
                run_cmd $sudo_cmd dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl git
            elif command -v pacman >/dev/null 2>&1; then
                log_info "Installing build prerequisites via pacman..."
                run_cmd $sudo_cmd pacman -Sy --noconfirm --needed base-devel pkgconf curl git
            elif command -v zypper >/dev/null 2>&1; then
                log_info "Installing build prerequisites via zypper..."
                run_cmd $sudo_cmd zypper --non-interactive install gcc gcc-c++ make pkg-config curl git
            elif command -v apk >/dev/null 2>&1; then
                log_info "Installing build prerequisites via apk..."
                run_cmd $sudo_cmd apk add --no-cache build-base pkgconf curl git
            else
                log_err "Could not detect Linux package manager. Please install gcc/clang, make, pkg-config, curl, and git manually."
                exit 1
            fi
            ;;
        macos)
            if ! xcode-select -p >/dev/null 2>&1; then
                log_info "Installing Xcode Command Line Tools..."
                run_cmd xcode-select --install || true
            fi
            if command -v brew >/dev/null 2>&1 && [[ "$need_git" -eq 1 || "$need_curl" -eq 1 ]]; then
                run_cmd brew install git curl pkg-config
            fi
            ;;
        windows-msys)
            log_warn "On Windows, we recommend running: powershell -ExecutionPolicy Bypass -File .\\scripts\\setup.ps1"
            ;;
        *)
            log_err "Unsupported OS for automatic system package install. Please install a C compiler, curl, and git."
            exit 1
            ;;
    esac
}

# 2. Check / Install Rustup, Cargo, and Rust Toolchain
check_or_install_rust() {
    if [[ -f "$HOME/.cargo/env" ]]; then
        # shellcheck disable=SC1091
        source "$HOME/.cargo/env"
    fi

    if ! command -v rustup >/dev/null 2>&1 && ! command -v cargo >/dev/null 2>&1; then
        log_warn "Rust / Cargo not found on PATH"
        if [[ "$MODE_CHECK" -eq 1 ]]; then
            MISSING_ITEMS=$((MISSING_ITEMS + 1))
            log_warn "Remedy: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${REQUIRED_RUST_CHANNEL}"
            return 0
        fi
        log_info "Installing rustup + Rust ${REQUIRED_RUST_CHANNEL}..."
        if [[ "$DRY_RUN" -eq 1 ]]; then
            printf "  [dry-run] curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain %s\n" "$REQUIRED_RUST_CHANNEL" >&2
        else
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain "$REQUIRED_RUST_CHANNEL" --component rustfmt,clippy
            if [[ -f "$HOME/.cargo/env" ]]; then
                # shellcheck disable=SC1091
                source "$HOME/.cargo/env"
            fi
            export PATH="$HOME/.cargo/bin:$PATH"
        fi
    fi

    if command -v rustc >/dev/null 2>&1; then
        local rustc_ver
        rustc_ver="$(rustc --version | awk '{print $2}')"
        if version_ge "$rustc_ver" "$MIN_RUST_VERSION"; then
            log_ok "rustc ${rustc_ver} meets MSRV (>= ${MIN_RUST_VERSION})"
        else
            log_warn "rustc ${rustc_ver} is older than MSRV ${MIN_RUST_VERSION}"
            if [[ "$MODE_CHECK" -eq 1 ]]; then
                MISSING_ITEMS=$((MISSING_ITEMS + 1))
                log_warn "Remedy: rustup toolchain install ${REQUIRED_RUST_CHANNEL}"
                return 0
            fi
            if command -v rustup >/dev/null 2>&1; then
                log_info "Installing Rust toolchain ${REQUIRED_RUST_CHANNEL} via rustup..."
                run_cmd rustup toolchain install "$REQUIRED_RUST_CHANNEL" --component rustfmt --component clippy
                run_cmd rustup default "$REQUIRED_RUST_CHANNEL"
            fi
        fi
    fi

    if command -v rustup >/dev/null 2>&1; then
        if ! cargo fmt --version >/dev/null 2>&1 || ! cargo clippy --version >/dev/null 2>&1; then
            if [[ "$MODE_CHECK" -eq 1 ]]; then
                log_warn "rustfmt or clippy component missing (Remedy: rustup component add rustfmt clippy)"
            else
                log_info "Adding rustfmt and clippy components..."
                run_cmd rustup component add rustfmt clippy
            fi
        else
            log_ok "rustfmt & clippy components ready"
        fi
    fi
}

# 3. Optional Dev Tools
check_or_install_dev_tools() {
    if [[ "$WITH_DEV_TOOLS" -eq 0 ]]; then
        return 0
    fi
    local tools=("cargo-nextest" "cargo-insta" "cargo-llvm-cov")
    for tool in "${tools[@]}"; do
        local sub="${tool#cargo-}"
        if cargo "$sub" --version >/dev/null 2>&1; then
            log_ok "${tool} already installed"
        elif [[ "$MODE_CHECK" -eq 1 ]]; then
            MISSING_ITEMS=$((MISSING_ITEMS + 1))
            log_warn "${tool} not installed (Remedy: cargo install --locked ${tool})"
        else
            log_info "Installing ${tool}..."
            run_cmd cargo install --locked "$tool"
        fi
    done
}

check_or_install_system_deps
check_or_install_rust
check_or_install_dev_tools

if [[ "$MODE_CHECK" -eq 1 ]]; then
    if [[ "$MISSING_ITEMS" -gt 0 ]]; then
        log_err "Environment check found ${MISSING_ITEMS} missing requirement(s). Run './scripts/setup.sh --yes' to install."
        exit 1
    fi
    log_ok "All Bilterm build requirements are satisfied!"
    cat >&2 <<'EOF'

Next steps:
  1. cargo build --release              # compile optimized bilterm binary
  2. ./scripts/install.sh --local       # install bilterm to ~/.local/bin
  3. bilterm                            # launch interactive IBM 3270 x Borland TUI
EOF
    exit 0
fi

if [[ "$DO_BUILD" -eq 1 ]]; then
    log_info "Building Bilterm release binary..."
    (cd "$REPO_ROOT" && run_cmd cargo build --release)
    log_ok "Built ${REPO_ROOT}/target/release/bilterm"
fi

if [[ "$DO_INSTALL" -eq 1 ]]; then
    log_info "Running plain installer (scripts/install.sh)..."
    local_args=("${INSTALL_ARGS[@]}")
    if [[ "$DRY_RUN" -eq 1 ]]; then
        local_args+=("--dry-run")
    fi
    "$REPO_ROOT/scripts/install.sh" "${local_args[@]}"
    exit 0
fi

log_ok "Bilterm development environment setup complete!"
cat >&2 <<'EOF'

Next steps:
  1. cargo build --release              # build release binary (target/release/bilterm)
  2. cargo test --all-targets           # run verification test suite
  3. ./scripts/install.sh --local       # install binary + manpage for everyday use
EOF
