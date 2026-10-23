---
phase: 19-security-hardening-of-the-automation-shell-integration-surfa
plan: 02
subsystem: remote-wm
tags: [rust, security, ssh, option-injection, control-path, session-save]

requires: [19-01]
provides:
  - "validate_destination function guarding SSH targets against option injection (leading '-'), control chars, and whitespace"
  - "Fallible build_ssh_args and build_ssh_tmux_cc_args emitting '--' before destination"
  - "Safe ControlPath generation in private directory (<control_dir>/ssh-%C) with 40-byte limit and sanitization"
  - "Pre-split validation in WindowManager::spawn_ssh_pane and WindowManager::connect_ssh_tmux preventing orphan panes"
  - "Private session directory fallback via secure_fs::private_runtime_dir in WindowManager::resolve_session_dir"
affects: [19-05, 19-06, 19-07]

tech-stack:
  added: []
  patterns: ["Pre-split input validation", "-- option delimiter before destination", "Private directory ControlPath"]

key-files:
  created: []
  modified: [src/remote/mod.rs, src/wm/mod.rs, tests/full_suite_battle_and_perf.rs]

key-decisions:
  - "build_ssh_args and build_ssh_tmux_cc_args now take control_dir: Option<&Path> and return Result<Vec<String>>"
  - "validate_destination is checked before self.split_focused to avoid orphan panes"
  - "resolve_session_dir falls back to secure_fs::private_runtime_dir()/sessions instead of a shared /tmp dir"

patterns-established:
  - "Option delimiter -- always precedes destination in external CLI invocations"

requirements-completed: []

duration: 6min
completed: 2026-10-23
status: complete
---

# Phase 19 Plan 02: SSH Destination Hardening & Private ControlPath Summary

**Hardened SSH argument builders against option injection (SEC-R04, D-05), emitted `--` before destination, confined OpenSSH `ControlPath` to the private directory (`ssh-%C`), moved workspace session fallback to the private runtime directory, and added pre-split destination validation in `WindowManager`.**

## Performance & Verification

- **Tests added/updated:** 6 unit tests in `src/remote/mod.rs`, 4 unit tests in `src/wm/mod.rs`, and updated assertions in `tests/full_suite_battle_and_perf.rs`.
- **Suite status:** 154 passing tests across the entire test suite; `cargo clippy --all-targets -- -D warnings` clean.
