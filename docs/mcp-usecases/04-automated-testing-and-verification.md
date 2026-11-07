# 04 — Automated Testing, Playbooks, PPM Snapshots & `.cast` Recording

Bilterm includes a built-in deterministic verification harness combining declarative TOML playbooks (`Playbook`), a software pixel framebuffer renderer (`GuiWorkspaceRenderer` -> `.ppm`), an Asciicast v2 recorder (`SessionRecorder` & `WorkspaceRecorder` -> `.cast`), and the Tektronix/Canvas engine (`bilterm_canvas`).

---

## 1. Declarative TOML Playbooks (`bilterm_run_playbook` / `bilterm --playbook`)

Defined in `src/api/playbook.rs`, a `Playbook` executes an ordered sequence of `[[steps]]` against an `AutomationControlPlane` and fails fast with a step-numbered diagnostic if any assertion fails.

### Supported Playbook Step Actions (`PlaybookStep.action`)

| `action` | Required / Optional Fields | Behavior |
|----------|----------------------------|----------|
| `"colon"` | `command = ":split v"` | Parses and applies any `:colon` command via `WindowManager::apply`. |
| `"exec"` | `command = "echo ok"`, `pane_id = 1` *(opt)* | Dispatches `pane.exec` (`CompositePane::submit_command`). |
| `"send_keys"` | `keys = ["Ctrl+C", "Enter"]` | Dispatches synthetic key chords via `pane.send_keys`. |
| `"feed_output"` | `data = "\u001b]133;A\u0007..."`, `pane_id = 1` *(opt)* | Feeds deterministic VT100/ANSI bytes directly into `pane.process_pty_output` and collects `AutomationEvent` items (ideal for hermetic tests without OS PTY timing jitter). |
| `"assert_screen_contains"` | `expected = "PASS"`, `pane_id = 1` *(opt)* | Verifies that at least one visible row of `pane_id` contains `expected`. |
| `"assert_trigger_fired"` | `pattern = "ERROR"` | Verifies that `AutomationEvent::TriggerMatched` fired for `pattern` (case-insensitive). |
| `"assert_exit_code"` | `exit_code = 0`, `pane_id = 1` *(opt)* | Verifies the latest OSC 133 `command_blocks` entry on `pane_id` has `Some(exit_code)`. |
| `"export_ppm"` | `path = "frame.ppm"` | Renders the entire workspace via `GuiWorkspaceRenderer` (8×16 glyph rasterizer) to a binary P6 `.ppm` image. |

### Example Playbook (`tests/e2e_smoke.toml`)

```toml
name = "multi_pane_agent_verification"

[[steps]]
action = "colon"
command = ":trigger add BUILD_OK"

[[steps]]
action = "colon"
command = ":split v"

[[steps]]
action = "feed_output"
pane_id = 2
data = "\u001b]133;A\u0007$ cargo test\r\nBUILD_OK: 42 passed\r\n\u001b]133;D;0\u0007"

[[steps]]
action = "assert_screen_contains"
pane_id = 2
expected = "BUILD_OK: 42 passed"

[[steps]]
action = "assert_trigger_fired"
pattern = "BUILD_OK"

[[steps]]
action = "assert_exit_code"
pane_id = 2
exit_code = 0

[[steps]]
action = "export_ppm"
path = "multi-pane-verified.ppm"
```

### Running a Playbook via CLI (`SnapshotPolicy::Trusted`)

```bash
bilterm --playbook tests/e2e_smoke.toml
# Or:
bilterm run tests/e2e_smoke.toml
# Output: {"playbook":"multi_pane_agent_verification","passed":true,"steps_completed":7}
```

### Running a Playbook via MCP (`bilterm_run_playbook`, `SnapshotPolicy::Confined`)

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
  "name":"bilterm_run_playbook",
  "arguments":{
    "toml": "name = \"mcp_smoke\"\n[[steps]]\naction = \"colon\"\ncommand = \":split h\"\n[[steps]]\naction = \"export_ppm\"\npath = \"mcp-smoke.ppm\"\n"
  }
}}
```

---

## 2. Visual Regression Testing with `.ppm` Framebuffer Snapshots

Bilterm's `GuiWorkspaceRenderer` renders composite panes, Borland double-line borders, drop shadows, themes, shaders, and Tektronix/Canvas graphics into a deterministic `P6` binary PPM image without needing an X11/Wayland display server.

### Security Confinement (`SnapshotPolicy::Confined` vs. `Trusted`)

- **Over MCP (`bilterm_run_playbook`) and JSON-RPC (`workspace.snapshot`, `playbook.run`)**: `SnapshotPolicy::Confined` is enforced (`src/api/rpc.rs`).
  - `ppm_path` / `path` **must be a plain filename** (e.g. `"frame-01.ppm"`). Any path containing `/`, `\`, `..`, or NUL is rejected (`validate_snapshot_name`).
  - Files are written with `0600` permissions inside the confined snapshot directory (`0700`):
    1. `--snapshot-dir <PATH>` if passed to `bilterm --mcp` or `bilterm --headless`,
    2. `$XDG_STATE_HOME/bilterm/snapshots`, or
    3. `$HOME/.local/state/bilterm/snapshots`.
- **Over CLI (`bilterm --gui-dump <PATH.ppm>` or `bilterm --playbook <FILE.toml>`)**: `SnapshotPolicy::Trusted` allows writing to any caller-specified path.

### Capturing a Live Workspace Snapshot via JSON-RPC

```bash
bilterm ctl workspace.snapshot '{"ppm_path":"before-refactor.ppm"}'
# Writes ~/.local/state/bilterm/snapshots/before-refactor.ppm (P6 header)
```

### One-Shot Initial Frame Dump via CLI

```bash
bilterm --theme 3278 --gui-dump /tmp/theme-3278.ppm
```

---

## 3. Asciicast v2 (`.cast`) Recording (`:record`)

Bilterm supports both **per-pane PTY stream recording** (`SessionRecorder`) and **whole-workspace multi-pane compositor recording** (`WorkspaceRecorder`, capturing split layouts, Z-order windows, and floating command prompts as Asciicast v2 `"o"` frames).

### Recording a Whole-Workspace Multi-Pane Session via MCP

```json
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":record workspace demo-run.cast"}
}}
```

Run your multi-pane workflow or playbook, then stop and flush the `.cast` file:

```json
{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":record stop"}
}}
```

### Recording Only the Active Pane's PTY Stream

```bash
bilterm ctl wm.command '{"command":":record pane pane-1.cast"}'
bilterm ctl pane.exec '{"pane_id":1,"command":"cargo test -q"}'
bilterm ctl wm.command '{"command":":record stop"}'
```

> **Confinement**: When `:record` is triggered over MCP/RPC (`apply_with_policy(..., SnapshotPolicy::Confined)`), the `.cast` filename is confined to the private snapshot directory just like `.ppm` exports.

---

## 4. Tektronix 4010/4014 & Retained Graphical Canvas (`bilterm_canvas`)

Agents can render vector plots, 2D sprites, and images directly inside a pane's retained graphical canvas using `bilterm_canvas` (`canvas.command` / `canvas.status`) or `bilterm plot`:

```json
{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{
  "name":"bilterm_canvas",
  "arguments":{"command":"plot candlestick 12,28,19,45,38,64,52,84"}
}}
```

Response payload:

```json
{
  "canvas_active": true,
  "effective_backend": "HalfBlock",
  "has_image": false,
  "plots": 1,
  "primitives": 0,
  "render_backend": "Auto",
  "sprite_collisions": 0,
  "sprites": 0,
  "subcell_mode": "Braille",
  "tektronix_active": false,
  "tektronix_decaying_segments": 0,
  "tektronix_segments": 0
}
```

CLI plot export to `.ppm` or `.svg`:

```bash
bilterm plot candlestick 12,28,19,45,38,64,52,84 -o /tmp/chart.svg
```
