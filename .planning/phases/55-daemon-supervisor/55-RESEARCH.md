# Phase 55: Daemon Supervisor & Multi-Daemon Awareness — Research & Audit

**Date**: 2026-11-06  
**Status**: Completed  
**Branch**: `feat/navigator-rows-and-launch`  
**Target Directory**: `src/daemon/` & `.planning/phases/55-daemon-supervisor/`

---

## 1. Executive Summary & Findings

This research audits the existing background session daemon infrastructure in Bilterm (`DAEM-01..04`), specifically focusing on:
1. Current socket layout and security confinement under `src/secure_fs.rs`.
2. The `:detach` lifecycle in `src/main.rs`, `src/wm/v2_runtime.rs`, and `src/daemon/process.rs`.
3. **The core PTY question posed by `ROADMAP.md`**: *Do live PTYs move to the daemon process on detach?*
4. Multi-daemon discovery, PID lockfile concurrency, and version skew safety.

### Key Conclusions:
- **PTY Ownership on Detach (Confirmed)**: **Live PTYs DO NOT move to the daemon on detach.** When a user runs `:detach` or chooses `(d)etach` from the Exit Guard, the client process sets `self.client_detached = true`, persists the layout snapshot (`save_session`) and crash journal (`save_journal_checkpoint`), and exits the host TUI. In `src/main.rs:499-505`, after `app.run` finishes, the client invokes `spawn_background_daemon_for_session(sess)`. This launches a **completely separate background OS process** (`bilterm --daemon --session <name> -l unix:<sock>`). That child process initializes a fresh headless `WindowManager` and restores the session via `restore_session`, which re-parses the TOML layout and spawns **fresh shell PTYs**. The client process then drops its own `MasterPty` handles, terminating the original shell processes.
- **Missing Concurrency Guard**: Currently, daemon spawning lacks an exclusive filesystem lock (`flock` or PID lockfile). If multiple commands or clients trigger daemon startup concurrently, processes race to bind the same Unix domain socket, resulting in `AddressInUse` errors or accidental socket unlinking.
- **Stale Socket Race Hazard**: `probe_or_cleanup_stale_socket` uses `UnixStream::connect`. If a daemon process is starting up or temporarily unresponsive, a client probing the socket can interpret the connection failure as a stale socket and unlink a live daemon's listening endpoint.
- **Absence of Peer Discovery & Daemon Registry**: `bilterm ls` only scans for `daemon-*.sock` files on disk. There is no live metadata registry (PID, version, uptime, pane/session counts, agent status), no cross-daemon communication, and no RPC/MCP API to query or manage multiple running daemons.
- **Version Skew Hazard**: The wire protocol (`DaemonRequest` / `DaemonResponse` and JSON-RPC) performs no version handshake (`CARGO_PKG_VERSION`). Upgrading the `bilterm` binary while a daemon is running causes silent deserialization failures or mysterious crashes.

---

## 2. Current Implementation Breakdown

### 2.1 File Map & Responsibilities

| File | Current Role | Limitations for Phase 55 |
|------|--------------|--------------------------|
| `src/daemon/mod.rs` | `DaemonServerState`, wire protocol `DaemonRequest`/`DaemonResponse`, viewport negotiation | Hard-coded `default_socket_path()` (`<runtime_dir>/daemon.sock`), in-memory state only, no PID tracking or process supervision. |
| `src/daemon/process.rs` | Process spawner (`spawn_background_daemon_for_session`), socket naming (`daemon-<session>.sock`), `list_active_daemon_sessions` | Blindly spawns subprocesses without `flock`; no lockfiles; no daemon metadata registry JSON; no daemon shutdown helper. |
| `src/main.rs:244-275` | `--daemon` / `--headless` CLI flag entry point | Spins up `AutomationControlPlane::new_headless(24, 80)` on socket, but does not register with a daemon supervisor or write a lockfile. |
| `src/main.rs:499-505` | Detach hook | Spawns a background daemon only after TUI exits; if daemon already exists, probing might destroy it or fail. |
| `src/main.rs:108-142` | `bilterm ls` / `bilterm sessions` | Unifies socket list with saved TOML list, but provides no live daemon telemetry (uptime, PIDs, active agents). |
| `src/main.rs:145-175` | `bilterm ctl` CLI client | Fails immediately with connection error if no daemon is running; does not auto-dispatch daemon. |
| `src/secure_fs.rs` | Confinement primitives (`0700` dirs, `0600` files, `validate_plain_file_name`) | Solid foundation, but needs file locking helpers (`flock` / `create_new`) and safe daemon identifier validation. |

### 2.2 Detailed Execution Trace of `:detach`

```mermaid
sequenceDiagram
    participant User
    participant Pane
    participant WM as WindowManager
    participant Main as Client main()
    participant OS as OS / Subprocess
    participant Daemon as Daemon Process

    User->>Pane: Types `:detach` or presses `Ctrl+X D`
    Pane->>WM: Action::DetachClient / ColonCommand::DetachClient
    WM->>WM: detach_client(): client_detached = true
    WM->>WM: save_session(sess) & save_journal_checkpoint()
    WM-->>Main: TUI event loop breaks; run() returns Ok(())
    Main->>Main: TerminalRestoreGuard restores raw mode
    Main->>OS: spawn_background_daemon_for_session(sess)
    OS->>Daemon: Exec: bilterm --daemon --session <sess> -l unix:<sock>
    Daemon->>Daemon: new_headless() -> restore_session(sess)
    Daemon->>Daemon: Reads layout TOML -> Spawns NEW shell PTYs
    Main->>OS: Client process exits (drops old MasterPty handles -> old shells die)
```

**Conclusion on PTY Transfer**: True Unix PTY file descriptor passing across non-forked processes (`SCM_RIGHTS` over Unix domain socket) is not implemented in Bilterm. Moving running interactive shell processes without SIGHUP/SIGWINCH disruption across detached processes requires `SCM_RIGHTS` fd passing of both master PTY and slave handles, which is neither portable nor necessary when a persistent daemon supervisor hosts the session from the beginning.
With **Phase 55 (`DAEM-05`)**, the singleton daemon can be **auto-dispatched on first need** (at startup or on `bilterm ctl`), allowing the daemon to own the PTYs from the outset while the client connects as an attached viewport (`DAEM-02`).

---

## 3. Concurrency, Locks & Stale Process Hazards

### 3.1 Lockfile Strategy (`DAEM-05`, `DAEM-06`)
To guarantee singleton execution and prevent multi-daemon race conditions:
1. **Directory**: `<runtime_dir>/daemons/` (enforced `0700` via `secure_fs::ensure_private_dir`).
2. **Lockfile naming**:
   - Singleton: `<runtime_dir>/daemons/daemon-singleton.lock` (or symlinked to default).
   - Named multi-daemon: `<runtime_dir>/daemons/daemon-<name>.lock`.
3. **Flock Semantics**:
   - Open lockfile with `O_RDWR | O_CREAT` (mode `0600`).
   - Call `libc::flock(fd, LOCK_EX | LOCK_NB)` on Unix.
   - If `flock` succeeds: write JSON payload containing `pid`, `name`, `version`, `started_at_ms`, and `socket_path`. Flush and keep fd open for daemon lifetime.
   - If `flock` fails with `EWOULDBLOCK` / `EAGAIN`: lock is held. Read lockfile content to verify PID.
4. **Stale Lock Recovery**:
   - If lock is held according to file content, probe process liveness (`libc::kill(pid, 0)`).
   - If PID does not exist (`ESRCH`), the previous daemon crashed without releasing the lock (kernel cleans OS flock on process death, but file remains). If OS flock succeeds after PID dies, overwrite lockfile with current PID.
   - If OS flock fails but PID is dead, wait 50ms and retry once before erroring.

### 3.2 Live Daemon Registry (`DAEM-07`)
Currently, `bilterm ls` only reads filenames matching `daemons/daemon-*.sock`.
Instead, every active daemon will maintain a live registry file:
- Path: `<runtime_dir>/daemons/<name>.json`
- Schema:
  ```json
  {
    "id": "daem_01jcb89z...",
    "name": "default",
    "pid": 34120,
    "version": "0.1.0",
    "started_at_ms": 1730920000000,
    "socket_path": "/run/user/1000/bilterm/daemons/daemon-default.sock",
    "sessions_count": 2,
    "panes_count": 5,
    "agents_count": 1,
    "last_heartbeat_ms": 1730920060000
  }
  ```
- Any Bilterm client or daemon can inspect `<runtime_dir>/daemons/*.json` to discover peer daemons.
- A dead daemon entry (PID dead or socket connection refused) is pruned automatically on query.

---

## 4. Multi-Surface Uniformity & Integration Gaps

1. **CLI Surface**:
   - `bilterm --daemon` (starts default singleton).
   - `bilterm --daemon --name <name>` (starts isolated named daemon).
   - `bilterm ls` / `bilterm sessions` (prints live daemon registry telemetry).
   - `bilterm ctl daemon.list`, `daemon.status`, `daemon.stop`, `daemon.start`.
2. **Colon Commands**:
   - `:daemon list` — Shows table of all active daemons, PIDs, sessions, panes, agents.
   - `:daemon status [name]` — Detailed view of named or current daemon.
   - `:daemon start [name]` — Auto-spawns daemon in background.
   - `:daemon stop [name]` — Gracefully stops daemon (checkpoints sessions, cleans sockets).
   - `:daemon switch <name>` — Switch active client attachment to peer daemon.
3. **Palette & Keymap**:
   - Palette catalog must contain `:daemon list`, `:daemon status`, `:daemon stop`.
   - Menubar `Tools > Daemons...` and `File > Detach to Daemon`.
4. **JSON-RPC & MCP**:
   - Methods: `daemon.list`, `daemon.status`, `daemon.stop`, `daemon.start`.
   - MCP tool: `bilterm_daemon` with `action: "list" | "status" | "stop" | "start" | "switch"`.
5. **Cross-Daemon `:find`**:
   - `:find` currently searches live state, saved sessions, and history logs.
   - With `DAEM-07`, `:find daemon:` or `:find session` can query peer daemon registries to discover sessions hosted in other background daemons.
