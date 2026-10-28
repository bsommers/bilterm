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

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 (… 18) → 19 → 20 → 21 → 22 → 23 → 24 → 25 → 26 → 27 → 28 → 29 → 30 → 31 → 32 → 33 → 34

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
