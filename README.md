# Bilterm

<p align="center">
  <img src="docs/assets/bilterm-logo.png" alt="Bilterm Retro IBM 3270 x Borland Turbo Vision Logo" width="840" />
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-v1.9.1-blue" alt="Version" />
  <img src="https://img.shields.io/badge/rust-1.88%2B%20(2024)-orange" alt="Rust" />
  <img src="https://img.shields.io/badge/tests-236%20passing-brightgreen" alt="Tests" />
  <img src="https://img.shields.io/badge/coverage-83.45%25-brightgreen" alt="Coverage" />
  <img src="https://img.shields.io/badge/license-Apache--2.0-cyan" alt="License" />
</p>

**Bilterm** is a terminal user interface (TUI) multiplexer, shell wrapper, terminal emulator, and agent/fleet automation platform written in Rust using [Ratatui](https://ratatui.rs/). Fusing classic **IBM 3270 Mainframe Terminals** (top-pinned floating command prompt over a scrolling PTY output canvas, Operator Information Area status bar, and full-screen cross-hair cursor) with **DOS Borland Turbo Vision** windowing (`╔═╗` double-line borders, character-cell drop shadows, Z-order window cascade decks, and function-key bars), Bilterm treats the **floating command prompt + underlying PTY output canvas as a single composite terminal pane**.

---

## Screenshots

### 1. Multi-Pane Composite Workspace & SSH Fleet (`turbo-blue`)
<p align="center">
  <img src="docs/assets/screenshot-workspace.png" alt="Bilterm Multi-Pane Composite Workspace & SSH Fleet (Borland Turbo Blue)" width="920" />
</p>

### 2. IBM 3278 Phosphor Green CRT & Live 3270 Terminal Inspector (`F9`) / IBM 3279 Amber CRT & Fuzzy Command Palette (`Ctrl+P`)
| IBM 3278 Phosphor Green + `F9` 3270 Inspector | IBM 3279 Amber + Z-Order Cascade + `Ctrl+P` Palette |
| :---: | :---: |
| <img src="docs/assets/screenshot-3278-inspector.png" alt="IBM 3278 Phosphor Green + F9 Inspector" width="450" /> | <img src="docs/assets/screenshot-3279-palette.png" alt="IBM 3279 Amber + Z-Order Cascade + Command Palette" width="450" /> |

---

## Core Highlights

- **Composite Terminal Panes (`OutputCanvas` + `FloatingCommandPrompt` + `3270 OIA Bar`)**:
  - Pin the command prompt to **Top** (`Alt+T`), **Bottom** (`Alt+B`), **Free-Float** (`Alt+Up`/`Down` or mouse drag), or **Non-Overlapping Split** (`Alt+S` / `:prompt split-top | split-bottom`) with **zero `SIGWINCH` geometry thrashing**.
  - Flood-immune streaming (`tail -f /dev/random` at 60 FPS), `DECSET 1049` alternate-screen auto-hide (`vim`, `htop`), and 3-signal `!ECHO LOCK` password protection (`sudo`, `ssh`).
- **Multi-Pane Tiling, Z-Order Window Deck & Guake Overlay**:
  - Binary H/V splits (`F2`/`Alt+H`, `Alt+V`), overlapping Borland Z-order cascade (`Alt+M`, `F6`/`Alt+Tab`, `Alt+1..9`), and a persistent Guake drop-down terminal (`F10` / `Alt+\``).
- **Retro Mainframe & Borland Aesthetics + Focal Spotlight & Lens Shaders**:
  - 5 palettes (`turbo-blue`, `turbo-cyan`, `3278`, `3279`, `mono`), 8 border outlines, 4 drop-shadow styles, CRT scanlines (`:crt`), pane/full-screen IBM 3270 cross-hairs (`Alt+X`), **Focal Spotlight** (`Alt+O` / `:spotlight`) & **Focal Zoom Magnifying Lens** (`Alt+Shift+O` / `:focalzoom`) with smooth Hermite `smoothstep` or pixelated `2×1` character-block falloff, plus **Kitty (`APC G`)** and **DEC Sixel (`DCS q`)** graphics.
- **First-Class Local & Remote `tmux` + `herdr`-Style SSH Fleet Management & Slide-Out Drawer (`v1.4.0`–`v1.8.0`)**:
  - Spawn remote SSH composite panes (`:ssh <user@host> [port]`), attach to persistent remote `tmux -CC` sessions over SSH (`:ssh-tmux <user@host> [session]`), centrally fan out commands across local and remote nodes by `@tag` (`:remote add|list|exec|disconnect`), and inspect live session/window/pane telemetry in the 4-edge **`herdr`-Style Slide-Out Drawer** (`F7` / `Alt+Shift+D` / `<Leader> d` / `:drawer`).
- **Tektronix 4010/4014 Vector Storage-Tube Emulation, High-Res Pixel/GPU Canvas & Adaptive SSH Graphics (`v1.8.0`–`v1.9.1+`)**:
  - Decode 10-bit/12-bit `4096×3120` Tektronix 4010/4014 vectors with green `3278` / amber `3279` phosphor persistence decay (`\x1b[?38h` / `:canvas tek`), render a modern retained scene-graph (vectors, 2D sprites with AABB collisions, live `line`/`scatter`/`bar`/`candlestick`/`heatmap` plots, and interactive image `pan`/`zoom`/`inspect`) via anti-aliased RGBA `PixelCanvasBuffer` (`:canvas hires`, `:canvas export <file.ppm|file.svg>`), adaptive SSH inline pixel protocols (`Kitty APC G`, `DEC Sixel DCS q`, `iTerm2 OSC 1337` with `tmux` DCS passthrough), `1×2` 24-bit TrueColor `HalfBlock` (`▀`/`▄`), Unicode `2×4` Braille / `2×3` Sextant / Quadrant blocks, `bilterm plot` CLI, and dynamic terminal typography (`Ctrl++`/`Ctrl+-`/`Ctrl+0` / `:fontsize`).
- **Full Robot / Agent Automation API, Native MCP Server & Playbooks (`v1.1.0`–`v1.8.0`)**:
  - Drive 100% of Bilterm via JSON-RPC 2.0 (`bilterm --headless`, `bilterm ctl`), Model Context Protocol (`bilterm --mcp` with 10 agent tools), or declarative TOML playbooks (`bilterm --playbook <file.toml>`).

---

## Quick Start

### Option A: Plain Binary Installer (Users)

```bash
# Linux & macOS — install bilterm binary, manpage & completions to ~/.local/bin (or --cargo / --global / --prefix <DIR>)
./scripts/install.sh

# Or one-liner directly from GitHub:
curl -fsSL https://raw.githubusercontent.com/bsommers/bilterm/main/scripts/install.sh | bash

# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1
```

### Option B: Fresh Clone Developer Setup & Build (Linux, macOS, Windows)

```bash
# Linux & macOS — check/install C compiler, rustup, Rust 1.90, clippy/rustfmt, build & install
./scripts/setup.sh --check
./scripts/setup.sh --yes --build --install

# Windows (PowerShell) — check/install Git, MSVC Build Tools, rustup, build & install
powershell -ExecutionPolicy Bypass -File .\scripts\setup.ps1 -Yes -Build -Install

# Manual Cargo build & verification suite
cargo build --release
./target/release/bilterm
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

### CLI & Automation Quick Reference

| Command | Description |
|---------|-------------|
| `bilterm` | Launch interactive IBM 3270 × Borland Turbo Vision TUI multiplexer |
| `bilterm -v` / `--version` | Print version information (`-V` also supported) |
| `bilterm -h` / `--help` | Print CLI help summary |
| `bilterm -m` / `--mcp` | Run native Model Context Protocol (MCP `2024-11-05`) server over `stdio` |
| `bilterm -d` / `--headless [-l <addr>]` | Run headless JSON-RPC daemon (default private Unix socket; loopback TCP via `-l 127.0.0.1:PORT`) |
| `bilterm -r` / `--allow-remote` | Allow listening on non-loopback TCP addresses (requires `-t` / `--token-file`) |
| `bilterm -t` / `--token-file <path>` | Path to 0600 file containing bearer token (or `BILTERM_TOKEN_FILE`) |
| `bilterm -S` / `--snapshot-dir <path>` | Directory for confined workspace snapshots (default: XDG state dir) |
| `bilterm ctl [-t <path>] [endpoint] <method> [json]` | Send JSON-RPC 2.0 command (`workspace.status`, `pane.exec`, `fleet.exec`, etc.; defaults to local Unix socket) |
| `bilterm plot [kind] [csv] [-o <file>] [-b <mode>]` | Plot `hires`, `line`, `bar`, `scatter`, `candlestick`, or `heatmap` inline over SSH/terminal or export `.svg`/`.ppm` |
| `bilterm -p` / `--playbook <file.toml>` | Execute declarative multi-step TOML robot playbook with assertions |
| `bilterm -g` / `--gui-dump <path.ppm>` | Render workspace via `GuiWorkspaceRenderer` to a Netpbm `.ppm` framebuffer |
| `bilterm -s` / `--session <name>` | Use named persistent command-history session |
| `bilterm -c` / `--config <file.toml>` | Load configuration from custom TOML file path |
| `bilterm -C` / `--completions <bash\|zsh\|fish>` | Print shell completion script to `stdout` |
| `bilterm -M` / `--man` | Print standalone `roff(7)` manual page (`docs/bilterm.1`) |

### High-Resolution Graphics, Live Plotting & Tektronix 4014 Examples (SSH & Native GUI)

Inside any running `bilterm` pane, type these `:canvas` commands in the floating command prompt:

```text
:canvas hires                                                      # Anti-aliased TrueColor Half-Block/Pixel demo scene
:canvas mode halfblock                                             # 24-bit TrueColor 1x2 pixel blocks (▀/▄)
:canvas plot line 10,25,18,45,38,72,64,95                          # High-DPI line chart with area fill & gridlines
:canvas plot candlestick 10,22,8,18,18,30,15,28,28,35,20,24        # OHLC financial candlestick chart
:canvas plot heatmap 1,4,9,16,25,36,49,64,81                       # 2D thermal heatmap
:canvas bezier 4 28 22 4 46 32 68 8                                # Anti-aliased cubic Bezier curve
:canvas circle 40 16 10 filled                                     # Signed-distance-field anti-aliased disc
:canvas image 32 20                                                # Interactive bilinear RGBA image viewer (:canvas zoom 2.0)
:tek demo                                                          # Classic 1974 Tektronix 4014 green/amber storage-tube wireframe
:focalzoom 2x circle                                               # Smooth radial magnifying lens over graphics (:focalzoom off)
:canvas export /tmp/canvas.svg                                     # Export 640x384 high-res SVG or PPM (:canvas export /tmp/c.ppm)
:canvas clear                                                      # Clear canvas and return to normal terminal text
```

Or run directly from your shell / SSH session using `bilterm plot`:

```bash
bilterm plot line 12,28,19,45,38,64,52,84                          # Plot onto active Bilterm pane canvas
bilterm plot hires -b kitty                                        # Emit Kitty APC G RGBA pixels over SSH (or -b sixel / -b iterm2)
bilterm plot hires -o /tmp/hires-demo.svg                          # Export standalone high-resolution SVG or PPM chart
```

---

## Essential Shortcuts & `:colon` Commands

> **Dual Leader Prefix (`Ctrl+B` & `Ctrl+X`):** Bilterm supports both **`Ctrl+B`** (`tmux`/`herdr` style) and **`Ctrl+X`** (Borland/DOS & macOS safe) out of the box (automatically falling back to `Ctrl+X` when nested inside an outer `$TMUX` session, and accepting held-`Ctrl` chords like `Ctrl+B Ctrl+V` or `Ctrl+X Ctrl+Z`).

| Action | Shortcut (`<L>` = `Ctrl+B` or `Ctrl+X`) | `:command` |
|--------|----------|------------|
| **Help / Command Palette / 3270 Inspector** | `F1` (`<L> ?`) / `Ctrl+P` (`<L> :` / `Alt+Space`) / `F9` (`<L> i`) | `:help` / `:palette` / `:inspector` |
| **Split Horizontal / Split Vertical / Zoom** | `F2` (`<L> "` / `<L> -` / `Alt+H`) / `<L> %` (`<L> \|` / `Alt+V`) / `F5` (`<L> z` / `Alt+Z`) | `:split h` / `:split v` / `:zoom` |
| **Toggle Tiled vs. Z-Stack / Shuffle Deck** | `<L> m` (`Alt+M`) / `F6` (`<L> n` / `<L> p` / `<L> Tab`) / `<L> 1..9` (`Alt+1..9`) | `:layout stack\|tile` / `:deck next\|prev` / `:focus <N>` |
| **Guake Quick Terminal / Raw PTY Toggle** | `F10` (`Ctrl+\` / `Alt+\``) / `F12` (`Ctrl+G`) | `:quake` / `:raw` |
| **Cycle Floating / Fixed / Regular Terminal** | `F4` (`<L> e` / `Alt+E`) / Click OIA Bar | `:mode [float \| fixed \| terminal \| cycle]` |
| **Prompt Pin Top / Bottom / Non-Overlapping Split**| `<L> t` (`Alt+T`) / `<L> b` / `<L> s` (`Alt+S`) | `:prompt top \| bottom \| split-top \| split-bottom` |
| **3270 Field Hints / Vi Copy / Canvas Search** | `<L> f` / `<L> [` (`Alt+C`) / `<L> /` (`Alt+/`) | `:hints` / `:copy` / `:search <query>` |
| **Whole-Workspace or Pane `.cast` Recording** | `<L> R` / — | `:record start [path] \| workspace [path] \| pane [path] \| stop` |
| **Read-Only Lock / Multi-Pane Broadcast** | `<L> l` (`Alt+L`) / — | `:lock [on\|off]` / `:broadcast [on\|off]` |
| **Theme / Cross-Hairs / CRT Scanlines** | — / `<L> x` (`Alt+X`) / — | `:theme <name>` / `:crosshair full\|pane\|off` / `:crt` |
| **Focal Spotlight / Focal Zoom Lens / Resize** | `Alt+O` (`<L> L`) / `Alt+Shift+O` / `Alt+Shift+]` & `[` | `:spotlight [circle\|ellipse] [fuzzy\|pixel]` / `:focalzoom [1.25x..3x]` |
| **Slide-Out Drawer / Dynamic Font Scaling** | `F7` (`Alt+Shift+D` / `<L> d`) / `Ctrl++`, `Ctrl+-`, `Ctrl+0` | `:drawer [toggle\|left\|right\|top\|bottom\|float\|push]` / `:fontsize [+|-|reset|<pt>|<pct>%]` |
| **Tektronix 4010/4014 & Graphical Canvas** | `\x1b[?38h` / `OSC 1337;Canvas=...` | `:canvas [tek\|mode\|line\|circle\|sprite\|plot\|image\|clear]` |
| **SSH Pane / Remote `tmux -CC` over SSH** | — | `:ssh <user@host> [port]` / `:ssh-tmux <user@host> [sess]` |
| **`herdr` Fleet Register / List / Tag Exec** | — | `:remote add <name> <host> [@tags]` / `:remote list` / `:remote exec <@tag\|all> <cmd>` |

*(See **[Chapter 6: Complete Keyboard, Mouse & `:command` Reference](docs/user-guide/06-keyboard-mouse-and-command-reference.md)** for the full table of shortcuts, mouse gestures, and commands).*

---

## Documentation Hub

All detailed engineering guides, user manual chapters, and architecture diagrams live in [`docs/`](docs/README.md):

- **[Documentation Index (`docs/README.md`)](docs/README.md)**
  - **[Developer, Maintainer & AI Agent Reference (`docs/DEVELOPER_AND_AGENT_GUIDE.md`)](docs/DEVELOPER_AND_AGENT_GUIDE.md)** — Complete 48-file source map, crate & library stack, encapsulated state invariants, unified `Action` + declarative keymap architecture, and live inspectability/debugging guide (`F9` Inspector, `bilterm.log`, JSON-RPC/MCP).
  - **[Purpose & Security Threat Model (`docs/purpose.md`)](docs/purpose.md)** — IBM 3270 × Borland design rationale, `!ECHO LOCK` password safety, `/dev/random` flood immunity, and paste-injection guards.
  - **[System Architecture & Internals (`docs/architecture.md`)](docs/architecture.md)** — 3-pass Ratatui compositor, `GridSnapshot` + `VtWorkerHandle` seam, 3-signal line-discipline detector, JSON-RPC/MCP control plane, and SSH/`tmux -CC` fleet bridge.
  - **[Running, Shells, Tab Completion & Config (`docs/running.md`)](docs/running.md)** — Shell resolution (`$SHELL` / `%COMSPEC%`), dual-mode `<Tab>` completion internals, `config.toml` & `[remotes.<name>]` schema, and CLI reference.
  - **[Operational How-To & Recipes (`docs/how-to.md`)](docs/how-to.md)** — Step-by-step recipes for split prompts, SSH/fleet fan-out, MCP agent integration, robot playbooks, and troubleshooting.
  - **[Visual Diagrams Catalog (`docs/diagrams/README.md`)](docs/diagrams/README.md)** — Terminal ASCII (`ascii_architecture.txt`), Mermaid (`.mmd`), and native draw.io XML (`.drawio`) diagrams.
  - **[Test Coverage & Battle Report Card (`docs/TEST_COVERAGE.md`)](docs/TEST_COVERAGE.md)** — `83.45%` instrumented line coverage across unit, API/MCP, integration, stress, performance, and `vttest`/`esctest` torture suites.
- **[7-Chapter User Guide (`docs/user-guide/README.md`)](docs/user-guide/README.md)**
  1. [Getting Started & Core Concepts](docs/user-guide/01-getting-started.md)
  2. [The Command Prompt, Tab Completion, History & Command Blocks](docs/user-guide/02-floating-command-prompt.md)
  3. [Multi-Pane Window Manager, Z-Order Stacking & Guake Quick Terminal](docs/user-guide/03-window-manager-and-z-stack.md)
  4. [Retro Mainframe & DOS Aesthetics, Outlines, Drop Shadows, Cross-Hairs & Config](docs/user-guide/04-themes-glyphs-and-config.md)
  5. [First-Class `tmux` Integration, SSH Domains & `herdr` Fleet Management](docs/user-guide/05-tmux-integration.md)
  6. [Complete Keyboard, Mouse & `:command` Reference](docs/user-guide/06-keyboard-mouse-and-command-reference.md)
  7. [Troubleshooting & FAQ](docs/user-guide/07-troubleshooting-and-faq.md)
- **[Changelog (`CHANGELOG.md`)](CHANGELOG.md)** — Full semantic versioning release history (`v0.1.0` – `v1.9.0`).
- **[License (`LICENSE`)](LICENSE)** — Licensed under the Apache License, Version 2.0 (`Apache-2.0`).
