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
This applies to `bilterm ctl`, `agent`, `pane`, `workspace`, `coords` and `daemon`. `--daemon-name` (and `-t/--token-file`) may come before or after the subcommand (`bilterm --daemon-name alpha pane read p1`); any other option placed before a client subcommand is rejected (exit 2) instead of launching the GUI/TUI. Order of precedence: an explicit `ctl` ENDPOINT, `--daemon-name`, `BILTERM_DAEMON`, `BILTERM_SOCKET`, then the default instance. A named target that is not running is an error (exit 1); it is never auto-started or replaced by another daemon. A daemon that listens on TCP only cannot be targeted by name; pass its `IP:PORT` as the `ctl` ENDPOINT.

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

## Agent instructions: `bilterm agents-md` and `:agent guide` (`AGUIDE-01`)

Bilterm ships its own instructions for AI coding agents: how to inspect panes, run commands and wait for them, target a named daemon, prompt and message other agents, and which writes are refused. The text is one file, [`docs/agent-guide.md`](agent-guide.md), built into the binary, so every surface returns the same words.

| Where | How |
| :--- | :--- |
| Shell | `bilterm agents-md` prints the guide (no daemon needed). |
| Project file | `bilterm agents-md --install [PATH]` adds the guide to `./AGENTS.md` (or `PATH`, for example `CLAUDE.md`) between `<!-- BILTERM:AGENTS-MD:BEGIN ... -->` and `<!-- BILTERM:AGENTS-MD:END -->`. Run it again after an upgrade to refresh the block. Text outside the markers is never changed, a second run changes nothing, and a file with broken markers is left alone with an error. |
| MCP | `initialize` returns short `instructions`; the resource `bilterm://agents-md` returns the full guide; `bilterm_agent_control` with `action: "guide"`. |
| RPC / HTTP / `bilterm ctl` | `agent.guide` (`op`: `get`, `inject`, `print`). |
| Running agent pane | `:agent guide` (see below). |

### Handing the guide to an agent pane

```text
:agent guide            focused pane
:agent guide 3          pane #3
:agent guide all        every detected agent pane
:agent guide --print    only write the guide file and show its path
```

- Keys: `Ctrl+Alt+G`, or `Ctrl+X ^` on hosts that swallow `Ctrl+Alt` chords. `Ctrl+Alt+G` also works while an agent's full-screen program owns the pane.
- Menu: **Help > Inject Agent Guide**. Palette: `:agent guide`. Shell: `bilterm agent guide SELECTOR` or `bilterm agent guide all`.
- What reaches the pane is one line, not the guide. Bilterm writes the guide to `agent-guide.md` (mode `0600`) in its private runtime directory and sends `Read <path> for bilterm control instructions ...`. An agent program receives the line as a prompt (pasted, then Enter), the same way `bilterm agent prompt` delivers text. A pane at a shell prompt receives it as a `#` comment line, so nothing is executed.
- Refused, with a status message and nothing written to the pane: a read-only pane (`:lock on`), a password prompt (`!ECHO LOCK`), and a shell pane that is busy running a command. With `all`, refused panes are listed in the result and the others still get the line.
