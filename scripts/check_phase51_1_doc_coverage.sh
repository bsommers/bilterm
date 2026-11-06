#!/usr/bin/env bash
# Phase 51.1 doc coverage gate:
# Verifies that all persistent SSH + tmux link verbs, key chords, states, troubleshooting,
# security invariants, API methods, MCP tools, and developer guide references are properly documented.
#
# Supports PHASE51_1_DOCS_PART=1 for Task 1 verification before Task 2 writes API/DevGuide/Changelog.
# Prints missing items, then ONE final line: `DOCS OK <n>` (exit 0) or `DOCS FAIL: <reason>` (exit 1).
set -euo pipefail
cd "$(dirname "$0")/.."

fail() { echo "DOCS FAIL: $1"; exit 1; }

CH05="docs/user-guide/05-tmux-integration.md"
CH06="docs/user-guide/06-keyboard-mouse-and-command-reference.md"
CH07="docs/user-guide/07-troubleshooting-and-faq.md"
CH10="docs/user-guide/10-security-and-best-practices.md"
CH12="docs/user-guide/12-find-registry-and-history.md"
API="docs/API.md"
DEV="docs/DEVELOPER_AND_AGENT_GUIDE.md"
CHANGELOG="CHANGELOG.md"
KEYMAP="src/input/keymap.rs"

[ -f "$CH05" ] || fail "$CH05 is missing"
[ -f "$CH06" ] || fail "$CH06 is missing"
[ -f "$CH07" ] || fail "$CH07 is missing"
[ -f "$CH10" ] || fail "$CH10 is missing"

n=0
missing=0

check() {
  local target_file=$1
  local phrase=$2
  n=$((n + 1))
  if ! grep -qF -- "$phrase" "$target_file"; then
    echo "missing '$phrase' in $target_file"
    missing=$((missing + 1))
  fi
}

check_not() {
  local target_file=$1
  local phrase=$2
  n=$((n + 1))
  local found=0
  if [ -d "$target_file" ]; then
    if grep -rqF -- "$phrase" "$target_file"; then
      found=1
    fi
  else
    if grep -qF -- "$phrase" "$target_file"; then
      found=1
    fi
  fi
  if [ "$found" -eq 1 ]; then
    echo "forbidden '$phrase' found in $target_file"
    missing=$((missing + 1))
  fi
}

# 1. Negative checks across docs
check_not "docs" "spawn_remote_ssh_attach_or_new"
for f in "$CH05" "$CH06" "$CH07" "$CH10" "$API" "$DEV" "$CHANGELOG"; do
  if [ -f "$f" ]; then
    check_not "$f" "StrictHostKeyChecking=no"
  fi
done

# 2. Chapter 05: Persistent SSH + tmux link core concepts
for phrase in "ssh -tt" "ServerAliveInterval" "/opt/homebrew/bin" "exec \$SHELL -lc" \
  ":ssh-tmux reconnect" ":ssh-tmux disconnect" ":ssh-tmux info" ":ssh-tmux prefill" \
  "capture-pane" "exit-empty" "destroy-unattached" "detach-on-destroy" \
  "ControlMaster=no" "ControlPath=none" "LINK CONNECTING" "LINK DOWN"; do
  check "$CH05" "$phrase"
done

# 3. Chapter 06: Key chords derived from keymap and reference tables
leader_chars=$(sed -n '/DEFAULT_LEADER_BINDINGS/,/];/p' "$KEYMAP" | grep "RemoteLink" | grep -o "KeyCode::Char('[^']*')" | sed "s/KeyCode::Char('//; s/')//" | sort -u)
for ch in $leader_chars; do
  n=$((n + 1))
  if ! grep -qE -- "(<L>|Ctrl\+X) $ch" "$CH06"; then
    echo "missing leader chord for key '$ch' in $CH06"
    missing=$((missing + 1))
  fi
done

for phrase in "Ctrl+Alt+R" ":ssh-tmux reconnect" ":ssh-tmux disconnect" ":ssh-tmux info" ":ssh-tmux prefill" \
  "interactive link banner" "Enter" "d"; do
  check "$CH06" "$phrase"
done

# 4. Chapter 07: Troubleshooting and diagnostics
for phrase in "link.down" "tmux ls" "RUST_LOG=bilterm=debug" "link.connect" "link.up" \
  "link.retry" "link.give_up" "link.control_exit" "pmset -g" "(not connected)"; do
  check "$CH07" "$phrase"
done

# 5. Chapter 10: Security invariants
for phrase in "redact_ssh_argv" "ControlMaster=no" "set_link_raw_input"; do
  check "$CH10" "$phrase"
done

# 6. Chapter 12: G1 and G2 resolutions (if present)
if [ -f "$CH12" ]; then
  check "$CH12" "Resolved in Phase 51.1"
fi

# Part 2 checks (skipped when PHASE51_1_DOCS_PART=1)
if [ "${PHASE51_1_DOCS_PART:-0}" != "1" ]; then
  [ -f "$API" ] || fail "$API is missing"
  [ -f "$DEV" ] || fail "$DEV is missing"
  [ -f "$CHANGELOG" ] || fail "$CHANGELOG is missing"

  # API reference
  check "$API" "remote.link"
  check "$API" "bilterm_remote_link"
  check_not "$API" "connected_ssh_tmux"

  # Developer guide
  check "$DEV" "src/tmux/reconnect.rs"
  check "$DEV" "src/tmux/link.rs"
  check "$DEV" "src/wm/tmux_link.rs"
  check "$DEV" "src/wm/tmux_io.rs"
  check "$DEV" "tests/phase51_1_"

  # Changelog
  check "$CHANGELOG" "ssh-tmux"
  check "$CHANGELOG" "Phase 51.1"
fi

[ "$missing" -eq 0 ] || fail "$missing missing"
echo "DOCS OK $n"
