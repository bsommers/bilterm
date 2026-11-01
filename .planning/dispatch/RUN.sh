#!/usr/bin/env bash
# Usage: .planning/dispatch/RUN.sh 45-03a oia   (run from repo root)
# Headless agy cannot prompt, so shell commands the tasks need are allow-listed
# (literal command prefixes, not regex) for this session only (--settings is never saved). Fails loudly if agy did nothing.
set -euo pipefail
id=$1; slug=$2
git worktree add "../bilterm-$slug" -b "ui/$slug" ui-api-frozen
cd "../bilterm-$slug"
settings='{"permissions":{"allow":["command(pwd)","command(ls)","command(cat)","command(grep)","command(head)","command(tail)","command(rg)","command(sed)","command(wc)","command(find)","command(git)","command(cargo)","command(scripts/verify.sh)"]}}'
out=$(agy -p "$(cat "/home/bill/src/ai/bilterm/.planning/dispatch/$id.md")" \
  --mode accept-edits --model gemini-3.8-flash-medium --settings "$settings" 2>&1) || rc=$?
printf '%s\n' "$out"
if grep -q 'no output produced' <<<"$out"; then
  echo "RUN.sh: FAILED - agy produced no output (permission denied?)" >&2
  exit 1
fi
exit "${rc:-0}"
