#!/usr/bin/env bash
# Phase 50 golden immutability gate: classic drawer goldens must be committed exactly once and clean.
# Prints ONE line: `GOLDENS OK <n>` or `GOLDENS FAIL: <reason>`. Exit 0 on OK, 1 on failure.
set -euo pipefail
cd "$(dirname "$0")/.."

fail() { echo "GOLDENS FAIL: $1"; exit 1; }

shopt -s nullglob
files=(tests/snapshots/drawer_classic_parity__*.snap)
n=${#files[@]}
[ "$n" -ge 11 ] || fail "expected at least 11 goldens, found $n"

for f in "${files[@]}"; do
  hist=$(git log --format=%H -- "$f") || fail "git log failed for $f"
  count=$(printf '%s\n' "$hist" | grep -c . || true)
  [ "$count" -eq 1 ] || fail "$f has $count commits (expected exactly 1)"
done

dirty=$(git status --porcelain -- tests/snapshots/) || fail "git status failed"
[ -z "$dirty" ] || fail "tests/snapshots/ is dirty: $(printf '%s' "$dirty" | head -1)"

news=(tests/snapshots/*.snap.new)
[ "${#news[@]}" -eq 0 ] || fail "pending snapshot ${news[0]}"

echo "GOLDENS OK $n"
