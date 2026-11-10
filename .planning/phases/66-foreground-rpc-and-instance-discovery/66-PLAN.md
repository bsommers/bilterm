---
phase: 66-foreground-rpc-and-instance-discovery
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/app.rs
  - src/gui/native_window.rs
  - src/api/foreground.rs
  - src/api/instances.rs
  - src/pane/spawn.rs
  - src/daemon/types.rs
  - src/colon_command/rpc.rs
  - src/action.rs
  - src/ui/oia.rs
  - src/cli/ctl.rs
  - tests/phase66_live_foreground_rpc.rs
autonomous: true
requirements:
  - LIVE-01
  - LIVE-02
  - LIVE-03
  - LIVE-04
recommended_model: sonnet
must_haves:
  truths:
    - "Both App::run (TUI) and run_native_gui_window (GUI) bind a 0600 RPC socket and serve requests through WindowManager::apply(Action) on the UI thread."
    - "Every child PTY receives BILTERM_SOCKET, BILTERM_INSTANCE_PID, BILTERM_WORKSPACE_ID, and BILTERM_PANE_ID; RPC calls from that socket auto-attribute from_pane."
    - "Instance sockets live at /run/user/<uid>/bilterm/instances/<pid>.sock; every PID is validated via SignalablePid and stale entries are pruned."
    - "bilterm ctl --instance <pid|all> routes or federates calls; ambiguity without --instance is a clear error."
    - "OIA shows [RPC:LIVE] while the socket serves; :rpc status|start|stop|clients and RPC/MCP equivalents exist (UNI-R01)."
  artifacts:
    - path: "src/api/foreground.rs"
      provides: "Foreground RPC server bridging socket requests to the UI-thread Action queue"
    - path: "src/api/instances.rs"
      provides: "Instance registry (register, list, prune, resolve) using secure_fs and SignalablePid"
    - path: "tests/phase66_live_foreground_rpc.rs"
      provides: "Headless integration tests for LIVE-01..04"
---

# Phase 66 Plan: Always-On Foreground RPC & Multi-Instance Discovery (LIVE-01..04)

## Objective

Make every foreground Bilterm instance a live control-plane endpoint so agents inside its panes can drive the same workspace they live in:
1. Always-on foreground RPC socket in `App::run` and `run_native_gui_window` (`LIVE-01`).
2. Child PTY environment injection with automatic `from_pane` attribution (`LIVE-02`).
3. Multi-instance socket registry and `--instance <pid|all>` federation (`LIVE-03`).
4. `[RPC:LIVE]` OIA indicator and `:rpc` command with `UNI-R01` parity (`LIVE-04`).

---

## Detailed Requirements

### LIVE-01: Always-On Foreground RPC Socket
- **Problem**: Only the headless daemon serves RPC; `bilterm ctl` cannot reach a foreground TUI/GUI session.
- **Specification**:
  - Start the RPC listener in `App::run` and `run_native_gui_window` (default on; `[rpc] foreground = false` disables).
  - Accept loop runs off the UI thread; requests are marshalled to the UI thread as `Action`s via a bounded channel and answered after `WindowManager::apply`.
  - Socket created through `src/secure_fs.rs` (`0700` dir, `0600` socket); remote TCP unchanged (`SEC-R01` bearer token).
  - Bounded request size and queue depth (backpressure error, never unbounded growth).

### LIVE-02: Child PTY Env Injection & `from_pane` Attribution
- **Specification**:
  - Inject `BILTERM_SOCKET`, `BILTERM_INSTANCE_PID`, `BILTERM_WORKSPACE_ID`, `BILTERM_PANE_ID` at PTY spawn in `src/pane/spawn.rs`.
  - CLI/MCP clients read these vars and send `from_pane` automatically; explicit `--from-pane` overrides.
  - Env vars are not forwarded over SSH/remote transports (`PaneTransport::Remote` skips injection).

### LIVE-03: Multi-Instance Registry & Federation
- **Specification**:
  - On start write `<runtime>/bilterm/instances/<pid>.sock` (+ metadata file: pid, start time, workspace name, surface TUI/GUI).
  - `parse` PIDs via `SignalablePid`; liveness via `check_signalable_pid_alive`; prune stale on list.
  - `bilterm ctl --instance <pid|all>`: single-target routing or fan-out with per-instance result envelope; default resolves `BILTERM_SOCKET` then sole live instance, else error listing candidates.
  - `validate_safe_name` for any filename derived from input.

### LIVE-04: OIA Indicator & `:rpc` Parity
- **Specification**:
  - OIA segment `[RPC:LIVE]` (clients count in inspector), absent when stopped; `GOLDENS OK 11` preserved (indicator hidden in golden fixtures).
  - `Action::Rpc(RpcCmd::{Status,Start,Stop,Clients})`; colon `:rpc status|start|stop|clients`; palette entry, menu item, RPC `rpc.*`, MCP `bilterm_rpc`.

---

## Threat Model & Security Considerations

| Threat ID | Threat Category | Description | Mitigation Strategy |
|-----------|-----------------|-------------|---------------------|
| **T-RPC-01** | Spoofing / Privilege | Another local user connects to the socket. | `0700` dir, `0600` socket, peer-credential (`SO_PEERCRED`) uid check. |
| **T-RPC-02** | Denial of Service | Flood of requests starves the UI thread. | Bounded channel, per-request size cap, per-tick drain budget. |
| **T-RPC-03** | Signal Misdirection | Stale/garbage instance file leads to signaling wrong PID. | `SignalablePid` validation + start-time check before any signal/liveness probe. |
| **T-RPC-04** | Information Disclosure | Env vars leak socket path to remote hosts. | No env injection on remote transports. |
| **T-RPC-05** | Path Traversal | Crafted `--instance` value escapes registry dir. | Numeric PID parse only; `validate_safe_name`. |

---

## Technical Design & Wave Breakdown

### Wave 1: Foreground server + registry (`LIVE-01`, `LIVE-03`)
1. `src/api/foreground.rs`: listener, UI-thread request queue, `drain_rpc_requests(&mut WindowManager)` called each tick from TUI and GUI loops.
2. `src/api/instances.rs`: register/unregister (RAII guard), list, prune, resolve.
3. Wire into `App::run` / `run_native_gui_window`; clean shutdown removes socket.

### Wave 2: Env injection & CLI routing (`LIVE-02`, `LIVE-03`)
1. Inject env in `src/pane/spawn.rs`; thread instance id/workspace id through `WindowManager`.
2. `src/cli/ctl.rs`: `--instance`, `from_pane` auto-attribution, federation envelope.

### Wave 3: Surface parity & verification (`LIVE-04`)
1. `Action::Rpc`, `:rpc`, palette/menu, RPC `rpc.*`, MCP tool, OIA segment, Inspector row.
2. `tests/phase66_live_foreground_rpc.rs` using `WindowManager::new_headless`; run `scripts/verify.sh`.

---

## Verification Gates

- [ ] **Gate 1 (Server)**: Headless test round-trips `workspace.status` through the foreground queue.
- [ ] **Gate 2 (Env)**: Spawned child observes all four env vars; remote panes observe none.
- [ ] **Gate 3 (Registry)**: Stale/forged entries (`0`, `1`, `-1`, dead PID) are rejected/pruned; `--instance all` federates.
- [ ] **Gate 4 (Parity)**: `:rpc`, palette, menu, RPC, MCP all reach the same `Action` (`UNI-R01`).
- [ ] **Gate 5 (Quality)**: `scripts/verify.sh` PASS; `GOLDENS OK 11`; files `<= 1000` lines (`ARCH-R04`); `handle_wm_key_event <= 80` lines (`ARCH-R01`).
