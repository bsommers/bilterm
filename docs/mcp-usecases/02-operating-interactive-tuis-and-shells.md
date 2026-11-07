# 02 — Operating Interactive TUIs, Shells, and Password Prompts

Bilterm's `CompositePane` combines an IBM 3270-style **Floating Command Prompt** (`InputRoutingMode::CommandPrompt`, `"cmd_box"`) with a full VT100/ANSI **Output Canvas** (`InputRoutingMode::RawPtyPassthrough`, `"raw_pty"`). AI agents must understand how Bilterm transitions between these two input modes when operating full-screen TUIs (`vim`, `htop`, `git rebase -i`, `less`), debuggers (`gdb`, `lldb`), and password prompts (`sudo`, `ssh`).

---

## 1. Input Routing Modes & Automatic Detection (`cmd_box` vs. `raw_pty`)

Every `bilterm_workspace_status` (`workspace.status`) and `bilterm_inspect_pane` (`pane.read`) response reports three state flags per pane:

| Field | Values | Meaning |
|-------|--------|---------|
| `input_target` | `"cmd_box"` \| `"raw_pty"` | Effective destination of keystrokes (`InputRoutingMode`). |
| `alternate_screen` | `true` \| `false` | Whether the child process has entered `DECSET 1049` (`\x1b[?1049h`) alternate screen (`vim`, `htop`, `less`, `tmux`). When `true`, Bilterm automatically hides the floating command prompt and routes keys to `"raw_pty"`. |
| `echo_lock` | `true` \| `false` | 3-signal `!ECHO LOCK` detector (`termios !ECHO` + canvas password-prompt heuristic + active command state). Protects `sudo`, `ssh`, and `gpg` passphrase prompts. |

### Checking Routing State Before Sending Input

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
  "name":"bilterm_inspect_pane",
  "arguments":{"pane_id": 1}
}}
```

- When `alternate_screen: false` and `echo_lock: false`: Use `bilterm_exec_command` (`pane.exec`) to submit complete shell commands cleanly through `CompositePane::submit_command`.
- When `alternate_screen: true`: Use `bilterm_send_keys` (`pane.send_keys`) to send synthetic keystrokes (`Esc`, `:w`, `Enter`, `Up`, `Down`, `Ctrl+C`, `F1..F12`) directly to the interactive TUI.

### Toggling Prompt Mode Explicitly (`:mode`, `:raw`)

If an interactive CLI (`python -i`, `gdb`, `sqlite3`, `node`) runs in the primary screen buffer without entering `DECSET 1049`, you can switch the pane between Floating Prompt, Fixed Docked Prompt, and Regular Terminal (Raw Passthrough) modes via `bilterm_colon_command`:

```bash
# Toggle raw PTY passthrough on the active pane
bilterm ctl wm.command '{"command":":raw"}'

# Or set explicit prompt mode: float | fixed | terminal | cycle
bilterm ctl wm.command '{"command":":mode terminal"}'
bilterm ctl wm.command '{"command":":mode float"}'
```

> **Asciinema / `script` Wrapper Note**: Outer PTY wrappers like `asciinema rec` disable `ECHO` on the outer pseudo-terminal even at a normal shell prompt. In interactive sessions, `Ctrl+X Ctrl+E` or `F4` (`:mode float`) sets `force_command_prompt = true` to restore the floating prompt while preserving automatic `!ECHO LOCK` fallback whenever a real `[sudo] password for ...:` prompt appears on the canvas.

---

## 2. Driving Full-Screen TUIs (`vim`, `git rebase -i`, `htop`)

### Supported Key Chord Syntax (`parse_key_chord_str` in `src/api/mcp.rs`)

`bilterm_send_keys` accepts space-separated chord tokens (and `pane.send_keys` over JSON-RPC accepts either a space-separated string or a JSON array of strings):

- **Modifiers**: `Ctrl+`, `Control+`, `Alt+`, `Meta+`, `Shift+` (combinable, e.g. `Ctrl+Shift+V`, `Alt+Shift+O`).
- **Named Keys**: `Enter` (`Return`), `Esc` (`Escape`), `Tab`, `BackTab`, `Backspace`, `Space`, `Up`, `Down`, `Left`, `Right`, `PageUp`, `PageDown`, `Home`, `End`, `F1` .. `F12`.
- **Single Characters**: Any single Unicode character (`a`, `i`, `:`, `/`, `1`, `%`, `"`).

### Example A: Editing a File in `vim` / `git rebase -i`

1. Launch `vim` via `bilterm_exec_command`:
   ```json
   {"jsonrpc":"2.0","id":10,"method":"tools/call","params":{
     "name":"bilterm_exec_command",
     "arguments":{"pane_id":1,"command":"vim Cargo.toml"}
   }}
   ```
2. Verify `alternate_screen: true` via `bilterm_inspect_pane` (or `events.poll` for `"type": "alternate_screen_changed", "active": true`).
3. Send navigation and edit keystrokes via `bilterm_send_keys`:
   ```json
   {"jsonrpc":"2.0","id":11,"method":"tools/call","params":{
     "name":"bilterm_send_keys",
     "arguments":{"keys":"G o # space edited space by space agent Esc : w q Enter"}
   }}
   ```
4. Verify `alternate_screen: false` and `exit_code: 0` in `command_blocks`.

### Example B: Function Key Routing in `htop` / `mc` (`:fkeys`)

By default, function keys `F1..F12` may be intercepted by Bilterm (`F1` Help, `F9` 3270 Inspector, `F10` Menu, `F12` Prompt Toggle). When driving `htop` (`F9` Kill, `F10` Quit) or Midnight Commander, either configure function-key passthrough or use `:fkeys send`:

```json
{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":fkeys app"}
}}
```

Or forward a specific function key directly to the child PTY without changing global F-key mode:

```json
{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":fkeys send F10"}
}}
```

Supported `:fkeys` modes: `:fkeys bilterm`, `:fkeys app`, `:fkeys auto`, `:fkeys toggle`, `:fkeys status`, `:fkeys send <F1..F12>`.

---

## 3. Password Prompts & `!ECHO LOCK` Safety (`sudo`, `ssh`)

When a command triggers a password prompt (`[sudo] password for user:`, `Password:`, `Enter passphrase for key`), Bilterm's 3-signal `!ECHO LOCK` detector sets `echo_lock: true` and emits `AutomationEvent::EchoLockChanged { pane_id, locked: true }`.

### Default Protection Behavior

Calling `bilterm_exec_command` (`pane.exec`) while `echo_lock: true` is **blocked by default** so an automated script or queued command does not accidentally echo commands into a password prompt or leak secrets:

```json
{
  "jsonrpc": "2.0",
  "id": 20,
  "error": {
    "code": -32000,
    "message": "pane #1 is in !ECHO LOCK password prompt; set allow_echo_lock=true to override"
  }
}
```

### Safe Handling Patterns for Agents

1. **Cancel & Switch to Non-Interactive Auth**: Send `Ctrl+C` via `bilterm_send_keys` (`{"keys": "Ctrl+C"}`), then re-run with `sudo -n` or SSH key-based batch mode (`ssh -o BatchMode=yes`).
2. **Explicit Override (`allow_echo_lock` in JSON-RPC `pane.exec`)**: Only in isolated test harnesses using synthetic credentials, pass `"allow_echo_lock": true` in `pane.exec`.

---

## 4. Vi Copy Mode, Rectangular Block Selection & Scrollback Search

Agents can search scrollback history, extract text regions, or manipulate Bilterm's yank ring via `:colon` commands (`bilterm_colon_command`):

```bash
# Search scrollback history and jump between matches
bilterm ctl wm.command '{"command":":search panicked at"}'
bilterm ctl wm.command '{"command":":search next"}'
bilterm ctl wm.command '{"command":":search prev"}'
bilterm ctl wm.command '{"command":":search off"}'

# Jump between OSC 133 semantic command blocks in scrollback
bilterm ctl wm.command '{"command":":block prev"}'
bilterm ctl wm.command '{"command":":block next"}'

# Control 3270 BlockPage ('MORE...') vs Smooth scroll mode
bilterm ctl wm.command '{"command":":scroll page"}'
bilterm ctl wm.command '{"command":":scroll next"}'
bilterm ctl wm.command '{"command":":scroll prev"}'
bilterm ctl wm.command '{"command":":scroll smooth"}'

# Enter Vi Copy Mode or Rectangular Block Selection (:copy, :rect, :select, :yank)
bilterm ctl wm.command '{"command":":copy"}'
bilterm ctl wm.command '{"command":":clipboard list"}'
```

---

## 5. 3270 QuickSelect Field Hints (`hints` in `bilterm_inspect_pane`)

Instead of writing regexes over raw terminal lines to extract URLs, file paths, IPv4 addresses, or git commit SHAs, read the `hints` array returned by `bilterm_inspect_pane` (`pane.read`) or `bilterm://pane/<id>`:

```json
{
  "hints": [
    {
      "label": "aa",
      "text": "https://localhost:8443/health",
      "kind": "Url",
      "row": 4,
      "col": 12
    },
    {
      "label": "ab",
      "text": "src/api/rpc.rs",
      "kind": "FilePath",
      "row": 7,
      "col": 5
    },
    {
      "label": "ac",
      "text": "10.0.0.42",
      "kind": "IpAddress",
      "row": 9,
      "col": 18
    }
  ]
}
```

To toggle the visual 3270 field-hint overlay or yank visible links in the UI:

```bash
bilterm ctl wm.command '{"command":":hints"}'
bilterm ctl wm.command '{"command":":links"}'
```
