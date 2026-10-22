# Operational How-To Recipes & Troubleshooting Matrix

[← Back to Documentation Hub](README.md) | [← Back to Project README](../README.md)

## Table of Contents

- [Recipe 1: Using Tab Completion for Commands, Paths & `:colon` Settings](#recipe-1-using-tab-completion-for-commands-paths--colon-settings)
- [Recipe 2: Non-Overlapping Split Command & Output Panes](#recipe-2-non-overlapping-split-command--output-panes)
- [Recipe 3: Resizing Command Prompt Width & Scrolling Multi-Line Scripts](#recipe-3-resizing-command-prompt-width--scrolling-multi-line-scripts)
- [Recipe 4: Full-Screen vs. Pane-Local IBM 3270 Cross-Hair Cursor](#recipe-4-full-screen-vs-pane-local-ibm-3270-cross-hair-cursor)
- [Recipe 5: Inspecting & Exporting Session Command History](#recipe-5-inspecting--exporting-session-command-history)
- [Recipe 6: Customizing Window Outlines, DOS Drop Shadows & Palettes](#recipe-6-customizing-window-outlines-dos-drop-shadows--palettes)
- [Recipe 7: Fuzzy Command Palette, Output Triggers & Session Recording (`v0.6.0`–`v1.0.0`)](#recipe-7-fuzzy-command-palette-output-triggers--session-recording-v060v100)
- [Recipe 8: Remote SSH Domains & `herdr`-Style Fleet Federation (`v1.4.0`)](#recipe-8-remote-ssh-domains--herdr-style-fleet-federation-v140)
- [Recipe 9: JSON-RPC 2.0 Automation API, MCP Server & Playbooks (`v1.3.0`)](#recipe-9-json-rpc-20-automation-api-mcp-server--playbooks-v130)
- [Recipe 10: Local Verification & Coverage Gate (CI Conservation)](#recipe-10-local-verification--coverage-gate-ci-conservation)
- [Troubleshooting Matrix](#troubleshooting-matrix)

---

## Recipe 1: Using Tab Completion for Commands, Paths & `:colon` Settings

1. **Complete a `:colon` command**:
   - Type `:out` and press **`Tab`** -> completes to `:outline `.
   - Press **`Tab`** again -> opens `TAB: [ascii]  dashed  dotted  double  quadruple  rounded  solid  thick`.
2. **Complete a binary in `$PATH`**:
   - Type `car` and press **`Tab`** -> completes `cargo` (and cycles `cargo-*` binaries on repeated `Tab`).
3. **Complete a path in the child shell's working directory**:
   - Type `ls sr` and press **`Tab`** -> completes `ls src/`.
   - Press **`Tab`** again -> cycles files inside `src/` (`src/app.rs`, `src/config/`, `src/input/`, etc.).

---

## Recipe 2: Non-Overlapping Split Command & Output Panes

To partition the window into a dedicated command prompt pane and output pane with **zero overlap**:

```text
==> :prompt split-top
```

Or place the split command pane at the bottom:

```text
==> :prompt split-bottom
```

Or toggle with **`Alt+S`**. In both modes, `CompositePane::layout_rects` allocates disjoint vertical rectangles so the command prompt never covers any row of terminal output.

---

## Recipe 3: Resizing Command Prompt Width & Scrolling Multi-Line Scripts

1. **Resize Width**:
   - Press **`Alt+[`** to narrow or **`Alt+]`** to widen the command prompt by 4 columns.
   - Or drag the prompt's **left border**, **right border**, or **bottom-right corner (`◢`)** with the mouse.
   - Or set an exact column width via `:prompt width 55` (and reset via `:prompt width full`).
2. **Write & Scroll Multi-Line Commands**:
   - Press **`Shift+Enter`** or **`Alt+Enter`** to insert newlines inside the command prompt.
   - When the script exceeds the prompt box height, use **`Shift+PageUp` / `Shift+PageDown`** or the **mouse scroll wheel** over the prompt box to scroll vertically (a `█`/`░` scrollbar indicator renders on the right border).

---

## Recipe 4: Full-Screen vs. Pane-Local IBM 3270 Cross-Hair Cursor

- **Via Keyboard (`Alt+X`)**:
  - 1st press: **Pane-Local Cross-Hairs** (spans only the focused pane).
  - 2nd press: **Full-Screen Cross-Hairs** (extends edge-to-edge across the entire terminal viewport).
  - 3rd press: **Off**.
- **Via `:command`**:
  ```text
  ==> :crosshair full
  ==> :crosshair pane
  ==> :crosshair off
  ```

---

## Recipe 5: Inspecting & Exporting Session Command History

1. **Open the Borland History Pop-up Window**:
   - Press **`Alt+P`** or run **`:history`**.
   - Use **`Up` / `Down`** to select a command and **`Enter`** to load it into the prompt.
   - Press **`s`** inside the pop-up to save all session commands to `/tmp/bilterm-commands-<session_id>-pane-<id>.sh`.
2. **Export Directly via Command**:
   ```text
   ==> :history save /tmp/my-commands.sh
   ```
3. **Configure Persistence & File Rotation**:
   ```text
   ==> :history max-files 50
   ==> :history off
   ==> :history on
   ```

---

## Recipe 6: Customizing Window Outlines, DOS Drop Shadows & Palettes

```text
==> :theme 3278
==> :outline window double
==> :outline prompt dotted
==> :shadows tinted
==> :glyphs cp437
==> :fkeybar on
```

---

## Recipe 7: Fuzzy Command Palette, Output Triggers & Session Recording (`v0.6.0`–`v1.0.0`)

1. **Open the Fuzzy Command Palette**:
   - Press **`Ctrl+P`** or **`Alt+Space`** (or run **`:palette`**) to search all panes, themes, sessions, and `:colon` commands via `nucleo-matcher`.
2. **Enable Retro CRT Scanlines & Output Highlight Triggers**:
   ```text
   ==> :crt on
   ==> :trigger add ERROR:
   ==> :trigger add PANIC
   ```
3. **Record an Asciicast v2 (`.cast`) Terminal Session**:
   ```text
   ==> :record start /tmp/demo.cast
   ==> echo "Recorded in Asciicast v2 format!"
   ==> :record stop
   ```

---

## Recipe 8: Remote SSH Domains & `herdr`-Style Fleet Federation (`v1.4.0`)

1. **Connect Focused Pane or Split Directly to an SSH Host (`:ssh`)**:
   ```text
   ==> :ssh deploy@prod-web-01
   ==> :ssh split v admin@db-primary:2222
   ```
   *(The OIA status bar automatically displays a bright cyan `SSH:deploy@prod-web-01` domain badge).*
2. **Attach to a Persistent Remote `tmux -CC` Session Over SSH (`:ssh-tmux`)**:
   ```text
   ==> :ssh-tmux deploy@prod-web-01 main
   ```
   *(Hydrates remote `tmux` panes 1:1 into local Bilterm Composite Panes with local floating command prompts).*
3. **Centrally Manage Local & Remote Nodes (`herdr`-Style Fleet Federation)**:
   ```text
   ==> :remote add prod deploy@prod-web-01 127.0.0.1:7370 main
   ==> :remote list
   ==> :remote exec prod workspace.status
   ==> :remote exec prod command.execute {"command":":split v"}
   ==> :remote disconnect prod
   ```

---

## Recipe 9: JSON-RPC 2.0 Automation API, MCP Server & Playbooks (`v1.3.0`)

1. **Drive a Live or Headless Bilterm Instance via `bilterm ctl`**:
   ```bash
   bilterm --headless --listen 127.0.0.1:7370 &
   bilterm ctl 127.0.0.1:7370 workspace.status
   bilterm ctl 127.0.0.1:7370 prompt.submit '{"pane_id":1,"command":"uname -a"}'
   bilterm ctl 127.0.0.1:7370 pane.wait_for '{"pane_id":1,"pattern":"Linux","timeout_ms":2000}'
   ```
2. **Expose Bilterm as an AI Agent MCP Server (`bilterm --mcp`)**:
   Configure your MCP client (Claude Desktop, Cursor, Antigravity) to invoke `bilterm --mcp` over stdio to expose 11 native tools (`bilterm_workspace_status`, `bilterm_pane_read`, `bilterm_prompt_submit`, `bilterm_pane_wait_for`, `bilterm_command_execute`, `bilterm_remote_list`, `bilterm_remote_add`, `bilterm_remote_exec`, etc.).
3. **Run a Declarative TOML Automation Playbook (`--playbook`)**:
   ```bash
   bilterm --playbook deploy-check.toml
   ```

---

## Recipe 10: Local Verification & Coverage Gate (CI Conservation)

Run the full test, torture, clippy, and coverage suite locally before committing or pushing:

```bash
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo llvm-cov --summary-only
```

---

## Troubleshooting Matrix

| Symptom | Root Cause | Remedy |
|---------|------------|--------|
| Floating prompt hidden while running `vim`, `htop`, or `less` | Alternate Screen (`DECSET 47/1047/1049`) auto-hide (`▣ ALT SCR`) | Exit the TUI app, or press `F12` / `Ctrl+G` (`:raw`) to toggle manually |
| Floating prompt hidden with red `♦ !ECHO LOCK` badge | Foreground process (`sudo`, `ssh`) disabled `termios ECHO` | Type your password and press `Enter`; prompt reappears automatically once `ECHO` is restored |
| Prompt covers top 3 lines of command output | Default `PIN:TOP` floating mode | Press `Alt+S` or run `:prompt split-top` for zero-overlap split mode |
| Need to stop `tail -f /dev/random` while in Command Box | Foreground job streaming continuously | Press `Ctrl+C` directly in the command box (sends `0x03` `SIGINT` to the PTY) |
| `:yank` (`OSC 52`) does not reach local clipboard inside `tmux` | Outer `tmux` has `allow-passthrough` disabled | Run `tmux set -g allow-passthrough on` |
