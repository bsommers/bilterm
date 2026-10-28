# Phase 35 Plan: Live Background OS Process Daemonization for `:detach` / `bilterm attach` (`v2.1`)

## Objective
Enable true background OS process survival across `:detach` (`Ctrl+X D` / Exit Guard `[d]`) and `bilterm attach [session]` (`bilterm -a` / `:attach`) by spawning/connecting to a detached background `bilterm --daemon --session <name>` process over a private `0600` Unix domain socket (`~/.local/state/bilterm/daemon-<session>.sock`), while keeping full multi-pane `SplitTree` + `ZStack` window layout persistence intact.

## Architectural & Security Guardrails
- **`ARCH-R04`**: All `src/**/*.rs` files MUST remain `<= 800` lines. Place new daemon process/socket helpers in a new submodule `src/daemon/process.rs`.
- **`SEC-R01..04`**: All sockets and state files MUST reside in `0700` private directories via `crate::secure_fs`, and stale socket files MUST only be unlinked when `connect()` returns `ErrorKind::ConnectionRefused` (never unconditionally unlinking a live daemon socket).
- **`INV-05` / `INV-09`**: Preserve strict separation between `SplitTree` ratios (`Tiled` mode) and `ZStack` `FloatingWindowGeometry` (`Stacked` mode); never block the async runtime on PTY teardown (`bilterm-pty-reaper`).

---

## Wave Breakdown

### Wave 1 (`35-01`): Background OS Daemon Spawner & `bilterm ls` CLI (`src/daemon/process.rs`, `src/daemon/mod.rs`, `src/main.rs`)
1. Create `src/daemon/process.rs`:
   - `pub fn session_daemon_socket_path(session: &str) -> Result<PathBuf>`: resolves `<private_runtime_dir>/daemon-<safe_session>.sock`.
   - `pub async fn probe_session_daemon(session: &str) -> bool`: sends a `workspace.status` JSON-RPC ping to `unix:<daemon_sock>`; if `ConnectionRefused`, removes the stale socket file.
   - `pub fn spawn_background_daemon_for_session(session: &str) -> Result<u32>`: spawns `current_exe() --daemon --session <session> -l unix:<daemon_sock>` with `stdin(Stdio::null())`, `stdout(Stdio::null())`, `stderr(Stdio::null())` (and `process_group(0)` on Unix so it survives terminal hangup).
   - `pub fn list_active_daemon_sockets() -> Vec<(String, PathBuf)>`: scans `private_runtime_dir()` for `daemon-*.sock`.
2. In `src/main.rs`:
   - Add `bilterm ls` / `bilterm sessions` subcommand printing both active background daemon sockets and saved workspace `.toml` sessions (`detached` vs `saved`).

### Wave 2 (`35-02`): Live Daemon State Handshake on `:detach` & `bilterm attach` (`src/api/rpc_handlers.rs`, `src/wm/v2_runtime.rs`, `src/wm/session.rs`)
1. In `src/api/rpc_handlers.rs`:
   - Add JSON-RPC methods `"session.save"` (triggers `wm.save_session(name)`) and `"session.restore"` (triggers `wm.restore_session(name)` and returns restored pane count + layout mode).
2. In `src/wm/v2_runtime.rs` (`WindowManager::detach_client`):
   - Save `<session>.toml` (`detached = true`) and atomic `WorkspaceJournal` checkpoint.
   - If not in `headless_mode`, ensure a background daemon for `sess` is spawned (`spawn_background_daemon_for_session(&sess)`) so the session remains discoverable in `bilterm ls` and its daemon socket stays active in the background.

### Wave 3 (`35-03`): Deterministic Verification Suite (`tests/live_daemon_detach_attach.rs`)
1. Write `tests/live_daemon_detach_attach.rs`:
   - Test `session_daemon_socket_path` sanitization (`SEC-R01`).
   - Test stale socket cleanup on `ConnectionRefused` (`probe_session_daemon`).
   - Test full `:detach` -> background daemon `session.restore` / `workspace.status` -> `:attach` round-trip preserving multi-pane `SplitTree`, `ZStack` geometries, prompt text, and canvas output.
2. Verify `wc -l src/**/*.rs <= 800`, `cargo clippy -q --all-targets -- -D warnings`, and `cargo test -q`.
