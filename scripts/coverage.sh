#!/usr/bin/env bash
# Line-coverage runner + ratchet. Prints ONE line; details live in files (same contract as verify.sh).
#   scripts/coverage.sh                 instrumented run of unit + every tests/*.rs, then report + floor check
#   scripts/coverage.sh --quick         instrumented run of unit tests only (fast; floor check is skipped)
#   scripts/coverage.sh --report        rebuild the tables from the existing target/coverage/lcov.info (no test run)
#   scripts/coverage.sh --gaps [N]      print the N (default 25) largest uncovered regions from lcov.info (no test run)
#   scripts/coverage.sh --update-floor  rewrite .github/coverage-floor.txt from the existing lcov.info (no test run)
# Needs cargo-llvm-cov (`cargo install cargo-llvm-cov --locked`) and the llvm-tools component
# (`rustup component add llvm-tools-preview`). No external coverage service is involved.
# Result tree (target/coverage/):
#   STATUS        PASS | FAIL
#   modules.tsv   module<TAB>covered<TAB>lines<TAB>percent<TAB>floor<TAB>OK|DROP|NEW
#   summary.md    the same table as Markdown (CI appends it to the job summary)
#   gaps.txt      largest uncovered regions (file:first-last<TAB>lines)
#   lcov.info     raw lcov tracefile          html/index.html   browsable report
#   run.log       output of the instrumented test run (test failures do not fail coverage; they are counted)
# Floor file (.github/coverage-floor.txt): `module<TAB>min_percent`. A module below its floor is a FAIL.
# Exit code: 0 pass, 1 a module dropped below its floor (or the run produced no data), 2 usage/tooling.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
OUT=target/coverage
FLOOR=.github/coverage-floor.txt
# Slack (percentage points) subtracted when --update-floor records a floor: instrumented runs of
# timing-dependent tests differ slightly between machines, so the floor sits just under the measurement.
SLACK="${COVERAGE_FLOOR_SLACK:-2}"
mkdir -p "$OUT"

mode=full
gaps_n=25
case "${1:-}" in
  "") ;;
  --quick) mode=quick ;;
  --report) mode=report ;;
  --gaps) mode=gaps; gaps_n="${2:-25}" ;;
  --update-floor) mode=update ;;
  *) sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac

# Per-module table from lcov: src/<dir>/** -> <dir>; src/<file>.rs -> (crate root).
modules_tsv() {
  awk -F: '
    /^SF:/ { f=substr($0,4); gsub(/\\/,"/",f); i=index(f,"/src/"); r=(i?substr(f,i+5):f); sub(/^src\//,"",r)
             n=index(r,"/"); m=(n?substr(r,1,n-1):"(crate root)") }
    /^LF:/ { lf[m]+=$2 }
    /^LH:/ { lh[m]+=$2 }
    END { for (k in lf) { printf "%s\t%d\t%d\t%.1f\n", k, lh[k], lf[k], (lf[k]?100*lh[k]/lf[k]:0); tl+=lf[k]; th+=lh[k] }
          printf "TOTAL\t%d\t%d\t%.1f\n", th, tl, (tl?100*th/tl:0) }' "$OUT/lcov.info" | sort
}

# Largest runs of consecutive never-executed lines (a covered line ends a run).
gaps() {
  awk -F'[:,]' '
    function flush() { if (n>0) printf "%d\t%s:%d-%d\n", n, r, s, e; n=0 }
    /^SF:/ { flush(); f=substr($0,4); gsub(/\\/,"/",f); i=index(f,"/src/"); r=(i?substr(f,i+1):f) }
    /^DA:/ { if ($3==0) { if (n==0) s=$2; e=$2; n++ } else flush() }
    /^end_of_record/ { flush() }' "$OUT/lcov.info" | sort -rn | head -n "$1" | awk -F'\t' '{printf "%s\t%s\n", $2, $1}'
}

report() {
  [ -s "$OUT/lcov.info" ] || { echo "FAIL no coverage data ($OUT/lcov.info missing or empty)"; echo FAIL >"$OUT/STATUS"; exit 1; }
  local drops=0 m cov tot pct fl st
  : >"$OUT/modules.tsv"
  while IFS=$'\t' read -r m cov tot pct; do
    fl=$(awk -F'\t' -v m="$m" '$1==m{print $2}' "$FLOOR" 2>/dev/null | head -1)
    if [ -z "$fl" ]; then st=NEW; fl=-
    elif awk -v p="$pct" -v f="$fl" 'BEGIN{exit !(p+0 < f+0)}'; then st=DROP; drops=$((drops+1))
    else st=OK; fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$m" "$cov" "$tot" "$pct" "$fl" "$st" >>"$OUT/modules.tsv"
  done < <(modules_tsv)
  gaps 25 >"$OUT/gaps.txt"
  {
    echo "## Line coverage per module"; echo
    echo "| Module | Covered | Lines | % | Floor | |"; echo "|---|---:|---:|---:|---:|---|"
    awk -F'\t' '{printf "| `%s` | %s | %s | %s | %s | %s |\n",$1,$2,$3,$4,$5,$6}' "$OUT/modules.tsv"
    echo; echo "<details><summary>25 largest uncovered regions</summary>"; echo; echo '```text'
    cat "$OUT/gaps.txt"; echo '```'; echo "</details>"
    [ -f "$OUT/test-failures.txt" ] && [ -s "$OUT/test-failures.txt" ] && {
      echo; echo "**Tests failing under instrumentation (not part of the coverage verdict):**"; echo '```text'
      head -40 "$OUT/test-failures.txt"; echo '```'; }
  } >"$OUT/summary.md"
  local total; total=$(awk -F'\t' '$1=="TOTAL"{print $4}' "$OUT/modules.tsv")
  if [ "$mode" = quick ]; then
    echo PASS >"$OUT/STATUS"; echo "PASS coverage ${total}% (unit tests only; floor not checked) -> $OUT/summary.md"
  elif [ "$drops" -gt 0 ]; then
    echo FAIL >"$OUT/STATUS"
    echo "FAIL coverage ${total}%: $(awk -F'\t' '$6=="DROP"{printf "%s %s%%<%s%% ",$1,$4,$5}' "$OUT/modules.tsv")-> $OUT/summary.md"
    exit 1
  else
    echo PASS >"$OUT/STATUS"; echo "PASS coverage ${total}% (no module below its floor) -> $OUT/summary.md"
  fi
}

case $mode in
  report) report; exit 0 ;;
  gaps)   [ -s "$OUT/lcov.info" ] || { echo "no $OUT/lcov.info (run scripts/coverage.sh first)" >&2; exit 2; }
          gaps "$gaps_n"; exit 0 ;;
  update) [ -s "$OUT/lcov.info" ] || { echo "no $OUT/lcov.info (run scripts/coverage.sh first)" >&2; exit 2; }
          { echo "# Per-module line-coverage floor (percent). Checked by scripts/coverage.sh; a module below its floor fails CI."
            echo "# Regenerate after raising coverage: scripts/coverage.sh && scripts/coverage.sh --update-floor"
            echo "# (recorded as measured minus ${SLACK} points of run-to-run slack, floored). Never lower a value by hand."
            modules_tsv | awk -F'\t' -v s="$SLACK" '{v=int($4-s); if (v<0) v=0; printf "%s\t%d\n",$1,v}'
          } >"$FLOOR"
          echo "PASS floor updated -> $FLOOR"; exit 0 ;;
esac

cargo llvm-cov --version >/dev/null 2>&1 || { echo "FAIL cargo-llvm-cov not installed (cargo install cargo-llvm-cov --locked)"; exit 2; }

# Same isolation as verify.sh: tests must never see the developer's real HOME / XDG state.
# Short root on purpose: macOS's per-user TMPDIR (/var/folders/.../T, ~50 bytes) pushes Unix socket
# paths past sun_path (104 bytes) and ssh ControlPath past its limit, and long temp paths get
# clipped in rendered frames. Tests therefore get a short TMPDIR and a private XDG_RUNTIME_DIR.
if [ -d /tmp ] && [ -w /tmp ]; then STATE="$(mktemp -d /tmp/btv.XXXXXX)"; else STATE="$(mktemp -d "${TMPDIR:-/tmp}/btv.XXXXXX")"; fi
trap 'rm -rf "$STATE"' EXIT
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export XDG_STATE_HOME="$STATE/state" XDG_CONFIG_HOME="$STATE/config" XDG_DATA_HOME="$STATE/data"
export HOME="$STATE/home" USERPROFILE="$STATE/home" APPDATA="$STATE/appdata/roaming" LOCALAPPDATA="$STATE/appdata/local"
export TMPDIR="$STATE/t" XDG_RUNTIME_DIR="$STATE/r"
mkdir -p "$TMPDIR" "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
mkdir -p "$XDG_STATE_HOME" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$HOME" "$APPDATA" "$LOCALAPPDATA"

sel=()
[ "$mode" = quick ] && sel=(--lib --bins)
cargo llvm-cov clean --workspace >/dev/null 2>&1 || true
# --ignore-run-fail: a test that fails only under instrumentation must not hide the coverage numbers.
cargo llvm-cov --no-report --ignore-run-fail ${sel[@]+"${sel[@]}"} -- --test-threads="${COVERAGE_TEST_THREADS:-8}" >"$OUT/run.log" 2>&1
run_rc=$?
grep -E '^test .* FAILED$|^error: test failed' "$OUT/run.log" >"$OUT/test-failures.txt" || true
cargo llvm-cov report --lcov --output-path "$OUT/lcov.info" >>"$OUT/run.log" 2>&1 \
  || { echo "FAIL coverage report failed (run rc=$run_rc) -> $OUT/run.log"; echo FAIL >"$OUT/STATUS"; exit 1; }
cargo llvm-cov report --html --output-dir "$OUT" >>"$OUT/run.log" 2>&1 || true
report
