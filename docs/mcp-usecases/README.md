# Bilterm MCP & Automation Control Plane — Agent Use-Case Guide

Bilterm exposes 100% of its `WindowManager`, `CompositePane`, `VtParser`, and `:colon` command capabilities through a unified, deterministic 3-tier automation architecture. AI coding agents (Claude Code, Google Antigravity, Cursor, Codex CLI, and custom MCP/JSON-RPC clients) can spawn, inspect, orchestrate, and verify multi-pane terminal workflows without screen-scraping fragility.

---

## Index of Guides

| Document | Scope |
|----------|-------|
| [`../agent-guide.md`](../agent-guide.md) | The agent control guide Bilterm serves itself: `bilterm agents-md` prints it, `bilterm agents-md --install` adds it to your `AGENTS.md` / `CLAUDE.md`, MCP serves it as `bilterm://agents-md`, and `:agent guide` hands it to a running agent pane |
| [`AGENTS_MD_SNIPPET.md`](AGENTS_MD_SNIPPET.md) | How to install that guide into a project (replaces the old copy-paste block) |
| [`01-agent-orchestration-and-herding.md`](01-agent-orchestration-and-herding.md) | Multi-agent "herder" patterns: spawning worker panes, fan-out (`bilterm_fleet_exec` / `:broadcast`), OSC 133 completion tracking, and Z-order management |
| [`02-operating-interactive-tuis-and-shells.md`](02-operating-interactive-tuis-and-shells.md) | Driving `vim`, `htop`, `git rebase -i`, `gdb`, `ssh`, and `sudo` prompts (`!ECHO LOCK`, `DECSET 1049`, `bilterm_send_keys`, Vi copy mode, and 3270 field hints) |
| [`03-configuration-themes-and-layouts.md`](03-configuration-themes-and-layouts.md) | Live workspace introspection and runtime styling: `[appearance]` profiles, 14 themes, shaders, fonts, layout profiles, and the write-gate security model |
| [`04-automated-testing-and-verification.md`](04-automated-testing-and-verification.md) | Deterministic E2E testing: declarative TOML playbooks (`bilterm --playbook`), confined `.ppm` visual snapshots, and Asciicast v2 (`.cast`) recordings |

---

## Architecture Overview: 3-Tier Automation Surface

All external automation transports converge on `AutomationControlPlane::dispatch_method` (`src/api/rpc.rs`) and lower workspace mutations into typed `crate::action::Action` variants executed by `WindowManager::apply(Action)` (`src/action.rs`).

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Tier 1: Agent & CLI Transports                                             │
│                                                                            │
│  • MCP Stdio Server:     bilterm --mcp [-s SESSION] [-S SNAPSHOT_DIR]      │
│  • JSON-RPC Daemon:      bilterm --headless [-l unix:PATH | IP:PORT]       │
│  • Loopback HTTP + SSE:  bilterm --headless --http 127.0.0.1:PORT          │
│  • CLI One-Shot Client:  bilterm ctl [-t TOKEN_FILE] [ENDPOINT] <METHOD>   │
│  • Playbook Runner:      bilterm --playbook <FILE.toml>                    │
└─────────────────────────────────────┬──────────────────────────────────────┘
                                      │ JSON-RPC 2.0
                                      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ Tier 2: AutomationControlPlane::dispatch_method (20 RPC_METHODS)           │
│                                                                            │
│  workspace.{status,snapshot,fontsize} | pane.{exec,read,feed_output,       │
│  send_keys} | wm.command | events.poll | playbook.run |                    │
│  remote.{connect_ssh,connect_tmux} | fleet.{register,list,exec,disconnect} │
│  canvas.{command,status} | session.{save,restore}                          │
└─────────────────────────────────────┬──────────────────────────────────────┘
                                      │ parse_colon_command(":...")
                                      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ Tier 3: Unified Action Dispatch — WindowManager::apply(Action)             │
│                                                                            │
│  • 100% parity across Keybindings, Command Palette, :colon, RPC, and MCP   │
│  • SnapshotPolicy::Confined enforces 0700/0600 path confinement            │
│  • [api] allow_config_write = false (default) blocks unauthorized writes   │
└────────────────────────────────────────────────────────────────────────────┘
```

---

## MCP Client Configuration Snippets

### 1. Claude Code / Project `.mcp.json`

Place at the repository root (`.mcp.json`):

```json
{
  "mcpServers": {
    "bilterm": {
      "command": "bilterm",
      "args": ["--mcp", "--session", "agent-workspace"]
    }
  }
}
```

### 2. Claude Desktop (`claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "bilterm": {
      "command": "/usr/local/bin/bilterm",
      "args": [
        "--mcp",
        "--session",
        "main",
        "--snapshot-dir",
        "/tmp/bilterm-snapshots"
      ]
    }
  }
}
```

### 3. Google Antigravity / Cursor (`mcp.json`)

```json
{
  "mcpServers": {
    "bilterm": {
      "command": "bilterm",
      "args": ["-m", "-s", "agy-dev"],
      "env": {
        "RUST_LOG": "bilterm=warn"
      }
    }
  }
}
```

### 4. Headless Daemon + Authenticated TCP / Unix Socket (`bilterm ctl`)

When sharing a live headless workspace across multiple processes or over an SSH tunnel, run `bilterm --headless` with a `0600` bearer token file (secrets are never passed on CLI argv per `SEC-R01..04`):

```bash
# 1. Generate a 0600 bearer token file
umask 077 && head -c 32 /dev/urandom | xxd -p -c 64 > ~/.local/state/bilterm/rpc.token

# 2. Start headless daemon on default Unix socket ($XDG_RUNTIME_DIR/bilterm/bilterm.sock)
bilterm --headless --session fleet-hub

# Or bind loopback TCP + loopback HTTP/SSE with bearer token enforcement:
bilterm --headless \
  --listen 127.0.0.1:7370 \
  --http 127.0.0.1:7371 \
  --token-file ~/.local/state/bilterm/rpc.token

# 3. Query from any agent or script via bilterm ctl (endpoint is optional for default Unix socket)
bilterm ctl workspace.status
bilterm ctl -t ~/.local/state/bilterm/rpc.token 127.0.0.1:7370 workspace.status
bilterm ctl -t ~/.local/state/bilterm/rpc.token 127.0.0.1:7370 wm.command '{"command":":split v"}'
```

---

## Complete MCP Tool Reference (`tools/list` / `tools/call`)

Bilterm's MCP server (`protocolVersion: "2024-11-05"`, server name `"bilterm-mcp"`) exposes **10 tools** in `src/api/mcp.rs`. Every tool returns a JSON-serialized payload inside `result.content[0].text`.

| MCP Tool Name | Mapped RPC Method | Arguments (`inputSchema.properties`) | Purpose |
|---------------|-------------------|--------------------------------------|---------|
| `bilterm_workspace_status` | `workspace.status` | *(none)* | Returns `layout_mode` (`"tiled"`\|`"stacked"`), `active_pane_id`, `pane_count`, `broadcast_input`, `menu_bar_visible`, `crt_scanlines`, `palette_visible`, and per-pane telemetry (`id`, `title`, `rows`, `cols`, `cursor_row`, `cursor_col`, `input_target`, `alternate_screen`, `echo_lock`, `read_only`, `recording`, `notifications`). |
| `bilterm_exec_command` | `pane.exec` | `command` (`string`, required): Shell command to execute.<br>`pane_id` (`integer`, optional): Target pane ID (defaults to active pane).<br>*(RPC also accepts `allow_echo_lock`: `boolean`)*. | Submits a command through `CompositePane::submit_command` with `!ECHO LOCK` password-prompt safety and read-only (`🔒 PROT`) enforcement. |
| `bilterm_inspect_pane` | `pane.read` | `pane_id` (`integer`, optional): Target pane ID (defaults to active pane). | Reads visible `lines` (`string[]`), cursor coordinates, `alternate_screen`, `echo_lock`, `read_only`, `recording`, `hints` (detected URLs, paths, IPs, git SHAs), OSC 133 `command_blocks` (`command`, `row`, `exit_code`, `duration_ms`), and `notifications`. |
| `bilterm_send_keys` | `pane.send_keys` | `keys` (`string`, required): Space-separated key chords or single key chord (e.g. `"Ctrl+C"`, `"Esc : w q Enter"`, `"F12"`, `"Alt+H"`). *(RPC also accepts `string[]`)*. | Dispatches synthetic `KeyEvent` chords through `handle_wm_key_event` with `api_origin = true` to drive interactive TUIs (`vim`, `htop`, `less`) or multiplexer shortcuts. |
| `bilterm_colon_command` | `wm.command` | `command` (`string`, required): Any Bilterm `:colon` command (leading `:` is optional, e.g. `":split v"`, `":layout stack"`, `":focus 2"`, `":state"`, `":get theme"`). | Parses and executes any of Bilterm's `Action` variants under `SnapshotPolicy::Confined`. Returns `status`, `command`, `active_pane_id`, `pane_count`, and optional structured `data`. |
| `bilterm_run_playbook` | `playbook.run` | `toml` (`string`, required): Declarative playbook TOML string (`name`, `[[steps]]`). | Executes a multi-step TOML playbook (`colon`, `exec`, `send_keys`, `feed_output`, `assert_screen_contains`, `assert_trigger_fired`, `assert_exit_code`, `export_ppm`) under `SnapshotPolicy::Confined`. |
| `bilterm_remote_connect` | `remote.connect_ssh` | `target` (`string`, required): SSH destination (`user@host`) or remote profile name.<br>`tmux_session` (`string`, optional): Remote `tmux -CC` session name to attach. | Spawns a remote SSH or remote `tmux -CC` `CompositePane` and registers it in the fleet registry. |
| `bilterm_fleet_exec` | `fleet.exec` | `selector` (`string`, required): Target selector (`all`, `local`, `remote`, `@tag`, or node name).<br>`command` (`string`, required): Shell command to fan out. | Fans out a shell command across all matching local and remote fleet panes and emits `CommandSubmitted` events. |
| `bilterm_workspace_fontsize` | `workspace.fontsize` | `action` (`string`, optional): `"in"`, `"out"`, `"reset"`, or size/percentage string.<br>`size` (`integer`, optional): Font point size (`6..49`) or scale percentage (`50..300`). | Queries or dynamically adjusts workspace/pane typography and returns pixel cell dimensions (`cell_width_px`, `cell_height_px`, `font_size_pt`, `font_scale_pct`). |
| `bilterm_canvas` | `canvas.command` | `command` (`string`, optional): Canvas command (e.g. `"tek demo"`, `"line 0 0 20 10"`, `"sprite 1 4 4 2 0"`, `"plot candlestick"`, `"image 24 16"`, `"clear"`). | Controls the Tektronix 4010/4014 vector engine and retained graphical canvas, returning segment/primitive/sprite/plot counts and collision telemetry. |

---

## MCP Read-Only Resources (`resources/list` / `resources/read`)

Defined in `src/api/mcp_resources.rs`, all MCP resources return `mimeType: "application/json"`:

| Resource URI | Name | Payload Summary |
|--------------|------|-----------------|
| `bilterm://state` | Workspace state | Combines `workspace` (`workspace.status`), `appearance_profile` (`"legacy"`\|`"obsidian"`\|`"midnight"`\|`"arctic"`\|`"liquid"`), `display_mode` (`"standard"`\|`"minimal"`\|`"zen"`), and `drawer` (`open`, `visible`, `edge`, `width_cols`, `height_rows`). |
| `bilterm://config` | Effective config | Read-only non-secret config snapshot: `theme`, `snazzy_mode`, `glass_mode`, `modern_mode`, `liquid_glass`, `crt_scanlines`, `show_top_menubar`, `show_dos_fkey_bar`, `copy_on_select`, `history_enabled`, `layout_autosave`, `leader_key`, `default_prompt_anchor`, `default_prompt_height`, `active_layout_profile`, `appearance_profile`, `display_mode`. |
| `bilterm://commands` | Colon command catalog | Full catalog from `src/command_registry.rs::catalog()`: `count` and `commands[]` (`name`, `summary`, `alias_of`, `subargs`, `examples`, `shortcut`). |
| `bilterm://pane/<id>` | Pane `#<id>` | Identical to `pane.read` / `bilterm_inspect_pane` for pane `<id>`: `lines`, `cursor_row`, `cursor_col`, `alternate_screen`, `echo_lock`, `command_blocks`, `hints`, `notifications`. |

---

## Complete JSON-RPC 2.0 Method Catalog (`RPC_METHODS`)

All 20 methods in `bilterm::api::RPC_METHODS` (`src/api/rpc.rs`) are available over Unix socket, TCP, and HTTP `POST /rpc`:

| Category | Method | Parameters |
|----------|--------|------------|
| **Workspace** | `workspace.status` | `{}` |
| | `workspace.snapshot` | `{"ppm_path": "frame.ppm"}` *(optional `ppm_path`; confined to snapshot dir)* |
| | `workspace.fontsize` | `{"action": "in"\|"out"\|"reset", "size": 16, "scale_pct": 125, "command": "ibm3270 16pt"}` *(all optional)* |
| **Pane I/O** | `pane.exec` | `{"command": "...", "pane_id": 1, "allow_echo_lock": false}` |
| | `pane.read` | `{"pane_id": 1}` *(optional `pane_id`)* |
| | `pane.feed_output` | `{"data": "raw ANSI/VT bytes", "pane_id": 1}` |
| | `pane.send_keys` | `{"keys": "Ctrl+C Enter"}` or `{"keys": ["Ctrl+C", "Enter"]}` |
| **Multiplexer** | `wm.command` | `{"command": ":split v"}` *(any `:colon` command)* |
| **Events & Playbooks** | `events.poll` | `{"drain": true}` *(drains up to 512 queued `AutomationEvent` items)* |
| | `playbook.run` | `{"toml": "name = ...\n[[steps]]\n..."}` |
| **Remote & Fleet** | `remote.connect_ssh` | `{"target": "user@host", "port": 22, "tmux_session": "main", "tags": ["prod"]}` |
| | `remote.connect_tmux` | `{"target": "user@host", "session": "bilterm", "tags": ["prod"]}` |
| | `fleet.register` | `{"name": "web1", "destination": "user@host", "tags": ["prod"], "rpc_endpoint": "127.0.0.1:7370"}` |
| | `fleet.list` | `{}` |
| | `fleet.exec` | `{"selector": "all"\|"local"\|"remote"\|"@tag"\|"node", "command": "uptime"}` |
| | `fleet.disconnect` | `{"name": "web1"}` |
| **Canvas** | `canvas.command` | `{"command": "plot line 10,20,30"}` |
| | `canvas.status` | `{}` |
| **Sessions** | `session.save` | `{"name": "default"}` |
| | `session.restore` | `{"name": "default"}` |

### Error Code Contract

- **JSON-RPC 2.0**: `-32700` (Parse error), `-32600` (Invalid Request), `-32001` (`RPC_AUTH_ERROR_CODE`: authentication required/failed), `-32000` (Dispatch/execution error).
- **MCP (`bilterm --mcp`)**: `-32601` (Unknown method), `-32602` (Invalid params / unknown tool / missing required argument). Tool execution errors return `Ok` with `"isError": true` in the MCP result object.
