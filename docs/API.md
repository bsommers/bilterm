# Bilterm Automation API Reference

Bilterm provides a comprehensive, multi-surface automation and control plane for programmatic terminal multiplexing, testing, AI agent orchestration, and remote fleet management.

This document serves as the canonical technical reference for all implemented automation surfaces, protocol specifications, methods, tools, resources, and security boundaries.

---

## 1. Architecture & Automation Surfaces

Bilterm exposes three primary automation surfaces backed by a unified internal state machine (`AutomationControlPlane`):

```
                        ┌─────────────────────────────────────────────────────────┐
                        │                 AutomationControlPlane                  │
                        │   (WindowManager + Event Buffer + Confined Snapshots)   │
                        └───────▲─────────────────▲─────────────────▲─────────────┘
                                │                 │                 │
                ┌───────────────┴────────┐ ┌──────┴──────┐   ┌──────┴──────────────┐
                │     JSON-RPC 2.0       │ │   HTTP/SSE  │   │  Model Context      │
                │   Unix Socket / TCP    │ │  (Loopback) │   │  Protocol (MCP)     │
                └───────────────▲────────┘ └──────▲──────┘   └──────▲──────────────┘
                                │                 │                 │
                        ┌───────┴────────┐ ┌──────┴──────┐          │
                        │  bilterm ctl   │ │ curl / SSE  │    stdio (JSON-RPC 2.0)
                        │ (CLI utility)  │ │   Clients   │    Claude / IDE Agents
                        └────────────────┘ └─────────────┘
```

1. **JSON-RPC 2.0 Daemon (Unix Domain Socket / TCP)**:
   - Line-delimited JSON-RPC 2.0 stream server running in headless mode (`bilterm --headless --listen <ADDR>`).
   - Default transport on Unix is a private Unix domain socket located at `$XDG_RUNTIME_DIR/bilterm/bilterm.sock` or `~/.local/state/bilterm/bilterm.sock` (mode `0600`, peer UID authenticated).
   - TCP listeners (loopback included) require bearer token authentication via `--token-file`. A line that is not JSON (e.g. an HTTP request line) gets one error reply and the connection is closed.
2. **Model Context Protocol (MCP 2024-11-05)**:
   - Native stdio-based MCP server (`bilterm --mcp`) conforming to the Model Context Protocol specification.
   - Provides AI coding agents (Claude Desktop, Antigravity, OpenCodeInterpreter, etc.) with structured tools (`tools/list`, `tools/call`) and dynamic read-only resources (`resources/list`, `resources/read`).
3. **HTTP/1.1 + Server-Sent Events (SSE) Transport**:
   - Loopback-only companion HTTP server (`bilterm --headless --http 127.0.0.1:PORT`).
   - Provides RESTful health checking (`GET /health`), RPC tunneling (`POST /rpc`), and live event streaming (`GET /events`).
4. **CLI Control Utility (`bilterm ctl`)**:
   - Built-in command-line client (`bilterm ctl [ENDPOINT] <METHOD> [JSON_PARAMS]`) allowing shell scripts to query and mutate any running Bilterm instance.

---

## 2. Server Configuration, Transports & Invocations

### Starting the Headless JSON-RPC Daemon

```bash
# Default: listens on private Unix domain socket
bilterm --headless

# Explicit Unix socket path
bilterm --headless --listen unix:/tmp/bilterm-custom.sock

# Loopback TCP listener (every TCP listener requires a 0600 --token-file)
bilterm --headless --listen 127.0.0.1:7370 --token-file /path/to/token.secret

# Remote TCP listening (strictly requires --allow-remote and a 0600 --token-file)
bilterm --headless --listen 0.0.0.0:7370 --allow-remote --token-file /path/to/token.secret

# Headless daemon with concurrent HTTP + SSE transport
bilterm --headless --http 127.0.0.1:7371
```

### Running the Native MCP Server

```bash
# Stdio server for AI assistants (e.g., configured in claude_desktop_config.json)
bilterm --mcp

# MCP server restoring a named session and confining snapshots
bilterm --mcp --session dev-workspace --snapshot-dir /path/to/snapshots
```

### Using the CLI Control Client (`bilterm ctl`)

Syntax:
```bash
bilterm ctl [-t|--token-file <PATH>] [ENDPOINT] <METHOD> [JSON_PARAMS]
```

- If `ENDPOINT` contains `:` (e.g. `127.0.0.1:7370` or `unix:/path/to/sock`), it is treated as the target address; otherwise, `bilterm ctl` connects to the default Unix domain socket (or `127.0.0.1:7370` on non-Unix).
- If omitted, `<METHOD>` defaults to `"workspace.status"`.
- If omitted, `[JSON_PARAMS]` defaults to `{}`.
- Bearer tokens can be supplied via `-t <PATH>` or the `BILTERM_TOKEN_FILE` environment variable.

Examples:
```bash
# Query workspace status from default socket
bilterm ctl workspace.status

# Inspect pane #1
bilterm ctl pane.read '{"pane_id": 1}'

# Execute a command in pane #1
bilterm ctl pane.exec '{"command": "git status", "pane_id": 1}'

# Send synthetic key chords
bilterm ctl pane.send_keys '{"keys": "Ctrl+C"}'

# Connect over TCP with token
bilterm ctl -t ~/.bilterm.token 127.0.0.1:7370 workspace.status
```

---

## 3. JSON-RPC 2.0 Method Reference

### Wire Protocol

- Format: JSON-RPC 2.0 (`{"jsonrpc": "2.0", "id": <id>, "method": "<method>", "params": { ... }}`).
- Framing: Newline-delimited JSON (`\n`). Maximum line length is 1 MiB (`MAX_RPC_LINE_BYTES = 1048576`). Lines exceeding this limit trigger immediate connection termination.
- Standard error response:
  ```json
  {"jsonrpc": "2.0", "id": 1, "error": {"code": -32000, "message": "error description"}}
  ```
- Error codes:
  - `-32700`: Parse error (invalid JSON).
  - `-32600`: Invalid request (missing method).
  - `-32601`: Method not found.
  - `-32602`: Invalid parameters.
  - `-32001`: Authentication required or authentication failed (`RPC_AUTH_ERROR_CODE`).
  - `-32000`: Application / execution error.

### Master Table of RPC Methods

| RPC Method | Parameters | Result Summary | Description |
| :--- | :--- | :--- | :--- |
| `auth.login` | `token` (string) | `{"authenticated": true}` | Authenticate a connection when daemon runs with `--token-file`. |
| `workspace.status` | None (`{}`) | Workspace layout, flags, and array of panes | Returns full topology, active pane ID, layout mode, and 3270 telemetry. |
| `workspace.snapshot` | `ppm_path` (string, optional) | Status object identical to `workspace.status` | Confined PPM pixel dump of workspace into snapshot directory. |
| `workspace.fontsize` | `action` (string), `size` (int), `scale_pct` (int), `command` (string) | Font dimensions, pt size, scale percentage | Query or adjust workspace and active pane typography. |
| `pane.exec` | `command` (string, req), `pane_id` (int), `allow_echo_lock` (bool), `from_pane` (int, optional: attribute the call to that pane in the agent traffic graph) | `{"status": "submitted", "pane_id": int, "command": str}` | Submit shell command into pane command box or raw PTY. |
| `pane.read` | `pane_id` (int) | Lines, cursor, hints, blocks, notifications | Extract visible screen text, cursor position, OSC 133 blocks, and field hints. |
| `pane.feed_output` | `data` (string, req), `pane_id` (int) | `{"status": "processed", "pane_id": int, "bytes": int}` | Feed raw VT100/ANSI bytes into pane parser for headless testing. |
| `pane.send_keys` | `keys` (string or string array, req) | `{"status": "dispatched", "count": int}` | Inject synthetic key chords into active pane. |
| `wm.command` | `command` (string, req) | `{"status": "handled", "command": str, "data": ...}` | Execute any `:colon` multiplexer command (e.g. `:split`, `:theme`, `:get`). |
| `events.poll` | `drain` (bool, default true) | `{"events": [...]}` | Poll and drain queued workspace lifecycle and trigger events. |
| `playbook.run` | `toml` (string, req) | Playbook completion report, passed flag, step count | Run an end-to-end declarative TOML automation script. |
| `remote.connect_ssh` | `target` (string, req), `port` (int), `tags` (string[]), `tmux_session` (str) | Connection status (`connected_ssh` or `connecting`), target, created `pane_id` | Spawn SSH or remote tmux -CC pane and register in fleet. |
| `remote.connect_tmux` | `target` (string, req), `session` (string), `tags` (string[]) | Connection status (`connecting`), state, target, session, `pane_id`, `link` | Connect directly to remote persistent `tmux -CC` session. |
| `remote.link` | `op` (string, optional: `"status"`, `"reconnect"`, `"disconnect"`) | `wm.command` execution fields plus top-level `link` object | Show status, reconnect immediately, or disconnect persistent remote tmux link. |
| `fleet.register` | `name` (string, req), `destination` (string, req), `tags` (string[]), `rpc_endpoint` (str) | Registration confirmation and node count | Register remote profile in fleet management registry. |
| `fleet.list` | None (`{}`) | Array of registered fleet nodes and connection states | List all known fleet nodes, tags, and status. |
| `fleet.exec` | `command` (string, req), `selector` (string) | Dispatched status, matched pane IDs, node names | Fan out shell command to nodes matching selector (all, @tag, name). |
| `fleet.disconnect` | `name` (string, req) | `{"removed": bool, "name": str}` | Deregister and disconnect remote fleet node. |
| `canvas.command` | `command` (string, optional) | Tektronix vector & retained canvas scene statistics | Execute canvas command (`tek`, `line`, `sprite`, `plot`) and return scene state. |
| `canvas.status` | `command` (string, optional) | Retained canvas scene statistics | Read canvas geometry, sprites, plots, and collision metrics. |
| `session.save` | `name` (string, optional, default `"default"`) | `{"saved": true, "session": str, "path": str, "panes": int}` | Save current workspace topology to disk. |
| `session.restore` | `name` (string, optional, default `"default"`) | `{"restored": true, "session": str, "panes": int}` | Restore saved workspace topology from disk. |
| `find.agents` | `query` (string, optional, max 128 chars), `limit` (int, optional, default 50, range 1..200) | Find envelope `{"kind","query","total","returned","truncated","masked","rows":[...]}`; each row has `source` | Ranked search of live agents and panes. Read-only: the Find window does not open. See [section 7](#unified-find-find-methods-and-bilterm_find-phase-501). |
| `find.sessions` | `query` (string, optional), `limit` (int, optional) | Find envelope; rows of kind `session` and `pane` | Live sessions first, then saved sessions and history-only sessions on disk. |
| `find.history` | `query` (string, optional), `limit` (int, optional) | Find envelope; rows of kind `history` | Commands captured in this run, then `history.log` files on disk. Secret-looking lines are masked and only counted in `masked`. |
| `agents.list` | `op` (string, optional: absent or `list` reads the state; otherwise one of `open close toggle dismiss blur select jump zoom pin unpin scale filter dock float move resize rect size scroll`), `args` (array of strings or integers, or one whitespace-separated string; optional, at most 8 tokens) | `wm.command` result fields plus a top-level `agents_list` object (`active`, `pinned`, `focused`, `docked`, `filter`, `selected`, `row_count`, `scale_pct`) | Read the AGENTS list state or run a list operation (`:agents list [op args]`). Also opens the AGENTS list window (Phase 51). See "Agents list and demo" below. |
| `agents.demo` | `stop` (boolean, optional, default `false`) | `wm.command` result fields plus `demo` (`running`, `endpoints`, `spawned`) | Start the agent traffic demo, or with `stop: true` close the panes the demo created and remove its traffic. No write gate. See "Agents list and demo" below. |

---

### Detailed Method Specifications & Examples

#### `auth.login`
When a daemon is started with `--token-file`, connections must send `auth.login` before any other request can be processed.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 1, "method": "auth.login", "params": {"token": "4f9b8c2d1e0a..."}}
  ```
- **Response**:
  ```json
  {"jsonrpc": "2.0", "id": 1, "result": {"authenticated": true}}
  ```
- **Error**:
  ```json
  {"jsonrpc": "2.0", "id": 1, "error": {"code": -32001, "message": "authentication failed"}}
  ```

#### `workspace.status`
Returns real-time workspace topology, UI flags, and telemetry for all active panes.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 2, "method": "workspace.status", "params": {}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 2,
    "result": {
      "layout_mode": "tiled",
      "active_pane_id": 1,
      "pane_count": 2,
      "broadcast_input": false,
      "menu_bar_visible": true,
      "crt_scanlines": false,
      "palette_visible": false,
      "panes": [
        {
          "id": 1,
          "title": "shell",
          "rows": 24,
          "cols": 80,
          "cursor_row": 5,
          "cursor_col": 0,
          "input_target": "cmd_box",
          "alternate_screen": false,
          "echo_lock": false,
          "read_only": false,
          "recording": false,
          "notifications": 0
        }
      ]
    }
  }
  ```

#### `workspace.snapshot`
Renders a GUI workspace frame to a Portable Pixmap (`.ppm`) file confined to the snapshot directory.
- **Parameters**:
  - `ppm_path` (string, optional): Base filename (must pass `validate_snapshot_name`: plain file name, no slashes or `..`).
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 3, "method": "workspace.snapshot", "params": {"ppm_path": "frame_01.ppm"}}
  ```
- **Response**: Returns the same result object as `workspace.status`.

#### `workspace.fontsize`
Dynamically queries or scales terminal font metrics and cell dimensions.
- **Parameters**:
  - `action` (string, optional): `"in"` | `"out"` | `"reset"` | point size / percentage string (e.g. `"16pt"`, `"120%"`).
  - `size` (integer, optional): Font point size (`6..49`).
  - `scale_pct` (integer, optional): Scale factor clamped to `50..300`.
  - `command` (string, optional): Direct font command.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 4, "method": "workspace.fontsize", "params": {"action": "zoom_in"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 4,
    "result": {
      "font_size_pt": 16,
      "base_font_size_pt": 14,
      "font_scale_pct": 114,
      "cell_width_px": 9,
      "cell_height_px": 18,
      "host_font_passthrough": false,
      "pane_font_family": "Cascadia Code",
      "pane_font_size_pt": 16,
      "pane_font_scale_pct": 114,
      "pane_font_bold": false,
      "pane_font_italic": false,
      "pane_font_double_width": false
    }
  }
  ```

#### `pane.exec`
Submits a command into a composite pane with protection against password prompt overrides and read-only locks.
- **Parameters**:
  - `command` (string, required): Shell command line to submit.
  - `pane_id` (integer, optional): Target pane ID (defaults to active pane).
  - `allow_echo_lock` (boolean, optional, default `false`): If `false`, execution is refused if the pane is currently detecting a password prompt (`!ECHO LOCK`).
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 5, "method": "pane.exec", "params": {"pane_id": 1, "command": "ls -la"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 5,
    "result": {
      "status": "submitted",
      "pane_id": 1,
      "command": "ls -la"
    }
  }
  ```
- **Errors**:
  - `pane #1 is locked read-only (🔒 PROT)`
  - `pane #1 is in !ECHO LOCK password prompt; set allow_echo_lock=true to override`

#### `pane.read`
Extracts full text lines, cursor coordinates, OSC 133 command blocks, visible field hints, and notifications.
- **Parameters**:
  - `pane_id` (integer, optional): Defaults to active pane ID.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 6, "method": "pane.read", "params": {"pane_id": 1}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 6,
    "result": {
      "pane_id": 1,
      "title": "shell",
      "rows": 24,
      "cols": 80,
      "cursor_row": 3,
      "cursor_col": 12,
      "alternate_screen": false,
      "echo_lock": false,
      "read_only": false,
      "recording": false,
      "lines": ["user@box:~$ ls -la", "total 16", "drwxr-xr-x 2 user user 4096 Nov 2 22:00 ."],
      "hints": [
        {
          "label": "a",
          "text": "https://github.com",
          "kind": "Url",
          "row": 1,
          "col": 10
        }
      ],
      "command_blocks": [
        {
          "command": "ls -la",
          "row": 0,
          "exit_code": 0,
          "duration_ms": 12
        }
      ],
      "notifications": []
    }
  }
  ```

#### `pane.feed_output`
Injects raw byte data directly into the virtual terminal parser.
- **Parameters**:
  - `data` (string, required): Stream text/ANSI data.
  - `pane_id` (integer, optional): Defaults to active pane ID.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 7, "method": "pane.feed_output", "params": {"pane_id": 1, "data": "\u001b[32mBuild Successful\u001b[0m\r\n"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 7,
    "result": {
      "status": "processed",
      "pane_id": 1,
      "bytes": 27
    }
  }
  ```

#### `pane.send_keys`
Dispatches keyboard chords to drive interactive full-screen TUIs (`vim`, `nano`, `htop`).
- **Parameters**:
  - `keys` (string or array of strings, required): Space-delimited string or JSON array of key chord names (e.g. `"Ctrl+C"`, `"F1"`, `"Enter"`, `"Alt+H"`).
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 8, "method": "pane.send_keys", "params": {"keys": ["Ctrl+X", "c"]}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 8,
    "result": {
      "status": "dispatched",
      "count": 2
    }
  }
  ```

#### `wm.command`
Executes any internal Bilterm `:colon` command. Can include leading `:` or omit it.
- **Parameters**:
  - `command` (string, required): e.g., `":split v"`, `":theme amber"`, `":layout stack"`.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 9, "method": "wm.command", "params": {"command": ":split v"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 9,
    "result": {
      "status": "handled",
      "command": ":split v",
      "active_pane_id": 2,
      "pane_count": 2
    }
  }
  ```
If the command produces structured data (such as `:get`, `:set`, `:config list`, `:state`), the result contains a `"data"` field containing the structured JSON object.

#### `events.poll`
Drains or inspects the asynchronous ring buffer of workspace events.
- **Parameters**:
  - `drain` (boolean, optional, default `true`): When `true`, empties the buffer.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 10, "method": "events.poll", "params": {"drain": true}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 10,
    "result": {
      "events": [
        {
          "type": "command_submitted",
          "pane_id": 1,
          "command": "make test"
        },
        {
          "type": "command_completed",
          "pane_id": 1,
          "command": "make test",
          "exit_code": 0,
          "duration_ms": 320
        },
        {
          "type": "echo_lock_changed",
          "pane_id": 1,
          "locked": false
        }
      ]
    }
  }
  ```

#### `playbook.run`
Executes a multi-step declarative TOML playbook in a confined sandbox.
- **Parameters**:
  - `toml` (string, required): Raw TOML playbook content.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 11, "method": "playbook.run", "params": {"toml": "name = 'test'\n[[steps]]\ncolon = ':split v'\n"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 11,
    "result": {
      "name": "test",
      "passed": true,
      "steps_completed": 1,
      "events": []
    }
  }
  ```

#### `remote.connect_ssh`
Connects to an SSH host and spawns an interactive pane, optionally attaching to remote `tmux -CC`.
- **Parameters**:
  - `target` (string, required): SSH target (`user@hostname`).
  - `port` (integer, optional): SSH port (u16).
  - `tags` (array of strings, optional): Metadata tags.
  - `tmux_session` (string, optional): Remote tmux session name to attach with control mode (`-CC`).
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 12, "method": "remote.connect_ssh", "params": {"target": "user@192.168.1.50", "tags": ["prod", "web"]}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 12,
    "result": {
      "status": "connected_ssh",
      "pane_id": 3,
      "target": "user@192.168.1.50"
    }
  }
  ```

#### `remote.connect_tmux`
Directly connects to a remote host and attaches to a specified `tmux -CC` session. Note that `status` reports `"connecting"` honestly until the remote control stream handshake completes.
- **Parameters**:
  - `target` (string, required): SSH target.
  - `session` (string, optional, default `"bilterm"`): Remote session name.
  - `tags` (array of strings, optional): Host tags.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 13, "method": "remote.connect_tmux", "params": {"target": "user@host", "session": "main"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 13,
    "result": {
      "status": "connecting",
      "state": "connecting",
      "pane_id": 4,
      "target": "user@host",
      "session": "main",
      "link": {
        "state": "connecting",
        "dest": "user@host",
        "session": "main",
        "attempt": 0,
        "max_attempts": 20,
        "retry_in_ms": null,
        "primary_pane": 4,
        "panes": [],
        "stale_panes": [],
        "last_exit": null
      }
    }
  }
  ```

#### `remote.link`
Inspects, reconnects, or disconnects the persistent remote `tmux -CC` session link.
- **Parameters**:
  - `op` (string, optional, default `"status"`): One of `"status"`, `"reconnect"`, or `"disconnect"`.
- **Result Object**:
  Returns standard `wm.command` execution fields (`status`, `command`, `active_pane_id`, `pane_count`, optional `data`) plus a top-level `"link"` object (`LinkSummary`):
  - `state` (string): Current link lifecycle state (`"idle"`, `"connecting"`, `"connected"`, `"reconnecting"`, `"detached"`, `"ended"`, `"failed"`).
  - `dest` (string or null): Target host destination (`user@host[:port]`).
  - `session` (string or null): Remote `tmux` session name.
  - `attempt` (integer): Current reconnection attempt count (`0..max_attempts`).
  - `max_attempts` (integer): Maximum backoff retry cap (20).
  - `retry_in_ms` (integer or null): Milliseconds remaining until the next automatic retry when in `reconnecting` (`Backoff`) state.
  - `primary_pane` (integer or null): Bilterm `PaneId` of the primary connection pane.
  - `panes` (array of objects): List of mapped panes `[{"pane": int, "tmux_pane": int}]`.
  - `stale_panes` (array of integers): Panes previously mapped that were closed or orphaned.
  - `last_exit` (object or null): Details of the last disconnect event:
    - `cause` (string): High-level disconnect cause.
    - `class` (string): Disconnect classification (`"transient"`, `"clean"`, `"fatal"`).
    - `code` (integer or null): Process exit code.
    - `reason` (string or null): `%exit` reason string or error diagnosis.
    - `tail` (array of strings): Last sanitised diagnostic output lines from stderr/PTY.
    - `at_ms` (integer): Timestamp (epoch milliseconds) of the disconnect.
- **Error Responses**:
  - If `params` is not an object or null: `-32602` `"invalid remote.link parameter: params must be an object"`.
  - If `op` is not a string: `-32602` `"invalid remote.link parameter: op must be a string"`.
  - If `op` is not one of the allowed operations: `-32602` `"invalid remote.link parameter: op must be one of status, reconnect, disconnect"`.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 14, "method": "remote.link", "params": {"op": "status"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 14,
    "result": {
      "status": "handled",
      "command": ":ssh-tmux info",
      "active_pane_id": 1,
      "pane_count": 2,
      "link": {
        "state": "connected",
        "dest": "user@remote",
        "session": "bilterm",
        "attempt": 0,
        "max_attempts": 20,
        "retry_in_ms": null,
        "primary_pane": 1,
        "panes": [
          {"pane": 1, "tmux_pane": 0}
        ],
        "stale_panes": [],
        "last_exit": null
      }
    }
  }
  ```

#### `fleet.register`
Registers a remote profile in the central fleet registry.
- **Parameters**:
  - `name` (string, required): Node alias.
  - `destination` (string, required): SSH destination (`user@host`).
  - `tags` (array of strings, optional): Node tags.
  - `rpc_endpoint` (string, optional): Target daemon endpoint.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 14, "method": "fleet.register", "params": {"name": "worker-1", "destination": "admin@10.0.0.12", "tags": ["worker"]}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 14,
    "result": {
      "status": "registered",
      "name": "worker-1",
      "node_count": 1
    }
  }
  ```

#### `fleet.list`
Lists all nodes in the fleet registry.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 15, "method": "fleet.list", "params": {}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 15,
    "result": {
      "nodes": [
        {
          "name": "worker-1",
          "connected": true,
          "tags": ["worker"],
          "last_command": "uptime"
        }
      ]
    }
  }
  ```

#### `fleet.exec`
Broadcasts a command to panes matching a selector (`all`, `local`, `remote`, `@<tag>`, or node name).
- **Parameters**:
  - `command` (string, required): Shell command.
  - `selector` (string, optional, default `"all"`): Target filter.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 16, "method": "fleet.exec", "params": {"selector": "@worker", "command": "uname -a"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 16,
    "result": {
      "status": "dispatched",
      "selector": "@worker",
      "panes": [2, 3],
      "nodes": ["worker-1", "worker-2"]
    }
  }
  ```

#### `fleet.disconnect`
Deregisters and disconnects a fleet node.
- **Parameters**:
  - `name` (string, required): Profile name.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 17, "method": "fleet.disconnect", "params": {"name": "worker-1"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 17,
    "result": {
      "removed": true,
      "name": "worker-1"
    }
  }
  ```

#### `canvas.command`
Sends commands to the Tektronix 4014 vector storage-tube emulator or retained graphics scene.
- **Parameters**:
  - `command` (string, optional): e.g. `"tek demo"`, `"line 0 0 20 10"`, `"sprite 1 4 4 2 0"`, `"plot candlestick"`, `"clear"`.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 18, "method": "canvas.command", "params": {"command": "tek demo"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 18,
    "result": {
      "tektronix_active": true,
      "tektronix_segments": 142,
      "tektronix_decaying_segments": 0,
      "canvas_active": true,
      "render_backend": "Auto",
      "effective_backend": "HalfBlock",
      "subcell_mode": "Braille",
      "primitives": 142,
      "sprites": 0,
      "plots": 0,
      "has_image": false,
      "sprite_collisions": 0
    }
  }
  ```

#### `canvas.status`
Queries the retained graphical canvas and Tektronix state without mutating it (or optionally executing an initial command).
- **Parameters**:
  - `command` (string, optional): Optional command executed prior to inspection.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 19, "method": "canvas.status", "params": {}}
  ```
- **Response**: Same result shape as `canvas.command`.

#### `session.save`
Saves current workspace configuration, pane counts, and layout tree to disk.
- **Parameters**:
  - `name` (string, optional, default `"default"`): Session name.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 20, "method": "session.save", "params": {"name": "my-session"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 20,
    "result": {
      "saved": true,
      "session": "my-session",
      "path": "/home/user/.local/state/bilterm/sessions/my-session.toml",
      "panes": 2
    }
  }
  ```

#### `session.restore`
Restores a previously saved workspace session.
- **Parameters**:
  - `name` (string, optional, default `"default"`): Session name.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 21, "method": "session.restore", "params": {"name": "my-session"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 21,
    "result": {
      "restored": true,
      "session": "my-session",
      "panes": 2
    }
  }
  ```

#### `window.move`
Moves a window (composite pane) to a target session preserving its live PTY, scrollback, running command, and prompt (`SES-01`).
- **Parameters**:
  - `target_session` (string, required): Destination session name or nested path.
  - `window_id` (integer, optional): Window/pane ID to move (defaults to active window).
  - `switch` (boolean, optional, default `false`): Switch active session to `target_session` after move.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 22, "method": "window.move", "params": {"target_session": "dev", "switch": true}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 22,
    "result": {
      "status": "ok",
      "window_id": 1,
      "source_session": "default",
      "target_session": "dev"
    }
  }
  ```

#### `window.rename`
Renames a window (composite pane) with safe validation and deterministic deduplication (`SES-02`).
- **Parameters**:
  - `new_name` (string, required): New title for the window.
  - `window_id` (integer, optional): Target window ID (defaults to active window).
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 23, "method": "window.rename", "params": {"new_name": "editor"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 23,
    "result": {
      "status": "ok",
      "window_id": 1,
      "name": "editor"
    }
  }
  ```

#### `session.rename`
Renames an existing session, updating active configuration and session hierarchy (`SES-02`).
- **Parameters**:
  - `new_name` (string, required): New session name.
  - `old_name` (string, optional): Target session name (defaults to active session).
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 24, "method": "session.rename", "params": {"new_name": "prod"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 24,
    "result": {
      "status": "ok",
      "old_name": "default",
      "new_name": "prod"
    }
  }
  ```

#### `window.select`
Updates multi-selection state for windows in the active session (`SES-03`).
- **Parameters**:
  - `action` (string, required): Selection operation (`"toggle"`, `"range"`, `"all"`, `"none"`, `"invert"`, `"explicit"`).
  - `window_id` (integer, optional): Target window ID for `"toggle"`.
  - `from` (integer, optional): Start window ID for `"range"`.
  - `to` (integer, optional): End window ID for `"range"`.
  - `windows` (array of integers, optional): Explicit list of window IDs for `"explicit"`.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 25, "method": "window.select", "params": {"action": "all"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 25,
    "result": {
      "status": "ok",
      "selected": [1, 2],
      "count": 2
    }
  }
  ```

#### `window.bulk`
Executes an atomic all-or-nothing operation across all multi-selected windows (`SES-03`).
- **Parameters**:
  - `action` (string, required): Bulk action (`"move"`, `"close"`, `"rename_prefix"`, `"tile"`, `"pin"`).
  - `target_session` (string, optional): Target session for `"move"`.
  - `prefix` (string, optional): Name prefix for `"rename_prefix"`.
  - `force` (boolean, optional, default `false`): Force close bypassing exit guard.
  - `pinned` (boolean, optional): Set pinned status for `"pin"`.
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 26, "method": "window.bulk", "params": {"action": "tile"}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 26,
    "result": {
      "status": "ok",
      "action": "tile",
      "affected_windows": [1, 2],
      "count": 2
    }
  }
  ```

#### `window.undo`
Reverts the last window or session mutation (`SES-04`).
- **Request**:
  ```json
  {"jsonrpc": "2.0", "id": 27, "method": "window.undo", "params": {}}
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 27,
    "result": {
      "status": "ok",
      "reverted": "Move window #2 to 'dev'"
    }
  }
  ```

---

## 4. Model Context Protocol (MCP) Server Reference

When started with `bilterm --mcp`, Bilterm provides a native Model Context Protocol (MCP) server over `stdin`/`stdout`.
- Protocol Version: `2024-11-05`
- Server Name: `bilterm-mcp`
- Standard Capabilities: `tools` (`listChanged: false`), `resources` (`subscribe: false`, `listChanged: false`)

### Master Table of MCP Tools

The server exposes **14 tools** (the `tools/list` result).

| Tool Name | Parameters (`inputSchema`) | Action / Purpose | Underlying RPC |
| :--- | :--- | :--- | :--- |
| `bilterm_workspace_status` | None (`{}`) | Inspect all composite panes, topology (Tiled/Stacked), active pane, and 3270 OIA telemetry. | `workspace.status` |
| `bilterm_exec_command` | `command` (string, req), `pane_id` (integer) | Submit shell command into a pane with `!ECHO` password-lock safety. | `pane.exec` |
| `bilterm_inspect_pane` | `pane_id` (integer) | Read visible lines, cursor coordinates, OSC 133 command blocks, field hints, and alerts. | `pane.read` |
| `bilterm_send_keys` | `keys` (string, req) | Send synthetic key chords (`Ctrl+C`, `F12`, `Enter`, `Up`, `Alt+H`) to drive TUIs. | `pane.send_keys` |
| `bilterm_colon_command` | `command` (string, req) | Execute any `:colon` command (`:split v`, `:layout stack`, `:theme 3278`, `:get theme`). | `wm.command` |
| `bilterm_run_playbook` | `toml` (string, req) | Execute a declarative TOML automation playbook against the workspace. | `playbook.run` |
| `bilterm_remote_connect` | `target` (string, req), `tmux_session` (string) | Spawn remote SSH or `tmux -CC` pane and register in fleet registry. | `remote.connect_ssh` |
| `bilterm_remote_link` | `op` (string, optional: `"status"`, `"reconnect"`, `"disconnect"`) | Show link status, reconnect immediately, or disconnect persistent remote tmux link. | `remote.link` |
| `bilterm_fleet_exec` | `selector` (string, req), `command` (string, req) | Fan out shell command across sessions matching selector (`all`, `local`, `remote`, `@tag`, node). | `fleet.exec` |
| `bilterm_workspace_fontsize`| `action` (string), `size` (integer) | Query or dynamically adjust terminal font size and scale percentage. | `workspace.fontsize` |
| `bilterm_canvas` | `command` (string) | Control Tektronix 4014 vector storage-tube and modern retained graphics canvas. | `canvas.command` |
| `bilterm_find` | `kind` (string, req: `agent`, `session` or `history`), `query` (string), `limit` (integer, default 50, max 200) | Search the unified registry (agents, sessions, command history). Every row carries `source`; the response carries `masked`. | `find.agents` / `find.sessions` / `find.history` |
| `bilterm_agents_list` | `op` (string, optional: `open`, `close`, `toggle`, `dismiss`, `blur`, `select`, `jump`, `zoom`, `pin`, `unpin`, `scale`, `filter`, `dock`, `float`, `move`, `resize`, `rect`, `size` or `scroll`; absent reads the state), `args` (string, space-separated) | Open, close, pin, select, jump, zoom, scale, filter, dock or position the AGENTS list window, or read the agent state. Returns the `agents_list` object (see "Agents list and demo"). | `agents.list` |
| `bilterm_agents_demo` | `stop` (boolean, optional; `true` stops the demo and closes the panes it spawned) | Start the agent traffic demo, or stop it. No write gate. | `agents.demo` |
| `bilterm_window_manage` | `action` (string, req: `move`, `rename`, `rename_session`, `select`, `bulk`, `undo`), plus action-specific parameters | Move, rename, multi-select, bulk mutate, and undo window and session topology (`SES-01..05`). | `window.move`, `window.rename`, `session.rename`, `window.select`, `window.bulk`, `window.undo` |

### MCP Tool Calling Format

Requests are sent via the standard MCP `tools/call` JSON-RPC wrapper:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "bilterm_exec_command",
    "arguments": {
      "command": "cargo test",
      "pane_id": 1
    }
  }
}
```

Successful replies return MCP content blocks:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\"command\":\"cargo test\",\"pane_id\":1,\"status\":\"submitted\"}"
      }
    ],
    "isError": false
  }
}
```

---

## 5. MCP Resources (`resources/list` & `resources/read`)

Bilterm implements dynamic read-only MCP resources providing structured state and catalogs. All resources use MIME type `application/json`.

| URI | Name | Description |
| :--- | :--- | :--- |
| `bilterm://state` | Workspace state | Full workspace status plus resolved appearance profile, display mode, and drawer state. |
| `bilterm://config` | Effective config | Non-secret effective configuration values (theme, snazzy mode, glass mode, etc.). |
| `bilterm://commands` | Colon command catalog | Complete catalog of every available `:colon` command with summaries, aliases, and examples. |
| `bilterm://pane/{id}` | Pane #{id} | Dynamic resource per pane: visible lines, cursor, command blocks, field hints, and notifications. |

### Reading Resources
- **Request**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 2,
    "method": "resources/read",
    "params": {
      "uri": "bilterm://config"
    }
  }
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 2,
    "result": {
      "contents": [
        {
          "uri": "bilterm://config",
          "mimeType": "application/json",
          "text": "{\"theme\":\"Borland\",\"snazzy_mode\":false,\"glass_mode\":false,\"crt_scanlines\":false,\"leader_key\":\"Ctrl+B\",...}"
        }
      ]
    }
  }
  ```

---

## 6. HTTP & SSE Transport (`--http`)

Bilterm includes an opt-in, loopback-only HTTP/1.1 and Server-Sent Events (SSE) server launched via `--http <ADDR>` in conjunction with `--headless`:

```bash
bilterm --headless --http 127.0.0.1:7371
```

### Addressing & Binding
- `validate_http_bind` strictly requires loopback addresses: `127.0.0.1:PORT`, `[::1]:PORT`, or `localhost:PORT`.
- Non-loopback addresses are rejected immediately on startup.

### Bearer Token Authentication
- A random 256-bit cryptographically secure token (64 hex characters from `/dev/urandom`) is generated automatically at daemon startup.
- The token is written atomically with mode `0600` to the private runtime directory at `<runtime_dir>/bilterm-http.token`.
- Every HTTP request must supply the header:
  ```http
  Authorization: Bearer <token>
  ```
- Missing or invalid tokens return `401 Unauthorized` with `WWW-Authenticate: Bearer` and a JSON error body:
  ```json
  {"error": "missing or invalid bearer token"}
  ```
- Token comparisons are executed in constant time (`constant_time_eq`) to mitigate timing attacks.

### Routes

#### 1. `GET /health`
Returns daemon status and version.
- **Request**:
  ```bash
  curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:7371/health
  ```
- **Response** (`200 OK`):
  ```json
  {"ok": true, "version": "0.1.0"}
  ```

#### 2. `POST /rpc`
Tunnels any JSON-RPC 2.0 request line through the internal control plane.
- **Request**:
  ```bash
  curl -X POST -H "Authorization: Bearer $TOKEN" \
       -H "Content-Type: application/json" \
       -d '{"jsonrpc":"2.0","id":1,"method":"workspace.status","params":{}}' \
       http://127.0.0.1:7371/rpc
  ```
- **Response** (`200 OK`): Returns the standard JSON-RPC 2.0 response.

#### 3. `GET /events`
Server-Sent Events (SSE) stream draining from `events.poll`.
- **Request**:
  ```bash
  curl -N -H "Authorization: Bearer $TOKEN" http://127.0.0.1:7371/events
  ```
- **Response** (`200 OK`, `Content-Type: text/event-stream`):
  ```http
  HTTP/1.1 200 OK
  Content-Type: text/event-stream
  Cache-Control: no-store
  X-Content-Type-Options: nosniff
  Connection: close

  : bilterm events

  id: 1
  data: {"command":"ls","pane_id":1,"type":"command_submitted"}

  id: 2
  data: {"command":"ls","duration_ms":10,"exit_code":0,"pane_id":1,"type":"command_completed"}

  : keepalive
  ```
- **Keepalive**: Emits `: keepalive\n\n` comments after 60 idle polling cycles (~15 seconds).

### Security Gates & HTTP Error Codes
- **DNS-Rebinding / Host Check**: The `Host` header must resolve to a loopback address (`localhost`, `127.0.0.1`, `::1`). Other hosts return `403 Forbidden`.
- **Origin Check**: If an `Origin` header is present, it must match `http://<host>`. Cross-origin browser requests are rejected with `403 Forbidden` (`cross-origin request refused`).
- **CORS**: No CORS response headers are ever set.
- **Body & Header Limits**:
  - Max body size: 1 MiB (`MAX_HTTP_BODY_BYTES = 1048576`). Returns `413 Payload Too Large`.
  - Max header size: 16 KiB (`MAX_HTTP_HEAD_BYTES = 16384`). Returns `431 Request Header Fields Too Large`.
  - Max header count: 64 headers (`MAX_HEADERS = 64`). Returns `431 Request Header Fields Too Large`.
- **Read Timeout**: 10 seconds (`READ_TIMEOUT`). Returns `408 Request Timeout`.
- **Connection Cap**: Max 64 concurrent active connections managed by semaphore.
- **Transfer-Encoding**: Chunked transfer encoding is unsupported; returns `400 Bad Request`.
- **Method Not Allowed**: Invalid HTTP methods on known routes return `405 Method Not Allowed`.

---

## 7. Configuration & State Automation (`:get`, `:set`, `:config`, `:state`)

Bilterm exposes generic dotted-path access to all configuration fields in `BiltermConfig`.

### Inspection & Mutation Commands

| Command | Arguments | Data Returned (`data` field in RPC result) | Description |
| :--- | :--- | :--- | :--- |
| `:get` | `<dotted.key>` | `{"key": str, "value": str}` | Read configuration setting at path (e.g. `:get theme`, `:get appearance.profile`). |
| `:set` | `<dotted.key> <value>` | `{"key": str, "value": str}` | Modify configuration in-memory for active session (e.g. `:set theme Dracula`). |
| `:config list` | None | `{"<key>": "<val>", ...}` | Returns full map of all set configuration keys and values. |
| `:config save` | None | `{"path": str, "written": [str]}` | Persists full modified configuration to `config.toml`. |
| `:appearance save`| None | `{"path": str, "written": [str]}` | Persists only `appearance.*` keys to `config.toml`. |
| `:state` | None | `{"appearance": str, "display_mode": str, "layout_mode": str, "active_pane_id": int, ...}` | Returns structured summary of workspace and UI profile state. |

### Agent State: `:agents list` (Phase 50)

`:agents list` (also `:agents` and `:agents status`) reports the per-pane agent state shown by the Navigator drawer. Run it through `wm.command` (RPC), `bilterm_colon_command` (MCP) or `bilterm ctl`. The data is a read and is not subject to `[api] allow_config_write`. Since Phase 51 the same command also opens the AGENTS list window, and the payload gained an `agents_list` object (see "Agents list and demo" below).

```json
{"agents": [
   {"pane_id": 3, "name": "cargo watch", "state": "working", "since_ms": 4210,
    "profile": null, "reported": false,
    "partners": [{"label": "you", "count": 2}]}
 ],
 "counts": {"working": 1, "blocked": 0, "idle": 2, "done": 0, "errored": 0},
 "traffic": {"latest_seq": 17, "len": 17, "dropped": 0}}
```

| Field | Type | Meaning |
| :--- | :--- | :--- |
| `agents[].pane_id` | int | Pane id |
| `agents[].name` | str | Sanitised pane name |
| `agents[].state` | str | `working`, `blocked`, `idle`, `done` or `errored` |
| `agents[].since_ms` | int | Milliseconds since the state last changed |
| `agents[].profile` | str or null | Name of the matching `[agents.<name>]` profile |
| `agents[].reported` | bool | `true` when the state was set explicitly rather than derived (reserved for Phase 51) |
| `agents[].partners[]` | `{label, count}` | Recent traffic partners in the last 60 s (`you`, `api`, `mcp`, `#<id>`, `@<session>`) |
| `counts` | object | Number of panes per state |
| `traffic` | object | Traffic bus counters: `latest_seq`, `len` (at most 512), `dropped` |

`:state` is unchanged by Phase 50. **Phase 50 does not publish traffic events on `/events` or `events.poll`**: the traffic bus is in-memory only, previews are sanitised, capped at 160 characters and replaced by `[redacted]` for password prompts, and nothing is persisted. See [ADR-008](architecture/adr/ADR-008-agent-traffic-bus.md); Phase 51 defines the external `agent.*` events. The Navigator and the Agent Traffic window are described in the [user guide, chapter 11](user-guide/11-navigator-sidebar-and-agent-traffic.md).

### Communication Topology: `:agents topology` (Phase 50)

`:agents topology` reports which pairs of endpoints communicated in this session. Like `:agents list` it is read-only and works through `wm.command`, `bilterm_colon_command` and `bilterm ctl`. It is independent of the 512-event traffic ring: a pair stays listed after its events were evicted. The data holds sanitised labels, counts and times only, never message content.

```json
{"version": 1, "pair_count": 2, "dropped_pairs": 0, "latest_seq": 9, "fade_secs": 60,
 "pairs": [
   {"a": "#1", "b": "#2", "a_key": "pane:1", "b_key": "pane:2", "count": 6,
    "first_seq": 1, "last_seq": 9, "first_ago_ms": 52000, "last_ago_ms": 800, "heat_permille": 610}
 ]}
```

Pairs are ordered by descending `count`; `*_ago_ms` is relative to the query; `heat_permille` is the current edge heat (0 once the fade window passed). At most 256 pairs are kept (`dropped_pairs` counts the least recently active ones that were dropped). `:agents topology export <name>` writes the same JSON to a private file and is refused over the API. `:agents fade [secs]` / `[agents_window] edge_fade_secs` set the fade window.

### Agents list and demo: `agents.list`, `agents.demo` and MCP tools (Phase 51)

`agents.list` and `agents.demo` (RPC, HTTP `POST /rpc`, `bilterm ctl`) and the MCP tools `bilterm_agents_list` and `bilterm_agents_demo` build the colon form (`:agents list [op args...]` or `:agents demo [stop]`), run it through the same parser and executor as `wm.command`, and add a small summary object. They are the same operations as the AGENTS list window's keys and mouse, not a second implementation. Neither is subject to `[api] allow_config_write`: they change window and demo state, never configuration.

**`agents.list`**

- `op` (optional string). Absent or `list` reads the state. Any other value must be one of `open`, `close`, `toggle`, `dismiss`, `blur`, `select`, `jump`, `zoom`, `pin`, `unpin`, `scale`, `filter`, `dock`, `float`, `move`, `resize`, `rect`, `size`, `scroll`. Anything else is rejected.
- `args` (optional): an array of strings or integers, or one whitespace-separated string. Each token must match `[A-Za-z0-9+\-_.#%:]`, be at most 32 characters, and not start with `:`. At most 8 tokens. `args` without an `op` (or with `op` `list`) is rejected. Because every token is checked, no argument can carry a second command.
- Rejections are `invalid agents.list parameter: <reason>` (JSON-RPC error `-32000`; MCP `isError: true`).
- **Opens the window.** Like `:agents list`, the call opens the AGENTS list window and gives it the keyboard (Phase 51, D-03).

Result: the `wm.command` result fields (`status`, `command`, `active_pane_id`, `pane_count`, and `data` when the verb returns data) plus a top-level `agents_list` object:

| Field | Type | Meaning |
| :--- | :--- | :--- |
| `active` | bool | The window is open |
| `pinned` | bool | The window is pinned |
| `focused` | bool | The window has the keyboard |
| `docked` | bool | The window is in the tiling strip |
| `filter` | str | `all` or `agents` |
| `selected` | int or null | Selected pane id |
| `row_count` | int | Rows after the filter |
| `scale_pct` | int | GUI scale, 75 to 200 |

The object carries flags, counts and ids only; it never includes pane names. When `op` is an absolute `scale <75..200>` the verb runs the generic panel-scale path and returns no `data`; `agents_list` is still present at the top level.

**`agents.demo`**

- `stop` (optional boolean, default `false`). `false` starts the demo; `true` stops it.
- Result: the `wm.command` result fields plus `demo`: `running` (true while the demo has endpoints or spawned panes), `endpoints` (pane ids the demo drew traffic between) and `spawned` (pane ids the demo created).
- Starting the demo again replaces the earlier demo events and demo topology pairs; it does not leak panes.
- **Stop closes only the panes the demo created** (`spawned`), removes the `[demo]` events and the demo's topology pairs, and never closes a pane it did not create. `agents.demo {"stop": true}` needs no write gate: it works with `[api] allow_config_write = false`.

**Requests**

```bash
bilterm ctl agents.list '{"op":"toggle"}'
bilterm ctl agents.list '{"op":"select","args":["down"]}'
bilterm ctl agents.demo '{}'
bilterm ctl agents.demo '{"stop":true}'
```

```bash
curl -X POST -H "Authorization: Bearer $TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":2,"method":"agents.demo","params":{"stop":true}}' \
     http://127.0.0.1:7371/rpc
```

MCP `tools/call` with `"name":"bilterm_agents_list"` and `"arguments":{"op":"open"}`, or `"name":"bilterm_agents_demo"` and `"arguments":{"stop":true}`. The `bilterm_agents_list` tool takes `op` and `args` (a space-separated string, for example `"args":"down"`); `bilterm_agents_demo` takes `stop`.

**The `:agents list` payload.** `wm.command ":agents list"`, bare `:agents`, `:agents ls` and `:agents status` all open the window, and their `data` gained the additive `agents_list` object (the same fields as above).

**Other Phase 51 API changes.**

- `bilterm://commands` (MCP resource): every command carries `palette_exclusions`, an array of `{probe, reason}` objects. The `reason` is one of `free-text`, `host-or-path`, `entity-id`, `numeric`, `combinatorial`, `destructive`, `reachable-via-picker`, `short-alias`. The list shows which forms have no palette entry and why. See [Chapter 11, Command palette coverage](user-guide/11-navigator-sidebar-and-agent-traffic.md#command-palette-coverage).
- MCP tools: `tools/list` returns **13 tools**, up from 11: `bilterm_agents_list` and `bilterm_agents_demo` are the two added at the end, after `bilterm_find`.

### Unified Find: `find.*` methods and `bilterm_find` (Phase 50.1)

`find.agents`, `find.sessions` and `find.history` (RPC) and `bilterm_find` (MCP, `kind` = `agent` | `session` | `history`) run the same search as the Find window (`Ctrl+Alt+F`, `:find agent|session|history`). They are read-only: they do not open the window, move focus or change any modal state, and they are not subject to `[api] allow_config_write`.

- **Request**: `bilterm ctl find.history '{"query":"cargo","limit":10}'`, or the `tools/call` wrapper with `"name":"bilterm_find"` and `"arguments":{"kind":"history","query":"cargo","limit":10}`.
- **Parameters**: `query` is optional and truncated to 128 characters. `limit` is optional, defaults to 50 and is clamped to 1..200. A non-string `query`, a non-integer `limit`, or a missing or unknown MCP `kind` is an invalid-parameters error (`-32602`).
- **Response**: the envelope is the result itself, not wrapped in `data`. `wm.command` with `:find ...` returns the same JSON in its `data` field.

```json
{"kind":"history","query":"cargo","total":2,"returned":2,"truncated":false,"masked":1,
 "rows":[
   {"id":"history:main/12","kind":"history","row":"history","label":"cargo test -q",
    "detail":"cwd /home/me/src/app","score":812,
    "source":{"kind":"live","name":"main","path":null},"also_in":[],
    "transport":"local","host":null,"tmux_session":null,"tmux_window":null,"tmux_pane":null,
    "tags":[],"metadata_only":false,"pane":1,"session":"main",
    "seq":12,"command":"cargo test -q","exit_code":0,"duration_ms":1210},
   {"id":"history:dev/3","kind":"history","row":"history","label":"cargo fmt --check",
    "detail":"cwd /home/me/src/app","score":640,
    "source":{"kind":"history-log","name":"dev","path":"/home/me/.local/state/bilterm/sessions/dev/history.log"},
    "also_in":[],"transport":"local","host":null,"tmux_session":null,"tmux_window":null,
    "tmux_pane":null,"tags":[],"metadata_only":false,"pane":null,"session":"dev",
    "seq":3,"command":"cargo fmt --check","exit_code":null,"duration_ms":null}
 ]}
```

| Field | Type | Meaning |
| :--- | :--- | :--- |
| `total` | int | Rows that matched, before `limit` |
| `returned` | int | Rows in `rows` |
| `truncated` | bool | `true` when `total` is greater than `returned` |
| `masked` | int | Secret-looking entries found and excluded. A count only; masked text is never returned |
| `rows[].kind` | str | `agent`, `session`, `pane` or `history` (the session kind returns `session` and `pane` rows) |
| `rows[].source` | object | Where the row came from: `kind` (`live`, `saved-session`, `history-log`), `name` (workspace session), `path` (absolute path for disk rows, `null` for live rows) |
| `rows[].also_in` | array | Other `source` objects that hold the same session name |
| `rows[].transport` | str | `local`, `ssh`, `tmux` or `ssh-tmux` |
| `rows[].metadata_only` | bool | `true` for an `:ssh-tmux` label row, which starts no tmux (gap G2) |
| `rows[].exit_code` | int or null | History rows: exit code when observed, else `null` |

Gaps that affect these results (remote exit codes and cwd need OSC 133 and OSC 7 on the remote shell; raw-mode input is not captured; no command attaches the `tmux -CC` bridge) are listed in [user guide chapter 12](user-guide/12-find-registry-and-history.md#9-known-limits-g1-to-g10).

### API Security Switch: `[api] allow_config_write`

All API-originated operations (invoked via RPC `wm.command` or MCP `bilterm_colon_command`) are subject to strict policy enforcement:
1. **Default Off**: In `config.toml`, `[api] allow_config_write = false` by default.
2. **Refusal Behavior**:
   When `allow_config_write` is `false`, any attempt to execute `:set`, `:config save`, or `:appearance save` over JSON-RPC or MCP fails with:
   ```
   config writes over the API are disabled; set [api] allow_config_write = true in config.toml
   ```
3. **Protection of `[api]` Section**:
   Even when `allow_config_write = true`, any attempt to modify keys in the `[api]` section over the API is unconditionally refused:
   ```
   the [api] section cannot be changed over the API
   ```
   This ensures that remote or automated agents cannot escalate their own privileges.

---

## 8. Relationship to Colon Commands (`:commands`)

Every user-triggerable operation in Bilterm lowers to the canonical `crate::action::Action` enum (`pub type ColonCommand = crate::action::Action;`).

- **Programmatic Invocation**:
  - Over JSON-RPC: Call `wm.command` with `{"command": ":<cmd>"}` or `{"command": "<cmd>"}`.
  - Over MCP: Call `bilterm_colon_command` with `{"command": "<cmd>"}`.
- **In-App Discovery (`:commands`)**:
  - Running `:commands`, `:palette`, or `:cmd` in Bilterm's command box (or pressing `Ctrl+X`) launches the interactive Fuzzy Command Palette.
  - Pressing `F1` (or executing `:help`) opens the interactive help modal displaying keybindings and command usage.
- **Resource Inspection**:
  - Reading the MCP resource `bilterm://commands` returns the entire typed command catalog including summaries, aliases, examples, and shortcuts.
- **Structured Data Integration**:
  - When an action or colon command produces structured output, `WindowManager::set_command_data` attaches it to the execution context.
  - `wm.command` and `bilterm_colon_command` return this data in the `"data"` field of the JSON result object.

---

## 9. Security & Sandboxing Architecture

Bilterm's automation plane enforces defence-in-depth isolation:

### Unix Domain Socket Security
- **Path Length Limit**: Sockets are bound to paths under 103 bytes (`MAX_SOCKET_PATH_BYTES`), well within POSIX limits (104 bytes on macOS, 108 bytes on Linux).
- **Directory & File Permissions**: The socket directory is created with `0700` (`rwx------`), and the socket file is chmodded to `0600` (`rw-------`).
- **Peer Credential Enforcement**: On every incoming Unix socket connection, Bilterm queries `stream.peer_cred()`. The peer UID must match the daemon's effective UID (`current_euid()`). Any mismatched connection is immediately rejected (`PermissionDenied`).
- **Stale Socket Handling**: Before binding, Bilterm probes existing files at the socket path. If an active daemon responds, startup fails. If the probe fails and the file is a socket, the stale socket file is unlinked. Bilterm refuses to unlink non-socket files.

### TCP & Network Security
- **Loopback Default**: TCP listeners default to `127.0.0.1:7370`.
- **Remote Binding Barrier**: Binding to `0.0.0.0` or any non-loopback interface requires both `--allow-remote` and `--token-file <PATH>` pointing to a file with mode `0600`.
- **Constant-Time Verification**: All token checks use `constant_time_eq` to prevent side-channel timing leaks.

### Snapshot File Confinement (`SnapshotPolicy::Confined`)
- **Path Validation**: Snapshot filenames passed to `workspace.snapshot` or playbooks are validated by `validate_snapshot_name`. Slashes (`/`, `\`), path traversal (`..`), and null bytes are rejected.
- **Directory Confinement**: Files must resolve strictly within the designated snapshot directory (defaults to XDG state directory: `~/.local/state/bilterm/snapshots/`). Canonical parent checks prevent symlink escapes.
- **File Permissions**: Snapshots are opened with exclusive private permissions (mode `0600`).

### PTY & Execution Safety
- **Password Prompt Lock (`!ECHO LOCK`)**: Bilterm's line discipline monitors terminal echo states. When a program requests a password (terminal echo disabled), `pane.exec` rejects commands unless `allow_echo_lock: true` is explicitly provided.
- **Read-Only Protection (`🔒 PROT`)**: Panes set to read-only mode reject any command submission via `pane.exec`.
