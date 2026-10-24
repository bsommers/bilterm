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
- [ ] **Phase 22: Unified `Action` Command Vocabulary, Data-Driven Keymap & Whole-Workspace `.cast` Recorder** - Typed `Action` vocabulary, `[keybindings]` data table, `Ctrl+B`/`Ctrl+X` dual leader (`$TMUX` auto-fallback, held-`Ctrl` chord normalization, `asciinema` PTY-wrapper harmony), `tmux`/`herdr` chord parity, and whole-workspace Asciicast v2 (`.cast`) compositor recorder (`ARCH-R01`, `REC-02`)
- [ ] **Phase 23: `TerminalBackend` Trait & Unwired-Subsystem Resolution** - Decouple `vt100_ctt` behind `TerminalBackend`, resolve `VtWorkerHandle`/`TmuxControlSubprocess`/`ghostty-vt`, and CI feature matrix
- [ ] **Phase 24: `herdr`-Style Slide-Out Sessions/Windows/Panes Drawer with Status Glyphs & Float/Push Modes** - 4-edge (`Left`/`Right`/`Top`/`Bottom`) slide-out drawer with `OnDemand`/`Fixed`/`AutoHide` policies, `Overlay` (float) vs `Push` (auto-resize) layout modes, and live telemetry glyphs (`DRW-01`, `DRW-02`)
- [ ] **Phase 25: Dynamic Terminal Text & Font Size Scaling** - Runtime `Ctrl++`/`Ctrl+-`/`Ctrl+0`/`:fontsize` scaling across `GuiWorkspaceRenderer` pixel/SVG cells, DEC double-width/height (`DECDWL`/`DECDHL`) TUI canvas lines, and host-terminal font-zoom OSC passthrough (`FONT-01`)
- [ ] **Phase 26: Tektronix 4010/4014 × Modern 2026/2027 Vector, Sprite, Plotting & Image Graphical Canvas** - Classic Tektronix 4010/4014 4096×3120 vector mode (`\x1b[?38h`) with phosphor decay plus modern retained scene-graph, 2D sprites, declarative plotting, and image viewer over Braille/Sextant/Quadrant TUI & full-res GUI rasterizers (`TEK-01`, `CANV-01`)

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

**Plans**: TBD (`/gsd-plan-phase 23`)

### Phase 24: `herdr`-Style Slide-Out Sessions/Windows/Panes Drawer with Status Glyphs & Float/Push Modes

**Goal**: As an operator managing multiple sessions, split trees, z-order decks, and remote fleet nodes, I want a `herdr`-inspired slide-out drawer that can dock to any screen edge (`Left`, `Right`, `Top`, `Bottom`), operate in `OnDemand`, `Fixed`, or `AutoHide` mode, either float over panes (`Overlay`) or bump/auto-resize the workspace (`Push`), and render expressive live status glyphs for every session, window, and pane.
**Depends on**: Phase 23
**Requirements**: DRW-01, DRW-02
**Success Criteria** (what must be TRUE):

  1. `DrawerConfig` and runtime `WorkspaceDrawer` support 4 docking edges (`Left`, `Right` vertical tree; `Top`, `Bottom` horizontal ribbon/bar), configurable width/height, and 3 visibility modes (`OnDemand`, `Fixed`, `AutoHide` triggered by leader-key activation, mouse edge hover, or background pane notification/exit alert).
  2. `DrawerLayoutMode::Overlay` renders the drawer over the active workspace using `occlude_rect` and Borland drop shadow (`render_drop_shadow_styled`) without emitting `SIGWINCH` to PTYs; `DrawerLayoutMode::Push` subtracts `drawer_rect` from `workspace_area` before `WindowManager::compute_workspace_rects` and `WindowManager::resize_all`, reflowing panes cleanly.
  3. Each session/window/pane row renders rich telemetry glyphs: hierarchy connectors (`├─`, `╰─`), focus & z-order rank (`► [1] Z:1/4`), live activity braille spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) + output throughput sparkline (`▁▂▃▅▆▇`), OSC 133 command state (`⚙ RUN`, `✓ 0`, `✗ <code >`), mode/security badges (`🖥 ALT`, `🔑 !ECHO`, `🔒 PROT`, `⏺ REC`, `📡 BCAST`, `🔔 N`), and transport badges (`🌐 SSH`, `🔗 TMUX`).
  4. Keyboard navigation (`j`/`k`/`h`/`l`, `Enter` to focus/jump, `Space` to preview, `x` to close, `z` to zoom, `d` to cycle dock edge, `m` to toggle `Overlay`/`Push`, `p` to pin/unpin `Fixed`/`AutoHide`) and mouse click/hover/drag interactions work across both TUI (`Ratatui`) and GUI (`GuiWorkspaceRenderer`) surfaces, with deterministic `insta` / `TestBackend` snapshot coverage.

**Plans**: TBD (`/gsd-plan-phase 24`)

### Phase 25: Dynamic Terminal Text & Font Size Scaling

**Goal**: As a user operating Bilterm across high-DPI monitors, projector demos, and nested terminals, I want on-the-fly terminal text and font size scaling (`Ctrl++`/`Ctrl+=`, `Ctrl+-`, `Ctrl+0`, `:fontsize`) across both the native pixel/SVG renderer and the host TUI canvas.
**Depends on**: Phase 24
**Requirements**: FONT-01
**Success Criteria** (what must be TRUE):

  1. `Action::FontZoomIn`, `Action::FontZoomOut`, `Action::FontZoomReset`, and `Action::FontSetSize(u16)` are wired to `Ctrl++`/`Ctrl+=`, `Ctrl+-`, `Ctrl+0`, `:fontsize <pt|pct|+|-|reset>`, JSON-RPC/MCP (`workspace.fontsize`), and `config.toml` (`font_size_pt`, `font_scale_pct`, `cell_width_px`, `cell_height_px`).
  2. `GuiWorkspaceRenderer` / `GuiFrameSurface` dynamically scales cell pixel dimensions (`50%..=300%`), SVG typography (`font-size`), and PPM raster glyph scaling, recalculating viewport `(rows, cols)` for a given pixel window size and resizing panes accordingly.
  3. In TUI mode (`Ratatui`), `VtParser` and the canvas renderer support DEC double-width (`DECDWL` `\x1b#6`) and double-height (`DECDHL` `\x1b#3` / `\x1b#4`) line attributes plus multi-cell scaled text rendering, paired with the focal zoom magnification lens (`render_focal_zoom_cursor`) and optional host-terminal `OSC 50` / Kitty font-size passthrough.

**Plans**: TBD (`/gsd-plan-phase 25`)

### Phase 26: Tektronix 4010/4014 × Modern 2026/2027 Vector, Sprite, Plotting & Image Graphical Canvas

**Goal**: As a systems programmer, scientist, or AI agent, I want each CompositePane to include an optional high-speed graphical canvas supporting both classic Tektronix 4010/4014 vector storage-tube emulation and a modern 2026/2027 retained scene-graph with 2D sprites, declarative plotting, and interactive image inspection.
**Depends on**: Phase 25
**Requirements**: TEK-01, CANV-01
**Success Criteria** (what must be TRUE):

  1. Classic Tektronix 4010/4014 state machine (`\x1b[?38h` / `\x1b[?38l` / `:canvas tek`) decodes 10-bit and 12-bit 4096×3120 vector coordinates (`GS` `0x1D` dark/bright vectors, `FS` point plot, `RS` incremental plot, `ESC` solid/dotted/dot-dash/short-dash/long-dash styles, `\x1b\x0c` screen erase with authentic green 3278 / amber 3279 phosphor persistence decay).
  2. Modern retained scene-graph (`PaneCanvasScene`) supports vector primitives (lines, polylines, bezier curves, circles, polygons, text labels), a 2D sprite engine (`id`, position, velocity, frame animation, z-index, AABB collision), declarative live data plots (`line`, `scatter`, `bar`, `candlestick`, `heatmap`), and an interactive image viewer (`pan`, `zoom`, pixel inspector).
  3. Dual-backend rasterizer renders the canvas into sub-cell Unicode 2×4 Braille (`U+2800..U+28FF`), 2×3 Sextant (`U+1FB00..U+1FB3B`), and Quadrant blocks with 24-bit TrueColor (plus Kitty/Sixel overlays) in TUI mode, and crisp full-resolution vector/pixel graphics in `GuiWorkspaceRenderer` (SVG/PPM), while keeping the floating command prompt cleanly composited on top.
  4. Controllable via in-band PTY escape sequences, `:canvas <subcommand>`, and JSON-RPC/MCP `canvas.*` automation endpoints, verified by deterministic vector/sprite/plot snapshot tests.

**Plans**: TBD (`/gsd-plan-phase 26`)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 (… 18) → 19 → 20 → 21 → 22 → 23 → 24 → 25 → 26

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
| 23. `TerminalBackend` Trait & Unwired-Subsystem Resolution (`ARCH-R03`, `ARCH-R05`) | 0/TBD | Not started | - |
| 24. `herdr`-Style Slide-Out Sessions/Windows/Panes Drawer with Status Glyphs & Float/Push Modes (`DRW-01`, `DRW-02`) | 0/TBD | Not started | - |
| 25. Dynamic Terminal Text & Font Size Scaling (`FONT-01`) | 0/TBD | Not started | - |
| 26. Tektronix 4010/4014 × Modern 2026/2027 Vector, Sprite, Plotting & Image Graphical Canvas (`TEK-01`, `CANV-01`) | 0/TBD | Not started | - |
