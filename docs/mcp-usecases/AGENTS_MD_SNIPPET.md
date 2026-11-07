# `AGENTS.md` / `CLAUDE.md` Drop-In Snippet for Bilterm Automation

Copy the block below into your project's `AGENTS.md`, `CLAUDE.md`, or `.cursorrules` to give AI coding agents zero-verbosity mastery over Bilterm's MCP tools and `bilterm ctl` control plane.

```markdown
## Bilterm Terminal Multiplexer & MCP Automation Rules

When operating inside or orchestrating a Bilterm workspace, use Bilterm's structured MCP tools (`bilterm --mcp`) or CLI control plane (`bilterm ctl`) instead of raw screen scraping or blind `sleep` loops.

### 1. Discovery & Fast State Inspection
- **MCP Resources (Zero-Side-Effect Reads)**:
  - Read `bilterm://state` for workspace topology (`layout_mode`, `active_pane_id`, `pane_count`, `appearance_profile`, `display_mode`, `drawer`).
  - Read `bilterm://pane/<id>` (or call `bilterm_inspect_pane` with `{"pane_id": <id>}`) to get `lines`, `cursor_row`, `cursor_col`, `alternate_screen`, `echo_lock`, OSC 133 `command_blocks` (`command`, `exit_code`, `duration_ms`), and 3270 `hints` (URLs, file paths, IPs, SHAs).
  - Read `bilterm://commands` to list all supported `:colon` commands, aliases, sub-arguments, and examples.
- **CLI Equivalent (`bilterm ctl`)**:
  - Default Unix socket (`$XDG_RUNTIME_DIR/bilterm/bilterm.sock`): `bilterm ctl workspace.status` or `bilterm ctl pane.read '{"pane_id":1}'`
  - Authenticated TCP endpoint: `bilterm ctl -t <TOKEN_FILE_0600> 127.0.0.1:7370 workspace.status`

### 2. Command Execution vs. Raw Keystrokes
- **Shell Commands (`bilterm_exec_command` / `pane.exec`)**:
  - Use `bilterm_exec_command` (`{"pane_id": 1, "command": "cargo test -q"}`) to submit shell commands through Bilterm's command pipeline.
  - **Safety Gate**: `bilterm_exec_command` automatically refuses execution if the target pane is in `!ECHO LOCK` (`echo_lock: true`, e.g., `sudo` or `ssh` password prompt) or read-only mode (`read_only: true`, `:lock on`). Never bypass `!ECHO LOCK` with plaintext secrets in agent logs.
- **Interactive TUIs (`bilterm_send_keys` / `pane.send_keys`)**:
  - First check `alternate_screen` (`DECSET 1049`) via `bilterm_inspect_pane`. When `alternate_screen: true` (`vim`, `htop`, `less`, `git rebase -i`) or when sending control chords (`Ctrl+C`, `Esc`, `F1..F12`, `PageUp`, `Up`), use `bilterm_send_keys` (`{"keys": "Esc : w q Enter"}`).
- **Completion Verification (No Blind Sleeps)**:
  - Inspect `command_blocks` in `bilterm_inspect_pane` for the latest OSC 133 entry's `exit_code` and `duration_ms`, or poll `events.poll` (`{"drain": true}`) via `bilterm ctl` for `command_completed` and `trigger_matched` events.

### 3. Multi-Pane Orchestration (`bilterm_colon_command` / `wm.command`)
Every workspace operation runs through `bilterm_colon_command` (`{"command": ":<cmd>"}`):
- **Split & Focus**: `:split v` (vertical split), `:split h` (horizontal split), `:focus <pane_id|left|right|up|down|next|prev>`, `:zoom` (toggle maximize), `:close`.
- **Layout & Stacking**: `:layout tiled`, `:layout stack` (Z-order overlapping windows), `:layout arrange <grid|columns|rows|main-side|main-top>`, `:deck next`, `:deck prev`, `:quake` (toggle drop-down terminal).
- **Fan-Out Execution**: Use `bilterm_fleet_exec` (`{"selector": "all|local|remote|@tag|<node>", "command": "..."}`) or toggle `:broadcast on` / `:broadcast off`.
- **Triggers & Alerts**: Register output watch patterns via `:trigger add <PATTERN>` (e.g., `:trigger add ERROR`) and clear with `:trigger clear`.

### 4. Security & Confinement Invariants
- Never pass bearer tokens on CLI argv; always use `-t / --token-file <PATH>` (`0600` permissions) or `BILTERM_TOKEN_FILE`.
- Snapshot exports over MCP/RPC (`workspace.snapshot` `ppm_path` or playbook `export_ppm`) are strictly confined to a plain filename inside the snapshot directory (`$XDG_STATE_HOME/bilterm/snapshots/` or `--snapshot-dir`). Path traversal (`..`, `/`) is rejected.
- Runtime config mutations (`:set`, `:config save`, `:appearance save`) over MCP/RPC are blocked unless `[api] allow_config_write = true` is set locally in `config.toml`.
```
