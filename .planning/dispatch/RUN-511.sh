#!/usr/bin/env bash
# Usage (from the integ worktree or repo root):  .planning/dispatch/RUN-511.sh 09
# Creates ../bilterm-511-NN on branch agy/511-NN from the tip of feat/navigator-rows-and-launch and runs agy headless.
# Refuses to run until the 51.1-01 seam summary is on that branch. Agents COMMIT by path in their own worktree (no push).
set -euo pipefail
n=$1
src=/home/bill/src/ai/bilterm/.claude/worktrees/integ
base=${BASE:-feat/navigator-rows-and-launch}
git -C "$src" cat-file -e "$base:.planning/phases/51.1-persistent-ssh-tmux-sessions/51.1-01-SUMMARY.md" 2>/dev/null \
  || { echo "RUN-511.sh: 51.1-01 seam not merged on $base yet - wait." >&2; exit 2; }
[ -f "$src/.planning/dispatch/51.1-$n.md" ] || { echo "no dispatch file for $n" >&2; exit 2; }
wt="/home/bill/src/ai/bilterm/.claude/worktrees/../bilterm-511-$n"
git -C "$src" worktree add "$wt" -b "agy/511-$n" "$base"
cd "$wt"
settings='{"permissions":{"allow":["command(pwd)","command(ls)","command(cat)","command(grep)","command(head)","command(tail)","command(rg)","command(sed)","command(wc)","command(find)","command(git)","command(cargo)","command(scripts/verify.sh)","command(scripts/check_classic_goldens.sh)","command(cd)","command(echo)","command(awk)","command(python3)","command(true)","command(test)","command(mkdir)","command(tee)","command(diff)","command(sort)","command(uniq)","command(cut)","command(tr)","command(export)","command(rm)"]}}'
prompt="$(cat "$src/.planning/dispatch/51.1-COMMON.md"; echo; cat "$src/.planning/dispatch/51.1-$n.md")"
out=$(agy -p "$prompt" --mode accept-edits --model gemini-3.8-flash-medium --settings "$settings" 2>&1) || rc=$?
printf '%s\n' "$out"
if grep -q 'no output produced' <<<"$out"; then echo "RUN-511.sh: FAILED - agy produced no output (permission denied?)" >&2; exit 1; fi
exit "${rc:-0}"
