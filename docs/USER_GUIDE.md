# Bilterm User Guide & Operator Reference

## Background Daemon Supervisor & Multi-Daemon Operations (`DAEM-05..08`)

Bilterm includes an enterprise-grade background session supervisor and multi-daemon registry that decouples terminal multiplexer execution from any single host terminal or graphical window.

### Background Daemon Supervisor

By default, Bilterm operates with at most one background daemon (`"default"`). The singleton lifecycle is strictly protected by an exclusive advisory file lock (`daemon-singleton.lock`) inside the private, mode `0700` runtime directory (`$XDG_RUNTIME_DIR/bilterm/daemons/` or `/tmp/bilterm-$UID/daemons/`).

#### 1. Automatic Singleton Startup
When issuing control commands via `bilterm ctl` without an active daemon, Bilterm automatically auto-spawns the default background daemon, waits for Unix domain socket readiness, and executes the requested operation seamlessly without manual intervention.

#### 2. Multi-Daemon Isolation
For environments requiring isolated environments, multi-daemon mode is strictly opt-in via:
```bash
bilterm --daemon --name <name>
```
or configuring `[daemon] mode = "multi"` in `~/.config/bilterm/config.toml`. Every named daemon maintains:
- Dedicated Unix domain socket: `daemons/daemon-<name>.sock`
- Dedicated exclusive lockfile: `daemons/daemon-<name>.lock`
- Live metadata registry JSON: `daemons/<name>.json`

Daemon names are strictly validated against `^[A-Za-z0-9_-]{1,64}$`, rejecting path traversals, whitespace, dots, and NUL bytes.

Any number of named daemons run side by side. Without `-l/--listen`, each named daemon listens on its own `daemons/daemon-<name>.sock`; only the unnamed (`default`) daemon uses the legacy `bilterm.sock`. An explicit `-l unix:PATH` (or TCP address) always wins.

Manage them from the shell (works with no daemon running, and for daemons started by hand):
```bash
bilterm daemon list [--json]            # NAME / STATUS / PID / VERSION / SOCKET of every daemon
bilterm daemon start [NAME]             # start NAME in the background (no-op if already running)
bilterm daemon status [NAME] [--json]   # exit 0 when running, 3 when stopped or crashed
bilterm daemon stop [NAME] [--force]    # SIGTERM (SIGKILL with --force)
```
`NAME` defaults to `default`. `bilterm ls` lists the same live daemons next to the saved sessions.

Client commands talk to one named daemon with `--daemon-name NAME` or `BILTERM_DAEMON=NAME`:
```bash
bilterm ctl --daemon-name alpha daemon.status     # -> {"name":"alpha", ... "is_current":true}
BILTERM_DAEMON=beta bilterm pane list
```
This applies to `bilterm ctl`, `agent`, `pane`, `workspace`, `coords` and `daemon`. Order of precedence: an explicit `ctl` ENDPOINT, `--daemon-name`, `BILTERM_DAEMON`, `BILTERM_SOCKET`, then the default instance. A named target that is not running is an error (exit 1); it is never auto-started or replaced by another daemon. A daemon that listens on TCP only cannot be targeted by name; pass its `IP:PORT` as the `ctl` ENDPOINT.

#### 3. Stale Lock & Dead Process Recovery
If a daemon crashes or is killed abruptly (`kill -9`), the kernel immediately releases POSIX `flock` locks. On the subsequent startup or query, Bilterm verifies PID liveness using `kill(pid, 0)`. Dead PIDs (`ESRCH`) trigger automatic stale-socket and stale-lock recovery, reclaiming resources without requiring manual file deletion.

#### 4. Colon Commands (`:daemon`)
- `:daemon` or `:daemon list` — Lists all running daemons, process IDs, session counts, pane counts, and agent statuses.
- `:daemon status [name]` — Inspects detailed telemetry of the current or named daemon.
- `:daemon start [name]` — Auto-spawns a background daemon if not already running.
- `:daemon stop [name] [--force]` — Gracefully terminates daemon PTY child processes, checkpoints session journals, and cleans up sockets and lockfiles.
- `:daemon switch <name>` — Switches the active client connection context to a named peer daemon.
- `:daemon prune` — Cleans up dead daemon lock and metadata entries.

#### 5. Cross-Daemon Discovery (`:find`)
All live peer daemons are indexed by the Unified Registry (`:find`). Searching `:find session` or `:find daemon:<query>` discovers active sessions and daemons running across the entire machine.

#### 6. Surface Uniformity & Keybindings
Every daemon operation is fully accessible across:
- **Leader Key Chords**: `Ctrl+X D L` (List), `Ctrl+X D S` (Status), `Ctrl+X D N` (Start), `Ctrl+X D Q` (Stop), `Ctrl+X D W` (Switch)
- **Top Menubar**: `Tools > Daemons > [List | Status | Start | Stop | Switch | Prune]`
- **Command Palette**: `:daemon list`, `:daemon status`, `:daemon start`, `:daemon stop`
- **JSON-RPC 2.0**: `daemon.list`, `daemon.status`, `daemon.start`, `daemon.stop`, `daemon.switch`
- **MCP Tool**: `bilterm_daemon` with `action="list" | "status" | "start" | "stop" | "switch"`
