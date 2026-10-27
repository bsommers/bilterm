# Bilterm User Guide (`v1.12.0`)

[← Back to Project README](../../README.md) | [Starter Packs (`starter-packs/`)](../../starter-packs/README.md)

Welcome to the official User Guide for **Bilterm (`v1.12.0`)** — the Rust + Ratatui terminal multiplexer, native desktop GUI terminal (`--gui`), composite-pane shell wrapper, remote SSH fleet manager, and AI-automatable terminal emulator inspired by **IBM 3270 Mainframe Terminals** and **DOS Borland Turbo Vision (Turbo C / Turbo Pascal IDE)**.

> **Quick Interactive Help (`<F1>`)**: At any time inside Bilterm, press **`F1`** or run **`:help`** to open the scrollable Borland Turbo Vision Help Modal directly on screen (`Up`/`Down`/`PageUp`/`PageDown` to scroll, `F1`/`Esc` to close).

![Bilterm Multi-Pane Composite Workspace & SSH Fleet](../assets/screenshot-workspace.svg)

---

## Table of Contents

- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
  - [Two-Minute First Run (TUI & Native `--gui`)](#two-minute-first-run-tui--native---gui)
- [10-Chapter User Guide Index](#10-chapter-user-guide-index)
- [Screen Renders & Visual Gallery](#screen-renders--visual-gallery)
- [Starter Packs (`starter-packs/`)](#starter-packs-starter-packs)
- [Interactive Examples](#interactive-examples)
  - [Example 1: Native Desktop GUI with Custom Fonts & Per-Pane Scaling (`Ctrl++` / `Ctrl+-`)](#example-1-native-desktop-gui-with-custom-fonts--per-pane-scaling-ctrl--ctrl-)
  - [Example 2: Floating Prompt Over a High-Throughput Stream (`tail -f /dev/random`)](#example-2-floating-prompt-over-a-high-throughput-stream-tail--f-devrandom)
  - [Example 3: Switching Between Floating Mode and Non-Overlapping Split Mode](#example-3-switching-between-floating-mode-and-non-overlapping-split-mode)
  - [Example 4: Splitting Panes, All-Border Mouse Drag Resizing, Z-Order Stacking & Deck Shuffling](#example-4-splitting-panes-all-border-mouse-drag-resizing-z-order-stacking--deck-shuffling)
  - [Example 5: Retro Themes, Border Outlines, DOS Shadows & Full-Screen Cross-Hairs](#example-5-retro-themes-border-outlines-dos-shadows--full-screen-cross-hairs)
- [Reference](#reference)
  - [Configuration Schema (`~/.config/bilterm/config.toml`)](#configuration-schema-configbiltermconfigtoml)
  - [Environment Variables](#environment-variables)
  - [Files & Runtime Paths](#files--runtime-paths)
  - [Exit Codes](#exit-codes)
- [Troubleshooting & FAQ](#troubleshooting--faq)

---

## Getting Started

### Prerequisites

- **Rust Toolchain**: `rustc` and `cargo` **`1.88+`** (Rust 2024 Edition).
- **Host Operating System**: Linux (primary POSIX PTY + native X11 `--gui`), macOS (POSIX PTY + Cocoa `--gui`), or Windows (`ConPTY` + Win32 GDI `--gui`).
- **Interactive Shell**: Any POSIX shell (`bash`, `zsh`, `fish`, `sh` via `$SHELL`) on Linux/macOS, or `powershell.exe` / `cmd.exe` (`%COMSPEC%`) on Windows.
- **Optional**: `tmux` `3.2+` for nested `DCS tmux;` clipboard passthrough or `tmux -CC` Control Mode integration.

### Installation

```bash
git clone https://github.com/bsommers/bilterm.git
cd bilterm
./scripts/install.sh
```

### Two-Minute First Run (TUI & Native `--gui`)

Launch Bilterm inside your existing terminal or as a standalone native desktop GUI window:

```bash
# Launch inside your current terminal:
bilterm

# Or launch a native desktop GUI window with JetBrains Mono at 16pt:
bilterm --gui --gui-font "JetBrains Mono" --gui-font-size 16
```

**Expected Initial Screen State:**
1. **Top Floating Command Prompt (`╔═╣ BILTERM 3270 ... ╠═╗`)**: Focused at the top of Composite Pane `#1` with the `==> ` input prompt.
2. **Underlying `OutputCanvas`**: Your interactive `$SHELL` running inside a full-height VT100/TrueColor PTY canvas underneath the floating prompt.
3. **Bottom IBM 3270 OIA Status Bar**: Displays `3270-OIA`, `● READY`, `INPUT: CMD BOX`, `PIN:TOP`, `≣ OUT:0L/0B`, and live cursor coordinates `⌖ R:01 C:001`.

Try running your first command inside the floating prompt:

```bash
ls -la
```

Press **`Enter`** — the command is sent to the underlying shell, the `OutputCanvas` scrolls smoothly underneath the floating prompt, and the top of the prompt displays `● LAST CMD:  ls -la`. Press **`F1`** at any time to view all keyboard shortcuts, or type **`:quit`** (or press **`Ctrl+Q`**) to exit cleanly.

---

## 10-Chapter User Guide Index

For deep dives into each subsystem, browse the dedicated chapters in [`docs/user-guide/`](.):

1. **[Chapter 1: Getting Started & Core Concepts](01-getting-started.md)**
   - Building & launching (TUI and `--gui`), Composite Window Pane architecture (`OutputCanvas` + `FloatingCommandPrompt` + `OIA Status Bar`), per-pane independent font scaling (`v1.12.0`), all-border mouse resizing, Alternate Screen (`DECSET 1049`) auto-hide, and `!ECHO LOCK` password protection.
2. **[Chapter 2: The Command Prompt, Tab Completion, History & Command Blocks](02-floating-command-prompt.md)**
   - Top/Bottom/Floating/Split positioning (`Alt+T`, `Alt+B`, `Alt+S`), resizable width (`Alt+[`/`Alt+]`, mouse border drag) & height (`Alt+Plus`/`Alt+Minus`), scrollable multi-line editing (`Shift+PageUp`/`Shift+PageDown`), running command banner (`⟳ RUNNING:`), Tab completion (`Tab`/`Shift+Tab`), Borland Command History Pop-up (`Alt+P` / `:history`), session persistence, and `OSC 133` block jumping.
3. **[Chapter 3: Multi-Pane Window Manager, Z-Order Stacking, Mouse Frame Resizing & Guake Quick Terminal](03-window-manager-and-z-stack.md)**
   - Horizontal/Vertical binary splits (`F2`/`Alt+H`, `Alt+V`), all-border & all-corner mouse frame drag resizing (`v1.12.0`), independent per-pane font scaling (`Ctrl++`/`Ctrl+-`), Borland Turbo Vision overlapping Z-order stack (`Alt+M`), deck shuffling (`F6`/`Alt+Tab`), selective raise (`Alt+1..9`, mouse click), Slide-Out Drawer (`F7` / `:drawer`), and the persistent Guake/Quake drop-down terminal (`F10` / `Alt+\`` / `:quake`).
4. **[Chapter 4: Retro Mainframe & DOS Aesthetics, Outlines, Drop Shadows, Cross-Hairs & Config](04-themes-glyphs-and-config.md)**
   - 8 border outline styles (`:outline`), 4 DOS drop-shadow modes (`:shadows`), 10 retro & modern palettes (`:theme`), 4 glyph packs (`:glyphs`), pane-local vs. full-screen IBM 3270 cross-hairs (`Alt+X` / `:crosshair`), OIA status bar telemetry, and `config.toml` / `.ini` configuration.
5. **[Chapter 5: First-Class `tmux` Integration, SSH Domains & `herdr` Fleet Management](05-tmux-integration.md)**
   - Outer `$TMUX` coexistence (`DCS tmux;` passthrough for `OSC 52` clipboard), `tmux -CC` Control Mode synchronization, `:ssh` composite panes, `:ssh-tmux` persistent sessions, and `:remote` fleet federation.
6. **[Chapter 6: Complete CLI, Keyboard, Mouse & `:command` Reference](06-keyboard-mouse-and-command-reference.md)**
   - Exhaustive tables of all CLI flags and subcommands, function keys (`F1`–`F12`), `Ctrl+B`/`Ctrl+X` leader chords, `Alt+` chords, per-pane font scaling shortcuts, mouse drag/click/scroll gestures, and every `:colon` command across all subsystems.
7. **[Chapter 7: Troubleshooting & FAQ](07-troubleshooting-and-faq.md)**
   - Diagnostic log inspection (`bilterm.log`), per-pane font zoom inside `nmon`/`htop`, multi-pane mouse resizing, terminal recovery, password prompt routing, CJK/Emoji border alignment, and common questions.
8. **[Chapter 8: Extended CLI, Native `--gui` Mode & Typography Examples](08-extended-cli-and-gui-examples.md)**
   - Copy-pasteable CLI recipes for `--gui --gui-font "..." --gui-font-size 16`, per-pane font scaling (`Ctrl++`/`Ctrl+-`), headless JSON-RPC daemons (`--headless`, `bilterm ctl`), MCP AI server (`--mcp`), TOML playbooks (`bilterm run`), and high-DPI charts (`bilterm plot`).
9. **[Chapter 9: Real-World Use Cases, Workflows & Architecture Ideas](09-use-cases-and-workflows.md)**
   - 9 comprehensive architectures & workflows: Multi-Agent AI Coding Swarms, Remote Agents over SSH, Remote Development (`:ssh-tmux`), Robot Scripting & Deterministic QA, CI/CD Verification Harnesses, Daily Power-User Operations, Polyglot Developer Layouts, Live Escape-Sequence Debugging (`F9`), and Custom Per-Pane Font Layouts (`nmon`).
10. **[Chapter 10: Security Architecture & Operational Best Practices](10-security-and-best-practices.md)**
    - Defense-in-depth engineering reference covering 3-Signal `!ECHO LOCK`, `/dev/random` flood & escape-trap sanitization, clipboard paste-injection guards, private `0700`/`0600` filesystem confinement, Unix socket `SO_PEERCRED` + bearer-token RPC security, and SSH argument-injection prevention.

---

## Screen Renders & Visual Gallery

### 1. Per-Pane Independent Font Scaling & 3-Way Sysadmin Incident War Room (`v1.12.0`)
![Bilterm Per-Pane Independent Font Scaling & 3-Way Split](../assets/screenshot-per-pane-scaling.svg)

### 2. Multi-Agent AI Coding Swarm & Slide-Out Fleet Drawer (`cyberpunk-neon`)
![Bilterm Multi-Agent AI Coding Swarm](../assets/screenshot-multi-agent-swarm.svg)

### 3. IBM 3278 Phosphor Green (`F9` Inspector) & IBM 3279 Amber (`Ctrl+P` Command Palette)
| IBM 3278 Phosphor Green + `F9` 3270 Inspector | IBM 3279 Amber + Z-Order Cascade + `Ctrl+P` Palette |
| :---: | :---: |
| ![IBM 3278 Phosphor Green + F9 Inspector](../assets/screenshot-3278-inspector.svg) | ![IBM 3279 Amber + Z-Order Cascade + Command Palette](../assets/screenshot-3279-palette.svg) |

---

## Starter Packs (`starter-packs/`)

Ready-to-run configuration profiles, automation playbooks, and launch scripts live in [`starter-packs/`](../../starter-packs/README.md):

| Starter Pack | Purpose | Quick Launch |
|--------------|---------|--------------|
| **[`01-multi-agent-swarm`](../../starter-packs/01-multi-agent-swarm/)** | 4-pane AI Coding Swarm (Architect, Builder, Verifier, Security Auditor) | `bilterm run starter-packs/01-multi-agent-swarm/playbook.toml` |
| **[`02-remote-dev-and-fleet`](../../starter-packs/02-remote-dev-and-fleet/)** | Remote SSH Development & `herdr` `@tag` Fleet Orchestration | `bilterm run starter-packs/02-remote-dev-and-fleet/playbook.toml` |
| **[`03-robot-scripting-and-qa`](../../starter-packs/03-robot-scripting-and-qa/)** | Deterministic TUI QA Testing, JSON-RPC Harness & Visual `.ppm` Diffs | `starter-packs/03-robot-scripting-and-qa/ci-harness.sh` |
| **[`04-sysadmin-incident-war-room`](../../starter-packs/04-sysadmin-incident-war-room/)** | 3-Way Incident War Room with Per-Pane Font Scaling (`125%` / `80%` / `100%`) | `bilterm run starter-packs/04-sysadmin-incident-war-room/playbook.toml` |
| **[`05-retro-crt-workstation`](../../starter-packs/05-retro-crt-workstation/)** | Authentic IBM 3278 Green, IBM 3279 Amber, Borland Turbo Vision & Cyberpunk themes | `bilterm --gui -c starter-packs/05-retro-crt-workstation/ibm-3278-green.toml` |

---

## Interactive Examples

### Example 1: Native Desktop GUI with Custom Fonts & Per-Pane Scaling (`Ctrl++` / `Ctrl+-`)

Launch Bilterm's native GUI window with JetBrains Mono at 16pt, create a 3-way split, and scale each pane's font independently:

```bash
bilterm --gui --gui-font "JetBrains Mono" --gui-font-size 16
```

Inside Bilterm:
```text
==> :split v
==> :split h
==> :focus 1
==> :fontsize 120%
==> :focus 2
==> :fontsize 80%
==> nmon
```
- Pane `#1` (left) renders at `120%` (`19pt`) for comfortable reading.
- Pane `#2` (top-right, running `nmon`) renders at `80%` (`13pt`) so `nmon` receives a wider/taller character grid (`cols × rows`) to display all CPU/disk/network meters without clipping.
- Press **`Ctrl++`**, **`Ctrl+-`**, or **`Ctrl+0`** (or hold **`Ctrl` + Mouse Wheel**) inside any pane — even while `nmon` is active — to adjust its font scale live!

### Example 2: Floating Prompt Over a High-Throughput Stream (`tail -f /dev/random`)

```text
==> base64 /dev/urandom | head -n 500
```

- The top banner of the command prompt shows `⟳ RUNNING:  base64 /dev/urandom | head -n 500` while streaming and `● LAST CMD:  base64 /dev/urandom | head -n 500` afterwards.
- Even if you stream raw binary (`tail -f /dev/random`), Bilterm's `VtParser::sanitize_stream_traps` neutralizes accidental `DECSET 47/1047/1049` or `DECSTBM` scroll-region bytes when the foreground process is not in raw TUI mode.

### Example 3: Switching Between Floating Mode and Non-Overlapping Split Mode

```text
==> :prompt split-top
```

*(Or press **`Alt+S`**, or run `:prompt split-bottom` to place the non-overlapping command pane at the bottom).*

### Example 4: Splitting Panes, All-Border Mouse Drag Resizing, Z-Order Stacking & Deck Shuffling

```text
==> :split v
==> :split h
==> :layout stack
==> :deck next
==> :focus 1
```

- Drag **any** pane border (`Top`, `Bottom`, `Left`, `Right`) or **any** corner (`TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`) with the mouse to resize panes in either `Tiled` or `Stacked` mode.

### Example 5: Retro Themes, Border Outlines, DOS Shadows & Full-Screen Cross-Hairs

```text
==> :theme 3279
==> :outline window double
==> :outline prompt rounded
==> :shadows tinted
==> :crosshair full
==> :fkeybar on
```

---

## Reference

### Configuration Schema (`~/.config/bilterm/config.toml`)

Bilterm loads `~/.config/bilterm/config.toml` (or `$XDG_CONFIG_HOME/bilterm/config.toml`). Every key is optional and falls back to `BiltermConfig::default()`:

| Key | Type | Allowed Values | Default | Description |
|-----|------|----------------|---------|-------------|
| `theme` | `string` | `"turbo-blue"`, `"turbo-cyan"`, `"ibm-3278-green"`, `"ibm-3279-amber"`, `"ibm-mono"`, `"cyberpunk-neon"`, `"solarized-dark"`, `"dracula"`, `"gruvbox-dark"`, `"tokyo-night"` | `"turbo-blue"` | Active color theme preset (`UI-02`) |
| `glyphs` | `string` | `"unicode"`, `"cp437"`, `"nerd-font"`, `"ascii"` | `"unicode"` | Status bar and badge glyph pack (`UI-04`) |
| `cursor_style` | `string` | `"solid-block"`, `"blinking-hollow-block"`, `"underscore"`, `"vertical-bar"` | `"solid-block"` | Software cursor caret shape (`UI-05`) |
| `window_outline` | `string` | `"double"`, `"solid"`, `"rounded"`, `"thick"`, `"dotted"`, `"dashed"`, `"quadruple"`, `"ascii"` | `"double"` | Border outline style for workspace windows |
| `prompt_outline` | `string` | `"double"`, `"solid"`, `"rounded"`, `"thick"`, `"dotted"`, `"dashed"`, `"quadruple"`, `"ascii"` | `"double"` | Border outline style for the command prompt |
| `shadow_style` | `string` | `"none"`, `"translucent"`, `"theme-tinted"`, `"solid-half-block"` | `"translucent"` | DOS drop-shadow rendering mode (`UI-01`) |
| `drop_shadows` | `bool` | `true`, `false` | `false` | Enables 2-col right / 1-row bottom drop shadows |
| `crosshair_cursor` | `bool` | `true`, `false` | `false` | Enables the IBM 3270 cross-hair cursor ruler (`UI-05`) |
| `crosshair_fullscreen` | `bool` | `true`, `false` | `false` | Spans cross-hair across full screen when `true` (pane-local when `false`) |
| `show_dos_fkey_bar` | `bool` | `true`, `false` | `false` | Shows bottom 1-row Borland DOS `F1 Help ...` bar (`UI-03`) |
| `history_enabled` | `bool` | `true`, `false` | `true` | Automatically persists submitted commands to session log files |
| `history_max_files` | `usize` | `1..=10000` | `25` | Max named session directories retained (least-recently-used pruned, never the active one) |
| `history_session` | `string` | Session name | `"default"` | Named persistent history session (`--session <name>` overrides) |
| `history_max_entries` | `usize` | `1..` | `100000` | Entries kept per session file before compaction |
| `history_load_limit` | `usize` | `0..` | `10000` | Prior session commands restored into each new pane |
| `history_dir` | `string` (optional) | Any directory path | `None` | Custom directory override for persisted session history logs |
| `default_prompt_anchor` | `string` | `"top"`, `"bottom"`, `"floating"`, `"split-top"`, `"split-bottom"` | `"top"` | Initial command prompt placement for new panes |
| `default_prompt_height` | `u16` | `3..=12` | `3` | Initial command prompt box height in rows |
| `show_menu_bar` | `bool` | `true`, `false` | `false` | Shows top 1-row Borland Turbo Vision pull-down menu bar (`ECO-03`) |
| `crt_scanlines` | `bool` | `true`, `false` | `false` | Enables alternating scanline luminance modulation (`CRT-01`) |
| `triggers` | `array of strings` | Case-insensitive substring patterns | `[]` | Highlights matching tokens on the `OutputCanvas` (`TRG-01`) |
| `[keybindings]` | `table` | `"<KeyChord>" = ":<command>"` | `{}` | Custom user keybinding remapping table (`CFG-02`) |
| `[remotes.<name>]` | `table` | `destination`, `port`, `identity_file`, `tmux_session`, `rpc_endpoint`, `jump_host` | `{}` | Named SSH / `herdr` remote fleet host profiles |

### Environment Variables

| Variable | Effect in Bilterm |
|----------|-------------------|
| `SHELL` | Default interactive shell spawned on Linux and macOS (falls back to `/bin/sh`) |
| `COMSPEC` | Default interactive shell spawned on Windows when `powershell.exe` is not explicitly configured |
| `TMUX` | Detected automatically (`TmuxEnv::detect_from_env`) to enable outer `tmux` coexistence and `DCS tmux;` passthrough wrapping |
| `BILTERM_TOKEN_FILE` | Path to a mode `0600` bearer token file for JSON-RPC 2.0 authentication |
| `XDG_CONFIG_HOME` | Base directory for `bilterm/config.toml` (falls back to `~/.config/bilterm/config.toml`) |
| `XDG_STATE_HOME` | Base directory for persisted command history logs and workspace snapshots (falls back to `~/.local/state/bilterm/`) |
| `XDG_RUNTIME_DIR` | Base directory for private `0700` Unix RPC socket, shell hooks, exports, and `.cast` recordings |
| `RUST_LOG` | Controls diagnostic log verbosity written to `./bilterm.log` (e.g. `RUST_LOG=bilterm=debug`) |

### Files & Runtime Paths

| Path | Mode | Purpose |
|------|------|---------|
| `~/.config/bilterm/config.toml` | `0600` | Declarative user configuration file |
| `~/.local/state/bilterm/history/sessions/<name>/history.log` | `0600` | Append-only history of a named session (all panes and runs, `run=` segments) |
| `~/.local/state/bilterm/sessions/<name>.toml` | `0600` | Saved multi-pane workspace session profiles (`:session save`) |
| `~/.local/state/bilterm/snapshots/<name>.ppm` | `0600` | Confined directory for RPC/MCP `workspace.snapshot` outputs |
| `$XDG_RUNTIME_DIR/bilterm/bilterm.sock` | `0600` | Default private Unix domain socket for `bilterm --headless` and `bilterm ctl` |
| `$XDG_RUNTIME_DIR/bilterm/exports/*.sh` | `0600` | Default output path when exporting commands via `:history save` |
| `$XDG_RUNTIME_DIR/bilterm/recordings/*.cast` | `0600` | Default output path for `:record` Asciicast v2 recordings |
| `./bilterm.log` | `0644` | Non-blocking background diagnostic log written by `tracing-appender` |

### Exit Codes

| Exit Code | Meaning |
|-----------|---------|
| `0` | Clean exit via `:quit` / `:q`, `Ctrl+Q`, playbook completion, or closing the last composite pane |
| `1` | Unrecoverable initialization, playbook assertion failure, or runtime I/O error (terminal state is automatically restored via `TerminalRestoreGuard` before printing the `color-eyre` report) |

---

## Troubleshooting & FAQ

See **[Chapter 7: Troubleshooting & FAQ](07-troubleshooting-and-faq.md)** for full diagnostics, **[Chapter 8: Extended CLI & GUI Examples](08-extended-cli-and-gui-examples.md)** for CLI/GUI recipes, **[Chapter 9: Use Cases & Workflows](09-use-cases-and-workflows.md)** for multi-agent and remote architectures, and **[Chapter 10: Security & Best Practices](10-security-and-best-practices.md)** for security hardening.
