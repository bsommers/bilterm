#!/usr/bin/env bash
# CI helper: turns the last scripts/verify.sh run into log groups + a job-summary section.
#   scripts/ci_report.sh "<title>" [key=value ...]
# Prints each failing stage's fail.txt as a collapsible log group, then appends to
# $GITHUB_STEP_SUMMARY (stdout when unset): the one-line result, the failing stages, the step
# outcomes passed as key=value, and every skipped/ignored test (target/verify/skips.txt).
# A step that is allowed to fail (`continue-on-error`) is therefore still reported as a failure
# here: nothing turns green silently. Always exits 0; the gate step decides the job result.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
OUT=target/verify
title="${1:-verify}"
shift || true

for f in "$OUT"/*/fail.txt; do
  [ -f "$f" ] || continue
  echo "::group::$f"
  cat "$f"
  echo "::endgroup::"
done

line=$(bash scripts/verify.sh --status 2>/dev/null || echo NORUN)
{
  echo "### $title"
  echo
  echo '```text'
  echo "$line"
  echo '```'
  failed=0
  for kv in "$@"; do
    case "$kv" in
      *=failure) failed=1; echo "- :x: \`${kv%%=*}\` **failed**" ;;
      *=success) echo "- :white_check_mark: \`${kv%%=*}\`" ;;
      *=skipped|*=cancelled) echo "- :fast_forward: \`${kv%%=*}\` ${kv##*=}" ;;
      non-blocking-tests=true) nonblocking=1 ;;
      *) ;;
    esac
  done
  if [ "$failed" = 1 ] && [ "${nonblocking:-0}" = 1 ]; then
    echo
    echo "> **Tests FAILED on this cell but do not block the run yet** (tracking item WIN-TESTS, docs/TEST_COVERAGE.md)."
  fi
  if [ -f "$OUT/summary.tsv" ] && grep -q $'\tFAIL\t' "$OUT/summary.tsv"; then
    echo
    echo "| Failing stage | Seconds |"
    echo "|---|---:|"
    awk -F'\t' '$2=="FAIL"{printf "| `%s` | %s |\n",$1,$3}' "$OUT/summary.tsv"
  fi
  if [ -s "$OUT/skips.txt" ]; then
    echo
    echo "<details><summary>Skipped / ignored tests ($(wc -l <"$OUT/skips.txt" | tr -d ' '))</summary>"
    echo
    echo '```text'
    cut -c1-300 "$OUT/skips.txt"
    echo '```'
    echo "</details>"
  fi
  echo
} >>"${GITHUB_STEP_SUMMARY:-/dev/stdout}"
exit 0
