# Bilterm

<p align="center">
  <img src="docs/assets/bilterm-logo.png" alt="Bilterm Retro IBM 3270 x Borland Turbo Vision Logo" width="840" />
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-v1.12.0-blue" alt="Version" />
  <img src="https://img.shields.io/badge/rust-1.88%2B%20(2024)-orange" alt="Rust" />
  <img src="https://img.shields.io/badge/tests-240%2B%20passing-brightgreen" alt="Tests" />
  <img src="https://img.shields.io/badge/coverage-83.45%25-brightgreen" alt="Coverage" />
  <img src="https://img.shields.io/badge/license-Apache--2.0-cyan" alt="License" />
</p>

**Bilterm** is a terminal user interface (TUI) multiplexer, native desktop GUI terminal (`--gui`), composite-pane shell wrapper, remote SSH fleet manager, and AI-automatable terminal emulator written in Rust using [Ratatui](https://ratatui.rs/). Fusing classic **IBM 3270 Mainframe Terminals** (top-pinned floating command prompt over a scrolling PTY output canvas, Operator Information Area status bar, and full-screen cross-hair cursor) with **DOS Borland Turbo Vision** windowing (`╔═╗` double-line borders, character-cell drop shadows, Z-order window cascade decks, and function-key bars), Bilterm treats the **floating command prompt + underlying PTY output canvas as a single composite terminal pane**.

---

## Screen Renders & Visual Gallery

### 1. Multi-Pane Composite Workspace & SSH Fleet (`turbo-blue`)
<p align="center">
  <img src="docs/assets/screenshot-workspace.svg" alt="Bilterm Multi-Pane Composite Workspace & SSH Fleet (Borland Turbo Blue)" width="920" />
</p>

### 2. Per-Pane Independent Font Scaling (`120%` / `80%` `nmon` / `100%`) & 3-Way Split (`v1.12.0`)
<p align="center">
  <img src="docs/assets/screenshot-per-pane-scaling.svg" alt="Bilterm Per-Pane Independent Font Scaling & 3-Way Incident War Room" width="920" />
</p>

### 3. Multi-Agent AI Coding Swarm & Slide-Out Fleet Drawer (`cyberpunk-neon`)
<p align="center">
  <img src="docs/assets/screenshot-multi-agent-swarm.svg" alt="Bilterm 4-Pane Multi-Agent Coding Swarm & Slide-Out Fleet Drawer" width="920" />
</p>

### 4. IBM 3278 Phosphor Green CRT (`F9` Inspector) & IBM 3279 Amber CRT (`Ctrl+P` Command Palette)
| IBM 3278 Phosphor Green + `F9` 3270 Inspector | IBM 3279 Amber + Z-Order Cascade + `Ctrl+P` Palette |
| :---: | :---: |
| <img src="docs/assets/screenshot-3278-inspector.svg" alt="IBM 3278 Phosphor Green + F9 Inspector" width="450" /> | <img src="docs/assets/screenshot-3279-palette.svg" alt="IBM 3279 Amber + Z-Order Cascade + Command Palette" width="450" /> |

---

## Core Highlights

- **Composite Terminal Panes (`OutputCanvas` + `FloatingCommandPrompt` + `3270 OIA Bar`)**:
  - Pin the command prompt to **Top** (`Alt+T`), **Bottom** (`Alt+B`), **Free-Float** (`Alt+Up`/`Down` or mouse drag), or **Non-Overlapping Split** (`Alt+S` / `:prompt split-top | split-bottom`) with **zero `SIGWINCH` geometry thrashing**.
  - Flood-immune streaming (`tail -f /dev/random` at 60 FPS), `DECSET 1049` alternate-screen auto-hide (`vim`, `htop`, `nmon`), and 3-signal `!ECHO LOCK` password protection (`sudo`, `ssh`).
- **Native Desktop `--gui` Mode, Per-Pane Independent Font Scaling & All-Border Mouse Resizing (`v1.10.0`–`v1.12.0`)**:
  - Launch a standalone native desktop GUI window (`bilterm --gui --gui-font "JetBrains Mono" --gui-font-size 16`) on Linux (X11), Windows (Win32 GDI), and macOS (Cocoa).
  - Scale **each pane's font independently** (`Ctrl++`, `Ctrl+-`, `Ctrl+0`, `Ctrl+MouseWheel`, `:fontsize`, `:font`) from `50%` to `300%` — dynamically recalculating that pane's `cols × rows` PTY grid (`TIOCSWINSZ`) even while running interactive TUIs like `nmon`, `htop`, or `vim`.
  - Drag **any** pane border (`Top`, `Bottom`, `Left`, `Right`) or **any** corner (`TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`) with the mouse to resize tiled splits or overlapping Z-Stack windows.
- **Multi-Pane Tiling, Z-Order Window Deck & Guake Overlay**:
  - Binary H/V splits (`F2`/`Alt+H`, `Alt+V`), overlapping Borland Z-order cascade (`Alt+M`, `F6`/`Alt+Tab`, `Alt+1..9`), and a persistent Guake drop-down terminal (`F10` / `Alt+\``).
- **Retro Mainframe & Borland Aesthetics + Focal Spotlight & Lens Shaders**:
  - 10 palettes (`turbo-blue`, `turbo-cyan`, `ibm-3278-green`, `ibm-3279-amber`, `ibm-mono`, `cyberpunk-neon`, `solarized-dark`, `dracula`, `gruvbox-dark`, `tokyo-night`), 8 border outlines, 4 drop-shadow styles, CRT scanlines (`:crt`), pane/full-screen IBM 3270 cross-hairs (`Alt+X`), **Focal Spotlight** (`Alt+O` / `:spotlight`) & **Focal Zoom Magnifying Lens** (`Alt+Shift+O` / `:focalzoom`), plus **Kitty (`APC G`)** and **DEC Sixel (`DCS q`)** graphics.
- **First-Class Local & Remote `tmux` + `herdr`-Style SSH Fleet Management & Slide-Out Drawer**:
  - Spawn remote SSH composite panes (`:ssh <user@host> [port]`), attach to persistent remote `tmux -CC` sessions over SSH (`:ssh-tmux <user@host> [session]`), centrally fan out commands across local and remote nodes by `@tag` (`:remote add|list|exec|disconnect`), and inspect live session/window/pane telemetry in the 4-edge **`herdr`-Style Slide-Out Drawer** (`F7` / `Alt+Shift+D` / `<Leader> d` / `:drawer`).
- **Tektronix 4010/4014 Vector Storage-Tube Emulation, High-Res Pixel/GPU Canvas & Adaptive SSH Graphics**:
  - Decode 10-bit/12-bit `4096×3120` Tektronix 4010/4014 vectors (`\x1b[?38h` / `:canvas tek`), render a modern retained scene-graph via anti-aliased RGBA `PixelCanvasBuffer` (`:canvas hires`, `:canvas export <file.ppm|file.svg>`), adaptive SSH inline pixel protocols (`Kitty APC G`, `DEC Sixel DCS q`, `iTerm2 OSC 1337`), and `bilterm plot` CLI.
- **Full Robot / Agent Automation API, Native MCP Server & Playbooks**:
  - Drive 100% of Bilterm via JSON-RPC 2.0 (`bilterm --headless`, `bilterm ctl`), Model Context Protocol (`bilterm --mcp` with 11 agent tools), or declarative TOML playbooks (`bilterm run <file.toml>` / `bilterm --playbook <file.toml>`).

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

---

## Extended CLI & Native `--gui` Examples

```bash
# 1. Launch native desktop GUI window with custom TrueType/OpenType font & size
bilterm --gui --gui-font "JetBrains Mono" --gui-font-size 16
bilterm --gui --gui-font "IBM 3270" --gui-font-size 18 --theme ibm-3278-green
bilterm --gui --gui-font-file ~/.local/share/fonts/IosevkaTerm-Regular.ttf --gui-font-size 15

# 2. Launch with a starter-pack configuration or named persistent history session
bilterm --gui -c starter-packs/05-retro-crt-workstation/ibm-3279-amber.toml
bilterm --session prod-incident-2026

# 3. Run declarative TOML robot playbooks (Multi-Agent Swarm, Remote Fleet, QA, War Room)
bilterm run starter-packs/01-multi-agent-swarm/playbook.toml
bilterm run starter-packs/04-sysadmin-incident-war-room/playbook.toml

# 4. Run a headless JSON-RPC 2.0 daemon and drive it from the CLI or AI agents
bilterm --headless &
bilterm ctl workspace.status
bilterm ctl wm.command '{"command":":split v"}'
bilterm ctl pane.exec '{"pane_id":1,"command":"cargo test -q"}'
bilterm ctl pane.wait_for '{"pane_id":1,"pattern":"test result: ok","timeout_ms":30000}'

# 5. Plot high-DPI charts inline over SSH or export standalone SVG / PPM files
bilterm plot line 12,28,19,45,38,64,52,84
bilterm plot hires -b kitty
bilterm plot candlestick 10,22,8,18,18,30,15,28,28,40,22,38 -o /tmp/ohlc.svg
```

*(See **[Chapter 8: Extended CLI, Native `--gui` Mode & Typography Examples](docs/user-guide/08-extended-cli-and-gui-examples.md)** for dozens more CLI & GUI recipes).*

### Complete CLI Options Summary

| Command | Description |
|---------|-------------|
| `bilterm` | Launch interactive IBM 3270 × Borland Turbo Vision TUI multiplexer |
| `bilterm -G` / `--gui` | Launch standalone native desktop GUI window (X11 / Win32 GDI / Cocoa) |
| `bilterm --gui-font <NAME>` (`-F` / `--font`) | Set font family or preset (`"JetBrains Mono"`, `"IBM 3270"`, `"Fira Code"`, `"vga"`, etc.) |
| `bilterm --gui-font-size <PT>` (`--font-size`) | Set base font size in points (`6..=72`, default `16`) |
| `bilterm --gui-font-file <PATH>` (`--font-file`) | Load custom `.ttf` / `.otf` font file directly from disk |
| `bilterm -T` / `--theme <THEME>` | Override color palette (`turbo-blue`, `ibm-3278-green`, `ibm-3279-amber`, `cyberpunk-neon`, etc.) |
| `bilterm -m` / `--mcp` | Run native Model Context Protocol (MCP `2024-11-05`) server over `stdio` |
| `bilterm -d` / `--headless [-l <addr>]` | Run headless JSON-RPC daemon (default private `0600` Unix socket; loopback TCP via `-l 127.0.0.1:PORT`) |
| `bilterm -r` / `--allow-remote` | Allow listening on non-loopback TCP addresses (requires `-t` / `--token-file`) |
| `bilterm -t` / `--token-file <path>` | Path to `0600` file containing bearer token (or `BILTERM_TOKEN_FILE`) |
| `bilterm -S` / `--snapshot-dir <path>` | Directory for confined workspace snapshots (default: `$XDG_STATE_HOME/bilterm/snapshots`) |
| `bilterm ctl [-t <path>] [endpoint] <method> [json]` | Send JSON-RPC 2.0 command (`workspace.status`, `pane.exec`, `fleet.exec`, etc.) |
| `bilterm plot [kind] [csv] [-o <file>] [-b <mode>]` | Plot `hires`, `line`, `bar`, `scatter`, `candlestick`, or `heatmap` inline or export `.svg`/`.ppm` |
| `bilterm run <file.toml>` (`-p` / `--playbook`) | Execute declarative multi-step TOML robot playbook with assertions |
| `bilterm -g` / `--gui-dump <path.ppm\|.svg>` | Render workspace via `GuiWorkspaceRenderer` to a `.ppm` or `.svg` framebuffer |
| `bilterm -s` / `--session <name>` | Use named persistent command-history session |
| `bilterm -c` / `--config <file.toml>` | Load configuration from custom TOML file path |
| `bilterm -C` / `--completions <bash\|zsh\|fish>` | Print shell completion script to `stdout` |
| `bilterm -M` / `--man` | Print standalone `roff(7)` manual page (`docs/bilterm.1`) |
| `bilterm -v` / `--version` (`-V`) | Print version information (`bilterm 1.12.0`) |
| `bilterm -h` / `--help` | Print CLI help summary |

---

## Use Cases & Starter Packs (`starter-packs/`)

Bilterm includes 5 ready-to-run **[Starter Packs (`starter-packs/`)](starter-packs/README.md)** and detailed architectural guides in **[Chapter 9: Real-World Use Cases, Workflows & Architecture Ideas](docs/user-guide/09-use-cases-and-workflows.md)**:

1. **Multi-Agent AI Coding Swarms ([`starter-packs/01-multi-agent-swarm/`](starter-packs/01-multi-agent-swarm/))**: Orchestrate 4+ specialized coding agents (Architect, Builder, Verifier, Security Auditor) via `bilterm --mcp` or `bilterm ctl` while human operators supervise in real time and intervene via the floating prompt without `SIGWINCH` disruption.
2. **Remote Agents & Federated SSH Fleet Management ([`starter-packs/02-remote-dev-and-fleet/`](starter-packs/02-remote-dev-and-fleet/))**: Connect panes to remote hosts (`:ssh`), attach to persistent remote `tmux -CC` sessions (`:ssh-tmux`), and fan out commands across `@web`, `@db`, and `@gpu` node tags (`:remote exec @web ...`).
3. **Robot Scripting, Deterministic QA & Visual Regression ([`starter-packs/03-robot-scripting-and-qa/`](starter-packs/03-robot-scripting-and-qa/))**: Drive interactive TUI apps in CI/CD without `sleep` flakiness using `wait_for` regex assertions and deterministic `.ppm`/`.svg` frame dumps.
4. **Sysadmin Incident War Room & Per-Pane Font Scaling ([`starter-packs/04-sysadmin-incident-war-room/`](starter-packs/04-sysadmin-incident-war-room/))**: Combine a `125%` zoomed root diagnostic shell, an `80%` compact high-density `nmon`/`htop` telemetry pane, and a `100%` Read-Only Locked (`🔒 PROT`) log stream with `ERROR:`/`CRITICAL` highlight triggers and `.cast` audit recording.
5. **Retro Mainframe & DOS CRT Workstations ([`starter-packs/05-retro-crt-workstation/`](starter-packs/05-retro-crt-workstation/))**: Authentic IBM 3278 Green Phosphor, IBM 3279 Amber CRT, Borland Turbo Vision IDE, and Cyberpunk Neon workstation profiles.

---

## Security & Operational Best Practices

Bilterm is engineered with defense-in-depth across terminal I/O, password handling, filesystem storage, and network RPC automation (see **[Chapter 10: Security Architecture & Operational Best Practices](docs/user-guide/10-security-and-best-practices.md)** and **[`docs/purpose.md`](docs/purpose.md)**):

- **3-Signal `!ECHO LOCK` Password Protection**: Detects `termios !ECHO` (`sudo`, `ssh`, `gpg`) via `tcgetattr(master_fd)` + prompt regex heuristics, immediately hiding the floating command prompt (`♦ !ECHO LOCK`) and routing keystrokes directly to the PTY so secrets never touch the editor buffer or disk history.
- **`/dev/random` Binary Flood & Escape-Trap Sanitization**: `VtParser::sanitize_stream_traps` neutralizes rogue `DECSET 47/1047/1049` and `DECSTBM` scroll-region traps unless the child process is genuinely in raw TUI mode (`!ICANON && !ECHO`), while `tokio::sync::watch` frame publishing guarantees 60 FPS UI responsiveness under multi-GB/s floods.
- **Clipboard Paste Injection & Bracketed-Paste Breakout Guard**: Strips dangerous C0/DEL control bytes and intercepts unbracketed multi-line pastes or `\x1b[201~` breakout attempts with the **`[!] UNSAFE PASTE GUARD`** confirmation modal.
- **Private `0700`/`0600` Filesystem & Snapshot Confinement (`src/util/secure_fs.rs`)**: All runtime sockets, history logs, session TOMLs, `.cast` recordings, and `.ppm` snapshots use `0700` directories, atomic `0600` file writes (`O_NOFOLLOW`), and `validate_safe_name` path-traversal rejection.
- **Zero-Trust JSON-RPC & SSH Hardening**: Headless mode defaults to a private `0600` Unix socket with kernel `SO_PEERCRED` UID verification; TCP listeners default to loopback and require `--allow-remote` + a `0600` `--token-file` (`auth.login` handshake) for non-loopback binds; `:ssh` destinations reject leading `-` flags to prevent OpenSSH `ProxyCommand` injection.

---

## Essential Shortcuts & `:colon` Commands

> **Dual Leader Prefix (`Ctrl+B` & `Ctrl+X`):** Bilterm supports both **`Ctrl+B`** (`tmux`/`herdr` style) and **`Ctrl+X`** (Borland/DOS & macOS safe) out of the box (automatically falling back to `Ctrl+X` when nested inside an outer `$TMUX` session, and accepting held-`Ctrl` chords like `Ctrl+B Ctrl+V` or `Ctrl+X Ctrl+Z`).

| Action | Shortcut (`<L>` = `Ctrl+B` or `Ctrl+X`) | `:command` |
|--------|----------|------------|
| **Help / Command Palette / 3270 Inspector** | `F1` (`<L> ?`) / `Ctrl+P` (`<L> :` / `Alt+Space`) / `F9` (`<L> i`) | `:help` / `:palette` / `:inspector` |
| **Split Horizontal / Split Vertical / Zoom** | `F2` (`<L> "` / `<L> -` / `Alt+H`) / `<L> %` (`<L> \|` / `Alt+V`) / `F5` (`<L> z` / `Alt+Z`) | `:split h` / `:split v` / `:zoom` |
| **Per-Pane Font Zoom In / Out / Reset** | `Ctrl++` (`Ctrl+=`) / `Ctrl+-` (`Ctrl+_`) / `Ctrl+0` / `Ctrl+Wheel` | `:fontsize [+ \| - \| reset \| <pt> \| <pct>%]` / `:font` |
| **Resize Pane Frame / Equalize Splits** | Drag ANY pane border or corner / `Ctrl+Alt+Arrows` / `<L> =` | `:resize <left\|right\|up\|down\|reset>` / `:equalize` |
| **Toggle Tiled vs. Z-Stack / Shuffle Deck** | `<L> M` (`Alt+M`) / `F6` (`<L> n` / `<L> p` / `Alt+Tab`) / `<L> 1..9` (`Alt+1..9`) | `:layout stack\|tile` / `:deck next\|prev` / `:focus <N>` |
| **Guake Quick Terminal / Raw PTY Toggle** | `F10` (`Ctrl+\` / `Alt+\``) / `F12` (`Ctrl+G`) | `:quake` / `:raw` |
| **Cycle Floating / Fixed / Regular Terminal** | `F4` (`<L> e` / `Alt+E`) / Click OIA Bar | `:mode [float \| fixed \| terminal \| cycle]` |
| **Prompt Pin Top / Bottom / Non-Overlapping Split**| `<L> t` (`Alt+T`) / `<L> b` / `<L> s` (`Alt+S`) | `:prompt top \| bottom \| split-top \| split-bottom` |
| **3270 Field Hints / Vi Copy / Canvas Search** | `F3` (`<L> f`) / `<L> [` (`Alt+C`) / `<L> /` (`Alt+/`) | `:hints` / `:copy` / `:search <query>` |
| **Whole-Workspace or Pane `.cast` Recording** | `<L> R` / — | `:record start [path] \| workspace [path] \| pane [path] \| stop` |
| **Read-Only Lock / Multi-Pane Broadcast** | `<L> l` (`Alt+L`) / — | `:lock [on\|off]` / `:broadcast [on\|off]` |
| **Theme / Cross-Hairs / CRT Scanlines** | — / `<L> x` (`Alt+X`) / — | `:theme <name>` / `:crosshair full\|pane\|off` / `:crt` |
| **Focal Spotlight / Focal Zoom Lens / Resize** | `Alt+O` (`<L> L`) / `Alt+Shift+O` / `Alt+}` & `Alt+{` | `:spotlight [circle\|ellipse] [fuzzy\|pixel]` / `:focalzoom [1.25x..3x]` |
| **Slide-Out Drawer** | `F7` (`Alt+Shift+D` / `<L> d`) | `:drawer [toggle\|left\|right\|top\|bottom\|float\|push]` |
| **Tektronix 4010/4014 & Graphical Canvas** | `\x1b[?38h` / `OSC 1337;Canvas=...` | `:canvas [hires\|tek\|mode\|res\|plot\|image\|export\|clear]` |
| **SSH Pane / Remote `tmux -CC` over SSH** | — | `:ssh <user@host> [port]` / `:ssh-tmux <user@host> [sess]` |
| **`herdr` Fleet Register / List / Tag Exec** | — | `:remote add <name> <host> [@tags]` / `:remote list` / `:remote exec <@tag\|all> <cmd>` |

*(See **[Chapter 6: Complete CLI, Keyboard, Mouse & `:command` Reference](docs/user-guide/06-keyboard-mouse-and-command-reference.md)** for the exhaustive reference tables).*

---

## Documentation Hub

All detailed engineering guides, user manual chapters, starter packs, and architecture diagrams live in [`docs/`](docs/README.md) and [`starter-packs/`](starter-packs/README.md):

- **[Documentation Index (`docs/README.md`)](docs/README.md)**
  - **[Developer, Maintainer & AI Agent Reference (`docs/DEVELOPER_AND_AGENT_GUIDE.md`)](docs/DEVELOPER_AND_AGENT_GUIDE.md)** — Complete modular source map, crate stack, encapsulated state invariants, unified `Action` + declarative keymap architecture, and live inspectability/debugging guide (`F9` Inspector, `bilterm.log`, JSON-RPC/MCP).
  - **[Purpose & Security Threat Model (`docs/purpose.md`)](docs/purpose.md)** — IBM 3270 × Borland design rationale, `!ECHO LOCK` password safety, `/dev/random` flood immunity, and paste-injection guards.
  - **[System Architecture & Internals (`docs/architecture.md`)](docs/architecture.md)** — 3-pass Ratatui compositor, `GridSnapshot` + `VtWorkerHandle` seam, 3-signal line-discipline detector, JSON-RPC/MCP control plane, and SSH/`tmux -CC` fleet bridge.
  - **[Running, Shells, Tab Completion & Config (`docs/running.md`)](docs/running.md)** — Shell resolution (`$SHELL` / `%COMSPEC%`), dual-mode `<Tab>` completion internals, `config.toml` & `[remotes.<name>]` schema, and CLI reference.
  - **[Operational How-To & Recipes (`docs/how-to.md`)](docs/how-to.md)** — Step-by-step recipes for `--gui` mode, per-pane font scaling, split prompts, SSH/fleet fan-out, MCP agent integration, robot playbooks, and troubleshooting.
  - **[Visual Diagrams Catalog (`docs/diagrams/README.md`)](docs/diagrams/README.md)** — Terminal ASCII (`ascii_architecture.txt`), Mermaid (`.mmd`), and native draw.io XML (`.drawio`) diagrams.
  - **[Test Coverage & Battle Report Card (`docs/TEST_COVERAGE.md`)](docs/TEST_COVERAGE.md)** — `83.45%` instrumented line coverage across unit, API/MCP, integration, stress, performance, and `vttest`/`esctest` torture suites.
- **[10-Chapter User Guide (`docs/user-guide/README.md`)](docs/user-guide/README.md)**
  1. [Getting Started & Core Concepts](docs/user-guide/01-getting-started.md)
  2. [The Command Prompt, Tab Completion, History & Command Blocks](docs/user-guide/02-floating-command-prompt.md)
  3. [Multi-Pane Window Manager, Z-Order Stacking, Mouse Frame Resizing & Guake Quick Terminal](docs/user-guide/03-window-manager-and-z-stack.md)
  4. [Retro Mainframe & DOS Aesthetics, Outlines, Drop Shadows, Cross-Hairs & Config](docs/user-guide/04-themes-glyphs-and-config.md)
  5. [First-Class `tmux` Integration, SSH Domains & `herdr` Fleet Management](docs/user-guide/05-tmux-integration.md)
  6. [Complete CLI, Keyboard, Mouse & `:command` Reference](docs/user-guide/06-keyboard-mouse-and-command-reference.md)
  7. [Troubleshooting & FAQ](docs/user-guide/07-troubleshooting-and-faq.md)
  8. [Extended CLI, Native `--gui` Mode & Typography Examples](docs/user-guide/08-extended-cli-and-gui-examples.md)
  9. [Real-World Use Cases, Workflows & Architecture Ideas](docs/user-guide/09-use-cases-and-workflows.md)
  10. [Security Architecture & Operational Best Practices](docs/user-guide/10-security-and-best-practices.md)
- **[Starter Packs (`starter-packs/README.md`)](starter-packs/README.md)** — Ready-to-run configs, TOML playbooks, and launch scripts for Multi-Agent Swarms, Remote Fleets, Robot QA, Sysadmin War Rooms, and Retro CRT Workstations.
- **[Changelog (`CHANGELOG.md`)](CHANGELOG.md)** — Full semantic versioning release history (`v0.1.0` – `v1.12.0`).
- **[License (`LICENSE`)](LICENSE)** — Licensed under the Apache License, Version 2.0 (`Apache-2.0`).
