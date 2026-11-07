# 01 — Agent Orchestration and Multi-Pane Herding

An orchestrator ("herder") AI agent can use Bilterm's MCP server (`bilterm --mcp`) or JSON-RPC daemon (`bilterm --headless`) to spawn multiple worker panes, launch subagents or build/test tasks (`claude`, `agy`, `codex`, `cargo watch`, remote `ssh` workers), monitor their progress via OSC 133 command blocks and output triggers, and coordinate results.

---

## 1. Spawning a Multi-Pane Worker Grid

Use `bilterm_colon_command` (`wm.command`) to partition the workspace into composite panes, then verify the topology with `bilterm_workspace_status` (`workspace.status`).

### Step 1: Create a 4-Pane Workspace & Arrange in a Grid

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":split v"}
}}
```
```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":split h"}
}}
```
```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":focus 1"}
}}
```
```json
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":split h"}
}}
```

Or switch directly to a built-in 4-pane layout profile in a single call:

```json
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":layout quad"}
}}
```

### Step 2: Inspect Pane IDs and Geometry

```json
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{
  "name":"bilterm_workspace_status",
  "arguments":{}
}}
```

Response payload (`result.content[0].text`):

```json
{
  "active_pane_id": 4,
  "broadcast_input": false,
  "crt_scanlines": false,
  "layout_mode": "tiled",
  "menu_bar_visible": true,
  "palette_visible": false,
  "pane_count": 4,
  "panes": [
    {"id": 1, "title": "Pane #1", "rows": 12, "cols": 40, "cursor_row": 0, "cursor_col": 0, "input_target": "cmd_box", "alternate_screen": false, "echo_lock": false, "read_only": false, "recording": false, "notifications": 0},
    {"id": 2, "title": "Pane #2", "rows": 12, "cols": 40, "cursor_row": 0, "cursor_col": 0, "input_target": "cmd_box", "alternate_screen": false, "echo_lock": false, "read_only": false, "recording": false, "notifications": 0},
    {"id": 3, "title": "Pane #3", "rows": 12, "cols": 40, "cursor_row": 0, "cursor_col": 0, "input_target": "cmd_box", "alternate_screen": false, "echo_lock": false, "read_only": false, "recording": false, "notifications": 0},
    {"id": 4, "title": "Pane #4", "rows": 12, "cols": 40, "cursor_row": 0, "cursor_col": 0, "input_target": "cmd_box", "alternate_screen": false, "echo_lock": false, "read_only": false, "recording": false, "notifications": 0}
  ]
}
```

---

## 2. Dispatching Subagents and Tasks to Specific Panes

`bilterm_exec_command` (`pane.exec`) accepts an explicit `pane_id` so the herder agent never needs to change workspace focus just to send commands to background workers:

```json
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{
  "name":"bilterm_exec_command",
  "arguments":{
    "pane_id": 1,
    "command": "claude -p 'Implement parser unit tests in tests/parser_spec.rs'"
  }
}}
```

```json
{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{
  "name":"bilterm_exec_command",
  "arguments":{
    "pane_id": 2,
    "command": "codex --quiet 'Audit src/api/ for input validation edge cases'"
  }
}}
```

```json
{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{
  "name":"bilterm_exec_command",
  "arguments":{
    "pane_id": 3,
    "command": "./scripts/verify.sh --quick"
  }
}}
```

### Locking Worker Panes Against Accidental Human Input (`:lock`)

To prevent accidental keystrokes in a pane running an unattended agent, focus the pane and engage read-only protection (`🔒 PROT`):

```bash
bilterm ctl wm.command '{"command":":focus 1"}'
bilterm ctl wm.command '{"command":":lock on"}'
```

When `read_only: true`, any subsequent `pane.exec` call to that pane returns `Err("pane #1 is locked read-only (🔒 PROT)")` until `:lock off` is issued.

---

## 3. Waiting for Completion & Output Triggers (No Blind Polling)

Bilterm provides three deterministic mechanisms to detect when a worker pane finishes a command, prints an error, or awaits confirmation:

### Mechanism A: OSC 133 Command Blocks (`bilterm_inspect_pane` / `pane.read`)

When a shell emits OSC 133 semantic prompt markers (`\x1b]133;A\x07` .. `\x1b]133;D;<exit_code>\x07`) or commands are submitted through `CompositePane::submit_command`, Bilterm records structured `command_blocks`:

```json
{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{
  "name":"bilterm_inspect_pane",
  "arguments":{"pane_id": 3}
}}
```

Response excerpt:

```json
{
  "pane_id": 3,
  "rows": 12,
  "cols": 40,
  "alternate_screen": false,
  "echo_lock": false,
  "lines": [
    "$ ./scripts/verify.sh --quick",
    "PASS 2/2 4s",
    ""
  ],
  "command_blocks": [
    {
      "command": "./scripts/verify.sh --quick",
      "row": 0,
      "exit_code": 0,
      "duration_ms": 4120
    }
  ],
  "hints": [],
  "notifications": []
}
```

### Mechanism B: Output Triggers + Event Bus (`:trigger add` + `events.poll`)

Register case-insensitive substring triggers across all panes. Whenever PTY output matches a trigger, Bilterm pushes `AutomationEvent::TriggerMatched` into the 512-event ring buffer:

```json
{"jsonrpc":"2.0","id":21,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":trigger add FAIL"}
}}
```
```json
{"jsonrpc":"2.0","id":22,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":trigger add Allow tool?"}
}}
```

Drain structured events via `bilterm ctl events.poll` or HTTP SSE (`GET /events`):

```bash
bilterm ctl events.poll '{"drain":true}'
```

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "events": [
      {
        "type": "command_submitted",
        "pane_id": 3,
        "command": "./scripts/verify.sh --quick"
      },
      {
        "type": "command_completed",
        "pane_id": 3,
        "command": "./scripts/verify.sh --quick",
        "exit_code": 0,
        "duration_ms": 4120
      },
      {
        "type": "trigger_matched",
        "pane_id": 1,
        "pattern": "Allow tool?",
        "row": 8,
        "line": "Allow tool? [y/n]: "
      }
    ]
  }
}
```

### Mechanism C: Unblocking a Worker Prompt (`bilterm_send_keys`)

When `trigger_matched` reveals a worker subagent waiting at an interactive confirmation prompt (`Allow tool? [y/n]:`), focus the pane and send the confirmation keystroke:

```json
{"jsonrpc":"2.0","id":23,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":focus 1"}
}}
```
```json
{"jsonrpc":"2.0","id":24,"method":"tools/call","params":{
  "name":"bilterm_send_keys",
  "arguments":{"keys":"y Enter"}
}}
```

---

## 4. Broadcast Mode & Fleet Fan-Out (`bilterm_fleet_exec`, `bilterm_remote_connect`)

### Local Broadcast Mode (`:broadcast`)

Toggle workspace-wide input broadcast so keystrokes or commands apply across panes simultaneously:

```json
{"jsonrpc":"2.0","id":30,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":broadcast on"}
}}
```

### Remote SSH & `tmux -CC` Fleet Management

Register and spawn remote SSH or `tmux -CC` worker panes and fan out commands using herdr-style selectors (`all`, `local`, `remote`, `@tag`, or node name):

```json
{"jsonrpc":"2.0","id":31,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":remote add gpu-worker-1 ubuntu@10.0.0.41 @gpu,@linux"}
}}
```

```json
{"jsonrpc":"2.0","id":32,"method":"tools/call","params":{
  "name":"bilterm_remote_connect",
  "arguments":{
    "target": "ubuntu@10.0.0.41",
    "tmux_session": "agent-worker"
  }
}}
```

Fan out a synchronization command across all `@gpu`-tagged nodes and local panes:

```json
{"jsonrpc":"2.0","id":33,"method":"tools/call","params":{
  "name":"bilterm_fleet_exec",
  "arguments":{
    "selector": "@gpu",
    "command": "git pull --ff-only && cargo test -q"
  }
}}
```

---

## 5. Z-Order Stacking, Deck Shuffling & Session Trees (`:layout stack`, `:mcp`)

When orchestrating 6+ worker panes, tiled splits become narrow. Switch the workspace into **Stacked (Z-order cascade)** mode so every worker pane retains a generous terminal grid while the herder cycles or raises panes on demand:

```bash
# Switch to overlapping Z-order stacked windows
bilterm ctl wm.command '{"command":":layout stack"}'

# Shuffle top-to-bottom deck order or raise a specific worker pane to the top of Z-order
bilterm ctl wm.command '{"command":":deck next"}'
bilterm ctl wm.command '{"command":":focus 3"}'

# Temporarily maximize the focused pane to full viewport (toggle)
bilterm ctl wm.command '{"command":":zoom"}'
```

### Master Control Program (`:mcp`) & Hierarchical Session Trees

Bilterm includes built-in session-tree and MCP supervisor commands (`Action::McpCommand` / `Action::SessionTreeCommand`) accessible via `bilterm_colon_command`:

```bash
# Inspect hierarchical session tree & MCP supervisor status
bilterm ctl wm.command '{"command":":mcp status"}'
bilterm ctl wm.command '{"command":":session tree"}'

# Branch a child session, freeze/thaw output, or checkpoint state
bilterm ctl wm.command '{"command":":session branch worker-experiment"}'
bilterm ctl wm.command '{"command":":mcp freeze"}'
bilterm ctl wm.command '{"command":":mcp thaw"}'
bilterm ctl wm.command '{"command":":session checkpoint pre-migration"}'
```
