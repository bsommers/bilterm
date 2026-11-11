#!/usr/bin/env bash
# Local CI: the merge gate for this repository. Hosted GitHub Actions no longer run automatically
# (.github/workflows/ci.yml is manual-only), so run this before pushing or merging.
#
#   scripts/ci-local.sh              build, full verify (clippy + unit + every tests/*.rs), goldens, cargo-deny
#   scripts/ci-local.sh --quick      build, clippy + unit, goldens, cargo-deny
#   scripts/ci-local.sh --coverage   the default gate, then scripts/coverage.sh (floor ratchet)
#
# Output: one line per stage, then `CI-LOCAL PASS` or `CI-LOCAL FAIL: <stages>`. Logs: target/ci-local/<stage>.log
# Exit code: 0 pass, 1 a stage failed, 2 usage.
# Tests run with a private XDG_RUNTIME_DIR, so a running bilterm on this machine is never touched.
set -uo pipefail
cd "$(dirname "$0")/.."

quick=0 coverage=0
for a in "$@"; do
  case "$a" in
    --quick) quick=1 ;;
    --coverage) coverage=1 ;;
    -h|--help) sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done

OUT=target/ci-local
mkdir -p "$OUT"
RT=$(mktemp -d /tmp/btrt.XXXX) && chmod 700 "$RT"
trap 'rm -rf "$RT"' EXIT
export XDG_RUNTIME_DIR="$RT"

failed=()
stage() { # name, command...
  local name=$1 start=$SECONDS; shift
  if "$@" >"$OUT/$name.log" 2>&1; then
    printf '%-9s PASS %4ss  %s\n' "$name" "$((SECONDS - start))" "$(tail -n 1 "$OUT/$name.log")"
  else
    printf '%-9s FAIL %4ss  %s\n' "$name" "$((SECONDS - start))" "$(tail -n 1 "$OUT/$name.log")"
    failed+=("$name")
  fi
}

stage build cargo build --locked
if [ "$quick" = 1 ]; then stage verify scripts/verify.sh --quick; else stage verify scripts/verify.sh; fi
stage goldens scripts/check_classic_goldens.sh
if command -v cargo-deny >/dev/null 2>&1; then
  stage deny cargo deny check
else
  echo "deny      FAIL    0s  cargo-deny not installed (cargo install cargo-deny --locked)"; failed+=(deny)
fi
[ "$coverage" = 1 ] && stage coverage scripts/coverage.sh

if [ ${#failed[@]} -eq 0 ]; then echo "CI-LOCAL PASS"; else echo "CI-LOCAL FAIL: ${failed[*]} -> $OUT/<stage>.log"; exit 1; fi
