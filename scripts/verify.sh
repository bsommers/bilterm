#!/usr/bin/env bash
# Token-cheap verification runner. Prints ONE line; details live in files.
#   scripts/verify.sh              full: clippy, unit, every tests/*.rs (fmt is opt-in: --only fmt; repo has legacy drift)
#   scripts/verify.sh --quick      clippy, unit
#   scripts/verify.sh --failed     re-run only stages that failed last run
#   scripts/verify.sh --only a b   named stages or hierarchical groups g:<name> (see --list / --groups)
#   scripts/verify.sh --list       stage names
#   scripts/verify.sh --groups     group names and member stages
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
# Isolate tests from the user's real saved sessions/state (drawer tests read them).
VERIFY_STATE="$(mktemp -d "${TMPDIR:-/tmp}/bilterm-verify-state.XXXXXX")"
trap 'rm -rf "$VERIFY_STATE"' EXIT
export XDG_STATE_HOME="$VERIFY_STATE"

all_stages() {
  echo clippy; echo unit
  local f; for f in tests/*.rs; do echo "t:$(basename "$f" .rs)"; done
}
quick_stages() { echo clippy; echo unit; }

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
  n=$(($(wc -l <"$OUT/summary.tsv"))); f=$(grep -c $'\tFAIL\t' "$OUT/summary.tsv"); p=$((n-f))
  secs=$(awk -F'\t' '{s+=$3} END{print s+0}' "$OUT/summary.tsv")
  if [ "$f" -eq 0 ]; then echo "PASS $p/$n ${secs}s"
  else echo "FAIL $f/$n: $(awk -F'\t' '$2=="FAIL"{printf "%s ",$1}' "$OUT/summary.tsv")-> $OUT/<stage>/fail.txt"; fi
}

# bash 3.2 (macOS /bin/bash) has no mapfile: read stdin lines into stages[].
read_stages() { stages=(); local l; while IFS= read -r l; do [ -n "$l" ] && stages+=("$l"); done; }

mode=full; stages=()
case "${1:-}" in
  "")       ;;
  --quick)  mode=quick ;;
  --failed) mode=failed ;;
  --list)   all_stages; exit 0 ;;
  --groups) list_groups; exit 0 ;;
  --status) cat "$OUT/STATUS" 2>/dev/null >/dev/null; line; exit 0 ;;
  --only)   shift; mode=only
            exp=$(expand_stages "$@") || { echo "unknown group in: $* (try --groups)"; exit 2; }
            read_stages <<<"$exp" ;;
  *)        sed -n '2,10p' "$0"; exit 2 ;;
esac
case $mode in
  full)   read_stages < <(all_stages) ;;
  quick)  read_stages < <(quick_stages) ;;
  failed) read_stages < <(awk -F'\t' '$2=="FAIL"{print $1}' "$OUT/summary.tsv" 2>/dev/null)
          [ ${#stages[@]} -eq 0 ] && { echo "PASS nothing-failed"; exit 0; } ;;
esac
[ ${#stages[@]} -eq 0 ] && { echo "no stages selected (try --list)"; exit 2; }

new=$(mktemp)
for s in ${stages[@]+"${stages[@]}"}; do
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
  { grep -vFf <(printf '%s\t\n' ${stages[@]+"${stages[@]}"}) "$OUT/summary.tsv"; cat "$new"; } | sort -u -k1,1 >"$OUT/summary.tsv.tmp"
  mv "$OUT/summary.tsv.tmp" "$OUT/summary.tsv"
fi
rm -f "$new"
r=$(line); echo "${r%% *}" >"$OUT/STATUS"; echo "$r"
[ "${r%% *}" = PASS ]
