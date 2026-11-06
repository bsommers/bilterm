#!/usr/bin/env bash
# Phase 50.1 doc coverage gate: every `:find` reserved word and every `:finder` verb listed in
# src/pane/completion.rs must appear in user-guide chapter 12 as `:find <verb>` / `:finder <verb>`.
# Chapter 12 must also carry the gap list G1..G10, the three source tags, `masked`, the entry chord
# and the automation names, and the guide index must link chapter 12.
# Verb lists are derived from src/pane/completion.rs, so new verbs are covered without editing this script.
# Prints missing items, then ONE final line: `DOCS OK <n>` (exit 0) or `DOCS FAIL: <reason>` (exit 1).
set -euo pipefail
cd "$(dirname "$0")/.."

CHAPTER=docs/user-guide/12-find-registry-and-history.md
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
[ -f "$INDEX" ] || fail "$INDEX is missing"

find_verbs=$(row_verbs find < src/pane/completion.rs) || fail "no :find row in src/pane/completion.rs"
finder_verbs=$(row_verbs finder < src/pane/completion.rs) || fail "no :finder row in src/pane/completion.rs"

n=0
missing=0
check_phrase() {
  local phrase=$1
  n=$((n + 1))
  if ! grep -qF -- "$phrase" "$CHAPTER"; then
    echo "missing $phrase"
    missing=$((missing + 1))
  fi
}

while IFS= read -r v; do
  [ -n "$v" ] || continue
  check_phrase ":find $v"
done <<< "$find_verbs"

while IFS= read -r v; do
  [ -n "$v" ] || continue
  check_phrase ":finder $v"
done <<< "$finder_verbs"

# Gap list: whole-token match so that G1 does not satisfy G10.
for i in 1 2 3 4 5 6 7 8 9 10; do
  n=$((n + 1))
  if ! grep -qE "\bG${i}\b" "$CHAPTER"; then
    echo "missing G${i}"
    missing=$((missing + 1))
  fi
done

for phrase in "[live]" "[saved:" "[log:" "masked" "Ctrl+Alt+F" "find.agents" "find.sessions" "find.history" "bilterm_find"; do
  check_phrase "$phrase"
done

n=$((n + 1))
grep -qF "12-find-registry-and-history.md" "$INDEX" || { echo "missing index link in $INDEX"; missing=$((missing + 1)); }

[ "$missing" -eq 0 ] || fail "$missing missing"
echo "DOCS OK $n"
