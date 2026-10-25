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
   - Press **`s`** inside the pop-up to save all session commands to `$XDG_RUNTIME_DIR/bilterm/exports/` (or `/tmp/bilterm-<uid>/exports/`) with mode `0600`.
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
3. **Record a Whole-Workspace or Single-Pane Asciicast v2 (`.cast`) Terminal Session**:
   ```text
   ==> :record start
   ==> :record workspace /tmp/workspace.cast
   ==> :record pane /tmp/pane-only.cast
   ==> :record stop
   ```
   - `:record start [path]` / `:record workspace [path]` (or leader chord `Ctrl+B R` / `Ctrl+X R`) records the **entire multi-pane Ratatui compositor buffer** (floating command prompts, Z-order window borders, DOS drop shadows, and OIA status bars) into an Asciicast v2 `.cast` file using cell-diffing (`WorkspaceRecorder`).
   - `:record pane [path]` records only the active pane's raw PTY stdout stream (`SessionRecorder`).
   - *(Omitting the path writes to `$XDG_RUNTIME_DIR/bilterm/recordings/` with race-free `0600` permissions).*
4. **Running Bilterm Inside `asciinema rec` (External Recorder Compatibility)**:
   - When `asciinema rec` spawns a sub-PTY inside a pane, `tcgetattr(master_fd)` reports non-canonical `!ECHO` raw mode, which initially hides the floating command prompt (`!ECHO LOCK`).
   - Press **`F4`**, **`F12`**, or **`Ctrl+X e` / `Ctrl+B e`** (`:mode float`) to enable `force_command_prompt` and restore the floating command prompt over the `asciinema` sub-shell while still auto-hiding whenever `sudo`/`ssh` prints an active `Password:` prompt.
   - You can also keep `Ctrl` held down across two-key leader chords (`Ctrl+X Ctrl+V`, `Ctrl+B Ctrl+Z`, `Ctrl+X Ctrl+E`).

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

## Recipe 9: JSON-RPC 2.0 Automation API, MCP Server & Playbooks (`v1.3.0`–`v1.6.0`)

1. **Drive a Live or Headless Bilterm Instance via `bilterm ctl`**:
   ```bash
   # Default: private 0600 Unix domain socket
   bilterm --headless &
   bilterm ctl workspace.status
   bilterm ctl pane.exec '{"pane_id":1,"command":"uname -a"}'

   # Authenticated TCP endpoint with bearer token file:
   umask 077; head -c 32 /dev/urandom | base64 > ~/.config/bilterm/token
   bilterm --headless --listen 127.0.0.1:7370 --token-file ~/.config/bilterm/token &
   bilterm ctl --token-file ~/.config/bilterm/token 127.0.0.1:7370 workspace.status
   ```
   *(Note: `workspace.snapshot` `ppm_path` must be a bare file name confined to the snapshot directory, e.g. `frame.ppm` inside `$XDG_STATE_HOME/bilterm/snapshots` or `--snapshot-dir <path>`).*
2. **Expose Bilterm as an AI Agent MCP Server (`bilterm --mcp`)**:
   Configure your MCP client (Claude Desktop, Cursor, Antigravity) to invoke `bilterm --mcp` over stdio to expose 11 native tools (`bilterm_workspace_status`, `bilterm_pane_read`, `bilterm_prompt_submit`, `bilterm_pane_wait_for`, `bilterm_command_execute`, `bilterm_remote_list`, `bilterm_remote_add`, `bilterm_remote_exec`, etc.).
3. **Run a Declarative TOML Automation Playbook (`--playbook`)**:
   ```bash
   bilterm --playbook deploy-check.toml
   ```

---

## Recipe 10: Live Inspection, Debugging & Local Verification Gate

1. **Live 3270 Terminal Inspector (`<F9>` / `:inspector`)**:
   - Press **`F9`** (or `Ctrl+B i` / `Ctrl+X i` / `:inspector`) at any time to inspect active `DECSET` modes (`ALT_SCREEN`, `BRACKETED_PASTE`, `SGR_MOUSE`), `termios` `ICANON`/`ECHO` state, Kitty keyboard flags, the cell under the cursor (`U+XXXX`, FG/BG RGB), and the last 16 CSI/OSC sequences.
2. **Non-Blocking Diagnostic Trace Logging (`bilterm.log`)**:
   ```bash
   RUST_LOG=bilterm=debug cargo run
   tail -f bilterm.log
   ```
3. **FullVerification & Architecture Gate (CI Conservation)**:
   Run the full test, torture, clippy, and line-budget suite locally before committing or pushing (see [`docs/DEVELOPER_AND_AGENT_GUIDE.md`](DEVELOPER_AND_AGENT_GUIDE.md) for the complete architecture & debugging manual):

```bash
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
awk 'ENDFILE { if (FNR > 800) { print FILENAME ": " FNR; bad=1 } } END { exit bad }' src/**/*.rs
```

---

## Recipe 11: High-Resolution Graphics, Live Plotting & Adaptive SSH/GUI Canvas (`v1.9.1+`)

Bilterm supports both **classic 1974 Tektronix 4010/4014 vector storage-tube emulation** and a **modern anti-aliased RGBA pixel/GPU canvas** (`PixelCanvasBuffer`) that works seamlessly across **Native GUI** (`GuiWorkspaceRenderer`) and **Pure SSH sessions** (auto-negotiating `Kitty APC G`, `DEC Sixel DCS q`, `iTerm2 OSC 1337`, `tmux` DCS passthrough `\x1bPtmux;...\x1b\\`, `1×2` 24-bit TrueColor `HalfBlock` `▀`/`▄`, or `2×4` Unicode `Braille`).

1. **Interactive `:canvas` Commands Inside the Floating Command Prompt**:
   ```text
   :canvas hires
   :canvas mode halfblock
   :canvas plot line 10,25,18,45,38,72,64,95
   :canvas plot candlestick 10,22,8,18,18,30,15,28,28,35,20,24,24,42,22,40
   :canvas plot heatmap 1,4,9,16,25,36,49,64,81
   :canvas bezier 4 28 22 4 46 32 68 8
   :canvas circle 40 16 10 filled
   :canvas image 32 20
   :canvas zoom 2.0
   ```
2. **Compare Sub-Cell & Storage-Tube Rendering Modes Live**:
   ```text
   :canvas mode halfblock    # 24-bit TrueColor 1x2 pixel blocks (▀/▄) with anti-aliasing & shading
   :canvas mode braille      # High-density 2x4 Unicode Braille vector dots (U+2800..U+28FF)
   :canvas mode sextant      # 2x3 Unicode Sextant blocks (U+1FB00..U+1FB3B)
   :canvas mode quadrant     # 2x2 Quadrant blocks
   :tek demo                 # Classic Tektronix 4014 green/amber phosphor vector storage tube
   :focalzoom 2x circle      # Smooth radial magnifying lens (:focalzoom off to disable)
   ```
3. **Export High-Resolution `.svg` or `.ppm` Framebuffers (`640×384` px) or Clear**:
   ```text
   :canvas export /tmp/canvas.svg
   :canvas export /tmp/canvas.ppm
   :canvas clear
   ```
4. **Plot & Export Directly from the Shell or SSH CLI (`bilterm plot`)**:
   ```bash
   # Render a live plot onto the current Bilterm pane canvas:
   bilterm plot line 12,28,19,45,38,64,52,84

   # Emit raw Kitty APC G or DEC Sixel pixel stream directly over SSH:
   bilterm plot hires -b kitty
   bilterm plot hires -b sixel

   # Export a standalone high-res SVG or PPM chart directly from the CLI:
   bilterm plot hires -o /tmp/hires-demo.svg
   bilterm plot candlestick 10,22,8,18,18,30,15,28,28,40,22,38 -o /tmp/ohlc.svg
   ```

---

## Troubleshooting Matrix

| Symptom | Root Cause | Remedy |
|---------|------------|--------|
| Floating prompt hidden while running `vim`, `htop`, or `less` | Alternate Screen (`DECSET 47/1047/1049`) auto-hide (`▣ ALT SCR`) | Exit the TUI app, or press `F12` / `Ctrl+G` (`:raw`) to toggle manually |
| Floating prompt hidden inside `asciinema rec` | `asciinema` puts its inner sub-PTY into non-canonical `!ECHO` raw mode | Press `F4`, `F12`, or `Ctrl+X e` / `Ctrl+B e` (`:mode float`) to restore the floating command prompt |
| `Ctrl+X <key>` or `Ctrl+B <key>` chord ignored when `Ctrl` stays held | Second key arrives with `KeyModifiers::CONTROL` | Supported natively: `Ctrl+X Ctrl+<key>` and `Ctrl+B Ctrl+<key>` normalize automatically to leader chords |
| Floating prompt hidden with red `♦ !ECHO LOCK` badge | Foreground process (`sudo`, `ssh`) disabled `termios ECHO` | Type your password and press `Enter`; prompt reappears automatically once `ECHO` is restored |
| Prompt covers top 3 lines of command output | Default `PIN:TOP` floating mode | Press `Alt+S` or run `:prompt split-top` for zero-overlap split mode |
| Need to stop `tail -f /dev/random` while in Command Box | Foreground job streaming continuously | Press `Ctrl+C` directly in the command box (sends `0x03` `SIGINT` to the PTY) |
| `:yank` (`OSC 52`) does not reach local clipboard inside `tmux` | Outer `tmux` has `allow-passthrough` disabled | Run `tmux set -g allow-passthrough on` |
