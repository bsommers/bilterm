# Phase 55-02 Summary: Auto-Spawn, `:daemon` Commands, RPC/MCP, and Version Handshake (`DAEM-05`, `DAEM-07`, `DAEM-08`)

## Completed Tasks
- **Unified Action Dispatch (`src/action.rs`)**:
  - Added `DaemonSupervisorOp` and `Action::DaemonSupervisor(DaemonSupervisorOp)` (with alias `Action::Daemon(DaemonOp)`).
  - Maintained reachability invariant in `assert_action_reachable`.
- **Colon Command Parsing & Execution (`src/colon_command/`)**:
  - Implemented `parse_daemon_colon_command` in `src/colon_command/parse_daemon.rs` for `:daemon list|status|start|stop|switch|prune`.
  - Implemented `execute_daemon_command` in `src/colon_command/execute_daemon.rs` with structured status messages and JSON output.
  - Wired into `src/colon_command/execute_wm.rs`.
- **JSON-RPC & MCP Interfaces (`src/api/`)**:
  - Implemented `handle_daemon_*` in `src/api/rpc_daemon.rs` for `daemon.list`, `daemon.status`, `daemon.start`, `daemon.stop`, `daemon.switch`.
  - Added `bilterm_daemon` tool to MCP server with mapped dispatch in `src/api/mcp.rs`.
- **Protocol Version Safety Handshake (`src/daemon/mod.rs`)**:
  - Added `DaemonRequest::Handshake` and `DaemonResponse::VersionMismatch` / `DaemonResponse::HandshakeOk`.
  - Handshake returns exact diagnostic string on client/server version mismatch without crashing.
- **Client Auto-Spawn (`src/main.rs`)**:
  - `bilterm ctl` auto-spawns the singleton background daemon when socket connection fails with `NotFound` or `ConnectionRefused`, retrying after readiness.
- **Wave 2 Test Suite (`tests/phase55_daemon_lifecycle.rs`)**:
  - Verified colon parsing and execution, RPC handlers, MCP tool, version mismatch diagnostics, and graceful stop.

## Verification
- `cargo test --test phase55_daemon_lifecycle -q` passes (5/5).
- Clean clippy (`-D warnings`).
