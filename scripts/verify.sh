#!/usr/bin/env bash
# Token-cheap verification runner. Prints ONE line; details live in files.
#   scripts/verify.sh              full: clippy, unit, every tests/*.rs (fmt is opt-in: --only fmt; repo has legacy drift)
#   scripts/verify.sh --quick      clippy, unit
#   scripts/verify.sh --failed     re-run only stages that failed last run
#   scripts/verify.sh --only a b   named stages (see --list)
#   scripts/verify.sh --list       stage names
#   scripts/verify.sh --status     print last result line (no run)
# Result tree (target/verify/):
#   STATUS            PASS | FAIL            (read this first)
#   summary.tsv       stage<TAB>PASS|FAIL<TAB>secs   (read on FAIL)
#   <stage>/status    PASS|FAIL   <stage>/fail.txt  compact failure excerpt (<=40 lines)
#   <stage>/log       full output (read only if fail.txt is not enough)
# Exit code: 0 all pass, 1 any fail, 2 usage.
set -uo pipefail
cd "$(dirname "$0")/.."
OUT=target/verify
mkdir -p "$OUT"

all_stages() {
  echo clippy; echo unit
  local f; for f in tests/*.rs; do echo "t:$(basename "$f" .rs)"; done
}
quick_stages() { echo clippy; echo unit; }

cmd_for() {
  case "$1" in
    fmt)    echo "cargo fmt --check" ;;
    clippy) echo "cargo clippy -q --all-targets -- -D warnings" ;;
    unit)   echo "cargo test -q --lib --bins --no-fail-fast" ;;
    t:*)    echo "cargo test -q --test ${1#t:} --no-fail-fast" ;;
    *)      return 1 ;;
  esac
}

line() { # one-line result from summary.tsv
  [ -f "$OUT/summary.tsv" ] || { echo "NORUN"; return; }
  local n p f secs
  n=$(wc -l <"$OUT/summary.tsv"); f=$(grep -c $'\tFAIL\t' "$OUT/summary.tsv"); p=$((n-f))
  secs=$(awk -F'\t' '{s+=$3} END{print s+0}' "$OUT/summary.tsv")
  if [ "$f" -eq 0 ]; then echo "PASS $p/$n ${secs}s"
  else echo "FAIL $f/$n: $(awk -F'\t' '$2=="FAIL"{printf "%s ",$1}' "$OUT/summary.tsv")-> $OUT/<stage>/fail.txt"; fi
}

mode=full; stages=()
case "${1:-}" in
  "")       ;;
  --quick)  mode=quick ;;
  --failed) mode=failed ;;
  --list)   all_stages; exit 0 ;;
  --status) cat "$OUT/STATUS" 2>/dev/null >/dev/null; line; exit 0 ;;
  --only)   shift; mode=only; stages=("$@") ;;
  *)        sed -n '2,9p' "$0"; exit 2 ;;
esac
case $mode in
  full)   mapfile -t stages < <(all_stages) ;;
  quick)  mapfile -t stages < <(quick_stages) ;;
  failed) mapfile -t stages < <(awk -F'\t' '$2=="FAIL"{print $1}' "$OUT/summary.tsv" 2>/dev/null)
          [ ${#stages[@]} -eq 0 ] && { echo "PASS nothing-failed"; exit 0; } ;;
esac

new=$(mktemp)
for s in "${stages[@]}"; do
  c=$(cmd_for "$s") || { echo "unknown stage: $s (try --list)"; exit 2; }
  d="$OUT/${s//:/_}"; rm -rf "$d"; mkdir -p "$d"
  t0=$SECONDS
  if bash -c "$c" >"$d/log" 2>&1; then st=PASS; else st=FAIL; fi
  echo "$st" >"$d/status"
  if [ $st = FAIL ]; then
    { grep -E '^(error|warning)(\[|:)|^---- |panicked at|^test .* FAILED|^failures:|^    [A-Za-z0-9_:]+$|^Diff in|^ *--> ' "$d/log" || tail -n 20 "$d/log"; } \
      | awk '!seen[$0]++' | head -40 >"$d/fail.txt"
  fi
  printf '%s\t%s\t%s\n' "$s" "$st" $((SECONDS-t0)) >>"$new"
done

# merge: partial runs replace only the stages they ran
if [ "$mode" = full ] || [ "$mode" = quick ] || [ ! -f "$OUT/summary.tsv" ]; then
  [ "$mode" = quick ] && [ -f "$OUT/summary.tsv" ] && grep -vE '^(clippy|unit)'$'\t' "$OUT/summary.tsv" >>"$new"
  sort -u -k1,1 "$new" >"$OUT/summary.tsv"
else
  { grep -vFf <(printf '%s\t\n' "${stages[@]}") "$OUT/summary.tsv"; cat "$new"; } | sort -u -k1,1 >"$OUT/summary.tsv.tmp"
  mv "$OUT/summary.tsv.tmp" "$OUT/summary.tsv"
fi
rm -f "$new"
r=$(line); echo "${r%% *}" >"$OUT/STATUS"; echo "$r"
[ "${r%% *}" = PASS ]
