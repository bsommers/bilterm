#!/usr/bin/env bash
# Phase 50 (+ Phase 51 sub-forms) doc coverage gate: every `:agents` verb and every `:drawer` verb added since the phase base
# commit (0915792) must appear in user-guide chapter 11 as `:<cmd> <verb>`; the guide index must link it.
# Lists are derived from src/pane/completion.rs, so new verbs are covered without editing this script.
# Phase 51 adds sub-forms the verb list cannot see (demo stop, list select, arrange grid, the list chord,
# the palette exclusion table) to the phrase loop below.
# Prints missing items, then ONE final line: `DOCS OK <n>` (exit 0) or `DOCS FAIL: <reason>` (exit 1).
set -euo pipefail
cd "$(dirname "$0")/.."

BASE=0915792
CHAPTER=docs/user-guide/11-navigator-sidebar-and-agent-traffic.md
INDEX=docs/user-guide/index.md

fail() { echo "DOCS FAIL: $1"; exit 1; }

# Quoted entries of the `(":<cmd>", &[...])` row, with rustfmt line breaks joined; one entry per line,
# trailing spaces trimmed. Reads the source from stdin.
row_verbs() {
  local cmd=$1 row
  row=$(tr '\n' ' ' | grep -o "(\":${cmd}\", *&\[[^]]*\])" | head -n 1) || true
  [ -n "$row" ] || return 1
  printf '%s' "$row" | grep -o '"[^"]*"' | sed '1d' | sed 's/^"//; s/"$//; s/[[:space:]]*$//'
}

[ -f "$CHAPTER" ] || fail "$CHAPTER is missing"

agents=$(row_verbs agents < src/pane/completion.rs) || fail "no :agents row in src/pane/completion.rs"
drawer_now=$(row_verbs drawer < src/pane/completion.rs) || fail "no :drawer row in src/pane/completion.rs"
base_src=$(git show "$BASE:src/pane/completion.rs") || fail "cannot read src/pane/completion.rs at $BASE"
drawer_base=$(printf '%s\n' "$base_src" | row_verbs drawer) || fail "no :drawer row at $BASE"

drawer_new=$(printf '%s\n' "$drawer_now" | grep -vxF -f <(printf '%s\n' "$drawer_base") || true)

n=0
missing=0
check() {
  local cmd=$1 verb=$2
  [ -n "$verb" ] || return 0
  n=$((n + 1))
  if ! grep -qF -- ":${cmd} ${verb}" "$CHAPTER"; then
    echo "missing :${cmd} ${verb}"
    missing=$((missing + 1))
  fi
}

while IFS= read -r v; do check agents "$v"; done <<< "$agents"
while IFS= read -r v; do check drawer "$v"; done <<< "$drawer_new"

# Sub-forms and config keys the verb list above cannot see (second word of a verb, or a [section] key).
for phrase in ":agents topology export <name>" "agents_window.edge_fade_secs" ":agents fade [secs]" \
  ":agents demo stop" ":agents list select <pane-id>" ":arrange grid" "Ctrl+Alt+N" "PALETTE_EXCLUSIONS"; do
  n=$((n + 1))
  if ! grep -qF -- "$phrase" "$CHAPTER"; then
    echo "missing $phrase"
    missing=$((missing + 1))
  fi
done

grep -qF "11-navigator-sidebar-and-agent-traffic.md" "$INDEX" || { echo "missing index link in $INDEX"; missing=$((missing + 1)); }

[ "$missing" -eq 0 ] || fail "$missing missing"
echo "DOCS OK $n"
