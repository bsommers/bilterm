# Roadmap: Bilterm

## Overview

Bilterm delivers a pure-TUI terminal multiplexer, shell wrapper, and window manager in Rust (`ratatui` + `crossterm` + `portable-pty`) built around the **Composite Window Pane** (`OutputCanvas` + `FloatingCommandPrompt` + `Modular OIA / Function-Key Status Bar`). The roadmap progresses bottom-up across five phases: first proving the single-pane full-height PTY canvas and floating overlay engine with line-discipline and alternate-screen safety; second completing intra-pane multi-line editing, triple-modality prompt positioning, and semantic command block navigation; third scaling to multi-pane H/V binary splits, z-order deck stacking/shuffling, a persistent Guake/Quake-style drop-down quick terminal, and wide-character-safe occlusion; fourth layering authentic IBM 3270 mainframe and DOS Borland Turbo Vision (`╔═╗`, drop shadows, CGA/VGA & phosphor themes, cross-hair cursors, Unicode/CP437/Nerd Font glyphs, modular status bars, and `config.toml`); and finally integrating first-class `tmux` coexistence/control mode, cross-platform parity (Linux, macOS, Windows), comprehensive test/snapshot verification, technical architecture documentation, and an extensive multi-chapter User Manual.

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Core PTY, VT100 Engine & Single CompositePane Prototype** - Single composite pane with full-height PTY output canvas, overlaid floating prompt, alternate-screen auto-hide, and safe password/REPL input routing (completed 2026-10-20)
- [x] **Phase 2: Floating Prompt Ergonomics, Triple-Modality Intra-Pane Controls & Command Blocks** - Multi-line syntax-highlighted floating editor, top/bottom/free-floating positioning via keyboard/mouse/command, clipboard integration, and OSC 133 command block jumping (completed 2026-10-20)
- [x] **Phase 3: Window Manager — H/V Composite Splits, Z-Order Stacking/Shuffling & Guake Drop-Down Terminal** - Binary split tree for H/V composite panes, z-order deck stacking/shuffling/selective raise, persistent Guake-style drop-down quick terminal, multi-pane mouse/keyboard controls, and wide-grapheme boundary occlusion (completed 2026-10-20)
- [x] **Phase 4: Retro Mainframe & DOS Aesthetics — IBM 3270 + Borland Turbo Vision Chrome, Modular Status Bars, Glyphs & Declarative Config** - Borland Turbo Vision window frames and drop shadows, IBM 3270 cross-hair cursors and OIA/F-key status bars, Unicode/CP437/Nerd Font glyph sets, and `config.toml` theme engine (completed 2026-10-20)
- [x] **Phase 5: First-Class `tmux` Integration, Cross-Platform Hardening, Test Verification & Documentation** - Outer `$TMUX` passthrough and collision avoidance, `tmux -CC` control mode session backend, Linux/macOS/Windows (`ConPTY`) parity, comprehensive test/snapshot coverage, architecture docs/ADRs, and extensive User Manual (completed 2026-10-20)

## Phase Details

### Phase 1: Core PTY, VT100 Engine & Single CompositePane Prototype

**Goal**: As a terminal user, I want to launch Bilterm into an interactive shell inside a full-height PTY output canvas with an overlaid floating command prompt, automatic `DECSET 47/1047/1049` alternate-screen prompt hiding, and safe `sudo`/REPL input routing, so that terminal output scrolls smoothly underneath the prompt without `SIGWINCH` geometry thrashing or password/TUI corruption.
**Depends on**: Nothing (first phase)
**Requirements**: PTY-01, PTY-02, PTY-03, PANE-01
**UI hint**: yes
**Success Criteria** (what must be TRUE):

  1. User can launch Bilterm into an interactive shell (`bash`, `zsh`, `fish`, or PowerShell/cmd) where VT100/ANSI/TrueColor output and scrollback span 100% of the pane height and scroll smoothly underneath the overlaid floating command prompt without resizing the child PTY (`SIGWINCH`) when the prompt moves or hides.
  2. Launching full-screen TUI applications (`vim`, `nvim`, `htop`, `less`, `fzf`) automatically hides the pane's floating command prompt via `DECSET 47/1047/1049` detection, routes raw keystrokes directly to the PTY, and restores the floating prompt on exit (or via manual toggle hotkey).
  3. Running `sudo`, `ssh`, or interactive sub-REPLs (`python`, `node`, `git add -p`) automatically switches input routing via three-signal detection (`OSC 133` + `tcgetpgrp` + `termios !ECHO`) so passwords are never echoed in cleartext and interactive prompts never deadlock.
  4. Unexpected panics or signals cleanly restore host terminal raw mode, mouse capture, and alternate screen state.

**Plans**: 3/3 plans complete

Plans:
**Wave 1**

- [x] 01-01-PLAN.md — Core Cargo workspace, panic/signal restoration guard, cross-platform PTY manager (`portable-pty`), and VT100/ANSI/TrueColor + DECSET 47/1047/1049 + OSC 133 state machine

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 01-02-PLAN.md — Three-signal line-discipline detector (`OSC 133` + `tcgetpgrp` + `termios !ECHO`), `CompositePane` zero-`SIGWINCH` geometry, and modal input router

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 01-03-PLAN.md — 3-pass Ratatui `CompositePaneWidget` compositor (`OutputCanvas` + `Clear` + `FloatingCommandPrompt` + `OiaStatusBar`), async Tokio `App` loop, and `TestBackend` + `insta` snapshot tests

### Phase 2: Floating Prompt Ergonomics, Triple-Modality Intra-Pane Controls & Command Blocks

**Goal**: As a terminal user, I want to edit multi-line syntax-highlighted commands, search fuzzy history, position or resize the floating prompt via keyboard/mouse/`:command`, use system clipboards, and jump between `OSC 133` command blocks, so that I can compose and navigate shell commands ergonomically inside any composite pane.
**Depends on**: Phase 1
**Requirements**: PTY-04, PANE-02, PANE-03, PANE-04, PANE-05
**UI hint**: yes
**Success Criteria** (what must be TRUE):

  1. User can pin the floating command prompt to the top of the pane (IBM 3270 mainframe default), pin it to the bottom, or position it at any free-floating vertical offset within the pane.
  2. User can move, pin, and resize the floating command prompt using keyboard shortcuts, mouse drag-and-drop on its border/titlebar, or in-prompt `:command` execution.
  3. User can edit multi-line commands with live shell syntax highlighting (`tree-sitter-bash`), search command history with fuzzy matching (`nucleo-matcher`), and copy/paste text seamlessly via bracketed paste (`DECSET 2004`) and `OSC 52` clipboard integration.
  4. User can jump directly between previous command invocations and their corresponding output blocks in scrollback using `OSC 133` semantic prompt markers and submission indexing.

**Plans**: 3 plans

Plans:
**Wave 1**

- [x] 02-01-PLAN.md — Multi-line command editor (`Vec<String>`), undo/redo stacks, live shell syntax highlighter (`src/pane/highlight.rs`), and `nucleo-matcher` fuzzy history search (`src/pane/fuzzy_history.rs`)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 02-02-PLAN.md — `DECSET 2004` bracketed paste, `OSC 52` clipboard codec, `OSC 133` command block scrollback jumping, `:command` parser (`src/pane/colon_command.rs`), and mouse drag/resize/pin routing

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 02-03-PLAN.md — `CompositePaneWidget` multi-line syntax-highlighted rendering, `[▲][▼]` border pin widgets, `Ctrl+R` fuzzy search UI, OIA `[BLK k/N]` badge, and Phase 2 `insta` snapshot tests

### Phase 3: Window Manager — H/V Composite Splits, Z-Order Stacking/Shuffling & Guake Drop-Down Terminal

**Goal**: As a terminal multiplexer user, I want to split composite panes horizontally or vertically, stack and shuffle panes in z-order or raise them via keyboard/mouse, and toggle a persistent Guake-style drop-down quick terminal with wide-character-safe occlusion, so that I can manage multiple tiled, stacked, and pop-over terminal workspaces without visual corruption.
**Depends on**: Phase 2
**Requirements**: WM-01, WM-02, WM-03, WM-04, WM-05
**UI hint**: yes
**Success Criteria** (what must be TRUE):

  1. User can split any composite pane horizontally or vertically, spawning a new independent `CompositePane` with its own PTY `OutputCanvas` and its own movable `FloatingCommandPrompt`.
  2. User can stack composite panes in z-order, rotate/shuffle the pane deck top-to-bottom or bottom-to-top, and selectively raise any pane to the front via keyboard shortcuts (`Alt+1`..`Alt+9` window numbers or picker) or mouse clicks.
  3. User can press a dedicated hotkey to toggle a persistent Guake/Quake-style drop-down quick terminal overlay for quick commands that preserves its live PTY session when hidden.
  4. User can move, resize, focus/raise, zoom/maximize, and close composite panes via keyboard chords, mouse dragging/clicking on window borders, or `:command` palette commands, while SGR (1006) mouse coordinates are accurately translated to the focused pane's local canvas coordinates.
  5. Overlapping floating prompts, stacked composite panes, drop-down overlays, and drop shadows sanitize 2-column wide Unicode/CJK/Emoji/Nerd Font graphemes at occlusion boundaries (`occlude_rect`) so multi-column characters never shift or corrupt borders.

**Plans**: 3 plans

Plans:
**Wave 1**

- [x] 03-01-PLAN.md — H/V Binary Split Tree (`src/wm/split_tree.rs`), Z-Order Deck Stack Manager (`src/wm/z_stack.rs`), and 2-column wide-grapheme occlusion sanitizer (`src/ui/occlusion.rs`)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 03-02-PLAN.md — Persistent Guake drop-down `QuickTerminal` (`src/wm/quick_terminal.rs`), multi-pane `WindowManager` (`src/wm/mod.rs`), and SGR 1006 local mouse/keyboard/`:command` routing

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 03-03-PLAN.md — `WorkspaceWidget` multi-pane compositor (`src/ui/mod.rs`), multi-pane `App` loop (`src/app.rs`), and Phase 3 `insta` snapshot tests (`tests/phase3_window_manager.rs`)

### Phase 4: Retro Mainframe & DOS Aesthetics — IBM 3270 + Borland Turbo Vision Chrome, Modular Status Bars, Glyphs & Declarative Config

**Goal**: As a retro-computing enthusiast, I want to customize Borland Turbo Vision window borders/shadows, CGA/VGA and IBM 3270 phosphor palettes, cross-hair cursors, Unicode/CP437/Nerd Font glyph sets, and modular OIA/F-key status bars via `config.toml`, so that Bilterm delivers an authentic mainframe and DOS IDE aesthetic tailored to my workflow.
**Depends on**: Phase 3
**Requirements**: UI-01, UI-02, UI-03, UI-04, UI-05, CFG-01
**UI hint**: yes
**Success Criteria** (what must be TRUE):

  1. Active composite panes and floating prompts render classic Borland Turbo Vision double-line borders (`╔═╗`) while inactive panes render single-line borders (`┌─┐`), complete with 2-col right / 1-row bottom translucent character-cell drop shadows, window number badges (`[1]`, `[2]`), and clickable border widgets (`[▪]` close, `[↕]` zoom/maximize, `◢` resize corner).
  2. User can switch between Borland Turbo C / Turbo Pascal color themes (Royal Blue, Cyan, Yellow, White CGA/EGA/VGA palettes), IBM 3270 Mainframe themes (3278 Phosphor Green, 3279 Amber, Monochrome), and custom user-defined palettes.
  3. User can customize modular status bars combining IBM 3270 Operator Information Area (OIA) indicators (`READY`, `X SYSTEM`, `INPUT: CMD BOX` vs `RAW PTY`, row/col coordinates, `[Stack: 1/N]`) and Borland DOS Function-Key bars (`F1 Help  F5 Zoom  F6 Next...`) with Unicode glyphs, dingbats, CP437 symbols, Powerline dividers, and Nerd Font icons that preserve strict cell-width alignment.
  4. User can enable the IBM 3270 full-pane cross-hair cursor ruler (non-destructive row/column tinting) and switch between solid block, blinking hollow block, underscore, and vertical bar carets without clashing with the host hardware cursor.
  5. User can configure default prompt position/height, keybindings, status bar segments, glyph packs, cursor styles, and color themes via `~/.config/bilterm/config.toml` (with INI-style theme file import support).

**Plans**: 3 plans

Plans:
**Wave 1**

- [x] 04-01-PLAN.md — Color theme engine (`src/ui/theme.rs`), Unicode/CP437/Nerd Font glyph registry (`src/ui/glyphs.rs`), and TOML/INI config parser (`src/config/mod.rs`)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 04-02-PLAN.md — Borland Turbo Vision chrome & translucent drop shadows (`src/ui/chrome.rs`), IBM 3270 cross-hair cursor ruler (`src/ui/cursor.rs`), and DOS F-Key bar (`src/ui/status_bar.rs`)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 04-03-PLAN.md — `WindowManager` + `WorkspaceWidget` retro aesthetic integration, runtime `:theme`/`:cursor`/`:crosshair` commands, and Phase 4 `insta` snapshot tests (`tests/phase4_retro_aesthetics.rs`)

### Phase 5: First-Class `tmux` Integration, Cross-Platform Hardening, Test Verification & Documentation

**Goal**: As a power user and developer, I want to run Bilterm seamlessly inside outer `tmux` sessions or attached via `tmux -CC` control mode across Linux, macOS, and Windows backed by comprehensive automated tests, architecture docs, and an extensive User Manual, so that my sessions persist reliably and every capability is verified and documented.
**Depends on**: Phase 4
**Requirements**: TMUX-01, TMUX-02, PLAT-01, QUAL-01, QUAL-02, QUAL-03
**UI hint**: yes
**Success Criteria** (what must be TRUE):

  1. When launched inside an outer `tmux` session (`$TMUX`), Bilterm automatically avoids prefix key collisions, wraps `OSC 52` clipboard sequences in `DCS tmux;` passthrough, and surfaces outer `tmux` session/window telemetry in the status bar.
  2. User can attach Bilterm composite panes to persistent `tmux` sessions/panes via `tmux` CLI and Control Mode (`tmux -CC`) synchronization.
  3. Bilterm compiles, passes automated verification, and runs interactive shells cleanly across Linux (primary target), macOS (POSIX PTY + slave FD/sysctl termios parity), and Windows (`ConPTY` pseudo-console support).
  4. Automated test suite (unit tests, Ratatui `TestBackend` + `insta` visual snapshot tests for overlays/borders/wide-chars, and headless PTY integration tests) passes with high coverage audited via `test-coverage` and `test-driven-development`.
  5. Complete technical architecture documentation, ADRs (`docs/architecture/`), and an extensive multi-chapter User Manual (`docs/user-guide/`) are published and synchronized with all v1 capabilities.

**Plans**: 3 plans

Plans:
**Wave 1**

- [x] 05-01-PLAN.md — Outer `$TMUX` detection, `DCS tmux;` passthrough wrapper, and `tmux -CC` control mode parser/bridge (`src/tmux/mod.rs`)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 05-02-PLAN.md — Cross-platform Linux/macOS/Windows (`ConPTY`) shell hardening (`src/pty/mod.rs`) and end-to-end test suite (`tests/phase5_tmux_and_platform.rs`)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 05-03-PLAN.md — Technical architecture documentation & ADRs (`docs/architecture/`) and extensive multi-chapter User Manual (`docs/user-guide/`, `README.md`)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Core PTY, VT100 Engine & Single CompositePane Prototype | 3/3 | Complete    | 2026-10-20 |
| 2. Floating Prompt Ergonomics, Triple-Modality Intra-Pane Controls & Command Blocks | 3/3 | Complete    | 2026-10-20 |
| 3. Window Manager — H/V Composite Splits, Z-Order Stacking/Shuffling & Guake Drop-Down Terminal | 3/3 | Complete    | 2026-10-20 |
| 4. Retro Mainframe & DOS Aesthetics — IBM 3270 + Borland Turbo Vision Chrome, Modular Status Bars, Glyphs & Declarative Config | 3/3 | Complete    | 2026-10-20 |
| 5. First-Class `tmux` Integration, Cross-Platform Hardening, Test Verification & Documentation | 3/3 | Complete    | 2026-10-20 |
| 6. `v0.2.0` Protocol Hardening, Shell Integration & Canvas Workflows (`FND-01..06`, `VAL-01..04,06`) | Complete | Complete | 2026-10-21 |
| 7. `v0.2.5` Optimal Tech Stack Blueprint Seam (`GridSnapshot`, `VtWorkerHandle`, `WorkspaceFrameRenderer`, Reflow & Torture Suite) | Complete | Complete | 2026-10-21 |
| 8. `v0.3.0` IBM 3270 Field Hints, Terminal Inspector & Multi-Session Workspaces (`BEL-01..03`, `FND-07`, `ECO-01..04`) | Complete | Complete | 2026-10-21 |
| 9. `v0.3.5` Living Documentation & User Guide Synchronization | Complete | Complete | 2026-10-21 |
| 10. `v0.4.0` Native GPU Windowing Renderer (`GuiWorkspaceRenderer`) & `libghostty-vt` C-ABI Vtable (`GPU-01`, `GHOSTTY-01`) | Complete | Complete | 2026-10-21 |
| 11. `v0.5.0` Bi-Directional `tmux -CC` Session Daemon & Kitty/Sixel Graphics (`TMUX-03`, `GFX-01`) | Complete | Complete | 2026-10-21 |
| 12. `v0.6.0` Fuzzy Command Palette (`Ctrl+P`), Custom Keybinding Remapping & CRT Scanline Shader (`CMD-01`, `CFG-02`, `CRT-01`) | Complete | Complete | 2026-10-21 |
| 13. `v0.7.0` `OSC 9`/`777` Notifications, Output Highlight Triggers & Asciicast v2 `.cast` Recorder (`NTF-01`, `TRG-01`, `REC-01`) | Complete | Complete | 2026-10-21 |
| 14. `v1.0.0` Production Release Packaging (`--completions`, `--man`, `docs/bilterm.1`, `flake.nix` & Full Docs Sync) (`PKG-01`) | Complete | Complete | 2026-10-21 |
| 15. `v1.1.0` JSON-RPC 2.0 Automation Control Plane, `--headless` Daemon & `bilterm ctl` CLI (`AUTO-01`) | Complete | Complete | 2026-10-21 |
| 16. `v1.2.0` Native Model Context Protocol (`bilterm --mcp`) Server for AI Agents (`AUTO-02`) | Complete | Complete | 2026-10-21 |
| 17. `v1.3.0` Structured Automation Event Subscription Bus & Declarative Robot Playbooks (`--playbook`) (`AUTO-03`) | Complete | Complete | 2026-10-21 |
| 18. `v1.4.0` First-Class SSH Domains (`:ssh`), Remote Persistent `tmux -CC` (`:ssh-tmux`) & `herdr`-Style Central Fleet Federation (`REM-01`) | Complete | Complete | 2026-10-21 |
