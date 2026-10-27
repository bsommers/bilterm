#!/usr/bin/env bash
set -euo pipefail
# Runs the deterministic headless robot QA playbook in CI/CD
bilterm run "$(dirname "$0")/playbook.toml"
