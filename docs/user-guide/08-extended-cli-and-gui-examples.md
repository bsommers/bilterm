# Chapter 8: Extended CLI, Native GUI & Typography Examples

[← Chapter 7: Troubleshooting & FAQ](07-troubleshooting-and-faq.md) | [User Guide Home](README.md) | [Chapter 9: Use Cases & Workflows →](09-use-cases-and-workflows.md)

Bilterm can run either **inside any host terminal emulator** (Ratatui + Crossterm TUI mode) or as a **standalone native desktop GUI window** (`bilterm --gui` / `-G`) powered by X11 + Xft/Fontconfig on Linux/Unix and Win32 GDI ClearType + `DIBSection` on Windows.

![3-Way Split with Independent Per-Pane Font Scaling](../assets/screenshot-per-pane-scaling.svg)

---

## Table of Contents

1. [Standalone Native GUI Mode (`--gui` / `-G`)](#1-standalone-native-gui-mode---gui---g)
2. [Font Family, Point Size & Custom `.ttf`/`.otf` Loading](#2-font-family-point-size--custom-ttfotf-loading)
3. [Independent Per-Pane Font Scaling (`Ctrl++` / `Ctrl+-` / `Ctrl+Wheel`)](#3-independent-per-pane-font-scaling-ctrl--ctrl--ctrlwheel)
4. [All-Border & Corner Mouse Frame Resizing](#4-all-border--corner-mouse-frame-resizing)
5. [Headless Automation, `ctl` JSON-RPC, MCP & Playbooks](#5-headless-automation-ctl-json-rpc-mcp--playbooks)
6. [Terminal Graphics & Sub-Cell Plotting (`bilterm plot`)](#6-terminal-graphics--sub-cell-plotting-bilterm-plot)

---

## 1. Standalone Native GUI Mode (`--gui` / `-G`)

Launch Bilterm directly as a native desktop window without an outer terminal emulator:

```bash
# Default native GUI window (uses config.toml or Cascadia Mono / JetBrains Mono / DejaVu Sans Mono at 14pt)
bilterm --gui

# Short flag equivalent
bilterm -G

# Environment variable trigger (ideal for desktop launchers or scripts)
BILTERM_GUI=1 bilterm
```

---

## 2. Font Family, Point Size & Custom `.ttf`/`.otf` Loading

Both `--font` and `--gui-font` (as well as `--font-size` and `--gui-font-size`, `--font-file` and `--gui-font-file`) are accepted interchangeably:

```bash
# Launch native GUI with JetBrains Mono at 16pt
bilterm --gui --gui-font "JetBrains Mono" --gui-font-size 16

# Launch native GUI with Fira Code at 15pt and IBM 3278 Phosphor Green theme
bilterm --gui --theme green --gui-font "Fira Code" --gui-font-size 15

# Launch native GUI with Cascadia Mono at 18pt and IBM 3279 Amber CRT theme
bilterm --gui --theme amber -F cascadia --font-size 18

# Launch with a custom downloaded TrueType/OpenType font file (.ttf/.otf/.ttc)
bilterm --gui --gui-font-file ~/.local/share/fonts/IosevkaTerm-Regular.ttf --gui-font-size 16

# Launch with a Starter Pack workstation configuration
bilterm --gui -c starter-packs/04-sysadmin-incident-war-room/config.toml --gui-font "JetBrains Mono" --gui-font-size 16
```

### Built-In Font Family Presets (`-F` / `--font` / `--gui-font` / `:font <preset>`)

| Preset Alias | Resolved Font Family | Primary Fallback Chain |
| :--- | :--- | :--- |
| `jetbrains` | `JetBrains Mono` | `JetBrains Mono` → `JetBrainsMono NF` → `DejaVu Sans Mono` |
| `cascadia` | `Cascadia Mono` | `Cascadia Mono` → `Cascadia Code` → `Consolas` |
| `consolas` | `Consolas` | `Consolas` → `Cascadia Mono` → `Liberation Mono` |
| `firacode` | `Fira Code` | `Fira Code` → `Fira Mono` → `DejaVu Sans Mono` |
| `iosevka` | `Iosevka Term` | `Iosevka Term` → `Iosevka` → `DejaVu Sans Mono` |
| `plex` | `IBM Plex Mono` | `IBM Plex Mono` → `DejaVu Sans Mono` |
| `hack` | `Hack` | `Hack` → `DejaVu Sans Mono` |
| `ibm3270` | `IBM 3270` | `IBM 3270` → `3270 Narrow` → `IBM Plex Mono` |
| `vga` | `PxPlus IBM VGA8` | `PxPlus IBM VGA8` → `IBM Plex Mono` → 5×7 VGA Bitmap |

---

## 3. Independent Per-Pane Font Scaling (`Ctrl++` / `Ctrl+-` / `Ctrl+Wheel`)

Each composite pane maintains its own independent font scale (`50%..300%`) and dynamically resizes its underlying PTY (`SIGWINCH` `rows × cols`) and `VtParser` viewport inversely to its font scale:

- **Zoom In (`+10%` on active pane)**: `Ctrl++`, `Ctrl+=`, `Ctrl+X` then `Ctrl++`, or `:fontsize +`
- **Zoom Out (`-10%` on active pane)**: `Ctrl+-`, `Ctrl+_`, `Ctrl+X` then `Ctrl+-`, or `:fontsize -`
- **Reset Scale (`100%` on active pane)**: `Ctrl+0`, `Ctrl+X` then `Ctrl+0`, or `:fontsize reset`
- **Mouse Wheel Zoom (in `--gui` mode)**: Hold `Ctrl` and scroll the mouse wheel (`Ctrl+WheelUp` / `Ctrl+WheelDown`) over any pane to focus and zoom that pane's font on the fly.
- **Interactive TUI Support (`nmon`, `htop`, `btop`, `vim`)**: `Ctrl++`, `Ctrl+-`, and `Ctrl+0` are intercepted by the Window Manager even when the focused pane is in `RawPtyPassthrough` / `DECSET 1049` alternate screen mode, immediately sending `SIGWINCH` with the new scaled `(rows, cols)` to `nmon` or `htop`.
- **Workspace-Wide Font Override**: Prefix any `:font` command with `all` (e.g. `:font all size 16`, `:font all scale 120%`, `:font all jetbrains`) to apply across every pane simultaneously.

---

## 4. All-Border & Corner Mouse Frame Resizing

Every border and corner of every pane frame is draggable with the mouse in both `Tiled` and `Stacked` modes:

- **Shared Internal Borders (`Left`, `Right`, `Top`, `Bottom`)**: Dragging any border between two tiled panes smoothly moves the corresponding `SplitTree` divider without swapping panes.
- **Outer Borders (`Left`, `Right`, `Top`, `Bottom`)**: Dragging an outer border inward/outward maps your drag delta onto the opposite `SplitTree` divider so you can resize any pane from any of its 4 sides.
- **All 4 Corners (`TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`)**: Dragging any corner resizes both horizontal and vertical splits simultaneously (or single-axis if dragged straight along one axis).
- **Moving / Swapping Windows**:
  - In `Stacked` mode (`Alt+M` / `:layout stacked`), drag the top title bar (`[1] ...`) to move the floating window anywhere on the workspace.
  - In `Tiled` mode, hold `Alt` and drag a pane's title bar (or use `Ctrl+Shift+Arrow`) to float or swap panes.

---

## 5. Headless Automation, `ctl` JSON-RPC, MCP & Playbooks

```bash
# 1. Run a declarative TOML automation playbook headlessly and print JSON report
bilterm run starter-packs/01-multi-agent-swarm/playbook.toml
bilterm --playbook starter-packs/03-robot-scripting-and-qa/playbook.toml

# 2. Start a headless JSON-RPC 2.0 daemon on the default 0600 Unix socket ($XDG_RUNTIME_DIR/bilterm/control.sock)
bilterm --headless

# 3. Query workspace status, split panes, or execute commands via `bilterm ctl`
bilterm ctl unix:/run/user/$(id -u)/bilterm/control.sock workspace.status
bilterm ctl unix:/run/user/$(id -u)/bilterm/control.sock wm.command '{"command":":vsplit"}'
bilterm ctl unix:/run/user/$(id -u)/bilterm/control.sock pane.exec '{"pane_id":1,"command":"cargo test -q"}'

# 4. Run a bearer-token-authenticated TCP daemon (loopback or remote with --allow-remote)
umask 077 && head -c 32 /dev/urandom | base64 > ~/.config/bilterm/rpc.token
bilterm --headless --listen 127.0.0.1:7370 --token-file ~/.config/bilterm/rpc.token
bilterm ctl --token-file ~/.config/bilterm/rpc.token 127.0.0.1:7370 workspace.status

# 5. Start the Model Context Protocol (MCP 2024-11-05) server over stdio for Claude Desktop / Cursor / AI agents
bilterm --mcp --snapshot-dir /tmp/bilterm-snapshots

# 6. Render a deterministic GPU/GUI framebuffer snapshot (.ppm) without a display server
bilterm --gui-dump /tmp/workspace.ppm
```

---

## 6. Terminal Graphics & Sub-Cell Plotting (`bilterm plot`)

```bash
# Render a high-resolution mathematical demo scene using auto-detected graphics (Kitty / Sixel / iTerm2 / Braille)
bilterm plot hires

# Plot CSV series as line, bar, scatter, candlestick, or heatmap charts
bilterm plot line "12,28,19,45,38,64,52,84"
bilterm plot bar "10,40,30,80,65,95" --backend kitty
bilterm plot heatmap "10,20,30,80,90,40,25,60,95" --output /tmp/heatmap.svg
bilterm plot candlestick "40,55,35,50,50,70,45,65" --output /tmp/candles.ppm
```
