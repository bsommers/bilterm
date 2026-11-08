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
- [x] **Phase 19: Security Hardening of the Automation & Shell-Integration Surfaces** - `0600` Unix socket default, bearer token gate for remote TCP, confined snapshot paths, `0700`/`0600` private temp files, and hardened SSH argv builders (completed 2026-10-23)
- [x] **Phase 20: Mega-Module Decomposition** - Split `vt/`, `pane/`, `input/`, `api/`, `ui/`, and `wm/` along functional seams (`<= 800` lines per file, no `pane ↔ wm` cycle) (completed 2026-10-23)
- [x] **Phase 21: Encapsulated Workspace Invariants** - Private `WindowManager` and `CompositePane` fields, unified `remove_pane`/`submit_command`, `enum Modal`, and `enum Focus` (completed 2026-10-24)
- [x] **Phase 22: Unified `Action` Command Vocabulary, Data-Driven Keymap & Whole-Workspace `.cast` Recorder** - Typed `Action` vocabulary, `[keybindings]` data table, `Ctrl+B`/`Ctrl+X` dual leader (`$TMUX` auto-fallback, held-`Ctrl` chord normalization, `asciinema` PTY-wrapper harmony), `tmux`/`herdr` chord parity, and whole-workspace Asciicast v2 (`.cast`) compositor recorder (`ARCH-R01`, `REC-02`) (completed 2026-10-24)
- [x] **Phase 23: `TerminalBackend` Trait & Unwired-Subsystem Resolution** - Decouple `vt100_ctt` behind `TerminalBackend`, resolve `VtWorkerHandle`/`TmuxControlSubprocess`/`ghostty-vt`, and CI feature matrix (completed 2026-10-24)
- [x] **Phase 24: `herdr`-Style Slide-Out Sessions/Windows/Panes Drawer with Status Glyphs & Float/Push Modes** - 4-edge (`Left`/`Right`/`Top`/`Bottom`) slide-out drawer with `OnDemand`/`Fixed`/`AutoHide` policies, `Overlay` (float) vs `Push` (auto-resize) layout modes, and live telemetry glyphs (`DRW-01`, `DRW-02`) (completed 2026-10-24)
- [x] **Phase 25: Dynamic Terminal Text & Font Size Scaling** - Runtime `Ctrl++`/`Ctrl+-`/`Ctrl+0`/`:fontsize` scaling across `GuiWorkspaceRenderer` pixel/SVG cells, DEC double-width/height (`DECDWL`/`DECDHL`) TUI canvas lines, and host-terminal font-zoom OSC passthrough (`FONT-01`) (completed 2026-10-24)
- [x] **Phase 26: Tektronix 4010/4014 × Modern 2026/2027 Vector, Sprite, Plotting & Image Graphical Canvas** - Classic Tektronix 4010/4014 4096×3120 vector mode (`\x1b[?38h`) with phosphor decay plus modern retained scene-graph, 2D sprites, declarative plotting, and image viewer over Braille/Sextant/Quadrant TUI & full-res GUI rasterizers (`TEK-01`, `CANV-01`) (completed 2026-10-24)
- [x] **Phase 27: High-Resolution Anti-Aliased Pixel Canvas & Adaptive SSH/GUI Graphics Engine** - Anti-aliased RGBA `PixelCanvasBuffer`, adaptive `Kitty`/`Sixel`/`iTerm2`/`HalfBlock` transport, and `bilterm plot` / `bilterm img` CLI (`CANV-02`, `SSH-GFX-01`, `CLI-GFX-01`) (completed 2026-10-25)
- [x] **Phase 28: Post-`v1.10.0` Architecture Hardening, Module DAG Decoupling & Runtime Efficiency** - Strict acyclic DAG, module ceiling decompression, macOS CoreText/Cocoa GUI (`Bilterm.app`), mouse/keyboard selection & clipboard, and Powerline/PUA geometric font rendering (`ARCH-R01..04`, `SEC-R01`) (completed 2026-10-27)
- [x] **Phase 29: Smooth Scrolling by Default & Mainframe/Timeshare Block/Page-Mode Scroller** - Kinetic sub-cell/inertial smooth scrolling by default plus per-pane IBM 3270 / VM/CMS `MORE...` / `HOLDING` block/page-mode output paging (`SCRL-01`, `SCRL-02`) (completed 2026-10-27)
- [x] **Phase 30: Explicit Exit Guard & Root/Final Session Protection** - Accidental `exit` / `logout` / `Ctrl+D` interception on final/root panes with interactive `Respawn / Detach / Quit / Cancel` guard and explicit `:quit!` / `:detach` semantics (`SAFE-01`, `SAFE-02`) (completed 2026-10-27)
- [x] **Phase 31: Universal Open Color Scheme & Font Engine (`iterm2-color-schemes` Compatibility)** - Native `.itermcolors` XML plist importer (`mbadolato/iterm2-color-schemes`), Alacritty/WezTerm/Ghostty/Base16 TOML/YAML theme loaders, automatic Borland/3270 chrome synthesis, and per-theme typography profiles without lock-in (`THM-01`, `THM-02`) (completed 2026-10-27)
- [x] **Phase 32: Persistent Background Multiplexer Daemon (`herdr`-Style Multi-Client Attach/Detach)** - Auto-spawned background daemon (`bilterm daemon`) hosting all PTYs, `VtWorkerHandle` state machines, and session trees over a `0600` Unix socket for seamless multi-client connect/disconnect/reconnect (`DAEM-01`, `DAEM-02`) (completed 2026-10-27)
- [x] **Phase 33: Independent, Rooted & Hierarchical Tree/Subtree Sessions + Crash-Safe Layout Persistence** - Forest of independent workspaces, rooted daemon sessions, and parent/child session trees & subtrees with continuous atomic crash/restart layout journaling (`TREE-01`, `TREE-02`, `PERS-01`, `PERS-02`) (completed 2026-10-27)
- [x] **Phase 34: TRON-Inspired Master Control Program (`MCP`) Root Terminal** - Non-destructive `MCP` root terminal interfacing to the background daemon with TRON grid aesthetics, global session tree routing, broadcast, freeze/thaw, and `derez` controls (`MCP-01`, `MCP-02`) (completed 2026-10-27)
- [x] **Phase 35: Live Background OS Process Daemonization for `:detach` / `bilterm attach`** - Detached `bilterm --daemon --session <name>` background process keeping live PTY master FDs and multi-pane layouts alive across client exits (`DAEM-03`, `DAEM-04`) (completed 2026-10-28)
- [x] **Phase 36: Interactive Mouse & Keyboard Navigation + Live Telemetry in `:mcp` Console Window** - Highlighted row selection cursor, 1-click/Enter actions, and live auto-refreshing telemetry in `:mcp` (`MCP-03`, `MCP-04`) (completed 2026-10-28)
- [x] **Phase 37: Interactive File & `.itermcolors` Browser, Cursor Customizer & Multi-Profile Layout Manager (`v2.1.1`–`v2.2.0`)** - Multi-profile layout templates & autosave, WCAG 2.1 contrast engine, interactive `:browse` / `:theme import` file browser, and `:cursor` customizer (`PROF-01`, `THM-03`, `CUR-01`) (completed 2026-10-29)
- [x] **Phase 38: Session Lifecycle Commands, Persistent Ordering, Blinking Software Cursor & Scaled Mouse Selection (`v2.3.0`)** - `:session new|blank|rename|switch|delete|move|reorder|swap`, `bilterm new|blank`, software cursor blink timer, and scaled-window mouse selection/menubar alignment (`SESS-01..02`, `CUR-02`) (completed 2026-10-29)
- [x] **Phase 39: Per-Window Themes, Outlines, Bold Borders, `[_][□][X]` Controls & `MIN_GRID_DIM` `2×2` Clamp (`v2.4.0`)** - Per-pane `PaneChromeStyle` (`:window theme|outline|bold|reset|show`), `[_][□][X]` window control badges, and `>= 2×2` VT grid clamp preventing 1-row soft-wrap underflow (`WIN-01`, `WIN-02`, `VT-CLAMP-01`) (completed 2026-10-29)
- [x] **Phase 40: Interactive Per-Window Theme Browser & Per-Window Element Color Overrides (`v2.4.1`)** - Interactive `:window` / `:window theme` browser, per-window `.itermcolors` file browser routing (`window_theme_target`), and 13 individual per-window theme element color overrides (`font-color`, `cursor-color`, `frame-color`, `inactive-border`, `controls-color`, `status-color`, `badge-color`, `title-color`, `bg`, `prompt-bg`, `prompt-color`, `crosshair-color`, `shadow-color`) with session TOML persistence (`WIN-03`, `WIN-04`) (completed 2026-10-31)
- [x] **Phase 41: 2D Rectangular Block Select/Highlight/Copy & Full Per-Pane Session Shell, Running Command & CWD Persistence (`v2.5.0`)** - First-class `:rect` / `:select` / `:copy rect` / `Alt+R` / `Alt+LeftDrag` 2D rectangular block selection, highlight, and copy plus full per-pane session persistence of layout, shell (`shell`, `shell_args`), running software/commands (`running_command`), and working directory (`cwd`) with auto-relaunch (`RECT-01`, `SESS-03`) (completed 2026-10-31)
- [x] **Phase 43: UI Modernization Step 1 — Architecture Audit & Plan Realignment** - Read-only audit, roadmap realignment, `ARCH-R04` relaxed to a soft ~800–1000-line goal (`UIM-00`)
- [x] **Phase 44: UI Modernization Step 2 — Visual Design Proposal with Working Previews** - Proposed redesign backed by real `--gui-dump` previews; user sign-off gate (`UIM-01`)
- [x] **Phase 45: UI Modernization Step 3 — Design System: Tokens, Materials, Themes & `[appearance]` Config** - Design tokens, 5 materials, 8 theme profiles, backward-compatible `[appearance]` config, animation clock (`UIM-02`) (completed 2026-11-07)
- [ ] **Phase 46: UI Modernization Step 4 — Main Interface: Shell, Display Modes, Workspace Navigation & Pane Chrome** - Standard/Minimal/Zen modes, navigator, pane chrome and contextual actions (`UIM-03`)
- [ ] **Phase 47: UI Modernization Step 5 — Drawers, Floating Panels, Command Palette, Context Menus & Live-Preview Appearance Settings** - Remaining UI on the design system plus live-preview settings modal (`UIM-04`)
- [ ] **Phase 48: UI Modernization Step 6 — Regression, Performance & Polish** - Full regression, perf, accessibility, visual-consistency review, release (`UIM-05`)
- [ ] **Phase 49: Command Parity & Full Public API** - Every capability a `:colon` command, reachable from MCP, exposed by a versioned documented API (`API-01`)
- [x] **Phase 50: Agent-Aware Workspace Navigator (Left Sidebar)** - Persistent left-side navigator in the slide-out drawer: workspaces > sessions > panes tree with per-pane agent-state dots, attention counts, jump-to-next-blocked, keyboard-first, plus the agent traffic bus, zoom window and snoop view and their GUI vector rendering (`FLT-01`, `FLT-02`, `FLT-03`)
- [x] **Phase 50.1: Unified Registry & Find (INSERTED)** - Searchable registry of agents, terminal sessions/panes and per-session command history, extended to SSH and tmux sessions, with `:find` command verbs (`FND-R01..04`)
- [x] **Phase 51: UI Tweaks & Command Parity** - `:agents demo stop`; every colon verb reachable from the command palette (incl. `:drawer style navigator`); `:agents list` as a floating, pinnable, tileable agents-list window (`UIT-01..04`)
- [ ] **Phase 51.1: Persistent SSH + tmux Sessions (INSERTED)** - Real `ssh -tt` + `tmux -CC new -A` transport with keepalive, honest status, reconnect with backoff, exited-pane hold banner, no fake remote badge on restore (`PST-01..04`)
- [x] **Phase 51.2: First-Class Utility Windows & Script Studio (INSERTED)** - Floating, pinnable, dockable utility windows (`FONTS` window with live preview, workbench-style pinned `COMMAND PALETTE` with execution history, and `SCRIPT STUDIO` with live recorder and single-step debugger) (`WIN-01..WIN-03`)
- [x] **Phase 52: Agent-to-Agent Messaging & Coordination** - Structured mailbox and addressing so agents running in panes can message, wait on, and coordinate with each other via RPC/MCP/`:agent`, with delivery modes, safety gates and a coordinator view (`AGT-01..03`)
- [x] **Phase 53: Session & Window Management** - Move windows between sessions, rename sessions and windows, and multi-select windows for bulk actions, by key sequence, `:` command, Navigator right-click menu and RPC/MCP, with undo-safe semantics and persistence (`SES-01..05`)
- [x] **Phase 54: Session Packing Viewer & Drag-and-Drop** - Non-modal text-mode and GUI viewer drawing several sessions at once as rectangular containers with their windows packed inside (treemap-style), plus drag-and-drop of windows between sessions and within the Navigator tree, with a keyboard equivalent for every gesture (`VIEW-01..04`)
- [x] **Phase 55: Daemon Supervisor & Multi-Daemon Awareness** - Auto-dispatched background daemon that is a singleton by default, optional named multi-daemon mode, daemons discover and report on each other, with `:daemon` verbs, mouse/key paths and RPC/MCP (`DAEM-05..08`)
- [x] **Phase 56: Drawer Window Peek & Live Preview Pane** - Transient non-destructive floating preview pane when scrolling/highlighting windows or sessions in the Classic or Navigator drawer, with click/Enter-to-focus and Esc-to-dismiss (`DRW-PEEK-01..03`) (completed 2026-11-07, `80994fb`, `PASS 129/129`, `GOLDENS OK 11`, PR #31)
- [x] **Phase 57: Smart Clipboard Paste Fix (`Herdr`/TUI `Ctrl+V`) & Filterable Clipboard Viewer Window** - Smart MIME-aware `Ctrl+V` / `DECSET 2004` bracketed paste fixing `"No image or video found in clipboard"` in Herdr/Claude Code, plus a floating, pinnable, dockable `CLIPBOARD` viewer window filterable by session, window, pin state, and fuzzy query (`CLIP-01..04`) (completed 2026-11-07, `2f069bf`, `PASS 129/129`, `GOLDENS OK 11`, PR #31)
- [x] **Phase 58: Local AI Speech-to-Text (`STT`) Optional Module** - Independent, zero-bloat local AI voice dictation module (`src/stt/`) supporting OpenAI-compatible Whisper servers (`whisper.cpp`, `speaches`, `LocalAI`, `vLLM`, `LM Studio`), Ollama, and local CLI pipes with push-to-talk/VAD and OIA `[🎙 REC]` telemetry (`STT-01..03`)
- [x] **Phase 59: GUI Chrome Engine Web Browser Canvas & 60fps Video Player Canvas** - Embedded Chromium engine canvas (CDP Screencast frame-sink + optional CEF off-screen renderer) and 60fps RGBA Video Player canvas (`ffmpeg` pipe + OSD transport controls + audio sync) inside composite panes across GUI (`GuiTextureQuad`) and TUI (`Kitty`/`Sixel`/`HalfBlock`) (`WEB-01..02`, `VID-01..02`)
- [x] **Phase 60: Multi-Surface Web Frontend Backend & `@agent-manager/protocol` Bridge (`WEB-FE-01..04`)** - RFC 6455 WebSocket `GET /ws` server implementing `@agent-manager/protocol` v1, atomic `workspace.view_model` RPC & REST endpoints with CSS design tokens export, multi-client `ClientViewState` registry, static SPA serving, and full `UNI-R01` surface parity (`WEB-FE-01..04`)

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

### Phase 19: Security Hardening of the Automation & Shell-Integration Surfaces

**Goal**: As a user on a shared or networked host, I want the automation daemon, shell-integration scripts, and SSH helpers to be safe by default, so that no other local user or network peer can write my files, inject shell code, or smuggle SSH options.
**Depends on**: Architecture review P0 fixes (`fix/architecture-review-p0`)
**Requirements**: SEC-R01..SEC-R04 (`.planning/reviews/ARCHITECTURE_REVIEW_PLAN.md` §5.1)
**Success Criteria** (what must be TRUE):

  1. `--headless` with no `--listen` defaults to a `0600` Unix socket under `$XDG_RUNTIME_DIR`; non-loopback TCP requires `--allow-remote` plus a bearer token read from a `0600` file.
  2. `workspace.snapshot` `ppm_path` is confined to a designated output directory (path traversal rejected by test).
  3. Shell-integration rc files, history exports, `.cast` recordings, and SSH `ControlPath` live in a per-user `0700` directory (`$XDG_RUNTIME_DIR` or `tempfile` with ownership check, `O_EXCL` writes).
  4. `build_ssh_args` / `build_ssh_tmux_cc_args` reject destinations starting with `-` and emit `--` before the destination.

**Plans**: 7 plans

Plans:
- [x] 19-01-PLAN.md — `src/secure_fs.rs`: private 0700 dir, O_EXCL/O_NOFOLLOW 0600 files, 0600 token reader, constant-time compare (wave 1)
- [x] 19-02-PLAN.md — SSH builders: `validate_destination`, `Result` + `--`, ControlPath `<private>/ssh-%C`, wm validation, private session fallback (wave 2)
- [x] 19-03-PLAN.md — rc files, history export/fallback, default `.cast` moved to private dir with O_EXCL writes (wave 2)
- [x] 19-04-PLAN.md — RPC daemon: listen policy, Unix socket 0600 + peer uid, `auth.login` gate, token-aware client (wave 2)
- [x] 19-05-PLAN.md — `ppm_path` confinement + snapshot dir, confined RPC playbooks, destination validation at registration (wave 3)
- [x] 19-06-PLAN.md — CLI wiring (`--headless` default socket, `--allow-remote`, `--token-file`, `--snapshot-dir`, `ctl --token-file`), help, completions, man page (wave 4)
- [x] 19-07-PLAN.md — README/docs, CHANGELOG security entry, mark SEC-R01..04 Complete in REQUIREMENTS.md (defined by 19-01 Task 3) (wave 5)

### Phase 20: Mega-Module Decomposition (behaviour-preserving)

**Goal**: As a maintainer, I want `vt/`, `pane/`, `input/`, and `api/` split along their existing seams, so that each file has one reason to change and later refactors produce small, reviewable diffs.
**Depends on**: Phase 19
**Requirements**: ARCH-R04 (review P1.4)
**Success Criteria** (what must be TRUE):

  1. `vt/mod.rs` is split into `scan`, `graphics`, `detect`, `codec`, `recording`; `cell_to_ratatui_style` moves to `ui/`.
  2. `pane/mod.rs` is split into `prompt_editor` (`FloatingPromptState`) and `modes` (copy/hints/search); `colon_command.rs` moves out of `pane/`.
  3. `input/mod.rs` is split into `encode`, `mouse`, `modal`, `keymap`; `api/mod.rs` into `json`, `rpc`, `mcp`, `playbook`, `server`.
  4. No source file exceeds ~800 lines, the `pane ↔ wm` cycle is gone, and the full test suite plus `insta` snapshots pass unchanged.

**Plans**: 5 plans

Plans:
- [x] 20-01-PLAN.md — `src/vt/` decomposition (`scan`, `graphics`, `detect`, `codec`, `recording`, `tests`, `scrollback_tests`), move `cell_to_ratatui_style` to `src/ui/`, define `ARCH-R01..05` in `REQUIREMENTS.md` (wave 1)
- [x] 20-02-PLAN.md — `src/pane/` decomposition (`prompt_editor`, `modes`, `history_store_tests`) and move `colon_command` to `src/colon_command/{mod,parse,execute}.rs`, eliminating `pane ↔ wm` cycle (wave 1)
- [x] 20-03-PLAN.md — `src/input/` decomposition (`encode`, `mouse`, `modal`, `keymap`, `mod.rs`) (wave 2)
- [x] 20-04-PLAN.md — `src/api/` decomposition (`json`, `rpc`, `mcp`, `playbook`, `server`, `tests`, `mod.rs`) (wave 2)
- [x] 20-05-PLAN.md — `src/ui/` (`modals`, `status_bar`) and `src/wm/` (`session`) decomposition for global `<= 800` line ceiling, full test/snapshot gate, and Phase 20 state sync (wave 3)

### Phase 21: Encapsulated Workspace Invariants

**Goal**: As a maintainer, I want `WindowManager` and `CompositePane` to own their invariants behind private fields, so that closing, reaping, focusing, and submitting commands behave identically from every caller.
**Depends on**: Phase 20
**Requirements**: ARCH-R02 (review P1.2)
**Success Criteria** (what must be TRUE):

  1. `WindowManager` and `CompositePane` fields are private; `app`, `api`, and `input` use methods only.
  2. A single `WindowManager::remove_pane` serves close, exit-reaping, and fleet removal (zoom/z-stack/tree cleanup covered by tests, including reaping a zoomed pane).
  3. A single `CompositePane::submit_command` serves the prompt and `pane.exec`.
  4. Modal state is one `enum Modal`, focus is `enum Focus { Quick, Pane(PaneId) }` (no `PaneId 0` sentinel), and `leader_pending` exists in one place.

**Plans**: 3/3 plans complete

Plans:
- [x] 21-01-PLAN.md — Encapsulate `CompositePane`: private fields, remove `pane.leader_pending`, deduplicate `new_base`, unified `submit_command`/`submit_prompt`, and method accessors across `src/` & `tests/` (wave 1)
- [x] 21-02-PLAN.md — Encapsulate `WindowManager`: private fields, `enum Modal`, `enum Focus { Quick, Pane(PaneId) }` (no `PaneId 0` sentinel), unified `remove_pane(id)` across close/reap/fleet, and method accessors across `src/` & `tests/` (wave 2)
- [x] 21-03-PLAN.md — Invariant & regression tests (reaping zoomed pane, fleet disconnect pane removal, `submit_command` parity, `Modal` exclusivity, `Focus` hit-testing) and Phase 21 state sync (wave 3)

### Phase 22: Unified `Action` Command Vocabulary, Data-Driven Keymap & Whole-Workspace `.cast` Recorder

**Goal**: As a user and maintainer, I want every key chord, `:command`, playbook step, and JSON-RPC/MCP call to lower into one typed `Action`, with harmonized `<Ctrl+X>`/`<Ctrl+B>` controls under `asciinema` and a built-in whole-workspace Asciicast v2 (`.cast`) compositor recorder that captures all windows, splits, z-order stacks, floating prompts, and chrome.
**Depends on**: Phase 21
**Requirements**: ARCH-R01, REC-02 (review P1.1)
**Success Criteria** (what must be TRUE):

  1. `crate::action::Action` replaces `ColonCommand`; `WindowManager::apply(Action)` is the only executor.
  2. Default chords are a data table merged with `config.toml` `[keybindings]`; `handle_wm_key_event` is under ~80 lines.
  3. Dual-prefix leader support (`Ctrl+B` and `Ctrl+X` accepted when standalone; automatic fallback to `Ctrl+X` when `$TMUX` is detected so `Ctrl+B` passes cleanly to outer `tmux`), held-`Ctrl` leader chord normalization (`Ctrl+X Ctrl+<key>` / `Ctrl+B Ctrl+<key>` match `Ctrl+X <key>`), `asciinema` PTY-wrapper harmony (prompt mode toggles `Ctrl+X e/t/b/s`, `F4`, `F12` override false-positive `!ECHO` passthrough when not in `ALT_SCREEN`), and full `tmux`/`herdr` post-leader chord parity (`"` → `SplitHorizontal`, `%` → `SplitVertical`, `n` → `DeckNext`, `p` → `DeckPrev`) alongside existing `-`/`|`/`h/j/k/l`/`o`/`z`/`[`/`1..9`/`:`/`?` chords and Borland `F1`–`F12`/`Alt+<key>` bindings.
  4. `api::dispatch_method` and the playbook engine parse into `Action` (typed params) rather than re-implementing operations.
  5. Built-in Whole-Workspace Asciicast v2 (`.cast`) compositor recorder (`:record start [path]` / `:record workspace [path]` / `:record pane [path]` / `:record stop`) diffs rendered `ratatui::buffer::Buffer` frames into ANSI escape sequences so multi-pane splits, z-order stacks, floating prompts, Borland menus, 3270 cross-hairs, and modals are recorded into a single standard `.cast` file.
  6. A table test asserts that every `Action` is reachable from keymap, colon parser, and RPC where applicable.

**Plans**: 3/3 plans complete

Plans:
- [x] 22-01-PLAN.md — Create `crate::action::Action`, `WindowManager::apply(Action)`, and `WorkspaceRecorder` whole-workspace Asciicast v2 (`.cast`) compositor frame recorder (wave 1)
- [x] 22-02-PLAN.md — Declarative `DEFAULT_LEADER_BINDINGS`/`DEFAULT_GLOBAL_BINDINGS` tables, `<80`-line `handle_wm_key_event`, `Ctrl+B`/`Ctrl+X` dual leader (`$TMUX` fallback), `tmux`/`herdr` chords (`"`, `%`, `n`, `p`), held-`Ctrl` chord normalization, and `asciinema` PTY-wrapper prompt override (wave 2)
- [x] 22-03-PLAN.md — Lower `api::dispatch_method` & Playbook engine into `Action`, add table reachability + whole-workspace `.cast` + `asciinema` harmony tests, and sync Phase 22 state (wave 3)

### Phase 23: `TerminalBackend` Trait & Unwired-Subsystem Resolution

**Goal**: As a maintainer, I want the VT emulator behind a `TerminalBackend` trait and every advertised subsystem either wired into production or explicitly marked experimental, so that the `alacritty_terminal`/Ghostty migration is a contained change and docs match reality.
**Depends on**: Phase 22
**Requirements**: ARCH-R03, ARCH-R05 (review P1.3, P1.5)
**Success Criteria** (what must be TRUE):

  1. No `vt100_ctt` type appears outside `src/vt/` (renderers read `GridSnapshot` / a cell view).
  2. `VtParser` implements `TerminalBackend`; a second (mock) backend passes the shared conformance tests.
  3. Decision recorded as an ADR for `VtWorkerHandle`, `TmuxControlSubprocess`, and the `ghostty-vt` feature: each is either used by the live app or marked experimental, and the `ghostty-vt` feature either builds in CI (`libghostty-vt` declared, Zig in `flake.nix`) or is removed.
  4. A CI feature matrix (`cargo hack check --each-feature`) passes.

**Plans**: 3/3 plans complete

Plans:
- [x] 23-01-PLAN.md — `TerminalBackend` trait, `MockTerminalBackend`, shared conformance test suite, and removal of all `vt100_ctt` types outside `src/vt/` (wave 1)
- [x] 23-02-PLAN.md — Wire `TmuxControlSubprocess` into `WindowManager`/`App`, implement `TerminalBackend` for `GhosttyVtEngine`, pin `rust-toolchain.toml` (`1.90`), update `flake.nix`, and record `ADR-007` (wave 2)
- [x] 23-03-PLAN.md — Architectural boundary enforcement test (`no_vt100_ctt_types_appear_outside_src_vt`), `TmuxControlSubprocess` polling test, and Phase 23 documentation/state sync (wave 3)

### Phase 24: `herdr`-Style Slide-Out Sessions/Windows/Panes Drawer with Status Glyphs & Float/Push Modes

**Goal**: As an operator managing multiple sessions, split trees, z-order decks, and remote fleet nodes, I want a `herdr`-inspired slide-out drawer that can dock to any screen edge (`Left`, `Right`, `Top`, `Bottom`), operate in `OnDemand`, `Fixed`, or `AutoHide` mode, either float over panes (`Overlay`) or bump/auto-resize the workspace (`Push`), and render expressive live status glyphs for every session, window, and pane.
**Depends on**: Phase 23
**Requirements**: DRW-01, DRW-02
**Success Criteria** (what must be TRUE):

  1. `DrawerConfig` and runtime `WorkspaceDrawer` support 4 docking edges (`Left`, `Right` vertical tree; `Top`, `Bottom` horizontal ribbon/bar), configurable width/height, and 3 visibility modes (`OnDemand`, `Fixed`, `AutoHide` triggered by leader-key activation, mouse edge hover, or background pane notification/exit alert).
  2. `DrawerLayoutMode::Overlay` renders the drawer over the active workspace using `occlude_rect` and Borland drop shadow (`render_drop_shadow_styled`) without emitting `SIGWINCH` to PTYs; `DrawerLayoutMode::Push` subtracts `drawer_rect` from `workspace_area` before `WindowManager::compute_workspace_rects` and `WindowManager::resize_all`, reflowing panes cleanly.
  3. Each session/window/pane row renders rich telemetry glyphs: hierarchy connectors (`├─`, `╰─`), focus & z-order rank (`► [1] Z:1/4`), live activity braille spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) + output throughput sparkline (`▁▂▃▅▆▇`), OSC 133 command state (`⚙ RUN`, `✓ 0`, `✗ <code >`), mode/security badges (`🖥 ALT`, `🔑 !ECHO`, `🔒 PROT`, `⏺ REC`, `📡 BCAST`, `🔔 N`), and transport badges (`🌐 SSH`, `🔗 TMUX`).
  4. Keyboard navigation (`j`/`k`/`h`/`l`, `Enter` to focus/jump, `Space` to preview, `x` to close, `z` to zoom, `d` to cycle dock edge, `m` to toggle `Overlay`/`Push`, `p` to pin/unpin `Fixed`/`AutoHide`) and mouse click/hover/drag interactions work across both TUI (`Ratatui`) and GUI (`GuiWorkspaceRenderer`) surfaces, with deterministic `insta` / `TestBackend` snapshot coverage.

**Plans**: 3/3 plans complete

Plans:
- [x] 24-01-PLAN.md — `src/wm/drawer.rs`, `DrawerConfig`, `WorkspaceDrawer`, `Overlay`/`Push` geometry, `AutoHide` triggers, and telemetry glyph builder (wave 1)
- [x] 24-02-PLAN.md — `src/ui/drawer.rs` TUI/GUI renderer, `Action::Drawer*`, `:drawer` colon commands, keyboard modal navigation, and mouse edge hover/click/drag handlers (wave 2)
- [x] 24-03-PLAN.md — `tests/slide_out_drawer_and_telemetry.rs` integration & snapshot suite, documentation updates, and Phase 24 state sync (wave 3)

### Phase 25: Dynamic Terminal Text & Font Size Scaling

**Goal**: As a user operating Bilterm across high-DPI monitors, projector demos, and nested terminals, I want on-the-fly terminal text and font size scaling (`Ctrl++`/`Ctrl+=`, `Ctrl+-`, `Ctrl+0`, `:fontsize`) across both the native pixel/SVG renderer and the host TUI canvas.
**Depends on**: Phase 24
**Requirements**: FONT-01
**Success Criteria** (what must be TRUE):

  1. `Action::FontZoomIn`, `Action::FontZoomOut`, `Action::FontZoomReset`, and `Action::FontSetSize(u16)` are wired to `Ctrl++`/`Ctrl+=`, `Ctrl+-`, `Ctrl+0`, `:fontsize <pt|pct|+|-|reset>`, JSON-RPC/MCP (`workspace.fontsize`), and `config.toml` (`font_size_pt`, `font_scale_pct`, `cell_width_px`, `cell_height_px`).
  2. `GuiWorkspaceRenderer` / `GuiFrameSurface` dynamically scales cell pixel dimensions (`50%..=300%`), SVG typography (`font-size`), and PPM raster glyph scaling, recalculating viewport `(rows, cols)` for a given pixel window size and resizing panes accordingly.
  3. In TUI mode (`Ratatui`), `VtParser` and the canvas renderer support DEC double-width (`DECDWL` `\x1b#6`) and double-height (`DECDHL` `\x1b#3` / `\x1b#4`) line attributes plus multi-cell scaled text rendering, paired with the focal zoom magnification lens (`render_focal_zoom_cursor`) and optional host-terminal `OSC 50` / Kitty font-size passthrough.

**Plans**: 3/3 plans complete

Plans:
- [x] 25-01-PLAN.md — `src/ui/typography.rs`, `BiltermConfig` font scaling fields, DEC `DECDWL`/`DECDHL` line attributes in `VtParser`/`TerminalBackend`, and TUI multi-cell rendering (wave 1)
- [x] 25-02-PLAN.md — `GuiWorkspaceRenderer`/`GuiFrameSurface` dynamic cell/SVG/PPM scaling & pixel-window viewport calculation, `Action::Font*`, `:fontsize`, `Ctrl++`/`Ctrl+-`/`Ctrl+0`, and JSON-RPC/MCP `workspace.fontsize` (wave 2)
- [x] 25-03-PLAN.md — `tests/font_scaling_and_dec_line_attrs.rs` integration suite, documentation updates, and Phase 25 state sync (wave 3)

### Phase 26: Tektronix 4010/4014 × Modern 2026/2027 Vector, Sprite, Plotting & Image Graphical Canvas

**Goal**: As a systems programmer, scientist, or AI agent, I want each CompositePane to include an optional high-speed graphical canvas supporting both classic Tektronix 4010/4014 vector storage-tube emulation and a modern 2026/2027 retained scene-graph with 2D sprites, declarative plotting, and interactive image inspection.
**Depends on**: Phase 25
**Requirements**: TEK-01, CANV-01
**Success Criteria** (what must be TRUE):

  1. Classic Tektronix 4010/4014 state machine (`\x1b[?38h` / `\x1b[?38l` / `:canvas tek`) decodes 10-bit and 12-bit 4096×3120 vector coordinates (`GS` `0x1D` dark/bright vectors, `FS` point plot, `RS` incremental plot, `ESC` solid/dotted/dot-dash/short-dash/long-dash styles, `\x1b\x0c` screen erase with authentic green 3278 / amber 3279 phosphor persistence decay).
  2. Modern retained scene-graph (`PaneCanvasScene`) supports vector primitives (lines, polylines, bezier curves, circles, polygons, text labels), a 2D sprite engine (`id`, position, velocity, frame animation, z-index, AABB collision), declarative live data plots (`line`, `scatter`, `bar`, `candlestick`, `heatmap`), and an interactive image viewer (`pan`, `zoom`, pixel inspector).
  3. Dual-backend rasterizer renders the canvas into sub-cell Unicode 2×4 Braille (`U+2800..U+28FF`), 2×3 Sextant (`U+1FB00..U+1FB3B`), and Quadrant blocks with 24-bit TrueColor (plus Kitty/Sixel overlays) in TUI mode, and crisp full-resolution vector/pixel graphics in `GuiWorkspaceRenderer` (SVG/PPM), while keeping the floating command prompt cleanly composited on top.
  4. Controllable via in-band PTY escape sequences, `:canvas <subcommand>`, and JSON-RPC/MCP `canvas.*` automation endpoints, verified by deterministic vector/sprite/plot snapshot tests.

**Plans**: 3/3 plans complete

Plans:
- [x] 26-01-PLAN.md — `src/vt/tektronix.rs` (`TektronixState` 4010/4014 vector state machine) and `src/pane/canvas_scene.rs` (`PaneCanvasScene` vectors, 2D sprites, declarative plots & image viewer) (wave 1)
- [x] 26-02-PLAN.md — `src/ui/canvas_raster.rs` (Unicode 2×4 Braille / 2×3 Sextant / Quadrant TUI rasterizer + GUI SVG/PPM renderer), `Action::CanvasCommand`, `:canvas`, and JSON-RPC/MCP `canvas.*` (wave 2)
- [x] 26-03-PLAN.md — `tests/tektronix_and_graphical_canvas.rs` integration suite, documentation updates, and Phase 26 state sync (wave 3)

### Phase 27: High-Resolution Anti-Aliased Pixel Canvas & Adaptive SSH/GUI Graphics Engine (`v2.0`)

**Goal**: As a user operating Bilterm in either a pure SSH session (over Ghostty/Kitty/WezTerm/iTerm2/Windows Terminal/xterm/tmux or a plain character terminal) or Native GUI mode (`GuiWorkspaceRenderer`), I want a high-resolution anti-aliased RGBA pixel canvas (`PixelCanvasBuffer`) that automatically negotiates the best graphics transport (`Kitty`, `Sixel`, `iTerm2`, `tmux` passthrough, `1×2` 24-bit TrueColor `HalfBlock`, `2×4` `Braille`, or native GUI pixel surface) and provides CLI/colon commands (`bilterm plot`, `bilterm img`, `:canvas backend`, `:canvas export`, `:canvas hires`) for rich charts, graphs, and images.
**Depends on**: Phase 26
**Requirements**: CANV-02, SSH-GFX-01, CLI-GFX-01
**Success Criteria** (what must be TRUE):

  1. `PixelCanvasBuffer` (`src/ui/pixel_canvas.rs`) renders `PaneCanvasScene` and `TektronixState` into an anti-aliased RGBA pixel framebuffer (Xiaolin Wu anti-aliased lines/curves, filled circles/rects, high-DPI plot axes/grids, bilinear image blitting, and smooth `SubcellFocalLens` radial magnification) and exports directly to `.ppm` or `.svg` (`:canvas export <file>`).
  2. `CanvasRenderBackend` (`Auto`, `Kitty`, `Sixel`, `ITerm2`, `HalfBlock`, `Braille`, `Sextant`, `Quadrant`) auto-detects host terminal pixel capabilities over SSH and local sessions, emits Kitty `APC G`, DEC Sixel `DCS q`, or iTerm2 `OSC 1337` pixel payloads (wrapped in `\x1bPtmux;...\x1b\\` when inside `$TMUX`), and renders 24-bit TrueColor `1×2` Half-Block (`▀`) bitmaps in Ratatui with clean `FloatingCommandPrompt` occlusion.
  3. `bilterm plot` and `bilterm img` CLI commands plus `:canvas backend <mode>`, `:canvas hires`, and `:canvas export <file>` allow one-command high-res plotting and image rendering from any shell or SSH session.

**Plans**: 3/3 plans complete

Plans:
- [x] 27-01-PLAN.md — `src/ui/pixel_canvas.rs`: Anti-aliased RGBA `PixelCanvasBuffer`, `HalfBlock` 24-bit TrueColor cell renderer, and `Kitty`/`Sixel`/`iTerm2`/`tmux` passthrough encoders (wave 1)
- [x] 27-02-PLAN.md — Wire `CanvasRenderBackend` auto-detection, `:canvas backend|export|hires`, `GuiWorkspaceRenderer` pixel canvas blitting, and `bilterm plot` / `bilterm img` CLI commands (wave 2)
- [x] 27-03-PLAN.md — `tests/high_res_pixel_and_ssh_canvas.rs` verification suite, documentation sync, and release (wave 3)

### Phase 28: Post-`v1.10.0` Architecture Hardening, Module DAG Decoupling & Runtime Efficiency

**Goal**: As a maintainer, I want all `v1.10.0` architecture review findings (`docs/ARCHITECTURE_REVIEW.md`) resolved — confining `:canvas export`, wiring dynamic `CanvasResolution` & cached `TIOCGWINSZ`, eliminating upward `ui` dependency cycles, decompressing near-ceiling modules, and adding dirty-driven rendering.
**Depends on**: Phase 27
**Requirements**: ARCH-R01, ARCH-R02, ARCH-R04, SEC-R01

**Plans**: 1/1 plan complete (Waves 1–3 complete)

Plans:
- [x] 28-01-PLAN.md — Wave 1 (P0.1 confined `:canvas export`, P0.2 cached `TIOCGWINSZ` & live `CanvasResolution`, P1.1a `src/api/rpc_handlers.rs` breaking `ui -> api` cycle, P1.2a `execute_wm.rs` / `pane/tests.rs` / `wm/tests.rs` ceiling decompression, and `Cargo.toml` `vt100-ctt` supply-chain pruning); Wave 2 (`error/secure_fs -> pty -> vt -> pane` strict acyclic DAG, `parse_visual.rs` & `scan_osc_dcs.rs` decompression, `FloatingPromptState` encapsulation); Wave 3 (dirty-driven `App::run` redraw, cached `PixelCanvasBuffer` rasterization, single-pass `ESC` scanner, and CI DAG acyclicity gate)

### Phase 29: Smooth Scrolling by Default & Mainframe/Timeshare Block/Page-Mode Scroller (`v2.0`)

**Goal**: As a terminal user, I want smooth kinetic scrolling enabled by default across all terminal panes, plus an optional per-pane Mainframe / Timeshare Block & Page-Mode Scroller (inspired by IBM 3270 / VM/CMS `MORE...` / `HOLDING` and TOPS-20 page-mode terminals) so that high-volume output can either glide smoothly or pause cleanly at full-page boundaries awaiting explicit page advance.
**Depends on**: Phase 28
**Requirements**: SCRL-01, SCRL-02
**Success Criteria** (what must be TRUE):

  1. Default `PaneScrollMode::Smooth` provides interpolated sub-cell pixel scrolling in Native GUI mode (`GuiWorkspaceRenderer` / `MacosGuiCanvas` / `LinuxGuiCanvas` / `Win32GuiCanvas`) and velocity-damped kinetic scrollback in TUI mode (`scroll_mode = "smooth"` default in `BiltermConfig`).
  2. Toggling per-pane `PaneScrollMode::BlockPage` (`:scroll page` / `:scroll block`, `Ctrl+X PageDown`, or clicking the `SCRL:SMOOTH/PAGE` OIA badge) causes incoming PTY output after a command submission or page clear to pause rendering once a full viewport page (`canvas.height` rows) is filled, displaying an authentic IBM 3270 `MORE...` (or `HOLDING` when inspected) indicator in the OIA status bar while continuing to buffer PTY output in `VtWorkerHandle` without dropping bytes.
  3. In `BlockPage` mode, pressing `Enter`, `Space`, `PageDown`, or `PF8`/`PA2` advances cleanly by one full page (returning to live tail when caught up), while `b` / `PageUp` / `PF7` steps backward by one full block/page.

**Plans**: 3/3 plans complete

Plans:
- [x] 29-01-PLAN.md — `PaneScrollMode` (`Smooth`, `Instant`, `BlockPage`), kinetic scroll velocity/sub-pixel state in `CompositePane`, and `VtWorkerHandle` page-gate snapshot freeze with `MORE...`/`HOLDING` telemetry (wave 1)
- [x] 29-02-PLAN.md — `Action::ScrollModeSet` / `:scroll [smooth|page|block|instant]`, `PF7`/`PF8`/`Space` page-advance bindings, OIA `MORE...`/`HOLDING` status badge, and GUI sub-cell smooth scroll rendering (wave 2)
- [x] 29-03-PLAN.md — `tests/smooth_and_mainframe_page_scrolling.rs` deterministic verification suite (wave 3)

### Phase 30: Explicit Exit Guard & Root/Final Session Protection (`v2.0`)

**Goal**: As a user running multi-pane or persistent sessions, I want exiting Bilterm to be explicit so that typing one-too-many `exit` commands or pressing `Ctrl+D` never accidentally kills the final terminal pane or root session.
**Depends on**: Phase 29
**Requirements**: SAFE-01, SAFE-02
**Success Criteria** (what must be TRUE):

  1. Submitting `exit` or `logout` (or pressing `Ctrl+D` on an empty prompt) in `FloatingCommandPrompt` when on the **final pane of a session** or inside a **Root Session** intercepts the command before killing the shell and opens an interactive Borland/3270 Exit Guard modal (`[EXIT GUARD: (r)espawn shell, (d)etach client, (q)uit client, (K)ill session, (Esc) cancel]`).
  2. Context-aware passthrough safety (from Compounded Lessons Learned): `Ctrl+D` inside `DECSET 1049` (`ALT_SCREEN` TUIs like `vim`, `nmon`, `less`) or during 3-signal `!ECHO LOCK` (`sudo` password prompts) passes cleanly to the foreground process without triggering false-positive Exit Guard modals.
  3. When a child shell in the final/root pane terminates via raw PTY (`RawPtyPassthrough` `Ctrl+D` or subshell exit), Bilterm does not exit the application; instead it reaps the exited child on the non-blocking background `bilterm-pty-reaper` thread, resets stale `TerminalBackend` modes (`ALT_SCREEN`, `BlockPage` `MORE...` anchor, mouse reporting), and holds the pane open (`🔒 [SHELL EXITED — Enter/r: Respawn Shell | d: Detach | :quit!: Exit]`) or auto-respawns according to `exit_protection = "guard" | "respawn" | "detach"`.
  4. Explicit exit commands (`:quit!`, `:detach`, `:mcp shutdown --confirm`) and `Ctrl+Q` confirmation cleanly separate detaching a UI client from terminating a session or daemon.

**Plans**: 2/2 plans complete

Plans:
- [x] 30-01-PLAN.md — `ExitProtectionPolicy` (`Guard`, `Respawn`, `Detach`), `FloatingCommandPrompt` `exit`/`logout`/`Ctrl+D` interception on final/root panes (with `ALT_SCREEN` & `!ECHO LOCK` passthrough guards), and non-blocking PTY process-exit hold/reset/respawn lifecycle (wave 1)
- [x] 30-02-PLAN.md — Borland/3270 Exit Guard confirmation modal, `:quit!` / `:detach` actions, and `tests/v2_phases_30_to_34_integration.rs` verification suite (wave 2)

### Phase 31: Universal Open Color Scheme & Font Engine (`iterm2-color-schemes` Compatibility without Lock-In, `v2.0`)

**Goal**: As a user customizing Bilterm's look and feel, I want first-class compatibility with [`mbadolato/iterm2-color-schemes`](https://github.com/mbadolato/iterm2-color-schemes) (`.itermcolors`) alongside open TOML/YAML/JSON theme formats (Alacritty, WezTerm, Ghostty, Base16) and per-theme typography profiles without being locked into any single format.
**Depends on**: Phase 30
**Requirements**: THM-01, THM-02
**Success Criteria** (what must be TRUE):

  1. Native `.itermcolors` Apple XML plist parser loads any color scheme from `mbadolato/iterm2-color-schemes` (`Ansi 0 Color`..`Ansi 15 Color`, `Foreground Color`, `Background Color`, `Bold Color`, `Cursor Color`, `Cursor Text Color`, `Selection Color`, `Selected Text Color`, `Badge Color`, `Link Color`) and automatically derives harmonious, contrast-verified Borland Turbo Vision double-border, drop-shadow, floating-prompt, cyan selection highlight, and 3270 OIA (`MORE...` / `HOLDING` / `!ECHO LOCK`) status bar colors.
  2. Multi-format open theme loader also parses Alacritty/WezTerm/Ghostty/Base16 TOML & YAML palettes and Bilterm `[theme.custom]` definitions so users can mix, import, and export themes freely without lock-in.
  3. `:theme import <path_or_dir>`, `:theme <name>`, and `Ctrl+P` Fuzzy Command Palette support live theme switching and per-theme/per-pane font family + size profiles (`~/.config/bilterm/themes/` auto-discovery), preserving geometric Powerline/PUA (`U+E0A0..=U+E0D7`) rasterization priority, macOS CoreText `.LastResort` rejection, and per-pane inverse `(cols, rows)` `TIOCSWINSZ` grid scaling.

**Plans**: 3/3 plans complete

Plans:
- [x] 31-01-PLAN.md — `src/ui/theme_import.rs`: Native `.itermcolors` XML plist parser + Alacritty/WezTerm/Ghostty/Base16 TOML/YAML adapter + automatic Borland/3270 chrome palette synthesizer (wave 1)
- [x] 31-02-PLAN.md — `:theme import|load|save|list`, `~/.config/bilterm/themes/` directory scanner, bundled popular `iterm2-color-schemes` presets, and per-theme/per-pane font profile binding with `TIOCSWINSZ` grid resize (wave 2)
- [x] 31-03-PLAN.md — `tests/v2_phases_30_to_34_integration.rs` verification suite and documentation updates (wave 3)

### Phase 32: Persistent Background Multiplexer Daemon (`herdr`-Style Multi-Client Attach/Detach, `v2.0`)

**Goal**: As a power user, I want Bilterm to run a persistent `herdr`-style background daemon so that I can connect multiple TUI/GUI sessions Simultaneously, or disconnect and reconnect at will without ever losing running shells, PTY state, or layouts.
**Depends on**: Phase 30
**Requirements**: DAEM-01, DAEM-02
**Success Criteria** (what must be TRUE):

  1. Launching `bilterm` or `bilterm --gui` (`Bilterm.app`) automatically spawns or connects to a persistent per-user background daemon (`bilterm daemon` listening on `0700`/`0600` `~/.local/state/bilterm/daemon.sock` with `SO_PEERCRED` UID verification and `ConnectionRefused`-only stale socket recovery) that owns all PTYs, `VtWorkerHandle` state machines, and workspace sessions.
  2. Clients can detach (`Ctrl+X d` / `:detach` / closing GUI window) while all background sessions continue running, and reattach at any time via `bilterm attach [session]`, `bilterm --gui`, or the `Herdr` Drawer.
  3. Multiple TUI and Native GUI clients (including heterogeneous viewports and per-pane font scales) can connect concurrently to the same daemon — either sharing a live session in real time with `Ord::clamp`-safe viewport negotiation or operating independent sessions hosted by the same daemon (`bilterm ls`, `bilterm attach`).

**Plans**: 3/3 plans complete

Plans:
- [x] 32-01-PLAN.md — `src/daemon/`: Background daemon process supervisor, auto-spawn/handshake over `0600` Unix domain socket (`SO_PEERCRED` + `ConnectionRefused`-only stale socket unlink), and multi-session `SessionRegistry` hosting `WindowManager` instances (wave 1)
- [x] 32-02-PLAN.md — Streaming frame/snapshot delta subscription, heterogeneous multi-client viewport/font-scale negotiation, and input forwarding protocol for attached TUI and Native GUI (`--gui`) clients, plus `bilterm attach|detach|ls` CLI subcommands (wave 2)
- [x] 32-03-PLAN.md — `tests/v2_phases_30_to_34_integration.rs` integration suite verifying concurrent multi-client attach, detach, and reconnect survival (wave 3)

### Phase 33: Independent, Rooted & Hierarchical Tree/Subtree Sessions + Crash-Safe Layout Persistence (`v2.0`)

**Goal**: As a user managing complex workflows, I want sessions organized as independent workspaces, rooted daemon sessions, or hierarchical trees/subtrees, with automatic crash-safe layout & state journaling that restores everything after a crash, intentional exit, or new connection (like `herdr`).
**Depends on**: Phase 32
**Requirements**: TREE-01, TREE-02, PERS-01, PERS-02
**Success Criteria** (what must be TRUE):

  1. `SessionTree` models sessions as **Independent** roots, **Rooted** children of the `MCP` daemon root, or nested **Parent → Child Trees & Subtrees** (`:session branch <parent>/<child>`, `:session graft <src> <dst>`, `:session detach-subtree <node>`), rendered with collapsible tree folding and rollup telemetry in the `Herdr` Drawer.
  2. Subtree-scoped actions allow broadcasting commands, searching scrollback, or recording `.cast` files across an entire session subtree.
  3. Continuous debounced atomic state journaling (`~/.local/state/bilterm/journal/` via `secure_fs` sibling `.tmp-<pid>-<seq>` `0600` atomic `fs::rename`) saves the full session forest, strictly separated `SplitTree` ratios vs. `ZStack` coordinates (never cross-contaminating `Tiled` mode), floating prompt geometries, themes, per-pane `PaneFontOptions` (`scale_pct`), per-pane `PaneScrollMode` (`Smooth` / `BlockPage`), working directories (`OSC 7` / OS CWD), command histories, and scrollback tails, automatically restoring them on restart after a crash, intentional exit, or fresh connection.

**Plans**: 3/3 plans complete

Plans:
- [x] 33-01-PLAN.md — `src/wm/session_tree.rs`: Independent, Rooted, and Parent/Child Tree & Subtree session DAG, `:session branch|graft|tree` commands, and collapsible tree rendering in `Herdr` Drawer (wave 1)
- [x] 33-02-PLAN.md — `src/wm/journal.rs`: Continuous crash-safe atomic sibling-temp-rename layout, `SplitTree`/`ZStack`, per-pane font/scroll mode, CWD, history & scrollback-tail journaling + automatic resurrection on daemon startup or reconnect (wave 2)
- [x] 33-03-PLAN.md — `tests/v2_phases_30_to_34_integration.rs` verification suite (wave 3)

### Phase 34: TRON-Inspired Master Control Program (`MCP`) Root Terminal (`v2.0`)

**Goal**: As a systems operator, I want a dedicated TRON-inspired **Master Control Program (`MCP`)** root terminal that interfaces with the overall background daemon to monitor, route, spawn, graft, broadcast, freeze/thaw, or `derez` sessions and programs — without ever killing the daemon when closed.
**Depends on**: Phase 33
**Requirements**: MCP-01, MCP-02
**Success Criteria** (what must be TRUE):

  1. Opening the **Root Terminal (`MCP`)** (`bilterm --mcp-console`, `:mcp`, or selecting `◈ ROOT [MCP — MASTER CONTROL PROGRAM]` at the root of the `Herdr` Drawer) connects to the background daemon's control plane; closing or exiting the `MCP` root terminal window never terminates the daemon or user sessions.
  2. The `MCP` Root Terminal provides a TRON / 3270 high-contrast cyber-grid dashboard and interactive command shell supporting `mcp status`, `mcp tree`, `mcp attach <node>`, `mcp spawn <tree/path>`, `mcp graft <src> <dst>`, `mcp broadcast <subtree> <cmd>`, `mcp freeze <target>` / `mcp thaw <target>` (backed by zero-drop `VtWorkerHandle` `HOLDING` page-lock buffering), `mcp derez <session|pane>` (clean termination with confirmation), `mcp checkpoint`, and `mcp shutdown --confirm` (`END OF LINE.`), with all operations unified under `crate::action::Action` (`ARCH-R01`) and `<= 800`-line modules (`ARCH-R04`).

**Plans**: 3/3 plans complete

Plans:
- [x] 34-01-PLAN.md — `src/mcp_console/`: Non-destructive Root `MCP` terminal session type, TRON Master Control Program command interpreter (`status`, `tree`, `spawn`, `graft`, `broadcast`, `freeze`, `thaw`, `derez`, `checkpoint`, `shutdown`), and daemon RPC bindings (wave 1)
- [x] 34-02-PLAN.md — TRON Red/Cyan/Gold cyber-grid visual theme & live daemon telemetry dashboard inside the `MCP` Root Terminal and `Herdr` Drawer root node (wave 2)
- [x] 34-03-PLAN.md — `tests/v2_phases_30_to_34_integration.rs` end-to-end verification suite and User Manual documentation (wave 3)

### Phase 35: Live Background OS Process Daemonization for `:detach` / `bilterm attach` (`v2.1`)

**Goal**: As a user detaching and re-attaching (`:detach`, `Ctrl+X D`, `bilterm attach [session]`, `bilterm -a`), I want a background OS process (`bilterm --daemon`) to keep live PTY master file descriptors and running foreground processes (`vim`, `htop`, `nmon`, builds, SSH sessions) alive across client exits, while preserving the complete multi-pane `SplitTree` and `ZStack` window layout.
**Depends on**: Phase 32, Phase 33, Phase 34
**Requirements**: DAEM-03, DAEM-04
**Success Criteria** (what must be TRUE):

  1. **Auto-Spawn & Socket Handshake (`DAEM-03`)**: When `bilterm` or `bilterm attach [session]` starts (or when `:detach` is invoked), Bilterm can automatically spawn/connect to a detached background OS daemon process (`bilterm --daemon --session <name>`, setsid/nohup background process listening on private `0700`/`0600` `~/.local/state/bilterm/daemon-<session>.sock` with `SO_PEERCRED` UID verification and `ConnectionRefused`-only stale socket cleanup).
  2. **Live PTY & Layout Bridge (`DAEM-04`)**: While detached, the background daemon keeps every pane's `PtySession` master FD and `VtParser` running (`DECSET 1049` alternate screen, `termios !ECHO`, scrollback, and running foreground child PIDs stay untouched). Re-attaching via `bilterm attach [session]` or `:attach [session]` hydrates the full multi-pane `SplitTree` + `ZStack` window geometries and bridges live PTY I/O (`pane.exec` / raw keystrokes / `GridSnapshot` sync) or falls back seamlessly to the TOML/Journal snapshot if the background daemon is not running.
  3. **Daemon Lifecycle CLI**: `bilterm ls` / `bilterm sessions` lists both live running background daemon sessions (with live PID, uptime, and pane count) and saved disk sessions.

**Plans**: 3/3 plans complete

Plans:
- [x] 35-01-PLAN.md — `src/daemon/process.rs`: Background OS daemon spawner (`ensure_background_daemon`), per-session `0600` Unix socket `daemon-<session>.sock` lifecycle (`ConnectionRefused` stale cleanup), and `bilterm ls` / `bilterm sessions` CLI subcommand (wave 1)
- [x] 35-02-PLAN.md — `src/wm/session.rs` & `src/api/rpc_handlers.rs`: RPC `session.save` / `session.restore` + live PTY bridge on `:detach` and `bilterm attach [session]` / `:attach [session]` (wave 2)
- [x] 35-03-PLAN.md — `tests/live_daemon_detach_attach.rs`: Deterministic end-to-end test verifying live background daemon spawn, multi-pane layout + running process survival across `:detach` -> `bilterm attach`, and graceful shutdown (wave 3)

### Phase 36: Interactive Mouse & Keyboard Navigation + Live Telemetry in `:mcp` Console Window (`v2.1`)

**Goal**: As a systems operator using the `:mcp` floating or push-out console window (`:mcp status`, `:mcp tree`, `:session tree`, `:theme list`), I want interactive keyboard cursor selection (`Up`/`Down`/`Enter`) and direct mouse click actions on tree/status/theme rows, plus live auto-refreshing telemetry while the `:mcp` window remains open.
**Depends on**: Phase 34, Phase 35
**Requirements**: MCP-03, MCP-04
**Success Criteria** (what must be TRUE):

  1. **Interactive Row Selection & Actions (`MCP-03`)**: Inside the open `:mcp` floating/push-out window, `Up`/`Down` (`j`/`k`) moves a highlighted selection cursor across interactive rows (Session nodes, Pane `#id` rows, or Theme presets), and pressing `Enter` or left-clicking a row immediately executes the context action:
     - Clicking/selecting a **Pane `#id` row** focuses and raises that pane (`focus_and_raise(id)`).
     - Clicking/selecting a **Session node row** attaches/switches to that session (`attach <node>`) or toggles `[f]` freeze/thaw and `[b]` branch child session.
     - Clicking/selecting a **Theme preset row** in `:theme list` immediately previews/applies that theme (`:theme <preset>`).
  2. **Action Hotkeys & Header Controls (`MCP-04`)**: While `:mcp` is open, pressing `r` refreshes live telemetry, `f` freezes/thaws the selected session node, `n` spawns a child session branch under the selected node, `m` toggles Floating (`Overlay`) vs. Push-Out (`Push`), and `d` cycles the dock edge (`Top`/`Bottom`/`Left`/`Right`). Live telemetry (`build_mcp_status_lines` / `build_full_tree_lines`) automatically refreshes when open so pane counts, dimensions, and session states stay current in real time.

**Plans**: 3/3 plans complete

Plans:
- [x] 36-01-PLAN.md — `src/mcp_console/mod.rs`: Interactive row metadata (`McpRowAction`: `FocusPane(PaneId)`, `SessionNode(String)`, `ApplyTheme(String)`, `None`), `selected_row` cursor state, and live view auto-refresh hook (wave 1)
- [x] 36-02-PLAN.md — `src/ui/mcp_ui.rs`, `src/ui/mod.rs`, `src/input/modal.rs`, & `src/input/mouse_wm.rs`: Highlighted selected row rendering, `Enter`/click dispatch (`Pane` focus, `Session` attach/freeze/branch, `Theme` apply), and action hotkeys (`r`, `f`, `n`, `m`, `d`) (wave 2)
- [x] 36-03-PLAN.md — `tests/mcp_interactive_navigation.rs`: Unit and integration tests verifying keyboard/mouse row selection, pane focus jump, theme live-apply, session freeze/branch, and live telemetry refresh inside the `:mcp` window (wave 3)

### Phase 37: Interactive File & `.itermcolors` Browser, Cursor Customizer & Multi-Profile Layout Manager (`v2.1.1`–`v2.2.0`)

**Goal**: As a user customizing workspaces and color schemes, I want multi-profile layout templates with automatic layout saving, WCAG 2.1 contrast enforcement, an interactive directory & `.itermcolors` theme browser (`:browse`, `:theme browse`, `:theme import`), and a full cursor customizer (`:cursor`), so that layouts, themes, and cursors can be discovered and configured interactively.
**Depends on**: Phase 36
**Requirements**: PROF-01, THM-03, CUR-01
**Plans**: 3/3 plans complete

### Phase 38: Session Lifecycle Commands, Persistent Ordering, Blinking Software Cursor & Scaled Mouse Selection (`v2.3.0`)

**Goal**: As a multi-session user, I want first-class session creation/renaming/reordering (`:session new|blank|rename|switch|delete|move|reorder|swap`, `bilterm new|blank`), software cursor blinking (`500ms`/`250ms`), and accurate mouse text selection & top menu bar hit-testing across scaled and normal panes.
**Depends on**: Phase 37
**Requirements**: SESS-01, SESS-02, CUR-02
**Plans**: 3/3 plans complete

### Phase 39: Per-Window Themes, Outlines, Bold Borders, `[_][□][X]` Controls & `MIN_GRID_DIM` `2×2` Clamp (`v2.4.0`)

**Goal**: As a user managing multiple composite windows, I want each window to have its own independent theme preset, border outline (`double`, `single`/`thin`, `rounded`, `thick`, `dotted`, `dashed`, `heavy-quad`, `ascii`), and bold border setting (`:window theme|outline|bold|reset|show`), clearer `[_][□][X]` window controls, and `MIN_GRID_DIM = 2×2` VT grid clamping to prevent 1-row soft-wrap underflow during 16-way splits.
**Depends on**: Phase 38
**Requirements**: WIN-01, WIN-02, VT-CLAMP-01
**Success Criteria** (what must be TRUE):

  1. `:window theme|outline|bold|reset|show|global-bold` (aliases `:win`, `:pane-style`, `:theme window <name>`, `:outline pane <style>`) customize each window's theme palette, outline style, and bold border independently, and persist `theme`, `outline`, and `bold` in saved session TOML files.
  2. Window title bars render `[_]` minimize, `[□]` maximize, and bold red `[X]` close controls.
  3. VT grids clamp dimensions to `>= 2×2` (`MIN_GRID_DIM` in `src/vt/mod.rs`), preventing `vt100-ctt` `Grid::col_wrap` underflow on 1-row panes.

**Plans**: 2/2 plans complete

Plans:
- [x] 39-01-PLAN.md — `src/wm/chrome_style.rs`, `src/wm/window_style.rs`, `src/ui/chrome.rs`, & `src/ui/glyphs.rs`: Per-window `PaneChromeStyle` (`theme`, `outline`, `bold`), `[_][□][X]` window controls, and session TOML persistence (wave 1)
- [x] 39-02-PLAN.md — `src/vt/mod.rs`: `MIN_GRID_DIM` `2×2` clamp and multi-split regression verification (wave 2)

### Phase 40: Interactive Per-Window Theme Browser & Per-Window Element Color Overrides (`v2.4.1`)

**Goal**: As a user styling individual terminal windows, I want an interactive Per-Window Theme Browser (`:window theme`, `:window theme list`, `:window theme browse [dir]`, `:window theme import [path|dir|glob]`), an interactive Window Style & Element Color Customizer (`:window` / `:window menu`), and granular per-window color overrides for 13 individual UI elements (`font-color`, `cursor-color`, `frame-color`, `inactive-border`, `controls-color`, `status-color`, `badge-color`, `title-color`, `bg`, `prompt-bg`, `prompt-color`, `crosshair-color`, `shadow-color`) with session TOML persistence.
**Depends on**: Phase 39
**Requirements**: WIN-03, WIN-04
**Success Criteria** (what must be TRUE):

  1. Running `:window theme` or `:window theme list` opens an interactive Per-Window Theme Browser in the `:mcp` console window (`window_theme_target`) where selecting any built-in preset or discovered `.itermcolors`/TOML/YAML scheme applies it exclusively to that window without altering the global workspace theme or sibling windows.
  2. Running `:window theme browse [dir]` / `:window browse [dir]` or `:window theme import [file|dir|glob]` routes interactive `.itermcolors` file selection exclusively to the target window.
  3. Running `:window <element> <color|reset>` (or `:window <elem> color <color>`, or `:window <element>` to open that element's interactive swatch picker) overrides any of the 13 theme elements (`font-color`, `cursor-color`, `frame-color`, `inactive-border`, `controls-color`, `status-color`, `badge-color`, `title-color`, `bg`, `prompt-bg`, `prompt-color`, `crosshair-color`, `shadow-color`) on the focused window and persists `colors` in saved session TOML files (`SavedPaneState.colors`).

**Plans**: 3/3 plans complete

Plans:
- [x] 40-01-PLAN.md — `src/ui/theme.rs`, `src/ui/contrast.rs`, `src/ui/chrome.rs`, `src/ui/status_bar.rs`, `src/ui/cursor_cmd.rs`, & `src/wm/chrome_style.rs`: Per-window `text_fg`, `controls_fg`, `cursor_fg`, and 13-element `PaneChromeStyle.colors` override synthesis + session TOML persistence (wave 1)
- [x] 40-02-PLAN.md — `src/wm/window_style.rs`, `src/colon_command/parse_visual.rs`, `src/mcp_console/mod.rs`, `src/wm/v2_runtime.rs`, & `src/pane/completion.rs`: Interactive `:window` customizer, `:window theme` browser (`window_theme_target`), per-window `.itermcolors` file browser routing, and `<Tab>` completions (wave 2)
- [x] 40-03-PLAN.md — Unit & integration test suite verification (`309` integration + `200` lib tests) and documentation/manual sync for `v2.4.1` (wave 3)

### Phase 41: 2D Rectangular Block Select/Highlight/Copy & Full Per-Pane Session Shell, Running Command & CWD Persistence (`v2.5.0`)

**Goal**: As a power user managing multi-window sessions, I want first-class 2D rectangular block selection, highlighting, and copying via mouse (`Alt+LeftDrag` or persistent `:rect mode`), keyboard (`Alt+R`, `v`/`r` in Copy Mode), and `:rect`/`:select`/`:copy` commands, and I want every saved session and crash journal to persist and restore not only the window layout and chrome, but also the shell running (`shell`, `shell_args`), the software/commands running (`running_command`), and the current working directory (`cwd`) in every window pane.
**Depends on**: Phase 40
**Requirements**: RECT-01, SESS-03
**Success Criteria** (what must be TRUE):

  1. User can select, highlight, and copy 2D rectangular blocks via `Alt+R`, `Alt+LeftDrag` on the output canvas, persistent `:rect mode [on|off|toggle]`, coordinate-based `:rect <r1> <c1> <r2> <c2> [copy|highlight]`, `:rect all [copy]`, `:rect copy`, `:rect cut`, and `:rect clear` (aliases `:select`, `:copy rect`).
  2. Every saved session TOML (`SavedPaneState`) and crash-recovery journal (`PaneJournalRecord`) captures each pane's `shell`, `shell_args`, `running_command` (via prompt submission tracking and `/proc/<fg_pgid>/cmdline`), `cwd` (via `OSC 7`, `/proc/<pid>/cwd`, and `cd` tracking), and `remote_target`.
  3. Restoring or switching sessions spawns each pane with its saved `shell`, `shell_args`, and `cwd` and automatically relaunches its saved `running_command` when `session_restore_processes` is enabled (`true` by default), with runtime controls `:session processes [on|off|toggle]`, `:session cmd`, `:session shell`, `:session cwd`, `:session relaunch`, and `:session info`.

**Plans**: 3/3 plans complete

Plans:
- [x] 41-01-PLAN.md — `src/pane/modes.rs`, `src/pane/scroll.rs`, `src/input/mouse.rs`, `src/input/modal.rs`, `src/input/keymap.rs`, `src/ui/oia.rs`, `src/action.rs`, & `src/colon_command/`: 2D rectangular block select, persistent highlight, and copy (`:rect`, `:select`, `:copy rect`, `Alt+R`, `Alt+LeftDrag`) (wave 1)
- [x] 41-02-PLAN.md — `src/wm/session.rs`, `src/wm/layout_profile.rs`, `src/wm/journal.rs`, `src/wm/session_manager.rs`, `src/pane/mod.rs`, `src/pane/scroll.rs`, & `src/config/mod.rs`: Per-pane `shell`, `shell_args`, `running_command`, and `cwd` persistence, auto-relaunch (`session_restore_processes`), and `:session processes|cmd|shell|cwd|relaunch|info` commands (wave 2)
- [x] 41-03-PLAN.md — Unit & integration test suite verification, full documentation & cross-link synchronization, and `v2.5.0` multi-platform release binaries (wave 3)

### Phase 42: Non-Retro Modern Minimal & Liquid Glass Modes, Per-Window vs. Global Theme & Cursor Isolation, Drawer Controls, RAW F-Keys & SSH Hardening (`v2.5.1`–`v2.5.2`)

**Goal**: As a user who wants either classic DOS/3270 retro chrome or a sleek, minimal modern/liquid-glass terminal aesthetic — plus independent per-window vs. global control over themes and cursors, enhanced drawer ergonomics, RAW mode F-key routing, and clean SSH login without pre-login `PROMPT_COMMAND` echo — I want first-class runtime and CLI switches for all of these capabilities.
**Depends on**: Phase 41
**Requirements**: MOD-01, LIQ-01, WIN-05, CUR-03, SSH-02
**Success Criteria** (what must be TRUE):

  1. `--modern` (`--minimal`, `--non-retro`) and `--liquid-glass` (`--liquid`), plus `:modern [on|off|toggle]`, `:liquid [on|off|toggle]`, `:style [classic|snazzy|glass|both|modern|liquid]`, and `F11` switch between Classic Borland/3270 Retro, Snazzy, Frosted Glass, Modern Minimal (`╭─╮` rounded chrome, obsidian slate palette, `● 1 · PANE #1` headers, `❯ Command · TOP` prompt bar, `◢ Bilterm` status pills), and Liquid Glass (optical specular top/left bevel, bottom/right chromatic refraction edge, and 2-cell ambient drop halo).
  2. Changing theme (`:theme <preset>`, `:theme` picker, `:itermcolors` picker) or cursor (`:cursor <args>`, `:cursor cursor-color <color>`, `:window cursor-color <color>`) applies to the currently focused active window pane by default without mutating sibling windows, while `:theme global <preset>` / `:theme all <preset>` and `:cursor global <args>` / `:cursor all <args>` update all windows globally (allowing any combination of global and per-window themes/cursors).
  3. Slide-Out Drawer (`F7` / `:drawer`) supports `[_][□][X]` minimize/maximize/close controls, `Ctrl+X` leader chords while focused, top-level `:save [name]` and `:rename [old] <new>` session commands, and configurable `raw_fkeys` (`bilterm|app|auto`, `Shift+F1..F12` escape passthrough).
  4. `:ssh` and `:ssh-tmux` work with or without arguments (`:ssh` / `:ssh list`, `:ssh [-p <port>] <user@host> [port]`), appear in `<Tab>` completion (including `~/.ssh/config` `Host` aliases) and `<F1>` `:help`, and spawn SSH sessions at the split pane's exact viewport size without echoing `PROMPT_COMMAND=...` before/after login.

**Plans**: 3/3 plans complete

Plans:
- [x] 42-01-PLAN.md — `src/ui/visual_style.rs`, `src/ui/oia.rs`, `src/ui/status_bar.rs`, `src/ui/mod.rs`, `src/config/mod.rs`, `src/main.rs`: Non-Retro Modern Minimal (`--modern`, `:modern`) and Liquid Glass (`--liquid-glass`, `:liquid`), Drawer controls, `:save`/`:rename`, and RAW F-Key routing (wave 1)
- [x] 42-02-PLAN.md — `src/wm/chrome_style.rs`, `src/wm/v2_runtime.rs`, `src/ui/file_browser.rs`, `src/ui/cursor_cmd.rs`, `src/remote/mod.rs`, `src/wm/session.rs`, `src/colon_command/parse.rs`, `src/pane/completion.rs`: Per-window vs. global `:theme` and `:cursor` (`:cursor cursor-color`), and `:ssh` parsing, `<Tab>` completion, `<F1>` help, and `PROMPT_COMMAND` echo fix (wave 2)
- [x] 42-03-PLAN.md — `tests/slide_out_drawer_and_telemetry.rs` regression suite, `.planning/ROADMAP.md` & `.planning/STATE.md` backfill, and full verification (wave 3)

### Phase 43: UI Modernization Step 1 — Architecture Audit & Plan Realignment

**Goal**: Produce Phase 0 audit deliverables with zero behavior change and realign the roadmap.
**Depends on**: Phase 42
**Requirements**: UIM-00
**Governing documents**: `spec/detailed_specification_EXISTING_TERMINAL_APPLICATION_UI_MOD.md`, `spec/pre_implemenation_and_goals.md`, `spec/Recommended execution sequence` (Phases 43–48 map 1:1 to its six steps).
**Harmony rule**: Add new paths; do not remove or rewire existing ones. Classic/Snazzy/Glass/Modern/Liquid styles, all CLI switches (`--modern`, `--liquid-glass`, `--glass`, `--gui`, ...) and `:` commands (`:style`, `:modern`, `:liquid`, `:theme`, `:window ...`) keep working and resolve through the new token layer. New features lower to `Action` and run via `WindowManager::apply`.
**Terminal engine rule**: `vt100-ctt` stays the default behind `TerminalBackend`. `libghostty-vt` (feature `ghostty-vt`) may be used where it adds value as an additional, selectable backend living alongside the default; never a forced replacement.
**File size**: `ARCH-R04` is a soft ~800–1000-line goal (1200 test backstop); split for modularity, not for a number.
**Success Criteria** (what must be TRUE):

  1. `AUDIT.md` covers the 9 spec Phase 0 deliverables and 15-point assessment.
  2. Native-Rust GUI finding recorded (ratatui buffer → `GuiWorkspaceRenderer` → Win32/X11/CoreText).
  3. clippy `-D warnings` and `cargo test -q` green; no behavior change.

**Plans**: 1/1 plans complete

Plans:
- [x] 43-01-PLAN.md — Persist audit, fix stale `CLAUDE.md` stack list, verify baseline (wave 1)

### Phase 44: UI Modernization Step 2 — Visual Design Proposal with Working Previews

**Goal**: Show before building: proposed redesign with real previews from the existing renderer.
**Depends on**: Phase 43
**Requirements**: UIM-01
**Governing documents**: `spec/detailed_specification_EXISTING_TERMINAL_APPLICATION_UI_MOD.md`, `spec/pre_implemenation_and_goals.md`, `spec/Recommended execution sequence` (Phases 43–48 map 1:1 to its six steps).
**Harmony rule**: Add new paths; do not remove or rewire existing ones. Classic/Snazzy/Glass/Modern/Liquid styles, all CLI switches (`--modern`, `--liquid-glass`, `--glass`, `--gui`, ...) and `:` commands (`:style`, `:modern`, `:liquid`, `:theme`, `:window ...`) keep working and resolve through the new token layer. New features lower to `Action` and run via `WindowManager::apply`.
**Terminal engine rule**: `vt100-ctt` stays the default behind `TerminalBackend`. `libghostty-vt` (feature `ghostty-vt`) may be used where it adds value as an additional, selectable backend living alongside the default; never a forced replacement.
**File size**: `ARCH-R04` is a soft ~800–1000-line goal (1200 test backstop); split for modularity, not for a number.
**Success Criteria** (what must be TRUE):

  1. Previews for Obsidian, Midnight, Arctic, Liquid across pane, split, drawer, palette, settings.
  2. Existing→redesigned component mapping and TUI-vs-GUI blur/radius fallback documented.
  3. User approves proposal before Phase 45.

**Plans**: 1/1 plans complete

Plans:
- [x] 44-01-PLAN.md — Preview harness, proposal doc, sign-off gate (wave 1)

### Phase 45: UI Modernization Step 3 — Design System: Tokens, Materials, Themes & `[appearance]` Config

**Goal**: One reusable design system: tokens, five materials, eight profiles, glass controls with opaque fallback, `[appearance]` config.
**Depends on**: Phase 44
**Requirements**: UIM-02
**Governing documents**: `spec/detailed_specification_EXISTING_TERMINAL_APPLICATION_UI_MOD.md`, `spec/pre_implemenation_and_goals.md`, `spec/Recommended execution sequence` (Phases 43–48 map 1:1 to its six steps).
**Harmony rule**: Add new paths; do not remove or rewire existing ones. Classic/Snazzy/Glass/Modern/Liquid styles, all CLI switches (`--modern`, `--liquid-glass`, `--glass`, `--gui`, ...) and `:` commands (`:style`, `:modern`, `:liquid`, `:theme`, `:window ...`) keep working and resolve through the new token layer. New features lower to `Action` and run via `WindowManager::apply`.
**Terminal engine rule**: `vt100-ctt` stays the default behind `TerminalBackend`. `libghostty-vt` (feature `ghostty-vt`) may be used where it adds value as an additional, selectable backend living alongside the default; never a forced replacement.
**File size**: `ARCH-R04` is a soft ~800–1000-line goal (1200 test backstop); split for modularity, not for a number.
**Success Criteria** (what must be TRUE):

  1. New `src/ui/design/` module; existing flags/`ThemePalette`/`PaneChromeStyle` resolve through it with identical default output (existing snapshots unchanged).
  2. Old configs load unchanged; missing keys default; invalid values degrade gracefully; no PTY effects.
  3. Profiles select/duplicate/modify/save/import/export/reset via `:appearance ...` Actions.

**Plans**: 4/4 plans complete (criterion 3 gap: see phases/45-design-system/GAPS.md)

Plans:
- [x] 45-01-PLAN.md — Tokens, materials, motion clock (wave 1)
- [x] 45-02-PLAN.md — Profiles, `[appearance]` config, migration tests (wave 2)
- [x] 45-03-PLAN.md — Re-point existing styles at tokens; equivalence tests (wave 3)
- [x] 45-04-PLAN.md — Bundled JetBrains Mono for non-Legacy profiles (wave 3)

### Phase 46: UI Modernization Step 4 — Main Interface: Shell, Display Modes, Workspace Navigation & Pane Chrome

**Goal**: Modernize shell and workspace presentation.
**Depends on**: Phase 45
**Requirements**: UIM-03
**Governing documents**: `spec/detailed_specification_EXISTING_TERMINAL_APPLICATION_UI_MOD.md`, `spec/pre_implemenation_and_goals.md`, `spec/Recommended execution sequence` (Phases 43–48 map 1:1 to its six steps).
**Harmony rule**: Add new paths; do not remove or rewire existing ones. Classic/Snazzy/Glass/Modern/Liquid styles, all CLI switches (`--modern`, `--liquid-glass`, `--glass`, `--gui`, ...) and `:` commands (`:style`, `:modern`, `:liquid`, `:theme`, `:window ...`) keep working and resolve through the new token layer. New features lower to `Action` and run via `WindowManager::apply`.
**Terminal engine rule**: `vt100-ctt` stays the default behind `TerminalBackend`. `libghostty-vt` (feature `ghostty-vt`) may be used where it adds value as an additional, selectable backend living alongside the default; never a forced replacement.
**File size**: `ARCH-R04` is a soft ~800–1000-line goal (1200 test backstop); split for modularity, not for a number.
**Success Criteria** (what must be TRUE):

  1. `:mode standard|minimal|zen`; Zen hides chrome, all actions still reachable.
  2. Pane radius/border/spacing/title/focus configurable; key delivery unchanged.
  3. Existing workspace/session system extended, not duplicated.

**Plans**: code complete (46-04..46-09 done); live-GUI pass pending

Plans:
- [ ] 46-01-PLAN.md — Display modes + chrome (wave 1) — 46-04, 46-05, 46-06 done; 46-06b resolved (prompt is already one text row)
- [ ] 46-04-PLAN.md — Drawer & MCP console: resize, scale, horizontal and vertical scroll (user request 2026-11-02) — DONE (code)
- [ ] 46-02-PLAN.md — Navigator + pane chrome options (wave 2)
- [ ] 46-03-PLAN.md — Contextual pane actions + tests (wave 3)

### Phase 47: UI Modernization Step 5 — Drawers, Floating Panels, Command Palette, Context Menus & Live-Preview Appearance Settings

**Goal**: Apply the system to drawers, panels, palette, menus, dialogs; add live-preview Appearance modal.
**Depends on**: Phase 46
**Requirements**: UIM-04
**Governing documents**: `spec/detailed_specification_EXISTING_TERMINAL_APPLICATION_UI_MOD.md`, `spec/pre_implemenation_and_goals.md`, `spec/Recommended execution sequence` (Phases 43–48 map 1:1 to its six steps).
**Harmony rule**: Add new paths; do not remove or rewire existing ones. Classic/Snazzy/Glass/Modern/Liquid styles, all CLI switches (`--modern`, `--liquid-glass`, `--glass`, `--gui`, ...) and `:` commands (`:style`, `:modern`, `:liquid`, `:theme`, `:window ...`) keep working and resolve through the new token layer. New features lower to `Action` and run via `WindowManager::apply`.
**Terminal engine rule**: `vt100-ctt` stays the default behind `TerminalBackend`. `libghostty-vt` (feature `ghostty-vt`) may be used where it adds value as an additional, selectable backend living alongside the default; never a forced replacement.
**File size**: `ARCH-R04` is a soft ~800–1000-line goal (1200 test backstop); split for modularity, not for a number.
**Success Criteria** (what must be TRUE):

  1. Drawers, MCP deck, exit guard, file browser, help/inspector use materials.
  2. Palette grouped/fuzzy with shortcuts, Action-only execution.
  3. Appearance changes apply live, persist via existing config, never restart sessions.

**Plans**: 0/3 plans complete

Plans:
- [ ] 47-01-PLAN.md — Drawers/panels/dialogs (wave 1)
- [ ] 47-02-PLAN.md — Palette + context menus (wave 2)
- [ ] 47-03-PLAN.md — Appearance modal + live preview (wave 3)

### Phase 48: UI Modernization Step 6 — Regression, Performance & Polish

**Goal**: Verify nothing regressed; polish.
**Depends on**: Phase 47
**Requirements**: UIM-05
**Governing documents**: `spec/detailed_specification_EXISTING_TERMINAL_APPLICATION_UI_MOD.md`, `spec/pre_implemenation_and_goals.md`, `spec/Recommended execution sequence` (Phases 43–48 map 1:1 to its six steps).
**Harmony rule**: Add new paths; do not remove or rewire existing ones. Classic/Snazzy/Glass/Modern/Liquid styles, all CLI switches (`--modern`, `--liquid-glass`, `--glass`, `--gui`, ...) and `:` commands (`:style`, `:modern`, `:liquid`, `:theme`, `:window ...`) keep working and resolve through the new token layer. New features lower to `Action` and run via `WindowManager::apply`.
**Terminal engine rule**: `vt100-ctt` stays the default behind `TerminalBackend`. `libghostty-vt` (feature `ghostty-vt`) may be used where it adds value as an additional, selectable backend living alongside the default; never a forced replacement.
**File size**: `ARCH-R04` is a soft ~800–1000-line goal (1200 test backstop); split for modularity, not for a number.
**Success Criteria** (what must be TRUE):

  1. Spec §13 checklist verified (I/O, shells, sessions, panes, scrolling, selection, clipboard, shortcuts, restore, config, themes, resize, shutdown, recovery).
  2. High-volume, long-running and appearance-change-while-active tests pass; no frame-time regression.
  3. Reduced motion/transparency and contrast checks pass; HiDPI checked where available; release built.

**Plans**: 0/2 plans complete

Plans:
- [ ] 48-01-PLAN.md — Regression + perf + accessibility (wave 1)
- [ ] 48-02-PLAN.md — Polish, docs, release (wave 2)

### Milestone v2.9 — Agent Fleet (Phases 50–52)

Goal: make Bilterm a place where several AI agents run side by side, are visible at a glance, and can talk to each other. Phase 50 is the human-facing view; Phase 51 is a UI and command-parity tweak pass on it; Phase 52 is the agent-facing channel. 52 builds on 50's agent-state model, so run 50 first; 51 runs before 52 so the agents-list window exists when messaging lands.

### Phase 50: Agent-Aware Workspace Navigator (Left Sidebar)

**Goal**: The slide-out drawer (Phase 24, scaled/pinnable since v2.8.0) looks and works like a compact left-side navigator: an always-available tree listing workspaces, sessions and panes with a live state per agent, so you can see who is working, blocked, idle or done and jump straight to it.
**Depends on**: Phase 47 (drawer on the design system); reuses `WorkspaceDrawer`, `DrawerRow`, pin (`Fixed`) and panel scale from v2.8.0.
**Requirements**: FLT-01, FLT-02, FLT-03
**Harmony rule**: Add new paths; do not remove or rewire existing ones. The current drawer rows, `F7`, `:drawer ...`, Legacy looks and every existing key keep working. The new layout is an additional `[drawer] style = "classic" | "navigator"` (default `classic` until signed off). New features lower to `Action` and run via `WindowManager::apply`.
**Success Criteria** (what must be TRUE):

  1. Tree view: workspace > session > pane, one line per pane, collapse/expand, current pane marked, narrow by default (about 24-30 columns), pinned-open friendly.
  2. Agent state per pane (working / waiting-for-input or blocked / idle / done / errored), derived from existing signals (OSC 133 phase, output activity, exit code, running command), shown as a state dot plus a per-workspace count summary. No per-agent configuration needed.
  3. Keyboard-first: `j`/`k`/arrows, Enter focuses the pane, a "jump to next blocked/needs-attention" key, new-session/new-pane/rename/close from the list, all with `:drawer` command equivalents.
  4. Mouse parity: click to focus, wheel scroll, Ctrl+wheel scale, resize by dragging the edge; works in TUI and GUI. *The GUI part is conditional on the 50-13 checkpoint decision:* `fix-all` = also in real GUI windows on Linux, Windows and macOS; `fix-linux` = also in the real X11 GUI, while on Windows/macOS Ctrl+wheel over a panel still zooms the font (`[` / `]` and `:... scale` do scale panels there); `scope-out` = proven only on the headless GUI surface, real GUI windows keep font zoom and panel scale stays a no-op there (documented known gap). 50-14 records which option holds.
  5. Legacy parity: with `style = "classic"` output is byte-identical to today; snapshot tests prove it.

**Plans**: 16/16 plans complete — scope expanded by 50-CONTEXT.md decisions D-01..D-10: FLT-01 (sidebar), FLT-02 (traffic bus, zoom window, snoop), FLT-03 (GUI vector rendering); every key and mouse gesture lowers to `Action` via `WindowManager::apply`, key tables in `src/input/keymap.rs`; SC4's GUI part is conditional on the 50-13 checkpoint (see criterion 4)

Plans:
- [x] 50-01-PLAN.md — Classic-drawer golden snapshots (cells + style digest + GUI digest) and immutability gate, before any src edit (wave 1)
- [x] 50-02-PLAN.md — `[drawer] style = "navigator"`: workspace > session > pane tree rows + renderer, `sanitize_display` (wave 2)
- [x] 50-03-PLAN.md — `AgentState` classifier/tracker, `[agents.<name>]` profiles, `:agents list` (wave 2)
- [x] 50-04-PLAN.md — State dots, per-workspace counts, jump-to-attention (`:drawer attention`, leader `!`), `:drawer style` (wave 3)
- [x] 50-05-PLAN.md — Traffic-event bus fed by broadcast, `:mcp broadcast`, RPC/MCP; ADR-008 contract for Phase 52 (wave 3)
- [x] 50-06-PLAN.md — Navigator keyboard: `NAVIGATOR_DRAWER_BINDINGS` in keymap.rs, full `DrawerNavOp` set with `:drawer` verbs (incl. `close-pane`), drift/no-shadow tests, pin/scale, throttled animation (wave 4)
- [x] 50-07-PLAN.md — Agent traffic zoom window skeleton: state, braille graph, stable layout, selection model, render (wave 4)
- [x] 50-08-PLAN.md — Zoom window keys: `AGENTS_WINDOW_BINDINGS` in keymap.rs, `:agents zoom/close/dismiss/select/edge/enter`, leader `a`, Esc, node/edge cursor, Enter-to-focus, op drift tests, redraw cadence (wave 5)
- [x] 50-09-PLAN.md — Zoom window chrome: title-drag/Shift+arrows move, edge-drag/Alt+arrows resize, maximize/close badges, splitter, node/edge clicks, node drag + grab mode, parity test (wave 6)
- [x] 50-10-PLAN.md — Snoop view: Ctrl+O pane focus, follow/filter/scroll/copy by key, mouse and command, disabled `send as me` compose line (wave 8)
- [x] 50-11-PLAN.md — GUI vector rendering: AA curved edges, flow particles, glow, sidebar state icons (wave 8)
- [x] 50-12-PLAN.md — Row comms indicators (pair / fan-out) opening the window on an edge, comms keys in keymap.rs (wave 9)
- [x] 50-13-PLAN.md — GUI runtime scope decision (checkpoint) + panel-scale/modifier wiring in GUI runners (wave 8)
- [x] 50-14-PLAN.md — User guide chapter 11 + doc coverage script, config/reference/API docs, SC4 outcome, final phase gate (wave 10)
- [x] 50-15-PLAN.md — Navigator sidebar mouse parity: row and fold-marker clicks, footer buttons, title badges, wheel, edge drag, key/mouse parity test (wave 6)
- [x] 50-16-PLAN.md — Zoom window pin badge + independent panel scale (`PanelTarget::Agents`) by key, mouse and command (wave 7)

### Phase 50.1: Unified Registry & Find (INSERTED)

**Goal**: One searchable registry of agents, terminal sessions/panes and their command history (including connected SSH and tmux sessions), with `:find` commands to search each.
**Depends on**: Phase 50 (agent state, `agent_statuses()`), Phase 18 (SSH domains), Phase 11 (tmux -CC), Phase 41 (per-pane command persistence).
**Requirements**: FND-R01, FND-R02, FND-R03, FND-R04
**Uniformity (`UNI-R01`)**: every capability ships a mouse path, a GUI+TUI key sequence and colon/palette/RPC/MCP commands, with parity tests.
**Harmony rule**: Add new paths; existing `:search` (scrollback) and history behaviour unchanged. Every key, mouse, colon and RPC path lowers to `Action` via `WindowManager::apply`. History capture is bounded, redacted (no password/`!ECHO` input), and stored via `secure_fs`.
**Execution model**: maximally autonomous: research, then GSD plan, then parallel subagents with disjoint file ownership (explicit `model` per subagent).
**Success Criteria** (what must be TRUE):

  1. `FND-R01`: a registry (in `wm`) exposes agents, panes/sessions (local, SSH, tmux) and per-session command history with stable ids, source kind and recency; read-only query API.
  2. `FND-R02`: `:find agent|session|history <text>` return ranked results (fuzzy, as the palette does) in a selectable result list; Enter jumps to the pane/session.
  3. `FND-R03`: SSH and tmux sessions register and are searchable (host, session name, window/pane, and the commands run there where capturable); gaps documented.
  4. `FND-R04`: RPC/MCP `find.*` methods, palette entries, completion, help and docs; bounded memory and redaction tested.

**Plans**: 15/15 plans complete — locked decisions D-U1 (reserved first words), D-U2 (secrets dropped/masked, only a count shown), D-U3 (live first; every result carries its `source`) in 50.1-CONTEXT.md; shared interface in 50.1-CONTRACT.md; waves 0-4 with disjoint file ownership per wave

Plans:
- [x] 50.1-01-PLAN.md — Wave 0 seam (orchestrator-only): `Action::Find`/`FindOp`/`Modal::Find`, `:find`/`:finder` grammar, registry contract types and provider skeletons, key table + `Ctrl+Alt+F`/leader `?`, mouse/key hooks (wave 0)
- [x] 50.1-02-PLAN.md — Secret-looking command classifier `pane::redact::classify` (D-U2) (wave 1)
- [x] 50.1-03-PLAN.md — Live registry rows: agents, workspace session, terminal sessions with stable ids and `source` live (wave 1)
- [x] 50.1-04-PLAN.md — Fuzzy ranking with live-first group order (D-U3), palette parity (wave 1)
- [x] 50.1-05-PLAN.md — history.log `host=`/`transport=`/`cwd=` fields and `~exit` annotations, backward compatible (wave 1)
- [x] 50.1-06-PLAN.md — SSH / ssh-tmux / tmux -CC transport data, tmux window tracking, honest `metadata_only`/`cwd_trusted` (wave 1)
- [x] 50.1-07-PLAN.md — Find window rendering: tabs, rows, source tag on every row, masked count, footer chips, snapshots, GUI parity (wave 1)
- [x] 50.1-08-PLAN.md — Find window keys and mouse lowered to `Action`, key/mouse/colon parity (wave 1)
- [x] 50.1-09-PLAN.md — Completion, palette, help, View-menu entries for `:find`/`:finder` (wave 1)
- [x] 50.1-10-PLAN.md — Command capture: password-prompt leak fix, secret drop, bounded ring, host/cwd/exit join (wave 2)
- [x] 50.1-11-PLAN.md — Disk sources: saved-session files and history.log with source path, mask-on-read (wave 2)
- [x] 50.1-12-PLAN.md — `FindOp` execution: open/rerank/kind switch, jump, prefill (never runs), session switch (wave 2)
- [x] 50.1-13-PLAN.md — RPC `find.*` and MCP `bilterm_find` with `source` and `masked`; tool-count pins updated (wave 2)
- [x] 50.1-14-PLAN.md — User guide chapter 12, API/CHANGELOG/AGENTS docs, SSH/tmux gap table, doc coverage script (wave 3)
- [x] 50.1-15-PLAN.md — UNI-R01 parity matrix, D-U1..D-U3 end-to-end, bounds, `g:find` group, final gates (wave 4)

### Phase 51: UI Tweaks & Command Parity

**Goal**: Close the gaps found in GUI testing of Phase 50: a way to undo `:agents demo`, a command-palette entry for every colon verb, and `:agents list` as a real floating window that tiles like the other panels.
**Depends on**: Phase 50.1 (registry and `:find`, which runs first), Phase 50 (agents window, `AgentsOp`, traffic bus, panel pin/scale), Phase 47 (command palette).
**Requirements**: UIT-01, UIT-02, UIT-03, UIT-04
**Uniformity (`UNI-R01`)**: every capability ships a mouse path, a GUI+TUI key sequence and colon/palette/RPC/MCP commands, with parity tests.
**Harmony rule**: Add new paths; do not remove or rewire existing ones. Legacy output stays byte-identical (11 goldens). Every key, mouse and colon path lowers to `Action` and runs via `WindowManager::apply`; new paths get parity tests in the style of `tests/phase50_*`.
**Execution model**: maximally autonomous. Research and GSD planning come first (`/gsd-plan-phase 51`), then execution by dispatched subagents with maximum parallelism (disjoint file ownership per plan, orchestrator-only `Action`/`apply` wiring). Subagents get an explicit `model` (sonnet/haiku for planning and mechanical work).
**Success Criteria** (what must be TRUE):

  1. `UIT-01`: `:agents demo stop` closes the panes the demo created and removes the `[demo]` traffic events; a no-op with a status message when no demo is running; usage, completion and help catalog updated.
  2. `UIT-02`: a sweep of all `ColonCommand` verbs produces an inventory; each verb that can run without free-form arguments, or has a small fixed argument set, has a command-palette entry that triggers it (e.g. `drawer style navigator|classic`); verbs deliberately excluded are listed with the reason; a test fails when a new verb has neither.
  3. `UIT-03`: `:agents list` opens a floating, pinnable window showing only agents (name, state, pane), with key and mouse controls and `:agents` verbs (select, jump, pin, scale, close); non-modal; never steals PTY input unless focused; renders in TUI and GUI.
  4. `UIT-04`: the existing window-tiling function includes the agents-list window (and the other floating panels where missing) alongside windows when tiling is invoked.

**Plans**: 13/13 plans complete — research done (51-RESEARCH-A/B/C), frozen contract in 51-CONTRACT.md (decisions D-01..D-04, UNI-R01 paths per capability); wave 1-2 seam plans are orchestrator-only, wave 3 is eight parallel plans with disjoint file ownership; assumes Phase 50.1 executes first

Plans:
- [x] 51-01-PLAN.md — Tracer: `:agents list` opens a rendered AGENTS window end to end; frozen `AgentsListOp`/`Action::AgentsList`/`PanelTarget::AgentsList`/`AgentsOp::DemoStop` and full colon grammar (wave 1, orchestrator-only seam)
- [x] 51-02-PLAN.md — Seams: frozen list geometry and hit-test, panel-dock and demo-state identity hooks, summaries (wave 2, orchestrator-only seam)
- [x] 51-03-PLAN.md — `:agents demo stop` (UIT-01): spawned-pane-only close, `[demo]` event and topology purge, replace-on-rerun, unredacted demo events (wave 3, sonnet)
- [x] 51-04-PLAN.md — Palette parity (UIT-02): probe sweep test, `PALETTE_EXCLUSIONS`, ~120 new palette entries incl. `drawer style navigator|classic`, completion/help rows for the Phase 51 verbs (wave 3, sonnet)
- [x] 51-05-PLAN.md — Agents-list core (UIT-03): rows, filter, selection, scroll, jump, zoom, pin, scale, dock flag, geometry ops, live refresh (wave 3, sonnet)
- [x] 51-06-PLAN.md — Agents-list rendering (UIT-03): TUI window and GUI scaled-panel path, occluders, texture overlap (wave 3, sonnet)
- [x] 51-07-PLAN.md — Agents-list keys and mouse (UIT-03, UIT-01): `Ctrl+Alt+N`, leader `A`/`y`/`Y`, in-window table, mouse plan/apply handler, GUI wheel routing (wave 3, sonnet)
- [x] 51-08-PLAN.md — Agents-list commands and menu (UIT-03): executor status and data, grammar hardening, View/Window menu entries (wave 3, sonnet)
- [x] 51-09-PLAN.md — Panel tiling (UIT-04): dock strip in the `:arrange` family for the list, traffic window and MCP console, release paths, no-panel identity (wave 3, sonnet)
- [x] 51-10-PLAN.md — RPC/HTTP/ctl/MCP `agents.list` and `agents.demo` (UNI-R01 API path), bounded closed parameters, transport parity (wave 3, sonnet)
- [x] 51-11-PLAN.md — Docs: chapter 11, key/mouse reference, API, developer guide; doc coverage gate green (wave 4, haiku)
- [x] 51-12-PLAN.md — Uniformity parity matrix (UNI-R01) across colon, key, mouse, palette, menu, RPC, MCP; Phase 50 parity test extended (wave 4, sonnet)
- [x] 51-13-PLAN.md — Final gate: `scripts/verify.sh`, `check_classic_goldens.sh` (11), `check_phase50_doc_coverage.sh` count update, bookkeeping (wave 5, sonnet)

### Phase 51.1: Persistent SSH + tmux Sessions (INSERTED)

**Goal**: A tmux session on a remote host (e.g. the user's Mac) stays alive across SSH drops, and Bilterm reconnects to it correctly, instead of today's local-shell facade.
**Depends on**: Phase 11 (tmux -CC bridge), Phase 18 (SSH domains), Phase 50.1 (registry rows for SSH/tmux).
**Requirements**: PST-01, PST-02, PST-03, PST-04
**Source**: `.planning/debug/ssh-tmux-early-quit.md` (diagnosis; fix plan steps 1-7). Today `:ssh-tmux` starts no ssh or tmux, the bridge is test-only, plain ssh has no keepalive, exited panes vanish silently, restore fakes a remote badge.
**Uniformity (`UNI-R01`)**: every capability ships a mouse path, a GUI+TUI key sequence and colon/palette/RPC/MCP commands, with parity tests.
**Success Criteria** (what must be TRUE):

  1. `PST-01`: `:ssh-tmux` runs a real `ssh -tt` with ServerAlive options and a login-shell wrapper around `tmux -CC new-session -A -s`; status text is honest until connected.
  2. `PST-02`: bridge hardening: exit reason stored, `%0` seed dropped, ids reconciled on reattach, `WindowClose`, outbound commands pumped, `capture-pane` hydrate; reconnect state machine with backoff (fake-clock tests).
  3. `PST-03`: exited remote panes are held with a banner (exit code, reason) instead of vanishing; restore never shows a remote badge on a local shell.
  4. `PST-04`: shorter ControlPersist plus keepalive on `:ssh`; tracing of connect/exit/`%exit`; headless tests via `TmuxControlSubprocess::from_channels`; macOS PATH handling documented/tested.

**Plans**: 16/16 plans complete — diagnosis in `.planning/debug/ssh-tmux-early-quit.md`, frozen contract in 51.1-CONTRACT.md (planner defaults P-01..P-16; no user decision is locked yet, three open questions in 51.1-CONTEXT.md); wave 1 seam is orchestrator-only, wave 2 is ten parallel plans with disjoint file ownership; assumes Phases 50.1 and 51 execute first

Plans:
- [x] 51.1-01-PLAN.md — Seam and tracer (orchestrator-only): honest `:ssh-tmux` end to end through a `from_channels` transport, `Action::RemoteLink` grammar/executor/key rows, shared link types, pane tap and banner fields, respawn dispatcher, skeletons for waves 2-3 (wave 1)
- [x] 51.1-02-PLAN.md — ssh argv: `ssh -tt`, ServerAlive options, login-shell wrapper with macOS Homebrew PATH, no ControlMaster for the link, `:ssh` keepalive and `ControlPersist=10s`, `redact_ssh_argv` (wave 2, sonnet)
- [x] 51.1-03-PLAN.md — PTY transport: DCS-aware control-line splitter (interactive prompts stay visible), `PtyLinkDriver`/`PtyLinkConnector`, `PtySession::exit_code` (wave 2, sonnet)
- [x] 51.1-04-PLAN.md — Reconnect state machine: capped backoff, exit classification, banner text, fake-clock tests (wave 2, sonnet)
- [x] 51.1-05-PLAN.md — Bridge hardening: `%0` seed dropped, primary-pane adoption, id reconcile on reattach, `WindowClose`/layout removals, `%exit` reason, safe reply blocks, queued commands with reply kinds (wave 2, sonnet)
- [x] 51.1-06-PLAN.md — Hydrate: FIFO reply correlator, hydrate planner, `capture-pane` commands and capture rendering (wave 2, sonnet)
- [x] 51.1-07-PLAN.md — Exited remote panes held with a banner (exit code, last line), reconnect via respawn, link-banner rendering (wave 2, sonnet)
- [x] 51.1-08-PLAN.md — Honest labels: restore never badges a local shell, drawer TMUX badge only on mapped panes, registry rows live when linked (wave 2, haiku)
- [x] 51.1-09-PLAN.md — Catalogs and mouse-by-menu: completion, palette, help, Tmux menu items for `reconnect|disconnect|info|prefill` (wave 2, haiku)
- [x] 51.1-10-PLAN.md — Banner keys (`Enter`/`r`/`x`/`d`), chord proofs, banner click, password-safe by construction (wave 2, sonnet)
- [x] 51.1-11-PLAN.md — RPC/HTTP/ctl `remote.link`, MCP `bilterm_remote_link`, honest connect results, tool-count pins (wave 2, sonnet)
- [x] 51.1-12-PLAN.md — Link lifecycle: connect, spawn, drop handling, backoff supervisor, banners, user verbs, summary, tracing (wave 3, sonnet)
- [x] 51.1-13-PLAN.md — Link I/O pumps: outbound commands, input to `send-keys`, hydrate with held output, window closures, fleet pane ids, resize (wave 3, sonnet)
- [x] 51.1-14-PLAN.md — Docs: tmux chapter corrected, reference rows, troubleshooting, security notes, API, developer guide, changelog, doc gate (wave 4, haiku)
- [x] 51.1-15-PLAN.md — UNI-R01 parity matrix, end-to-end drop/reconnect/hydrate scenario, control-stream fuzz, tracing contract (wave 4, sonnet)
- [x] 51.1-16-PLAN.md — Final gate: `scripts/verify.sh`, `check_classic_goldens.sh` (11), doc gates, bookkeeping, real-Mac UAT script (wave 5, sonnet)

### Phase 51.2: First-Class Utility Windows & Script Studio (INSERTED)

**Goal**: Deliver three separate first-class floating, pinnable, and dockable utility windows: a `FONTS` window for real-time typography preview and configuration, a workbench-style pinned `COMMAND PALETTE` window with scrollable command execution history, and a `SCRIPT STUDIO` window with a multi-line editor, single-step debugger, live action recorder, and secure script persistence.
**Depends on**: Phase 51 (utility window & docking foundations), Phase 50.1 (registry find & command history).
**Requirements**: WIN-01, WIN-02, WIN-03
**Uniformity (`UNI-R01`)**: Every utility window supports keyboard navigation, mouse interactions (title bar chips, dragging, resizing), colon commands, fuzzy command palette entries, top menubar items, JSON-RPC, and MCP tools.
**Success Criteria** (what must be TRUE):

  1. `WIN-01` (`FONTS` Window): Opened by `:fonts [verb]`, `:font list`, `:font presets`, `:font scan [dir]`, and menubar `View -> Fonts Window`. Supports `[•]` Pin (unpinned = `Enter` applies font and dismisses; pinned = stays open for live A/B comparison across presets & custom fonts), `[□]` Dock into tiling strip, `[×]` Close, `+`/`-` point-size step, `[`/`]` scale step, `a` apply to all panes, `s` rescan custom fonts dir, and drag/resize.
  2. `WIN-02` (Pinnable `COMMAND PALETTE` Window): Opened by `:commands [verb]`, `:palette [verb]`, or pinning `Ctrl+P` (`Alt+P` / `*` in `Modal::Palette`). Supports unpinned one-shot execution (dismisses on `Enter`/`Esc`) and pinned workbench mode (`[•]` Pin keeps it open, executes commands repeatedly, and records command + output status in a scrollable `OUTPUT / HISTORY` pane at the bottom).
  3. `WIN-03` (`SCRIPT STUDIO` Window): Opened by `:script [verb]` or `:studio [verb]`. Multi-line `:colon` & shell script editor + `[▶ Run (F5)]`, `[⏭ Step (F10)]` single-step debugger with program counter `▶`, `[↺ Reset]`, `[● Rec (Ctrl+R)]` live action recorder, `[✎ Edit (i)]`, `[💾 Save]` / `[📂 Load]` (`~/.config/bilterm/scripts/*.bt` via `secure_fs`), and live `RUN LOG` pane.

**Plans**: 4/4 plans complete

Plans:
- [x] 51.2-01-PLAN.md — Window 1 (`WIN-01`): `FONTS` Window + `:font list` / `:font scan` pop-up integration + `tests/phase51_2_fonts_window.rs` (wave 1, sonnet)
- [x] 51.2-02-PLAN.md — Window 2 (`WIN-02`): Pinnable `COMMAND PALETTE` Window + `Ctrl+P` pin promotion + `tests/phase51_2_command_window.rs` (wave 1, sonnet)
- [x] 51.2-03-PLAN.md — Window 3 (`WIN-03`): `SCRIPT STUDIO` Window + live recorder + single-step runner + `tests/phase51_2_script_studio.rs` (wave 1, sonnet)
- [x] 51.2-04-PLAN.md — Wave 2 integration merge, menubar/completion/help parity, full verification (`scripts/verify.sh`, `scripts/check_classic_goldens.sh`), and release build (wave 2, sonnet)

### Phase 52: Agent-to-Agent Messaging & Coordination

**Goal**: AI agents running in different panes or sessions can address each other, pass structured messages, wait for replies, and be coordinated, without the sender needing to know terminal internals.
**Depends on**: Phase 49 (API/MCP surface), Phase 50 (`AgentState`).
**Requirements**: AGT-01, AGT-02, AGT-03
**Today (baseline)**: agents can only talk by keystroke injection: `pane.send_keys` over RPC/HTTP/MCP, `:mcp broadcast <subtree> <cmd>`, shared files, and `GET /events`. There is no mailbox, no addressing by name, no delivery receipt, and no reply path.
**Harmony rule**: Add new paths; existing RPC methods, `pane.send_keys`, `:mcp broadcast` and their gating are unchanged. New features lower to `Action`, are reachable from `bilterm ctl`, HTTP, MCP and `:` commands, and are documented in `docs/API.md`.
**Success Criteria** (what must be TRUE):

  1. Addressing and mailbox (`AGT-01`): panes/sessions get stable agent names (`:agent name <n>`, env `BILTERM_AGENT`); `agent.send {to, body, kind, reply_to?}`, `agent.inbox`, `agent.ack`, `agent.list` exist as RPC methods and MCP tools; messages are queued per recipient, ordered, size-limited and survive a pane redraw; an unknown recipient is a clear error, not a silent drop.
  2. Delivery modes (`AGT-02`): `mailbox` (read via API/MCP resource `bilterm://agent/<name>/inbox`), `nudge` (a short one-line "you have mail from X: read ..." typed into the pane only when its state is idle/waiting, never into a running full-screen app), and `file` (message body written under a private `0600` runtime dir, path passed in the nudge). `wait` lets a sender block with a timeout until a reply or ack arrives.
  3. Coordination (`AGT-03`): `agent.broadcast` to a subtree; `/events` carries `agent.message`, `agent.state` events; a sidebar badge shows unread mail per agent (Phase 50 hook); a `:agent` command family (`list`, `send`, `inbox`, `broadcast`, `name`) and a documented coordinator pattern with an example script.
  4. Safety: off by default behind `[api] allow_agent_messaging` (or a per-session opt-in); per-sender rate limits and max queue depth; loop guard (hop count / `reply_to` chain limit) so two agents cannot ping-pong forever; all messages and nudges logged; nothing is injected into password prompts (`!ECHO` lock) or alternate-screen apps; paths use `secure_fs`.
  5. Tests: headless two-agent round trip, rate-limit and loop-guard cases, gating off/on, nudge refused when busy, restart behavior of the queue, fuzz for oversized or malformed bodies.

**Plans**: 3/3 plans complete

Plans:
- [x] 52-01-PLAN.md — Spec: message schema, addressing, delivery semantics, threat model (prompt-injection between agents), loop guard; user sign-off before code (wave 1)
- [x] 52-02-PLAN.md — Core: `AgentRegistry`/mailbox in `wm`, `agent.*` RPC + MCP tools/resource, `:agent` commands, gating and limits (wave 2)
- [x] 52-03-PLAN.md — Nudge/file delivery, `/events` and sidebar unread badges, `bilterm ctl agent ...`, docs/API.md + coordinator example, tests (wave 3)

### Milestone v2.10 — Session Management & Packing Viewer (Phases 53–54)

Goal: make the hierarchy Workspace > Session (can nest) > Window (composite pane) > Pane fully manageable and visible. Phase 53 adds the management actions; Phase 54 adds the visual packing viewer and drag-and-drop on top of 52's move action.

Sequencing: Phase 49 (Command Parity & Full Public API) should precede any scripting language (see `.planning/seeds/scripting-language.md`). Phases 53 and 54 can run alongside Phase 49; they add new `Action`s that Phase 49 will then document, so they must follow the same parity rules (every path lowers to `Action` via `WindowManager::apply`).

### Phase 53: Session & Window Management

**Goal**: A user can reorganise their workspace without restarting anything: move a window from one session to another, rename sessions and windows, and select several windows in a session to act on them together.
**Depends on**: Phase 50 (Navigator tree, right-click menu, `src/wm/drawer/navigator.rs`, `src/input/drawer_navigator.rs`); builds on Phase 38 (`:session rename|move|reorder`) and Phase 33 (session tree persistence).
**Requirements**: SES-01, SES-02, SES-03, SES-04, SES-05
**Harmony rule**: Add new paths; do not remove or rewire existing ones. Existing `:session ...` commands, drawer keys and Phase 50 menu entries keep working. Every key, mouse and colon path lowers to `Action` and runs via `WindowManager::apply`; every path has a parity test in the style of `tests/phase50_*`.
**Success Criteria** (what must be TRUE):

  1. Move window (`SES-01`): a window moves to another session (including a nested session) by key sequence, by `:window move <session>` (or equivalent `:` command), and from the Navigator right-click menu; pane PTYs, scrollback and running commands are untouched; moving into the current session or an unknown session is a clear error; z-order and focus are well defined after the move.
  2. Rename (`SES-02`): sessions and windows are renamed by right-click menu, by key sequence (inline edit in the Navigator) and by `:session rename` / `:window rename`; names pass `validate_safe_name` / `sanitize_display`, duplicates within a parent are rejected or disambiguated deterministically, and the new name appears in the Navigator, title bars and status bar.
  3. Multi-select (`SES-03`): windows within a session can be multi-selected (key toggle, range, select-all, Ctrl+click in the Navigator, `:window select ...`); bulk actions are move, close, rename-prefix, tile and pin, each by key, `:` command and menu; a bulk action is all-or-nothing, and bulk close honours the exit guard.
  4. Undo-safe and persistent (`SES-04`): each move, rename and bulk action is a single undoable unit (undo restores placement, name, order and selection), cannot orphan a pane or drop the last window silently, and survives session save/restore and daemon re-attach (TOML and journal round trip, older session files still load).
  5. API and parity (`SES-05`): all new actions are reachable via `bilterm ctl`, RPC and MCP (`window.move`, `window.rename`, `session.rename`, `window.select`, bulk variants) with the existing `[api]` gating; names from RPC are sanitised; exit-guard interplay is tested (closing the last window of a session, moving a window with a running foreground process); key, mouse and colon parity tests exist for every action.

**Plans**: 3/3 plans complete

Plans:
- [x] 53-01-PLAN.md — Spec: Action set, selection model, undo model, persistence schema, naming rules; user sign-off before code (wave 1)
- [x] 53-02-PLAN.md — Core: move and rename actions in `wm` with `apply`, `:` commands, persistence, undo, tests (wave 2)
- [x] 53-03-PLAN.md — Multi-select and bulk actions, Navigator menu and keys, RPC/MCP/ctl, docs and parity tests (wave 3)

### Phase 54: Session Packing Viewer & Drag-and-Drop

**Goal**: A user can see how windows are packed into sessions, several sessions at once, and rearrange them by dragging, with the same result available from the keyboard.
**Depends on**: Phase 53 (move action, rename, selection), Phase 50 (`graph_raster`/vector rendering, non-modal agents-window pattern, Navigator tree).
**Requirements**: VIEW-01, VIEW-02, VIEW-03, VIEW-04
**Harmony rule**: Add new paths; existing drawer, Navigator and agents-window behaviour is unchanged. The viewer is non-modal (like the Phase 50 agents window) and never steals PTY input unless focused. Every gesture lowers to `Action` via `WindowManager::apply`.
**Success Criteria** (what must be TRUE):

  1. Layout (`VIEW-01`): a pure, deterministic treemap-style layout packs each session as a rectangular container with its windows packed inside, scaled to the available area; several sessions are shown at once, nested sessions are nested containers; same input gives the same rectangles; tiny areas degrade to labels or counts rather than panic.
  2. Rendering (`VIEW-02`): text mode (braille/box drawing) in the TUI and a vector rendering in the GUI (reusing Phase 50 `graph_raster`/vector paths, no new dependency); a non-modal window that can be moved, resized, pinned, scaled and closed by key, mouse and `:` command (`:sessions map` or similar); focused/current window and session highlighted; labels sanitised.
  3. Drag-and-drop (`VIEW-03`): windows can be dragged between session containers in the viewer and between sessions in the Navigator tree, by mouse in the TUI and GUI; drop targets are highlighted, Esc cancels, an invalid drop is a no-op with feedback; a drop lowers to Phase 53's move action (single undoable unit).
  4. Keyboard equivalents (`VIEW-04`): every drag gesture has a keyboard path (pick up, move target, drop, cancel) and a `:` command, listed in `src/input/keymap.rs` with drift tests; parity tests prove mouse, key and colon give identical results.
  5. Tests: layout golden/property tests (no overlap, full containment, determinism), TUI snapshot and GUI-dump previews, drag/drop and keyboard parity tests in the style of `tests/phase50_*`.

**Plans**: 3/3 plans complete

Plans:
- [x] 54-01-PLAN.md — Spec and pure treemap packing layout with property tests; user sign-off on look before rendering (wave 1)
- [x] 54-02-PLAN.md — Non-modal viewer window: text-mode and GUI vector rendering, window chrome, `:` commands and keys (wave 2)
- [x] 54-03-PLAN.md — Drag-and-drop in viewer and Navigator (mouse), keyboard equivalents, parity tests, docs (wave 3)

### Phase 55: Daemon Supervisor & Multi-Daemon Awareness

**Goal**: Bilterm starts its background daemon automatically when needed. Normally exactly one daemon runs (singleton, enforced by a lock); several named daemons can run when the user asks, and every daemon knows about the others.
**Depends on**: Phase 32/35 (`bilterm --daemon --session`, per-session sockets), Phase 49 (API/MCP), Phase 50 (agent state).
**Requirements**: DAEM-05, DAEM-06, DAEM-07, DAEM-08
**Today (baseline)**: one daemon process per session name (`daemon-<session>.sock`, `bilterm ls` scans the directory). A daemon is spawned only when the client detaches (`main.rs`, after the TUI exits) and there is no singleton lock, no supervisor and no daemon-to-daemon discovery. Research must first confirm whether live PTYs actually move to the daemon on detach.
**Uniformity (`UNI-R01`)**: every capability ships a mouse path, a GUI+TUI key sequence and colon/palette/RPC/MCP commands, with parity tests.
**Success Criteria** (what must be TRUE):

  1. `DAEM-05`: default singleton daemon, auto-dispatched on first need (start, `:detach`, `bilterm ctl` with no daemon) under an exclusive lock with stale-lock recovery; double start is a clean no-op.
  2. `DAEM-06`: opt-in multi-daemon (`--daemon --name <n>` or `[daemon] mode`), each with its own socket, lock and runtime dir via `secure_fs`.
  3. `DAEM-07`: peer awareness: a registry of live daemons (id, name, pid, uptime, panes, sessions, agents) that every daemon and client can read; `daemon.list` RPC/MCP, `:daemon list|status|stop|start`, drawer/palette entries, and `:find` can search across daemons.
  4. `DAEM-08`: lifecycle safety: graceful stop, crash cleanup, version-mismatch handling, no cross-daemon PTY/input leakage unless explicitly allowed.

**Plans**: 3/3 plans complete

Plans:
- [x] 55-01-PLAN.md — Singleton lock, stale-lock recovery, `DaemonRegistry`, and `--daemon --name <n>` multi-daemon isolation (wave 1)
- [x] 55-02-PLAN.md — Auto-spawn on `:detach` / `bilterm ctl`, `:daemon list|status|start|stop|switch`, JSON-RPC `daemon.*` & MCP `bilterm_daemon`, version-handshake safety (wave 2)
- [x] 55-03-PLAN.md — Cross-daemon `:find` integration, Navigator/Palette/Menu surface parity (`UNI-R01`), documentation, and uniformity test suite (wave 3)

### Phase 56: Drawer Window Peek & Live Preview Pane

**Goal**: As a user navigating windows or sessions in the Classic or Navigator drawer, I want a transient non-destructive floating preview pane showing the live output canvas or saved-session snapshot of whatever row is currently selected/highlighted, so that I can instantly identify and inspect window contents without activating, unminimizing, or shuffling the window z-stack.
**Depends on**: Phase 24 (`WorkspaceDrawer`), Phase 33 (`SavedWorkspaceSession`), Phase 50 (`Navigator`), Phase 53 (`WindowManageState`).
**Requirements**: DRW-PEEK-01, DRW-PEEK-02, DRW-PEEK-03
**Today (baseline)**: Moving the selection in Classic or Navigator drawer updates `selected_index` without showing window contents; occluded, minimized, or background session windows cannot be previewed before pressing Enter or clicking.
**Uniformity (`UNI-R01`)**: Mouse wheel hover / click-to-focus, keyboard navigation (`j`/`k`, `Up`/`Down`, `PageUp`/`PageDown`, `Enter` focus, `Esc` dismiss), `:drawer peek [on|off|toggle]`, Command Palette entry, Top Menu toggle, and JSON-RPC / MCP inspection.
**Success Criteria** (what must be TRUE):

  1. `DRW-PEEK-01`: Highlighting any live window (active, minimized, stacked, or background session) or saved session in the Classic or Navigator drawer displays a transient floating preview pane adjacent to the drawer without modifying active pane focus, z-order, or triggering child PTY `SIGWINCH` resizes.
  2. `DRW-PEEK-02`: Keyboard navigation (`Up`/`Down`/`j`/`k`/`PageUp`/`PageDown`) and mouse wheel scrolling in the drawer update the preview target; pressing `Enter` or clicking inside the preview commits focus to that window/session; pressing `Esc`, clicking outside, or closing the drawer cleanly dismisses the preview.
  3. `DRW-PEEK-03`: Zero-copy live PTY canvas rendering for active and background sessions, cached snapshot rendering for cold saved sessions, configurable preview geometry/delay (`[drawer] peek_preview = true`), and full parity across Classic and Navigator drawer styles.

**Artifacts**:
- Research: `.planning/phases/56-drawer-window-peek-preview/56-RESEARCH.md`
- Contract: `.planning/phases/56-drawer-window-peek-preview/56-CONTRACT.md`
- Plans:
  - [x] 56-01-PLAN.md — Peek State, Config, Zero-SIGWINCH Live & Snapshot Preview Renderer, and Drawer Row Layout Geometry (wave 1)
  - [x] 56-02-PLAN.md — Transient Peek Activation, Keyboard/Mouse Wheel Triggers, Click-to-Focus, and Esc/Dismissal Transitions (wave 2)
  - [x] 56-03-PLAN.md — Background Session Navigator Window Hierarchy, Drawer Style Parity, and End-to-End Verification (wave 3)
- Status: **COMPLETE** (`80994fb`, `PASS 129/129`, `GOLDENS OK 11`, PR #31)

### Phase 57: Smart Clipboard Paste Fix (`Herdr`/TUI `Ctrl+V`) & Filterable Clipboard Viewer Window

**Goal**: Fix the longstanding `"No image or video found in clipboard"` error when pasting text into `herdr`, Claude Code, or other nested TUIs in raw-passthrough mode, and provide a first-class floating, pinnable, dockable `CLIPBOARD` viewer window filterable by session, window, pin state, media, and fuzzy search.
**Depends on**: Phase 19 (`SEC-R01..04`), Phase 22 (`ARCH-R01`), Phase 49 (`API-01`), Phase 51.2 (`PanelDock` utility windows).
**Requirements**: CLIP-01, CLIP-02, CLIP-03, CLIP-04
**Today (baseline)**: In `RawPtyPassthrough` mode, pressing `Ctrl+V` passes raw `0x16` to the child PTY, causing tools like Herdr/Claude Code to attempt an image/video clipboard probe which fails on text content; right/middle click is swallowed by SGR mouse tracking; inbound OSC 52 is discarded in GUI mode; and there is no unified multi-session clipboard history window.
**Uniformity (`UNI-R01`)**: Mouse (GUI & TUI click/drag/resize/pin/dock), Keyboard (`Ctrl+Alt+V`, Leader `;`, in-window hotkeys), `:clipboard` / `:clips` colon commands, Command Palette, Top Menu (`Edit -> Clipboard Viewer...`), JSON-RPC (`clipboard.*`), and MCP (`bilterm_clipboard`).
**Success Criteria** (what must be TRUE):

  1. `CLIP-01`: Pressing `Ctrl+V` in `RawPtyPassthrough` / `ALT_SCREEN` mode inspects the OS clipboard MIME kind: text pastes are forwarded wrapped in `DECSET 2004` bracketed paste (`\x1b[200~<text>\x1b[201~`), fixing Herdr and Claude Code text pastes; media clipboards pass raw `0x16` through; `Shift+RightClick` pastes text over SGR mouse tracking; and inbound `OSC 52` sequences update the system clipboard and registry in both GUI and TUI modes.
  2. `CLIP-02`: Multi-session, multi-window `ClipboardRegistry` records all copy/cut/yank/kill/OSC-52/OS-sync events up to 200 entries with session, window title, timestamp, MIME kind, and secret classification (`pane::redact`) masking sensitive tokens per `SEC-R01..04`.
  3. `CLIP-03`: First-class floating, pinnable, dockable `CLIPBOARD` viewer window (`src/wm/clipboard_window.rs`) with dual TUI and native GUI rendering (`CLIPBOARD_WINDOW_TEXTURE_ID = 980_000`), filterable by scope (`All`, `Session`, `Window`, `Pinned`, `Media`) and live fuzzy search query, with entry paste, prompt-insert, pin/star, delete, clear, and per-panel typography scaling.
  4. `CLIP-04`: 100% surface uniformity across Mouse, Keyboard (`Ctrl+Alt+V` / Leader `;`), `:clipboard` colon verbs, Command Palette, Top Menu, JSON-RPC, and MCP with end-to-end parity test suite.

**Artifacts**:
- Research: `.planning/phases/57-clipboard-fix-and-viewer-window/57-RESEARCH.md`
- Contract: `.planning/phases/57-clipboard-fix-and-viewer-window/57-CONTRACT.md`
- Plans:
  - [x] 57-01-PLAN.md — Smart Clipboard Paste & Bracketed Paste Fix for Herdr / Claude Code / TUIs (wave 1)
  - [x] 57-02-PLAN.md — Multi-Session Clipboard History Registry, Metadata Tagging & Secret Redaction (wave 2)
  - [x] 57-03-PLAN.md — First-Class Filterable `CLIPBOARD` Viewer Window, Surface Parity & Test Suite (wave 3)
- Status: **COMPLETE** (`2f069bf`, `PASS 129/129`, `GOLDENS OK 11`, PR #31)

### Phase 58: Local AI Speech-to-Text (`STT`) Optional Module

**Goal**: Provide an independent, zero-bloat local AI voice dictation and command synthesis module (`src/stt/`) supporting OpenAI-compatible Whisper servers (`whisper.cpp`, `speaches`, `LocalAI`, `vLLM`, `LM Studio`), Ollama, and local CLI pipes with push-to-talk, VAD, and OIA status bar telemetry, safely staging transcribed text into the pane's floating command prompt before execution.
**Depends on**: Phase 1 (`CompositePane`), Phase 4 (`OiaStatusBar`), Phase 19 (`SEC-R01..04`), Phase 22 (`Action`), Phase 49 (`API-01`).
**Requirements**: STT-01, STT-02, STT-03
**Today (baseline)**: Bilterm is strictly keyboard and mouse driven with no audio capture, voice input, or local AI speech transcription capabilities.
**Uniformity (`UNI-R01`)**: Push-to-talk mouse button / titlebar mic icon, Keyboard toggle (`Ctrl+Space` / Leader `v`), `:stt` colon verbs, Command Palette, Top Menu (`Tools -> Voice Dictation...`), JSON-RPC (`stt.*`), and MCP (`bilterm_stt`).
**Success Criteria** (what must be TRUE):

  1. `STT-01`: Pluggable Local AI STT backends supporting OpenAI-compatible `POST /v1/audio/transcriptions` (via zero-dependency HTTP/1.1 client over `TcpStream` / `UnixStream`), Ollama native multimodal/audio endpoints, and local CLI subprocess pipes (`whisper-cli`, `mlx_whisper`) with prompt context biasing and optional voice-to-shell translation.
  2. `STT-02`: Two-tier zero-bloat audio capture architecture: Tier 1 default uses OS recorder pipes (`pw-record`/`parecord`/`arecord`, `rec`/`sox`/`ffmpeg`, Windows helpers) writing 16kHz mono WAV to `0600` private pipes via `secure_fs` without C/ALSA link dependencies; Tier 2 opt-in `--features stt-native-audio` enables in-process `cpal` ring-buffer capture.
  3. `STT-03`: Isolated subsystem seam in `src/stt/` communicating exclusively via `Action::Voice` and `AppEvent::SttEvent`, push-to-talk and VAD support, OIA `[🎙 REC]` / `[🎙 BUSY]` telemetry, staging into `FloatingCommandPrompt` without auto-executing, zero runtime overhead when `[stt] enabled = false`, and full surface parity.

**Artifacts**:
- Research Spike: `.planning/phases/58-speech-to-text-spike-and-plan/58-RESEARCH-SPIKE.md`
- Architecture ADR: `.planning/phases/58-speech-to-text-spike-and-plan/58-ARCHITECTURE-ADR.md`
- Plans:
  - [x] 58-01-PLAN.md — Pluggable Local AI STT Engine & Wire Protocols (OpenAI/Whisper, Ollama, CLI) (wave 1)
  - [x] 58-02-PLAN.md — Audio Capture Pipeline, Push-to-Talk / VAD, UI Telemetry & Surface Parity (wave 2)
- Status: **COMPLETE** (`a31d0cf`, `PASS 134/134`, `GOLDENS OK 11`)

### Phase 59: GUI Chrome Engine Web Browser Canvas & 60fps Video Player Canvas

**Goal**: Deliver embedded Chromium engine web browser canvases (CDP Screencast frame-sink + optional CEF off-screen renderer) and 60fps RGBA hardware-accelerated video player canvases (`ffmpeg` pipe + OSD transport controls + audio sync) inside composite panes, composited seamlessly with Borland/OIA chrome and floating prompts in GUI mode (`GuiTextureQuad`) and adaptive sub-cell graphics in TUI mode (`Kitty`/`Sixel`/`HalfBlock`).
**Depends on**: Phase 10 (`GuiWorkspaceRenderer`), Phase 26/27 (`PixelCanvasBuffer`), Phase 49 (`API-01`).
**Requirements**: WEB-01, WEB-02, VID-01, VID-02
**Today (baseline)**: Panes display either PTY terminal emulation or Tektronix/vector/image pixel canvases; no embedded live web browser engine or full-motion 60fps video playback canvas exists.
**Uniformity (`UNI-R01`)**: Full mouse click/scroll/drag, keyboard shortcuts, `:browse <url>`, `:video <path|url>`, Command Palette, Top Menu (`View -> New Browser Pane / Video Pane`), JSON-RPC (`browser.*`, `video.*`), and MCP (`bilterm_browse`, `bilterm_video`).
**Success Criteria** (what must be TRUE):

  1. `WEB-01`: Out-of-process Chromium engine web browser canvas via CDP Screencast (`CdpBrowserEngine`) delivering live RGBA frames into `GuiWorkspaceRenderer` (Pass 2) under private `0700` profile sandboxing, plus optional in-process CEF off-screen renderer (`--features cef-browser`).
  2. `WEB-02`: Full interactive browser navigation: interactive URL/Navigation bar, mouse click/scroll forwarding via CDP `Input.dispatchMouseEvent`, keyboard typing via `Input.dispatchKeyEvent`, navigation history back/forward/reload, zoom, and adaptive TUI sub-cell fallback.
  3. `VID-01`: Embedded 60fps RGBA video player canvas (`VideoCanvasEngine`) driven by `ffmpeg` subprocess pipe (or optional `libavcodec`) decoding multi-format video into shared pixel buffers with monotonic audio clock synchronization.
  4. `VID-02`: On-screen transport controls (Play/Pause, Seek bar, Volume slider, Speed, Loop toggle), keyboard hotkeys (`Space`, `Left`/`Right`, `m`, `f`), Borland OIA status bar timecode display, and surface parity (`UNI-R01`).

**Artifacts**:
- Research Spike: `.planning/phases/59-browser-and-video-canvas-spike/59-RESEARCH-SPIKE.md`
- Architecture ADR: `.planning/phases/59-browser-and-video-canvas-spike/59-ARCHITECTURE-ADR.md`
- Plans:
  - [x] 59-01-PLAN.md — Chrome Engine (CDP Screencast + CEF Off-Screen) Web Browser Canvas Pipeline (wave 1)
  - [x] 59-02-PLAN.md — 60fps RGBA Video Player Canvas Engine, Surface Parity & Test Suite (wave 2)
- Status: **COMPLETE** (`7304993`, `PASS 134/134`, `GOLDENS OK 11`)

### Phase 60: Multi-Surface Web Frontend Backend & `@agent-manager/protocol` Bridge

**Goal**: Deliver a high-performance, multi-surface web frontend backend and RFC 6455 WebSocket bridge implementing `@agent-manager/protocol` v1, decoupling web client viewports via a `ClientViewState` registry, exposing atomic `workspace.view_model` REST/RPC endpoints with Phase 45 CSS design token exports, and serving static SPA assets under `SEC-R01` confinement with full `UNI-R01` surface parity.
**Depends on**: Phase 19 (`SEC-R01..04`), Phase 22 (`Action`), Phase 45 (`DesignTokens`), Phase 49 (`API-01`), Phase 50 (`Navigator`), Phase 52 (`AgentCoordination`).
**Requirements**: WEB-FE-01, WEB-FE-02, WEB-FE-03, WEB-FE-04
**Today (baseline)**: Bilterm provides JSON-RPC 2.0 over Unix domain socket / TCP and an MCP server, but lacks an RFC 6455 WebSocket transport for web browsers, has no protocol bridge for `@agent-manager/protocol` v1, cannot serve static SPA web assets, and couples viewport/focus state directly to the local TUI/GUI window manager.
**Uniformity (`UNI-R01`)**: `:web [status|open|port|stop]` colon commands, Command Palette (`Web: Open Web UI`, `Web: Server Status`), Top Menu (`Tools -> Web UI...`), JSON-RPC (`workspace.view_model`, `web.*`), and MCP (`bilterm_web_surface`).
**Success Criteria** (what must be TRUE):

  1. `WEB-FE-01`: RFC 6455 WebSocket `GET /ws` server implementing `@agent-manager/protocol` v1 (`hello`/`hello_ack`, `agent:init_list`, `agent:output` delta streaming with `seq`/`resumeFrom`, `agent:input`/`resize`/`create`/`kill`/`update`/`broadcast`/`history`/`approve`).
  2. `WEB-FE-02`: Atomic `workspace.view_model` RPC & REST endpoints (`GET /agents`, `GET /agents/:id`, `GET /logs/:id`, `POST /search`, `GET /tokens.css` exporting Phase 45 `DesignTokens` as CSS custom properties).
  3. `WEB-FE-03`: Multi-client `ClientViewState` registry (`src/api/client_view.rs`) decoupling Web client viewport, focus, scrollback offset, and overlays from local TUI/GUI `WindowManager` state.
  4. `WEB-FE-04`: `SEC-R01`-confined static SPA serving (`--web-dir` for `agent-manager-fe/dist` with SPA `index.html` fallback), opt-in `--cors-origin` for local Vite dev servers, and full `UNI-R01` surface parity (`:web` command, RPC `workspace.view_model`, MCP `bilterm_web_surface`).

**Artifacts**:
- Research Spike: `.planning/phases/60-web-frontend-surface-and-protocol-bridge/60-RESEARCH-SPIKE.md`
- Architecture ADR: `.planning/phases/60-web-frontend-surface-and-protocol-bridge/60-ARCHITECTURE-ADR.md`
- Plans:
  - [x] 60-01-PLAN.md — RFC 6455 WebSocket Server & `@agent-manager/protocol` v1 Streaming Engine (wave 1)
  - [x] 60-02-PLAN.md — Atomic `workspace.view_model` RPC & REST Endpoints, CSS Design Tokens & Static SPA Server (wave 2)
  - [x] 60-03-PLAN.md — Multi-Client `ClientViewState` Registry, Surface Parity & Test Suite (wave 3)
- Status: **COMPLETE** (`ea0aea3`, `PASS 134/134`, `GOLDENS OK 11`)

## UI Modernization Execution Model (Phases 43–48, subagent orchestration)

Governed by swe-skills `orchestrating-subagents` (synced from `/home/bill/src/swe-skills-2`, identical to `~/.claude/skills`). Rules: full task text per dispatch, terse report (status · files · verify output · blockers ≤10 lines), one writer per file, **subagents never commit — the orchestrator (Claude Code) reviews diffs, runs the verify command, and commits**, one worktree per concurrent agent (`git worktree add ../bilterm-<task> -b <branch>`), merge serially.

| Phase | Parallelism | Who | Model tier | File ownership | Verify |
|---|---|---|---|---|---|
| 43 audit | none (done inline) | orchestrator | — | `.planning/phases/43-*` | `cargo clippy -q`, `cargo test -q` |
| 44 previews | per style×theme×scene renders independent | haiku (renders, PNG conversion); orchestrator writes proposal | haiku / orchestrator | `examples/design_preview.rs`, `.planning/phases/44-*` | `cargo run -q --example design_preview -- <dir>` |
| 45 design system | **serial for 45-01/02** (one design); 45-03 fans out by file **only after the "API FROZEN" gate** | opus for token API design; sonnet for config migration; haiku for per-file colour migration | opus / sonnet / haiku | 45-01/02: `src/ui/design/**`, `src/config/mod.rs` · 45-03: one file per agent (`oia.rs`, `status_bar.rs`, `modals.rs`, `cursor_cmd.rs`, ...) | `cargo clippy -q --all-targets -- -D warnings`, `cargo test -q`, existing insta snapshots unchanged |
| 46 shell/modes | **serial** (touches `wm/mod.rs`, `ui/mod.rs`, `chrome.rs` together) | sonnet | sonnet | `wm/mod.rs`, `ui/mod.rs`, `ui/chrome.rs`, `wm/chrome_style.rs` | clippy + test |
| 47 aux UI | **best fan-out**: drawers / palette+menus / settings modal are mostly disjoint | sonnet per area; orchestrator wires `Action` | sonnet | 47-01 `ui/drawer.rs`,`ui/mcp_ui.rs`,`ui/exit_guard_ui.rs` · 47-02 `ui/modals.rs`,palette · 47-03 new `ui/appearance/**` | clippy + test + snapshot review |
| 48 regression | tests, perf runs, a11y checks in parallel | haiku (test writing to spec), sonnet (perf analysis) | haiku / sonnet | `tests/**` only | `cargo test -q`, perf budget vs. baseline |

**Gates**: (G1) Phase 44 user sign-off before Phase 45. (G2) **API FROZEN** after 45-01/02 — no token API changes while 45-03 / Phase 47 agents run. (G3) `Action`/`apply` wiring, config migration, and anything at the `vt`/`TerminalBackend` seam is orchestrator-only (Claude Code). (G4) Independent fresh-context review (spec compliance, then quality) before each phase commit.

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 (… 18) → 19 → 20 → 21 → 22 → 23 → 24 → 25 → 26 → 27 → 28 → 29 → 30 → 31 → 32 → 33 → 34 → 35 → 36 → 37 → 38 → 39 → 40 → 41 → 42 → 43 → 44 → 45 → 46 → 47 → 48 → 49 → 50 → 50.1 → 51 → 51.1 → 52 → 53 → 54 → 55 → 56 → 57 → 58 → 59 → 60
*(Note: Phase 35 and Phase 36 touch disjoint primary modules and can be executed sequentially or in parallel waves).*

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
| 19. Security Hardening of Automation & Shell-Integration Surfaces (`SEC-R01..04`) | 7/7 | Complete | 2026-10-23 |
| 20. Mega-Module Decomposition (`ARCH-R04`) | 5/5 | Complete | 2026-10-23 |
| 21. Encapsulated Workspace Invariants (`ARCH-R02`) | 3/3 | Complete | 2026-10-24 |
| 22. Unified `Action` Command Vocabulary, Data-Driven Keymap & Whole-Workspace `.cast` Recorder (`ARCH-R01`, `REC-02`) | 3/3 | Complete | 2026-10-24 |
| 23. `TerminalBackend` Trait & Unwired-Subsystem Resolution (`ARCH-R03`, `ARCH-R05`) | 3/3 | Complete | 2026-10-24 |
| 24. `herdr`-Style Slide-Out Sessions/Windows/Panes Drawer with Status Glyphs & Float/Push Modes (`DRW-01`, `DRW-02`) | 3/3 | Complete | 2026-10-24 |
| 25. Dynamic Terminal Text & Font Size Scaling (`FONT-01`) | 3/3 | Complete | 2026-10-24 |
| 26. Tektronix 4010/4014 × Modern 2026/2027 Vector, Sprite, Plotting & Image Graphical Canvas (`TEK-01`, `CANV-01`) | 3/3 | Complete | 2026-10-24 |
| 27. High-Resolution Anti-Aliased Pixel Canvas & Adaptive SSH/GUI Graphics Engine (`CANV-02`, `SSH-GFX-01`, `CLI-GFX-01`) | 3/3 | Complete | 2026-10-25 |
| 28. Post-`v1.10.0` Architecture Hardening, Module DAG Decoupling & Runtime Efficiency (`ARCH-R01..04`, `SEC-R01`) | 1/1 | Complete | 2026-10-27 |
| 29. Smooth Scrolling by Default & Mainframe/Timeshare Block/Page-Mode Scroller (`SCRL-01`, `SCRL-02`) | 3/3 | Complete | 2026-10-27 |
| 30. Explicit Exit Guard & Root/Final Session Protection (`SAFE-01`, `SAFE-02`) | 2/2 | Complete | 2026-10-27 |
| 31. Universal Open Color Scheme & Font Engine (`iterm2-color-schemes` Compatibility) (`THM-01`, `THM-02`) | 3/3 | Complete | 2026-10-27 |
| 32. Persistent Background Multiplexer Daemon (`herdr`-Style Multi-Client Attach/Detach) (`DAEM-01`, `DAEM-02`) | 3/3 | Complete | 2026-10-27 |
| 33. Independent, Rooted & Hierarchical Tree/Subtree Sessions + Crash-Safe Layout Persistence (`TREE-01..02`, `PERS-01..02`) | 3/3 | Complete | 2026-10-27 |
| 34. TRON-Inspired Master Control Program (`MCP`) Root Terminal (`MCP-01`, `MCP-02`) | 3/3 | Complete | 2026-10-27 |
| 35. Live Background OS Process Daemonization for `:detach` / `bilterm attach` (`DAEM-03`, `DAEM-04`) | 3/3 | Complete | 2026-10-28 |
| 36. Interactive Mouse & Keyboard Navigation + Live Telemetry in `:mcp` Console Window (`MCP-03`, `MCP-04`) | 3/3 | Complete | 2026-10-28 |
| 37. `v2.1.1`–`v2.2.0` Interactive File & `.itermcolors` Browser, Cursor Customizer & Multi-Profile Layout Manager (`PROF-01`, `THM-03`, `CUR-01`) | 3/3 | Complete | 2026-10-29 |
| 38. `v2.3.0` Session Lifecycle Commands, Persistent Ordering, Blinking Software Cursor & Scaled Mouse Selection (`SESS-01..02`, `CUR-02`) | 3/3 | Complete | 2026-10-29 |
| 39. `v2.4.0` Per-Window Themes, Outlines, Bold Borders, `[_][□][X]` Controls & `MIN_GRID_DIM` `2×2` Clamp (`WIN-01..02`, `VT-CLAMP-01`) | 2/2 | Complete | 2026-10-29 |
| 40. `v2.4.1` Interactive Per-Window Theme Browser & Per-Window Element Color Overrides (`WIN-03`, `WIN-04`) | 3/3 | Complete | 2026-10-31 |
| 41. `v2.5.0` 2D Rectangular Block Select/Highlight/Copy & Full Per-Pane Session Shell, Running Command & CWD Persistence (`RECT-01`, `SESS-03`) | 3/3 | Complete | 2026-10-31 |
| 42. `v2.5.1`–`v2.5.2` Non-Retro Modern Minimal & Liquid Glass Modes, Per-Window vs. Global Theme & Cursor Isolation, Drawer Controls & SSH Hardening (`MOD-01`, `LIQ-01`, `WIN-05`, `CUR-03`, `SSH-02`) | 3/3 | Complete | 2026-10-31 |
| 43. UI Modernization Step 1 — Architecture Audit & Plan Realignment (`UIM-00`) | 0/1 | Planned | - |
| 44. UI Modernization Step 2 — Visual Design Proposal with Working Previews (`UIM-01`) | 0/1 | Planned | - |
| 45. UI Modernization Step 3 — Design System: Tokens, Materials, Themes & `[appearance]` Config (`UIM-02`) | 3/3 | Complete | 2026-11-07 |
| 46. UI Modernization Step 4 — Main Interface: Shell, Display Modes, Workspace Navigation & Pane Chrome (`UIM-03`) | 0/3 | Planned | - |
| 47. UI Modernization Step 5 — Drawers, Floating Panels, Command Palette, Context Menus & Live-Preview Appearance Settings (`UIM-04`) | 0/3 | Planned | - |
| 48. UI Modernization Step 6 — Regression, Performance & Polish (`UIM-05`) | 0/2 | Planned | - |
| 50. Agent-Aware Workspace Navigator (Left Sidebar) (`FLT-01`) | 16/16 | In Progress | - |
| 50.1. Unified Registry & Find (INSERTED) (`FND-R01..04`) | 15/15 | Complete | 2026-11-05 |
| 51. UI Tweaks & Command Parity (`UIT-01..04`) | 13/13 | Complete | 2026-11-05 |
| 51.1. Persistent SSH + tmux Sessions (INSERTED) (`PST-01..04`) | 16/16 | Complete | 2026-11-06 |
| 51.2. First-Class Utility Windows & Script Studio (INSERTED) (`WIN-01..WIN-03`) | 4/4 | Complete | 2026-11-06 |
| 52. Agent-to-Agent Messaging & Coordination (`AGT-01..03`) | 3/3 | Complete | 2026-11-06 |
| 53. Session & Window Management (`SES-01..05`) | 3/3 | Complete | 2026-11-06 |
| 54. Session Packing Viewer & Drag-and-Drop (`VIEW-01..04`) | 3/3 | Complete | 2026-11-06 |
| 55. Daemon Supervisor & Multi-Daemon Awareness (`DAEM-05..08`) | 3/3 | Complete | 2026-11-06 |
| 56. Drawer Window Peek & Live Preview Pane (`DRW-PEEK-01..03`) | 3/3 | Complete (`80994fb`, `PASS 129/129`, `GOLDENS OK 11`, PR #31) | 2026-11-07 |
| 57. Smart Clipboard Paste Fix (`Herdr`/TUI `Ctrl+V`) & Filterable Clipboard Viewer Window (`CLIP-01..04`) | 3/3 | Complete (`2f069bf`, `PASS 129/129`, `GOLDENS OK 11`, PR #31) | 2026-11-07 |
| 58. Local AI Speech-to-Text (`STT`) Optional Module (`STT-01..03`) | 2/2 | Complete | 2026-11-08 |
| 59. GUI Chrome Engine Web Browser Canvas & 60fps Video Player Canvas (`WEB-01..02`, `VID-01..02`) | 2/2 | Complete | 2026-11-08 |
| 60. Multi-Surface Web Frontend Backend & `@agent-manager/protocol` Bridge (`WEB-FE-01..04`) | 3/3 | Complete | 2026-11-08 |
