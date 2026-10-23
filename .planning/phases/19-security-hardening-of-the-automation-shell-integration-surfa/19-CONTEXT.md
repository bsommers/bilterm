# Phase 19 — Context (User Decisions)

**Captured:** 2026-10-23 (from `/gsd-plan-phase 19` session; no discuss-phase run)
**Sources:** `.planning/CHECKPOINT.md` §2, `.planning/reviews/ARCHITECTURE_REVIEW_PLAN.md` §5.1, `19-RESEARCH.md`

## Requirement ID mapping (inferred — SEC-R* not defined in REQUIREMENTS.md)

- **SEC-R01** — headless listener: Unix socket default, `--allow-remote`, bearer token
- **SEC-R02** — `workspace.snapshot` `ppm_path` confinement
- **SEC-R03** — per-user `0700` private dir + `O_EXCL` writes (shell-integration rc, history exports/fallback, `.cast`, SSH ControlPath, session-save fallback)
- **SEC-R04** — SSH destination option-injection guard

## Locked Decisions

- **D-01 Token wire protocol:** per-connection `auth.login {token}` handshake. When a token is configured, the first request on a connection MUST be `auth.login`; any other request is rejected and the connection closed. Request shapes are otherwise unchanged. `ctl` gains `--token-file`.
- **D-02 Playbooks unconfined:** only RPC/MCP clients are confined to the snapshot dir. Playbook `export_ppm` (operator-authored, local) uses a trusted, unconfined internal helper; the existing playbook test keeps passing unchanged.
  - **Amended 2026-10-23 (user):** playbooks submitted over RPC/MCP (`playbook.run`, `bilterm_run_playbook`) ARE confined (`SnapshotPolicy::Confined`); only CLI `--playbook` files are trusted/unconfined. Closes the RPC bypass.
- **D-03 Snapshot dir:** default `$XDG_STATE_HOME/bilterm/snapshots` (fallback `~/.local/state/bilterm/snapshots`), overridable with `--snapshot-dir`. RPC `ppm_path` accepts only names resolving inside it (traversal, absolute paths, symlink escape rejected).
  - **Amended 2026-10-23 (user):** bare file names only (single `Component::Normal`, e.g. `shot.ppm`); `sub/x.ppm` is rejected.
- **D-04 Loopback TCP:** token optional on loopback (`127.0.0.1` / `::1`); honored if supplied. Non-loopback requires `--allow-remote` AND a token from a `0600` file. Unix socket (`0600`, in `0700` dir under `$XDG_RUNTIME_DIR`) is the default when `--headless` has no `--listen`.
- **D-05 SSH builders:** `build_ssh_args` / `build_ssh_tmux_cc_args` return `Result`; reject empty, leading `-`, control chars; emit `--` before destination. ControlPath uses `<private dir>/ssh-%C` (sun_path length).
- **D-06 No new runtime deps** unless unavoidable (`libc` + `tokio` already present); helpers take a base path parameter (no env mutation in tests); Unix-only bits behind `#[cfg(unix)]` with Windows loopback-TCP+token fallback.

## Carried constraints

- Gate: `cargo test` (baseline 127 pass) + `cargo clippy --all-targets -- -D warnings`.
- Tests rewrite `docs/assets/screenshot-3278-inspector.svg` — revert before committing.
- Update `--help`, shell completions, man page, docs for new flags/defaults.
