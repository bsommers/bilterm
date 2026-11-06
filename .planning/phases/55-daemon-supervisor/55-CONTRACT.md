# Phase 55 Contract: Daemon Supervisor & Multi-Daemon Awareness (`DAEM-05..08`)

**Status**: Contract Interface Spec  
**Target Architecture**: Bilterm Core & Multiplexer Runtime  
**Conventions**: `ARCH-R01` (Unified Action), `ARCH-R02` (State Invariants), `ARCH-R04` (Module Ceilings), `SEC-R01..04` (Secure FS), `UNI-R01` (Surface Uniformity)

---

## 1. Architectural Decisions & Invariants

### 1.1 Invariants & Rules
- **D-01 (Singleton Default)**: By default, Bilterm operates with at most one background daemon (`"default"`). The singleton lockfile `daemon-singleton.lock` guards its lifecycle.
- **D-02 (Multi-Daemon Isolation)**: Multi-daemon mode is strictly opt-in via `--daemon --name <name>` on the CLI or `[daemon] mode = "multi"` in `config.toml`. Every named daemon `<name>` gets its own isolated socket `daemon-<name>.sock`, lockfile `daemon-<name>.lock`, and registry entry `daemons/<name>.json`.
- **D-03 (Sanitized Names)**: Daemon names MUST satisfy `validate_safe_name(name)`: matching regex `^[A-Za-z0-9_-]{1,64}$`, rejecting any `.`, `..`, `/`, `\`, NUL, or whitespace (`SEC-R01`).
- **D-04 (Stale Lock & Socket Recovery)**: Lockfiles store JSON containing `pid`, `name`, `version`, `started_at_ms`, and `socket_path`. On startup, if a lock cannot be acquired or socket connection fails, the process checks if the PID is alive (`libc::kill(pid, 0)`). If the PID is dead (`ESRCH`), the stale lock and socket are reclaimed automatically without manual user intervention.
- **D-05 (Idempotent Start)**: Calling `:daemon start` or spawning `bilterm --daemon` when an identical daemon is already running is a clean idempotent no-op returning `AlreadyRunning { pid, socket_path }`.
- **D-06 (Peer Discovery Registry)**: Every live daemon continuously or periodically refreshes its metadata in `<runtime_dir>/daemons/<name>.json`. Clients and peer daemons query this directory to discover running instances.
- **D-07 (Protocol Version Handshake)**: Every connection to a daemon socket begins with a version check against `env!("CARGO_PKG_VERSION")`. A version mismatch yields a structured error with exact version strings and remediation advice.
- **D-08 (Zero PTY / Input Bleed)**: Daemons are completely isolated processes. PTY master/slave handles and input buffers NEVER cross daemon boundaries. Cross-daemon operations occur exclusively through Unix domain socket RPC or registry queries.

---

## 2. File Map & Submodule Organization

All source files respect the ~800–1000 line ceiling (`ARCH-R04`) and preserve encapsulation (`ARCH-R02`):

| File | Purpose |
|------|---------|
| `src/daemon/supervisor.rs` | Lockfile management (`DaemonLock`), flock acquisition, stale-PID verification, process spawning, and daemon lifecycle management. |
| `src/daemon/registry.rs` | Live peer discovery, reading/writing `daemons/<name>.json`, automatic stale registry pruning, and heartbeat updates. |
| `src/daemon/mod.rs` | Protocol extensions: `DaemonRequest::Handshake`, `DaemonResponse::VersionMismatch`, `DaemonRequest::PeerList`. |
| `src/action.rs` | Adds `Action::Daemon(DaemonOp)` to the canonical action enum. |
| `src/colon_command/parse_daemon.rs` | Parses `:daemon list`, `:daemon status [name]`, `:daemon start [name]`, `:daemon stop [name]`, `:daemon switch <name>`. |
| `src/colon_command/execute_daemon.rs` | Dispatches `DaemonOp` via `WindowManager::apply`. |
| `src/api/rpc_daemon.rs` | JSON-RPC handler for `daemon.list`, `daemon.status`, `daemon.stop`, `daemon.start`, `daemon.switch`. |
| `src/api/mcp.rs` | Adds `bilterm_daemon` tool to the MCP surface. |
| `src/wm/registry/peer.rs` | Feeds peer daemon sessions and panes into the unified `:find` index. |
| `src/ui/modals.rs` | Top Menubar and Palette items for daemon management. |

---

## 3. Data Structures & Schemas

### 3.1 Lockfile Record (`daemons/daemon-<name>.lock` or `daemon-singleton.lock`)
```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonLockRecord {
    pub pid: u32,
    pub name: String,
    pub version: String,
    pub started_at_ms: u64,
    pub socket_path: PathBuf,
}
```

### 3.2 Registry Entry (`daemons/<name>.json`)
```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonRegistryEntry {
    pub id: String,                // Unique instance ID, e.g. "daem_01jcb89z..."
    pub name: String,              // Validated safe name, e.g. "default", "worker-1"
    pub pid: u32,                  // OS process ID
    pub version: String,          // CARGO_PKG_VERSION of the daemon binary
    pub started_at_ms: u64,       // Epoch timestamp in milliseconds
    pub socket_path: PathBuf,     // Path to Unix domain socket
    pub sessions_count: usize,    // Number of active sessions hosted
    pub panes_count: usize,       // Total panes running across all sessions
    pub agents_count: usize,      // Number of registered agents
    pub last_heartbeat_ms: u64,   // Heartbeat refreshed every 5 seconds
}
```

### 3.3 Daemon Operations Enum (`src/action.rs`)
```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonOp {
    List,
    Status { name: Option<String> },
    Start { name: Option<String> },
    Stop { name: Option<String>, force: bool },
    Switch { name: String },
}
```

### 3.4 Wire Protocol Handshake (`src/daemon/mod.rs`)
```rust
// Added to DaemonRequest:
Handshake {
    client_version: String,
    client_id: u64,
},

// Added to DaemonResponse:
HandshakeOk {
    server_version: String,
    daemon_pid: u32,
    daemon_name: String,
},
VersionMismatch {
    server_version: String,
    client_version: String,
    diagnostic: String,
},
```

---

## 4. Lifecycle & Safety (`DAEM-05`, `DAEM-08`)

### 4.1 Daemon Startup Workflow (`DaemonSupervisor::start`)
```mermaid
flowchart TD
    A[Start Request: name] --> B[Sanitize & Validate Name]
    B --> C{Name Valid?}
    C -- No --> D[Return ValidationError]
    C -- Yes --> E[Acquire flock on daemon-name.lock]
    E -- Success --> F[Write Lockfile & Registry JSON]
    F --> G[Bind Unix Domain Socket]
    G --> H[Run Event Loop]
    E -- EWOULDBLOCK --> I[Read PID from Lockfile]
    I --> J{kill(pid, 0) alive?}
    J -- Alive --> K[Return Ok(AlreadyRunning)]
    J -- Dead (ESRCH) --> L[Log Stale Lock Recovery]
    L --> M[Unlink Stale Socket & Retry flock]
    M --> F
```

### 4.2 Graceful Shutdown Workflow (`DaemonSupervisor::stop`)
1. Receive `Stop` request (via signal `SIGTERM`, colon command `:daemon stop`, RPC `daemon.stop`, or MCP).
2. Checkpoint all active session journals via `save_journal_checkpoint`.
3. Notify all attached clients via `DaemonResponse::Stopping` or RPC event `daemon.stopping`.
4. Gracefully terminate child PTY processes:
   - Send `SIGHUP` to session process groups.
   - Wait 100ms; send `SIGTERM` if still alive.
   - Wait 100ms; send `SIGKILL` if still alive.
5. Unlink Unix domain socket (`daemons/daemon-<name>.sock`).
6. Remove registry file (`daemons/<name>.json`).
7. Release flock and unlink lockfile (`daemons/daemon-<name>.lock`).
8. Process exits cleanly with code `0`.

---

## 5. API Contracts: JSON-RPC & MCP

### 5.1 JSON-RPC 2.0 Methods

#### `daemon.list`
- **Request**: `{ "jsonrpc": "2.0", "method": "daemon.list", "params": {}, "id": 1 }`
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "result": {
      "daemons": [
        {
          "name": "default",
          "pid": 34120,
          "version": "0.1.0",
          "started_at_ms": 1730920000000,
          "socket_path": "/run/user/1000/bilterm/daemons/daemon-default.sock",
          "sessions_count": 2,
          "panes_count": 4,
          "agents_count": 1,
          "is_current": true
        }
      ]
    },
    "id": 1
  }
  ```

#### `daemon.status`
- **Request**: `{ "jsonrpc": "2.0", "method": "daemon.status", "params": { "name": "default" }, "id": 2 }`
- **Response**: Detailed telemetry object including uptime, CPU/memory stats if available, and hosted sessions list.

#### `daemon.stop`
- **Request**: `{ "jsonrpc": "2.0", "method": "daemon.stop", "params": { "name": "default", "force": false }, "id": 3 }`
- **Response**: `{ "jsonrpc": "2.0", "result": { "status": "stopped", "name": "default" }, "id": 3 }`

#### `daemon.start`
- **Request**: `{ "jsonrpc": "2.0", "method": "daemon.start", "params": { "name": "worker-1" }, "id": 4 }`
- **Response**: `{ "jsonrpc": "2.0", "result": { "status": "started", "name": "worker-1", "pid": 35210 }, "id": 4 }`

### 5.2 MCP Tool: `bilterm_daemon`
- **Schema**:
  ```json
  {
    "name": "bilterm_daemon",
    "description": "Supervise and inspect Bilterm background daemons (list, status, start, stop, switch)",
    "inputSchema": {
      "type": "object",
      "properties": {
        "action": {
          "type": "string",
          "enum": ["list", "status", "start", "stop", "switch"],
          "description": "Daemon operation to perform"
        },
        "name": {
          "type": "string",
          "description": "Optional daemon name (defaults to 'default' or current)"
        },
        "force": {
          "type": "boolean",
          "description": "Force termination without waiting for graceful PTY exit"
        }
      },
      "required": ["action"]
    }
  }
  ```

---

## 6. Surface Uniformity Matrix (`UNI-R01`)

Every capability in Phase 55 MUST be accessible across all user-facing surfaces:

| Capability | Mouse Path | Key Sequence | Colon Command | Palette Item | JSON-RPC | MCP Tool |
|------------|------------|--------------|---------------|--------------|----------|----------|
| **List Daemons** | Menubar `Tools > Daemons > List` | `Ctrl+X D L` | `:daemon list` | `:daemon list` | `daemon.list` | `bilterm_daemon` (`action="list"`) |
| **Daemon Status** | Click daemon row badge in Drawer | `Ctrl+X D S` | `:daemon status [name]` | `:daemon status` | `daemon.status` | `bilterm_daemon` (`action="status"`) |
| **Start Daemon** | Menubar `Tools > Daemons > Start` | `Ctrl+X D N` | `:daemon start [name]` | `:daemon start` | `daemon.start` | `bilterm_daemon` (`action="start"`) |
| **Stop Daemon** | Menubar `Tools > Daemons > Stop` | `Ctrl+X D Q` | `:daemon stop [name]` | `:daemon stop` | `daemon.stop` | `bilterm_daemon` (`action="stop"`) |
| **Switch Daemon** | Double-click peer daemon in Drawer | `Ctrl+X D W` | `:daemon switch <name>` | `:daemon switch` | `daemon.switch` | `bilterm_daemon` (`action="switch"`) |
| **Find Across Daemons** | Click Search in Menubar | `Ctrl+Alt+F` | `:find daemon:<query>` | `:find daemon:` | `workspace.find` (`scope="daemon"`) | `bilterm_find` (`kind="daemon"`) |

---

## 7. Version Mismatch Diagnostic Spec (`DAEM-08`)

When a client with version `C` connects to a daemon running version `D` where `C != D`:
- The client MUST NOT panic or crash.
- The connection is aborted with the exact diagnostic:
  ```
  bilterm: daemon version mismatch
    client version: 0.2.0
    daemon version: 0.1.0 (PID 12345 at /run/user/1000/bilterm/daemons/daemon-default.sock)
    action required: restart daemon using ':daemon stop' or 'bilterm ctl daemon.stop --name default'
  ```
- Exit status code: `2` (configuration/environment incompatibility).
