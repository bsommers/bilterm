#!/usr/bin/env bash
# Phase 64 doc coverage gate: every `:traffic` completion verb (src/pane/completion.rs) must appear in
# user-guide chapter 13 as `:traffic <verb>`, every `traffic.*` RPC method and `bilterm_traffic` must be
# named there, and the guide index must link the chapter.
# Prints missing items, then ONE final line: `DOCS OK <n>` (exit 0) or `DOCS FAIL: <reason>` (exit 1).
set -euo pipefail
cd "$(dirname "$0")/.."

CHAPTER=docs/user-guide/13-traffic-monitor.md
INDEX=docs/user-guide/index.md
fail() { echo "DOCS FAIL: $1"; exit 1; }
[ -f "$CHAPTER" ] || fail "$CHAPTER is missing"

row=$(tr '\n' ' ' < src/pane/completion.rs | grep -o '( *":traffic", *&\[[^]]*\][ ,]*)' | head -n 1) || true
[ -n "$row" ] || fail "no :traffic row in src/pane/completion.rs"
verbs=$(printf '%s' "$row" | grep -o '"[^"]*"' | sed '1d' | sed 's/^"//; s/"$//; s/[[:space:]]*$//')

n=0
missing=0
check() {
  n=$((n + 1))
  if ! grep -qF -- "$1" "$CHAPTER"; then
    echo "missing $1"
    missing=$((missing + 1))
  fi
}
while IFS= read -r v; do
  if [ -n "$v" ]; then check ":traffic $v"; fi
done <<< "$verbs"
for m in traffic.snapshot traffic.history traffic.window traffic.reset traffic.config bilterm_traffic ":netmon" "Ctrl+Alt+T" 'Ctrl+X $'; do
  check "$m"
done
grep -qF "13-traffic-monitor.md" "$INDEX" || { echo "missing index link"; missing=$((missing + 1)); }

[ "$missing" -eq 0 ] || fail "$missing item(s) missing"
echo "DOCS OK $n"
