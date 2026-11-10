#!/usr/bin/env bash
# Token-cheap verification runner. Prints ONE line; details live in files.
#   scripts/verify.sh              full: clippy, unit, every tests/*.rs (fmt is opt-in: --only fmt; repo has legacy drift)
#   scripts/verify.sh --quick      clippy, unit
#   scripts/verify.sh --failed     re-run only stages that failed last run
#   scripts/verify.sh --integration every tests/*.rs, without clippy/unit (CI runs those in the quick job)
#   scripts/verify.sh --shard K/N  with full/--integration: only every N-th tests/*.rs stage, slice K (1-based); CI fan-out
#   scripts/verify.sh --tier <name> smoke|unit|integration|system|perf|stress|torture|security
#   scripts/verify.sh --only a b   named stages or hierarchical groups g:<name> (see --list / --groups)
#   scripts/verify.sh --match <pat> pass pattern to cargo test stages
#   scripts/verify.sh --coverage   run cargo llvm-cov --summary-only -> target/verify/coverage.txt
#   scripts/verify.sh --xvfb       wrap test execution in xvfb-run -a when on Linux (graceful fallback on macOS/Win)
#   scripts/verify.sh --fmt        check cargo fmt
#   scripts/verify.sh --list       stage names
#   scripts/verify.sh --groups     group names and member stages
#   scripts/verify.sh --status     print last result line (no run)
# Result tree (target/verify/):
#   STATUS            PASS | FAIL            (read this first)
#   summary.tsv       stage<TAB>PASS|FAIL<TAB>secs   (read on FAIL)
#   report.md         comprehensive Markdown verification report
#   junit.xml         standard JUnit XML test report
#   coverage.txt      cargo llvm-cov summary (when --coverage requested)
#   skips.txt         every test that did not really run: `#[ignore]`d tests per stage + runtime `SKIPPED` lines
#   <stage>/status    PASS|FAIL   <stage>/fail.txt  compact failure excerpt (<=60 lines, with panic messages)
#   <stage>/log       full output (read only if fail.txt is not enough)
# Exit code: 0 all pass, 1 any fail, 2 usage.
set -uo pipefail
cd "$(dirname "$0")/.."
OUT=target/verify
mkdir -p "$OUT"

# Cross-platform CPU count detection (Linux: nproc, macOS: sysctl, Windows/MSYS: NUMBER_OF_PROCESSORS, fallback: 4)
detect_cpus() {
  if command -v nproc >/dev/null 2>&1; then
    nproc
  elif command -v sysctl >/dev/null 2>&1 && sysctl -n hw.ncpu >/dev/null 2>&1; then
    sysctl -n hw.ncpu
  elif [ -n "${NUMBER_OF_PROCESSORS:-}" ]; then
    echo "$NUMBER_OF_PROCESSORS"
  else
    echo 4
  fi
}
export CARGO_BUILD_JOBS="$(detect_cpus)"

# Cross-platform environment and temp directory isolation:
# Covers Linux/POSIX (XDG_*), macOS ($HOME, XDG_*), and Windows/MSYS ($USERPROFILE, $APPDATA, $LOCALAPPDATA).
VERIFY_STATE="$(mktemp -d "${TMPDIR:-/tmp}/bilterm-verify-state.XXXXXX")"
trap 'rm -rf "$VERIFY_STATE"' EXIT
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export XDG_STATE_HOME="$VERIFY_STATE/state"
export XDG_CONFIG_HOME="$VERIFY_STATE/config"
export XDG_DATA_HOME="$VERIFY_STATE/data"
export HOME="$VERIFY_STATE/home"
export USERPROFILE="$VERIFY_STATE/home"
export APPDATA="$VERIFY_STATE/appdata/roaming"
export LOCALAPPDATA="$VERIFY_STATE/appdata/local"
mkdir -p "$XDG_STATE_HOME" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$HOME" "$APPDATA" "$LOCALAPPDATA"

# Git Bash / MSYS exports SHELL=/usr/bin/bash, a path native Windows programs cannot start. A real
# Windows launch has no SHELL at all (the product then falls back to COMSPEC), so tests see the same.
case "$(uname -s 2>/dev/null)" in MINGW*|MSYS*|CYGWIN*) unset SHELL ;; esac

MATCH_PATTERN=""
SHARD=""
DO_COVERAGE=false
USE_XVFB=false
tier_name=""

all_stages() {
  echo clippy; echo unit
  local f; for f in tests/*.rs; do echo "t:$(basename "$f" .rs)"; done
}
quick_stages() { echo clippy; echo unit; }
integration_stages() { local f; for f in tests/*.rs; do echo "t:$(basename "$f" .rs)"; done; }
# Keeps clippy/unit, and of the t:* stages only slice K of N (round-robin over the sorted list, so
# slices stay balanced as test files are added).
shard_filter() {
  local k="${SHARD%%/*}" n="${SHARD##*/}"
  awk -v k="$k" -v n="$n" '/^t:/ { if (i++ % n == k - 1) print; next } { print }'
}

tier_stages() {
  case "$1" in
    smoke)
      echo unit
      ;;
    unit)
      echo clippy
      echo unit
      ;;
    integration)
      for s in \
        t:composite_pane_snapshots \
        t:phase2_prompt_ergonomics \
        t:phase3_window_manager \
        t:phase4_retro_aesthetics \
        t:phase5_tmux_and_platform \
        t:slide_out_drawer_and_telemetry \
        t:itermcolors_and_paste_fixes \
        t:theme_import_coverage \
        t:drawer_classic_parity \
        t:drawer_behaviour_coverage \
        t:panel_dock \
        t:panel_pin_scale \
        t:phase51_panel_tiling \
        t:phase53_window_manage \
        t:layout_profile_coverage \
        t:layout_profiles_and_autosave \
        t:pane_typography_coverage \
        t:design_system_coverage \
        t:linux_session_regression_and_gui_scaling \
        t:high_res_pixel_and_ssh_canvas \
        t:font_scaling_and_dec_line_attrs \
        t:tektronix_and_graphical_canvas \
        t:v040_v050_gpu_ghostty_tmux_graphics \
        t:copy_paste_unicode; do
        if [ -f "tests/${s#t:}.rs" ]; then echo "$s"; fi
      done
      ;;
    system)
      for s in \
        t:phase50_1_e2e \
        t:phase51_1_e2e \
        t:phase51_integration \
        t:v110_v120_v130_automation_api_mcp_playbooks \
        t:mcp_interactive_navigation \
        t:mcp_list_edge_status \
        t:api_conformance \
        t:api_error_paths \
        t:api_http \
        t:api_mcp_resources \
        t:api_server_edges \
        t:phase50_agents_window \
        t:live_daemon_detach_attach \
        t:bilterm_live_driver_suite; do
        if [ -f "tests/${s#t:}.rs" ]; then echo "$s"; fi
      done
      ;;
    perf)
      for s in t:full_suite_battle_and_perf t:phase64_traffic_core; do
        if [ -f "tests/${s#t:}.rs" ]; then echo "$s"; fi
      done
      ;;
    stress|torture)
      for s in t:terminal_torture_and_stress t:vttest_esctest_fuzz_torture; do
        if [ -f "tests/${s#t:}.rs" ]; then echo "$s"; fi
      done
      ;;
    security)
      for s in t:phase19_security_cli t:security_hardening_audit; do
        if [ -f "tests/${s#t:}.rs" ]; then echo "$s"; fi
      done
      ;;
    *)
      return 1
      ;;
  esac
}

group_stages() {
  case "$1" in
    g:lint)     printf '%s\n' clippy unit ;;
    g:ui)       printf '%s\n' t:composite_pane_snapshots t:phase4_retro_aesthetics t:slide_out_drawer_and_telemetry t:itermcolors_and_paste_fixes t:linux_session_regression_and_gui_scaling t:high_res_pixel_and_ssh_canvas ;;
    g:vt)       printf '%s\n' t:vt_worker_and_snapshot_seam t:pty_lifecycle t:vttest_esctest_fuzz_torture t:font_scaling_and_dec_line_attrs t:tektronix_and_graphical_canvas t:v040_v050_gpu_ghostty_tmux_graphics ;;
    g:wm)       printf '%s\n' t:phase2_prompt_ergonomics t:phase3_window_manager t:phase5_tmux_and_platform t:layout_profiles_and_autosave t:live_daemon_detach_attach t:smooth_and_mainframe_page_scrolling t:tab_completion_and_streaming ;;
    g:api)      printf '%s\n' t:mcp_interactive_navigation t:v110_v120_v130_automation_api_mcp_playbooks t:v020_features t:v030_features t:v060_v070_v100_release_features t:v2_phases_30_to_34_integration t:session_lessons_and_regression_suite ;;
    g:security) printf '%s\n' t:phase19_security_cli ;;
    g:perf)     printf '%s\n' t:full_suite_battle_and_perf t:terminal_torture_and_stress ;;
    g:find)     printf '%s\n' t:phase50_1_find_seam t:phase50_1_redact t:phase50_1_registry t:phase50_1_rank t:phase50_1_remote t:phase50_1_find_ui t:phase50_1_find_input t:phase50_1_surfaces t:phase50_1_capture t:phase50_1_saved t:phase50_1_find_ops t:phase50_1_api t:phase50_1_parity t:phase50_1_e2e ;;
    *)          return 1 ;;
  esac
}

list_groups() {
  local g; for g in g:lint g:ui g:vt g:wm g:api g:security g:perf; do
    printf '%s\t%s\n' "$g" "$(group_stages "$g" | paste -sd' ' -)"
  done
  printf '%s\t%s\n' g:find "$(group_stages g:find | paste -sd' ' -)"
}

expand_stages() {
  local s; for s in "$@"; do
    if [[ "$s" == g:* ]]; then group_stages "$s" || return 1; else echo "$s"; fi
  done
}

cmd_for() {
  local extra=""
  if [ -n "$MATCH_PATTERN" ]; then
    extra=" -- $MATCH_PATTERN"
  fi
  case "$1" in
    fmt)    echo "cargo fmt --check" ;;
    clippy) echo "cargo clippy -q --all-targets -- -D warnings" ;;
    unit)   echo "cargo test -q --lib --bins --no-fail-fast$extra" ;;
    t:*)    echo "cargo test -q --test ${1#t:} --no-fail-fast$extra" ;;
    *)      return 1 ;;
  esac
}

line() { # one-line result from summary.tsv
  [ -f "$OUT/summary.tsv" ] || { echo "NORUN"; return; }
  local n p f secs
  n=$(($(wc -l <"$OUT/summary.tsv"))); f=$(grep -c $'\tFAIL\t' "$OUT/summary.tsv" || true); p=$((n-f))
  secs=$(awk -F'\t' '{s+=$3} END{print s+0}' "$OUT/summary.tsv")
  if [ "$f" -eq 0 ]; then echo "PASS $p/$n ${secs}s"
  else echo "FAIL $f/$n: $(awk -F'\t' '$2=="FAIL"{printf "%s ",$1}' "$OUT/summary.tsv")-> $OUT/<stage>/fail.txt"; fi
}

generate_reports() {
  local status_str="$1"
  local total_stages="$2"
  local passed_stages="$3"
  local failed_stages="$4"
  local total_secs="$5"
  local iso_date
  iso_date=$(date -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || date +"%Y-%m-%dT%H:%M:%SZ")

  # Generate target/verify/report.md
  cat <<EOF >"$OUT/report.md"
# Bilterm Verification Report

- **Overall Status:** $status_str
- **Stages Run:** $total_stages
- **Passed:** $passed_stages
- **Failed:** $failed_stages
- **Total Duration:** ${total_secs}s
- **Timestamp:** $iso_date

## Summary Table

| Stage | Status | Duration (s) |
|---|---|---|
EOF

  while IFS=$'\t' read -r stage st secs; do
    [ -z "$stage" ] && continue
    echo "| \`$stage\` | **$st** | ${secs}s |" >>"$OUT/report.md"
  done <"$OUT/summary.tsv"

  if [ "$failed_stages" -gt 0 ]; then
    cat <<EOF >>"$OUT/report.md"

## Failure Details

EOF
    while IFS=$'\t' read -r stage st secs; do
      if [ "$st" = "FAIL" ]; then
        local stage_dir="$OUT/${stage//:/_}"
        echo "### \`$stage\`" >>"$OUT/report.md"
        echo '```text' >>"$OUT/report.md"
        if [ -f "$stage_dir/fail.txt" ]; then
          cat "$stage_dir/fail.txt" >>"$OUT/report.md"
        elif [ -f "$stage_dir/log" ]; then
          tail -n 25 "$stage_dir/log" >>"$OUT/report.md"
        fi
        echo '```' >>"$OUT/report.md"
        echo "" >>"$OUT/report.md"
      fi
    done <"$OUT/summary.tsv"
  fi

  # Generate target/verify/junit.xml
  cat <<EOF >"$OUT/junit.xml"
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="bilterm-verify" tests="$total_stages" failures="$failed_stages" errors="0" time="$total_secs">
  <testsuite name="verification-suite" tests="$total_stages" failures="$failed_stages" errors="0" time="$total_secs" timestamp="$iso_date">
EOF

  while IFS=$'\t' read -r stage st secs; do
    [ -z "$stage" ] && continue
    local stage_dir="$OUT/${stage//:/_}"
    if [ "$st" = "FAIL" ]; then
      cat <<EOF >>"$OUT/junit.xml"
    <testcase name="$stage" classname="verify" time="$secs">
      <failure message="Stage $stage failed"><![CDATA[
EOF
      if [ -f "$stage_dir/fail.txt" ]; then
        sed 's/]]>/]]&gt;/g' "$stage_dir/fail.txt" >>"$OUT/junit.xml"
      elif [ -f "$stage_dir/log" ]; then
        tail -n 25 "$stage_dir/log" | sed 's/]]>/]]&gt;/g' >>"$OUT/junit.xml"
      fi
      cat <<EOF >>"$OUT/junit.xml"
]]></failure>
    </testcase>
EOF
    else
      cat <<EOF >>"$OUT/junit.xml"
    <testcase name="$stage" classname="verify" time="$secs" />
EOF
    fi
  done <"$OUT/summary.tsv"

  cat <<EOF >>"$OUT/junit.xml"
  </testsuite>
</testsuites>
EOF
}

mode=full
stages=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --quick)
      mode=quick
      shift
      ;;
    --failed)
      mode=failed
      shift
      ;;
    --integration)
      mode=integration
      shift
      ;;
    --shard)
      if [[ $# -lt 2 ]] || ! [[ "$2" =~ ^[1-9][0-9]*/[1-9][0-9]*$ ]] || [ "${2%%/*}" -gt "${2##*/}" ]; then
        echo "--shard needs K/N with 1 <= K <= N (e.g. --shard 2/3)" >&2
        exit 2
      fi
      SHARD="$2"
      shift 2
      ;;
    --list)
      all_stages
      exit 0
      ;;
    --groups)
      list_groups
      exit 0
      ;;
    --status)
      cat "$OUT/STATUS" 2>/dev/null >/dev/null || true
      line
      exit 0
      ;;
    --fmt)
      mode=only
      stages+=("fmt")
      shift
      ;;
    --tier)
      if [[ $# -lt 2 ]]; then
        echo "Missing argument for --tier (expected: smoke|unit|integration|system|perf|stress|torture|security)" >&2
        exit 2
      fi
      mode=tier
      tier_name="$2"
      shift 2
      ;;
    --match)
      if [[ $# -lt 2 ]]; then
        echo "Missing argument for --match <pattern>" >&2
        exit 2
      fi
      MATCH_PATTERN="$2"
      shift 2
      ;;
    --coverage)
      DO_COVERAGE=true
      shift
      ;;
    --xvfb)
      USE_XVFB=true
      shift
      ;;
    --only)
      shift
      mode=only
      while [[ $# -gt 0 ]] && [[ "$1" != --* ]]; do
        exp=$(expand_stages "$1") || { echo "unknown group in: $1 (try --groups)"; exit 2; }
        while IFS= read -r l; do [ -n "$l" ] && stages+=("$l"); done <<<"$exp"
        shift
      done
      if [ ${#stages[@]} -eq 0 ]; then
        echo "Missing stages for --only" >&2
        exit 2
      fi
      ;;
    *)
      echo "Usage: scripts/verify.sh [OPTIONS] [STAGE...]" >&2
      echo "  scripts/verify.sh              full: clippy, unit, every tests/*.rs" >&2
      echo "  scripts/verify.sh --quick      clippy, unit" >&2
      echo "  scripts/verify.sh --failed     re-run only stages that failed last run" >&2
      echo "  scripts/verify.sh --integration every tests/*.rs without clippy/unit" >&2
      echo "  scripts/verify.sh --shard K/N  with full/--integration: slice K of N of the tests/*.rs stages" >&2
      echo "  scripts/verify.sh --tier <name> smoke|unit|integration|system|perf|stress|torture|security" >&2
      echo "  scripts/verify.sh --only a b   named stages or groups g:<name> (--list / --groups)" >&2
      echo "  scripts/verify.sh --match <pat> pass pattern to cargo test stages" >&2
      echo "  scripts/verify.sh --coverage   run cargo llvm-cov --summary-only -> target/verify/coverage.txt" >&2
      echo "  scripts/verify.sh --xvfb       wrap test execution in xvfb-run -a when on Linux" >&2
      echo "  scripts/verify.sh --fmt        check cargo fmt" >&2
      echo "  scripts/verify.sh --list       list all stage names" >&2
      echo "  scripts/verify.sh --groups     list group names and stages" >&2
      echo "  scripts/verify.sh --status     print last result line (no run)" >&2
      exit 2
      ;;
  esac
done

# bash 3.2 (macOS /bin/bash) has no mapfile: read stdin lines into stages[].
read_stages() { stages=(); local l; while IFS= read -r l; do [ -n "$l" ] && stages+=("$l"); done; }

case $mode in
  full)   if [ -n "$SHARD" ]; then read_stages < <(all_stages | shard_filter); else read_stages < <(all_stages); fi ;;
  integration)
          if [ -n "$SHARD" ]; then read_stages < <(integration_stages | shard_filter); else read_stages < <(integration_stages); fi ;;
  quick)  read_stages < <(quick_stages) ;;
  failed) read_stages < <(awk -F'\t' '$2=="FAIL"{print $1}' "$OUT/summary.tsv" 2>/dev/null)
          [ ${#stages[@]} -eq 0 ] && { echo "PASS nothing-failed"; exit 0; } ;;
  tier)   exp=$(tier_stages "$tier_name") || {
            echo "unknown tier: $tier_name (expected: smoke|unit|integration|system|perf|stress|torture|security)" >&2
            exit 2
          }
          read_stages <<<"$exp" ;;
  only)   ;;
esac
[ ${#stages[@]} -eq 0 ] && { echo "no stages selected (try --list)"; exit 2; }

# OS-aware headless display detection:
# - Linux: Use xvfb-run -a when available and requested via --xvfb.
# - macOS (Darwin) & Windows (MSYS/MINGW/CYGWIN): native headless/GUI dump without requiring xvfb-run.
OS_NAME="$(uname -s 2>/dev/null || echo "Unknown")"
HAVE_XVFB=false
case "$OS_NAME" in
  Linux*)
    if command -v xvfb-run >/dev/null 2>&1; then
      HAVE_XVFB=true
    fi
    ;;
  Darwin*|MINGW*|MSYS*|CYGWIN*)
    HAVE_XVFB=false
    ;;
esac

# Pre-build every selected test binary in ONE cargo invocation: cargo then compiles them in parallel
# across all cores, instead of one binary per stage. A build error here is not fatal; the stage
# that owns the broken target reports it (with its own log) when it runs below.
if [ ${#stages[@]} -ge 4 ]; then
  prebuild=()
  for s in "${stages[@]}"; do
    case "$s" in
      unit) prebuild+=(--lib --bins) ;;
      t:*)  prebuild+=(--test "${s#t:}") ;;
    esac
  done
  if [ ${#prebuild[@]} -gt 0 ]; then
    cargo test -q --no-run "${prebuild[@]}" >"$OUT/prebuild.log" 2>&1 || true
  fi
fi

new=$(mktemp)
for s in ${stages[@]+"${stages[@]}"}; do
  c=$(cmd_for "$s") || { echo "unknown stage: $s (try --list)"; exit 2; }
  d="$OUT/${s//:/_}"; rm -rf "$d"; mkdir -p "$d"
  t0=$SECONDS
  if [ "$USE_XVFB" = true ] && [ "$HAVE_XVFB" = true ]; then
    if xvfb-run -a bash -c "$c" >"$d/log" 2>&1; then st=PASS; else st=FAIL; fi
  else
    if bash -c "$c" >"$d/log" 2>&1; then st=PASS; else st=FAIL; fi
  fi
  echo "$st" >"$d/status"
  if [ "$st" = FAIL ]; then
    # Keep the panic MESSAGE (the lines after `panicked at`): without it a CI-only failure cannot
    # be diagnosed from fail.txt. Long lines are clipped so one huge assertion cannot flood it.
    { awk '/panicked at/ { print; keep = 4; hit = 1; next }
           keep > 0 { keep--; if ($0 !~ /^note: run with/ && $0 != "") print; next }
           /^(error|warning)(\[|:)|^---- |^test .* FAILED|^failures:|^    [A-Za-z0-9_:]+$|^Diff in|^ *--> / { print; hit = 1 }
           END { exit !hit }' "$d/log" || tail -n 20 "$d/log"; } \
      | cut -c1-600 | awk '!seen[$0]++' | head -60 >"$d/fail.txt"
  fi
  printf '%s\t%s\t%s\n' "$s" "$st" $((SECONDS-t0)) >>"$new"
done

# merge: partial runs replace only the stages they ran
if { [ "$mode" = full ] && [ -z "$SHARD" ]; } || [ ! -f "$OUT/summary.tsv" ]; then
  sort -u -k1,1 "$new" >"$OUT/summary.tsv"
elif [ "$mode" = quick ]; then
  grep -vE '^(clippy|unit)'$'\t' "$OUT/summary.tsv" >>"$new" 2>/dev/null || true
  sort -u -k1,1 "$new" >"$OUT/summary.tsv"
else
  { grep -vFf <(printf '%s\t\n' ${stages[@]+"${stages[@]}"}) "$OUT/summary.tsv" 2>/dev/null || true; cat "$new"; } | sort -u -k1,1 >"$OUT/summary.tsv.tmp"
  mv "$OUT/summary.tsv.tmp" "$OUT/summary.tsv"
fi
rm -f "$new"

if [ "$DO_COVERAGE" = true ]; then
  if command -v cargo-llvm-cov >/dev/null 2>&1 || cargo llvm-cov --version >/dev/null 2>&1; then
    cargo llvm-cov --summary-only >"$OUT/coverage.txt" 2>&1 || true
  fi
fi

# Nothing may be skipped silently: list `#[ignore]`d tests (with the stage they belong to) and
# every runtime `SKIPPED <test>: <reason>` line (tests/common `skip()`) from this run's logs.
{
  for d in "$OUT"/t_* "$OUT"/unit; do
    [ -f "$d/log" ] || continue
    ign=$(awk '/^test result:/ { for (i = 1; i < NF; i++) if ($(i + 1) ~ /^ignored/) n += $i } END { print n + 0 }' "$d/log")
    [ "$ign" -gt 0 ] && printf 'IGNORED\t%s\t%s test(s) marked #[ignore] (reasons: cargo test --test %s -- --ignored --list)\n' "$(basename "$d")" "$ign" "$(basename "$d" | sed 's/^t_//')"
    grep -ho 'SKIPPED .*' "$d/log" 2>/dev/null | sed "s|^|$(basename "$d")\t|"
  done
} >"$OUT/skips.txt" 2>/dev/null || true

r=$(line)
st_overall="${r%% *}"
echo "$st_overall" >"$OUT/STATUS"

n_total=$(($(wc -l <"$OUT/summary.tsv")))
n_fail=$(grep -c $'\tFAIL\t' "$OUT/summary.tsv" || true)
n_pass=$((n_total - n_fail))
s_secs=$(awk -F'\t' '{s+=$3} END{print s+0}' "$OUT/summary.tsv")

generate_reports "$st_overall" "$n_total" "$n_pass" "$n_fail" "$s_secs"

echo "$r"
[ "$st_overall" = PASS ]
