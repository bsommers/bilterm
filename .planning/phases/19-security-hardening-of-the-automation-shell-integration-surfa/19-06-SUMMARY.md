---
phase: 19-security-hardening-of-the-automation-shell-integration-surfa
plan: 06
subsystem: cli-wiring
tags: [rust, security, cli, headless, ctl, allow-remote, token-file, snapshot-dir, manpage]

requires: [19-04, 19-05]
provides:
  - "Default Unix domain socket listener for bilterm --headless"
  - "--allow-remote flag requiring --token-file for non-loopback TCP listeners"
  - "--token-file flag with BILTERM_TOKEN_FILE fallback enforcing 0600 file permissions"
  - "--snapshot-dir flag configuring snapshot isolation directory for --headless and --mcp"
  - "ctl --token-file and BILTERM_TOKEN_FILE authentication support targeting default Unix socket"
  - "Updated --help and bash/zsh/fish completions exposing new security flags"
  - "docs/bilterm.1 manual page documenting security flags, defaults, and SECURITY considerations"
  - "Comprehensive integration test suite in tests/phase19_security_cli.rs"
affects: [19-07]

tech-stack:
  added: []
  patterns: ["0600 file-only token loading (no argv token flags)", "Unix socket default endpoint for daemon and ctl"]

key-files:
  created: [tests/phase19_security_cli.rs]
  modified: [src/main.rs, docs/bilterm.1]

key-decisions:
  - "bilterm --headless defaults to Unix domain socket without requiring --listen"
  - "ctl default endpoint is local Unix domain socket"
  - "Bearer token is strictly loaded from 0600 files (no command-line flag accepts raw token values)"
  - "docs/bilterm.1 updated with .SH SECURITY and .SH ENVIRONMENT sections"

patterns-established:
  - "All CLI entry points strictly validate listen policy and pass bearer tokens via file paths only"

requirements-completed: [SEC-R01, SEC-R02]

duration: 10min
completed: 2026-10-23
status: complete
---

# Phase 19 Plan 06: Binary CLI Wiring, Shell Completions & Man Page Summary

**Wired the Phase 19 security hardening policies into the `bilterm` binary (SEC-R01, SEC-R02): default Unix domain socket for `--headless` and `ctl`, `--allow-remote` with mandatory `--token-file`, `--snapshot-dir` for snapshot isolation, shell completions, and updated roff man page (`docs/bilterm.1`).**

## Performance & Verification

- **Tests added:** 7 binary-level integration tests in `tests/phase19_security_cli.rs`:
  - `headless_refuses_non_loopback_without_allow_remote`
  - `headless_allow_remote_requires_token_file`
  - `headless_rejects_world_readable_token_file`
  - `headless_unix_socket_roundtrip_with_ctl`
  - `ctl_token_flow`
  - `help_and_completions_list_new_flags`
  - `man_page_documents_new_flags`
- **Suite status:** 138 unit tests + 9 integration test suites (100% passing); `cargo clippy --all-targets -- -D warnings` 0 warnings.
