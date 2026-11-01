#!/usr/bin/env bash
# Usage: .planning/dispatch/RUN.sh 45-03a oia   (run from repo root, in the agy window)
set -euo pipefail
id=$1; slug=$2
git worktree add "../bilterm-$slug" -b "ui/$slug" ui-api-frozen
cd "../bilterm-$slug"
agy -p "$(cat "/home/bill/src/ai/bilterm/.planning/dispatch/$id.md")" --mode accept-edits --model gemini-3.8-flash-medium
