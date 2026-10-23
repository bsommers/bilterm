---
phase: 19-security-hardening-of-the-automation-shell-integration-surfa
plan: 03
subsystem: temp-files
tags: [rust, security, shell-integration, history-store, asciicast, o_excl, private-dir]

requires: [19-01]
provides:
  - "Per-PID private 0700 runtime directory for shell-integration rc files with 0600 permissions"
  - "Atomic unlinking of pre-planted symlinks during shell integration rc generation"
  - "Exclusive O_CREAT|O_EXCL 0600 writes for default history exports under <private_runtime_dir>/exports/"
  - "Fallback history persistence confined to <private_runtime_dir>/history, guarded against relative writes when empty"
  - "Exclusive O_CREAT|O_EXCL 0600 default asciicast v2 recordings under <private_runtime_dir>/recordings/"
  - "start_recording_private method on VtParser with recording_private state tracking"
affects: [19-05, 19-06, 19-07]

tech-stack:
  added: []
  patterns: ["O_EXCL private recordings", "Per-PID verified shell integration directories", "Unique-suffix export file names"]

key-files:
  created: []
  modified: [src/pty/shell_integration.rs, src/pane/history_store.rs, src/vt/mod.rs, src/pane/colon_command.rs]

key-decisions:
  - "Shell integration rc files written to <private_runtime_dir>/shell-integration-<pid>/"
  - "Default history export uses unique_suffix and fails on collision rather than truncating"
  - "Empty base_dir in HistoryStore disables all persistence paths to avoid cwd-relative file creation"
  - ":record start without arguments writes to <private_runtime_dir>/recordings/ using start_recording_private"

patterns-established:
  - "Bilterm-chosen temp files always reside in verified private 0700 subdirectories with 0600 modes"

requirements-completed: []

duration: 7min
completed: 2026-10-23
status: complete
---

# Phase 19 Plan 03: Private Temporary Files & Recording Hardening Summary

**Migrated all Bilterm-managed temporary files (shell integration rc files, command history exports, history store fallback directory, and asciicast recordings) to verified per-user 0700 private directories with race-free `O_EXCL` 0600 writes (SEC-R03).**

## Performance & Verification

- **Tests added/updated:** 3 tests in `src/pty/shell_integration.rs`, 4 tests in `src/pane/history_store.rs`, 2 tests in `src/vt/mod.rs`, and 1 test in `src/pane/colon_command.rs`.
- **Suite status:** 164 passing tests across the entire test suite; `cargo clippy --all-targets -- -D warnings` clean.
