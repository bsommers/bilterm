# Bilterm Session Checkpoint (`v2.1.0`)

- **Timestamp**: 2026-10-28T07:10:00-04:00
- **Branch & Tag**: `main` (tagged `v2.1.0`, pushed to `origin/main`)
- **Installed Binary**: `~/.local/bin/bilterm` & `~/.cargo/bin/bilterm` (`bilterm 2.1.0`)

## 1. Goal & User Decisions (Verbatim)
- User decisions:
  1. *"shorten readme by moving content tonothrr files and linking . add an about section for the repo"* -> Completed (`README.md` reduced from `235` to `101` lines with `## About`; moved gallery & reference tables to `docs/VISUAL_GALLERY.md` and `docs/README.md`).
  2. *"let's plan 1 and 2. later we'll do the rest 3,4"*
  3. *"Yes, can these be launched in parallel? If so, do it with subagents. Update test cases to cover this new functionality."*
  4. *"/release"* -> Released **`v2.1.0`**.

## 2. Completed in `v2.1.0` (`Phases 35 & 36`)
- **Phase 35 (`src/daemon/process.rs`, `src/daemon/mod.rs`, `src/api/rpc_handlers.rs`, `src/main.rs`, `tests/live_daemon_detach_attach.rs`)**:
  - Live background OS daemon process (`spawn_background_daemon_for_session` with `process_group(0)`), per-session `0700`/`0600` Unix socket (`session_daemon_socket_path_in`), `ConnectionRefused`-only stale socket cleanup (`probe_or_cleanup_stale_socket`), `"session.save"` / `"session.restore"` RPC handlers, and `bilterm ls` / `bilterm sessions` CLI command.
- **Phase 36 (`src/mcp_console/mod.rs`, `src/ui/mcp_ui.rs`, `src/ui/mod.rs`, `src/wm/v2_runtime.rs`, `src/input/modal.rs`, `src/input/mouse_wm.rs`, `tests/mcp_interactive_navigation.rs`)**:
  - Interactive `McpRowAction` (`FocusPane`, `SessionNode`, `ApplyTheme`, `None`), highlighted `► ` cursor bar, `Up`/`Down`/`j`/`k`/`Enter` + mouse left-click row activation, `r` (refresh live telemetry), `f` (freeze/thaw node), `n` (branch child session), `m` (Float↔Push), and `d` (cycle dock edge).

## 3. Exact Next Step (When Resuming)
Resume from `docs/plans/checkpoint.md` (or `.planning/CHECKPOINT.md`) to tackle deferred items **3 & 4** (cross-machine `macOS`/`Windows` synchronization & prebuilt release artifacts, or defining the next milestone).
