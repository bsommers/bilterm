#!/usr/bin/env bash
set -euo pipefail
# Launches Bilterm in native GUI mode with the Multi-Agent Coding Swarm preset
exec bilterm --gui \
  -c "$(dirname "$0")/config.toml" \
  --gui-font "JetBrains Mono" \
  --gui-font-size 15 \
  --session multi-agent-swarm
