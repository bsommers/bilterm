# Developer, Maintainer & AI Agent Engineering Guide

[← Back to Documentation Hub](README.md) | [← Back to Project README](../README.md) | [Lessons Learned (`v0.1` → `v2.0`)](LESSONS_LEARNED.md)

This guide is the authoritative technical reference for human developers, maintainers, and autonomous AI coding agents working on **Bilterm**. It documents the complete crate dependency stack, the 233-file modular source tree, core architectural invariants (`ARCH-R01..04`, `SEC-R01..04`, `REC-02`), runtime inspectability and debugging tools, and step-by-step extension workflows.

---

## Table of Contents

1. [Crate & Library Dependency Stack (`Cargo.toml`)](#1-crate--library-dependency-stack-cargotoml)
2. [Complete Source Tree & Module Boundaries (`<= 800` Lines Gate)](#2-complete-source-tree--module-boundaries--800-lines-gate)
3. [Core Architectural Invariants (`Phases 19–22`)](#3-core-architectural-invariants-phases-1922)
4. [End-to-End `Action` & Input Dispatch Pipeline](#4-end-to-end-action--input-dispatch-pipeline)
5. [Inspectability, Observability & Live Debugging Playbook](#5-inspectability-observability--live-debugging-playbook)
6. [How to Add a New Command, Keybinding, or Feature](#6-how-to-add-a-new-command-keybinding-or-feature)
7. [Verification & CI Conservation Gates](#7-verification--ci-conservation-gates)

---

## 1. Crate & Library Dependency Stack (`Cargo.toml`)

Bilterm targets **Rust 2024 Edition** (`rust-version = "1.88"`). Every dependency is selected for cross-platform portability (Linux primary, macOS secondary, Windows third) and sub-millisecond immediate-mode cell compositing:

| Crate | Version | Role in Bilterm | Primary Modules |
|-------|---------|-----------------|-----------------|
| **`ratatui`** | `0.30.0` | Immediate-mode 2D `Buffer` (`Cell` grid) compositor and `Widget` / `TestBackend` rendering engine | `src/ui/*`, `src/vt/recording.rs` |
| **`crossterm`** | `0.29.0` (`event-stream`) | Cross-platform raw mode, alternate screen, SGR 1006 mouse capture, bracketed paste, and async `EventStream` | `src/app.rs`, `src/error.rs`, `src/input/*` |
| **`portable-pty`** | `0.9.0` | Cross-platform PTY allocation (`openpty`/`forkpty` on POSIX, `ConPTY` on Windows) and child process management | `src/pty/mod.rs` |
| **`vt100-ctt`** | `0.17.1` | Virtual terminal screen state machine backing `VtParser` (wrapped by `GridSnapshot` and custom CSI/OSC/DCS interceptors) | `src/vt/*` |
| **`libghostty-vt`** | `0.2.2` *(optional, `ghostty-vt` feature)* | Experimental Zig/C-ABI vtable terminal backend (`cargo +1.90 test --features ghostty-vt`) | `src/vt/ghostty_backend.rs` |
| **`tokio`** | `1.44.0` (`full`) | Async `tokio::select!` event loop, `spawn_blocking` 64KB PTY reader threads, and JSON-RPC 2.0 Unix/TCP server | `src/app.rs`, `src/pty/mod.rs`, `src/api/server.rs` |
| **`nucleo-matcher`** | `0.3.1` | High-speed fuzzy matching (from Helix) for `Ctrl+R` reverse command history and `Ctrl+P` Fuzzy Command Palette | `src/pane/fuzzy_history.rs`, `src/wm/session.rs` |
| **`unicode-width`** | `0.2.0` | East Asian / CJK / Emoji 2-column cell width calculation for `occlude_rect` boundary sanitization | `src/ui/occlusion.rs`, `src/vt/*` |
| **`serde`** + **`toml`** | `1.0.219` / `0.8.20` | Serialization for `~/.config/bilterm/config.toml`, saved workspace sessions (`:session`), and `--playbook` files | `src/config/mod.rs`, `src/wm/session.rs`, `src/api/playbook.rs` |
| **`libc`** | `0.2.170` | POSIX `tcgetattr` / `tcgetpgrp` termios inspection (`!ECHO`, `ICANON`) and `SO_PEERCRED` / `LOCAL_PEERCRED` socket UID verification | `src/pty/line_discipline.rs`, `src/secure_fs.rs`, `src/api/server.rs` |
| **`tracing`** + **`tracing-subscriber`** + **`tracing-appender`** | `0.1` / `0.3` / `0.2` | Non-blocking background file logging (`bilterm.log`) so diagnostics never corrupt the alternate screen | `src/main.rs` |
| **`color-eyre`** + **`thiserror`** | `0.6.3` / `2.0.12` | Structured error types (`BiltermError`) and panic hooks that restore terminal state before printing backtraces | `src/error.rs` |
| **`insta`** + **`tempfile`** *(dev)* | `1.42.2` / `3.19.1` | Deterministic ASCII/ANSI `TestBackend` snapshot testing and isolated temporary directories | `tests/*`, `src/**/tests.rs` |

---

## 2. Complete Source Tree & Module Boundaries (`<= 800` Lines Gate)

Every file under `src/**/*.rs` targets **~800–1000 lines** (`ARCH-R04`, soft goal; 1200-line test backstop). There are **no circular dependencies** between `pane` and `wm` (`wm` depends on `pane`; `colon_command` and `action` sit above or alongside cleanly).

```text
src/
├── lib.rs                      # Crate root exporting public modules
├── main.rs                     # Binary entrypoint, CLI dispatch, tracing init, panic guard
├── app.rs                      # Async tokio::select! 60 FPS event & render loop
├── launcher.rs                 # OS desktop launcher integration (.desktop / .plist / .lnk) & GUI-launch terminal auto-relaunch
├── cli.rs                      # CLI flag parser (--headless, --mcp, --playbook, ctl, --man, --completions)
├── error.rs                    # BiltermError enum & idempotent TerminalRestoreGuard (RAII)
├── secure_fs.rs                # 0700 private runtime dir, 0600 O_EXCL/O_NOFOLLOW files, constant_time_eq
├── action.rs                   # Unified Action enum (all key chords, :commands, playbooks & RPC lower here)
├── colon_command/
│   ├── mod.rs                  # Re-exports Action as ColonCommand alias, parse & execute entrypoints
│   ├── parse.rs                # Parses pane/WM ":..." strings into Result<Action, String>
│   ├── parse_visual.rs         # Parses visual/theme/font/drawer/canvas ":..." strings into Result<Action, String>
│   ├── parse_agents_list.rs    # `:agents list <verb>` grammar; every non-matching form is one usage error (Phase 51)
│   ├── execute_agents_list.rs  # Runs an `AgentsListOp`, sets its status and the scalars-only window summary (Phase 51)
│   ├── parse_agents_hud.rs     # `:agents hud ...`, `:agent replay ...`, `:agents watchdog` grammar (Phase 70)
│   ├── execute_agents_hud.rs   # Runs HudOp / ReplayOp / watchdog status, sets status + command data (Phase 70)
│   ├── parse_link.rs           # `:ssh-tmux` sub-verb grammar (reconnect, disconnect, info, prefill) (Phase 51.1)
│   ├── execute_link.rs         # Remote link execution and link_summary_json (Phase 51.1)
│   ├── execute.rs              # Executes ColonCommand against the active CompositePane
│   └── execute_wm.rs           # Executes Action against WindowManager (WindowManager::apply)
├── config/
│   └── mod.rs                  # BiltermConfig (config.toml + .ini theme loader + Spotlight/FocalZoom state)
├── input/
│   ├── mod.rs                  # Top-level handle_wm_key_event (<= 80 lines), handle_key_event, handle_paste_event
│   ├── keymap.rs               # Declarative DEFAULT_LEADER_BINDINGS, DEFAULT_GLOBAL_BINDINGS, DEFAULT_PANE_BINDINGS
│   ├── link_mouse.rs           # Banner hotkeys and mouse click routing for link and held banners (Phase 51.1)
│   ├── agents_list.rs          # AGENTS list mouse: pure `plan_agents_list_mouse` plus a thin applier (Phase 51, UIT-03)
│   ├── agents_list_keys.rs     # AGENTS list key table `AGENTS_LIST_BINDINGS` (Phase 51, UIT-03)
│   ├── modal.rs                # Modal key handlers (Help, Inspector, Palette, Menubar, Copy, Hints, Fuzzy, History)
│   ├── mouse.rs                # SGR 1006 mouse drag/resize/click/wheel routing across WM and CompositePane
│   └── encode.rs               # VT100/Xterm/DECCKM/Kitty CSI-u keystroke & alternate-scroll byte encoders
├── pane/
│   ├── mod.rs                  # Encapsulated CompositePane (private fields, submit_command, effective_routing_mode)
│   ├── prompt_editor.rs        # FloatingPromptState (encapsulated buffer/cursor, undo/redo, PromptAnchor, PromptMode)
│   ├── typography.rs           # Domain FontFamilyPreset, PaneFontOptions, CanvasResolution & cached TIOCGWINSZ
│   ├── canvas_scene.rs         # Retained PaneCanvasScene (vectors, 2D sprites, plots, images) & CanvasRenderBackend
│   ├── modes.rs                # Vi CopyModeState, QuickSelect HintsModeState, CanvasSearchState, accessors
│   ├── completion.rs           # Interactive Tab / Shift+Tab completion (:commands, $PATH, CWD files, $ENV)
│   ├── fuzzy_history.rs        # nucleo-matcher Ctrl+R fuzzy history search state
│   ├── highlight.rs            # Live shell lexical tokenizer & syntax highlighter
│   ├── history_store.rs        # Per-session 0600 command history persistence, pruning, and .sh export
│   ├── history_store_tests.rs  # Unit tests for HistoryStore
│   ├── redact.rs              # Secret-looking command classifier for Find (D-U2); never stores or returns the text
│   └── tests.rs                # Unit tests for CompositePane
├── pty/
│   ├── mod.rs                  # PtySession (portable-pty spawn, 64KB blocking reader thread, SIGWINCH dedup)
│   ├── line_discipline.rs      # 3-signal detector (Osc133Phase + tcgetpgrp + termios !ECHO/ICANON)
│   └── shell_integration.rs    # Automatic bash/zsh/fish OSC 133 + OSC 7 rc/hook injection in 0700 dir
├── vt/
│   ├── mod.rs                  # VtParser state machine, CommandBlock tracking, scrollback & size management
│   ├── backend.rs              # TerminalBackend port trait, MockTerminalBackend, and conformance suite (ARCH-R03)
│   ├── scan.rs                 # Single-pass ESC (\x1b) stream pre-scanner (DECSET traps, CSI DECRPM/DA1/DSR)
│   ├── scan_osc_dcs.rs         # OSC 7/8/9/52/133/777/1337, DCS XTGETTCAP/Sixel & APC Kitty scanners
│   ├── graphics.rs             # Kitty Graphics (APC G) & DEC Sixel (DCS q) decoders and placements
│   ├── tektronix.rs            # Tektronix 4010/4014 10-bit/12-bit vector storage-tube state machine & phosphor decay
│   ├── detect.rs               # Auto-detection of URLs and QuickSelect 3270 Field Hints (URLs, paths, IPs, SHAs)
│   ├── codec.rs                # OSC 52 base64 clipboard encoder and paste control-byte sanitizer
│   ├── recording.rs            # WorkspaceRecorder (whole-workspace Buffer->ANSI .cast) & single-pane .cast recorder
│   ├── scrollback.rs           # Dynamic RAM-aware ScrollbackGovernor and ring-buffer line reflow
│   ├── snapshot.rs             # Renderer-agnostic GridSnapshot, CellSnapshot & DEC LineSizeAttr
│   ├── worker.rs               # Dedicated background VtWorkerHandle thread seam
│   ├── ghostty_backend.rs      # Optional libghostty-vt C-ABI vtable integration
│   ├── tests.rs                # VtParser unit tests
│   └── scrollback_tests.rs     # ScrollbackGovernor unit tests
├── wm/
│   ├── mod.rs                  # Encapsulated WindowManager (private fields, apply(Action), remove_pane, Modal, Focus)
│   ├── v2_runtime.rs           # v2.1 runtime handlers (:detach, :attach, :mcp interactive actions, :exit-guard, :scroll)
│   ├── session.rs              # Workspace session save/restore/list (SplitTree + ZStack + prompts + lines), SSH/fleet helpers
│   ├── split_tree.rs           # Recursive H/V binary space partitioning (SplitTree, SplitDirection)
│   ├── z_stack.rs              # Overlapping Borland Turbo Vision z-order window deck (ZStack)
│   ├── drawer.rs               # slide-out (classic + Navigator styles; `src/wm/drawer/navigator.rs`, `src/ui/drawer/navigator.rs`) Sessions/Windows/Panes drawer & telemetry builder (DRW-01..02)
│   ├── quick_terminal.rs       # Persistent Guake/Quake-style F10 drop-down terminal (QuickTerminal) & frame helpers
│   ├── registry/              # Find registry (Phase 50.1): live rows, fuzzy rank, capture ring, saved/history.log reads (masked on read), transport facts
│   ├── agents_list.rs          # AGENTS list window state, row model and `agents_list_apply` (Phase 51, UIT-03)
│   ├── agents_list_cmd.rs      # `AgentsListOp` and `ListFilter`: the list's colon/RPC/key vocabulary (Phase 51)
│   ├── agents_list_geometry.rs # AGENTS list rect, layout and hit-test; the one source for render and mouse (Phase 51)
│   ├── agents_demo.rs          # `:agents demo` / `:agents demo stop`: spawned-pane record and demo event purge (Phase 51, UIT-01)
│   ├── traffic_monitor.rs      # TRAFFIC MONITOR window state, `TrafficMonitorOp`, apply (Phase 64-03)
│   ├── traffic_monitor/data.rs # group/sort/locality names, `TrafficQuery`, the one snapshot JSON builder (Phase 64-03)
│   ├── traffic_monitor_geometry.rs # TRAFFIC MONITOR rect, layout, hit-test; single source for render and mouse (Phase 64-03)
│   ├── panel_dock.rs           # Dock strip for `:arrange`: docked panels, slots and `set_panel_docked` (Phase 51, UIT-04)
│   ├── tmux_link.rs            # Persistent link lifecycle, supervisor, connect/reconnect/disconnect (Phase 51.1)
│   ├── tmux_io.rs              # I/O pumps: input send-keys, outbound greeting gate, hydrate capture, resize (Phase 51.1)
│   ├── pane_hold.rs            # Exited remote pane hold and respawn_active_pane (Phase 51.1)
│   ├── link_cmd.rs             # RemoteLinkOp vocabulary for :ssh-tmux sub-verbs (Phase 51.1)
│   └── tests.rs                # Unit tests for WindowManager & ZStack
├── daemon/
│   ├── mod.rs                  # Background Multiplexer Daemon exports, DaemonAction & DaemonHandshake
│   ├── process.rs              # Live OS process daemonization (daemon-<session>.sock, stale cleanup, session discovery)
│   ├── journal.rs              # Atomic WorkspaceJournal & crash-safe checkpoint rotation
│   └── session_tree.rs         # Hierarchical SessionTree (root -> branch -> leaf, graft, freeze/thaw, derez)
├── mcp_console/
│   └── mod.rs                  # Master Control Program (:mcp) state, McpDisplayMode (Float/Push), McpRowAction & parser
├── ui/
│   ├── mod.rs                  # 3-pass Ratatui compositor (WorkspaceWidget, CompositePaneWidget) + cell_to_ratatui_style
│   ├── mcp_ui.rs               # Interactive :mcp Floating/Push-Out Console & Exit Guard banner renderer
│   ├── help_catalog.rs         # Complete <F1>/:help reference lines & Ctrl+P/:palette command catalog
│   ├── gui_window.rs           # Standalone native OS GUI window runner (bilterm --gui / -G, Win32 GDI + Unix fallback)
│   ├── bitmap_font.rs          # IBM VGA / CP437 5x7 fallback bitmap font & exact box-drawing/block rasterizer
│   ├── gui_font/
│   │   ├── mod.rs              # Native ClearType TrueType/OpenType GUI font subsystem exports
│   │   ├── geometric.rs        # Pixel-perfect geometric box-drawing, block elements, Braille & Sextant renderer
│   │   └── win32_gdi.rs        # Win32 ClearType HFONT engine, custom .ttf/.otf loader & 1:1 CreateDIBSection BitBlt
│   ├── canvas_raster.rs        # SubcellBuffer (Braille/Sextant/Quadrant) & GUI SVG/PPM canvas rasterizer (CANV-01)
│   ├── pixel_canvas.rs         # Anti-aliased RGBA PixelCanvasBuffer, HalfBlock TrueColor blitter & SSH Kitty/Sixel/iTerm2 encoders
│   ├── typography.rs           # Dynamic font scaling (50%..=300%), DEC DECDWL/DECDHL & host OSC 50 passthrough (FONT-01)
│   ├── drawer.rs               # 4-edge slide-out drawer TUI/GUI renderer (Overlay drop shadow + Push reflow)
│   ├── agents_list_ui.rs       # AGENTS list window renderer: TUI cells and GUI scaled panel, drawn only from the layout (Phase 51)
│   ├── chrome.rs               # Borland borders (8 OutlineStyles), 4 DOS ShadowStyles, Top Menu Bar
│   ├── cursor.rs               # IBM 3270 cross-hairs, software carets, Focal Spotlight & Focal Zoom lens shaders
│   ├── glyphs.rs               # Unicode, CP437, Nerd Font, and ASCII glyph packs
│   ├── modals.rs               # Renderers for Help, 3270 Inspector, Command Palette, History Popup, Unsafe Paste
│   ├── occlusion.rs            # occlude_rect 2-column wide CJK/Emoji boundary sanitizer
│   ├── renderer.rs             # WorkspaceFrameRenderer trait, RatatuiWorkspaceRenderer, GuiWorkspaceRenderer (SVG/PPM)
│   ├── status_bar.rs           # IBM 3270 OIA status bar & Borland DOS F1..F10 Function-Key bar
│   └── theme.rs                # 5 built-in retro ThemePresets (Borland Blue/Cyan, IBM 3278/3279, Mono)
├── tmux/
│   ├── mod.rs                  # TmuxEnv ($TMUX detection), DCS passthrough, TmuxControlBridge (tmux -CC)
│   ├── link.rs                 # LinkState, LinkTarget, LinkSummary, LinkBanner, LinkEvent, LinkCmd (Phase 51.1)
│   ├── reconnect.rs            # Pure LinkMachine, capped exponential backoff, exit classification (Phase 51.1)
│   ├── splitter.rs             # ControlLineSplitter: raw PTY stream and ESC P 1000 p sentinel (Phase 51.1)
│   ├── transport.rs            # PtyLinkDriver, PtyLinkConnector, LinkDriver and LinkConnector traits (Phase 51.1)
│   └── hydrate.rs              # ReplyCorrelator, HydratePlanner, capture-pane hydration (Phase 51.1)
├── remote/
│   └── mod.rs                  # Validated SSH argv builders, RemoteHostProfile, FleetRegistry & FleetSelector
└── api/
    ├── mod.rs                  # Public automation API exports & ListenConfig/ListenEndpoint policy
    ├── json.rs                 # Zero-dependency recursive-descent JSON parser/serializer (depth-guarded <= 64)
    ├── rpc.rs                  # AutomationControlPlane JSON-RPC 2.0 method dispatcher & confined PPM snapshots
    ├── rpc_handlers.rs         # JSON-RPC 2.0 workspace/pane/session.save/session.restore/fleet handlers
    ├── rpc_agents.rs           # `agents.list` and `agents.demo` RPC methods; both lower to `:agents ...` colon commands (Phase 51)
    ├── rpc_link.rs             # remote.link RPC handler and honest connect result (Phase 51.1)
    ├── mcp.rs                  # Model Context Protocol (MCP 2024-11-05) stdio server (26 tools)
    ├── playbook.rs             # AutomationEvent bus & declarative TOML Playbook runner
    ├── server.rs               # Async Unix domain socket (0600 + peer UID) & TCP bearer-token RPC server/client
    └── tests.rs                # Security & RPC unit tests
```

---

## 3. Core Architectural Invariants (`Phases 19–22`)

### 3.1 Encapsulated `WindowManager` and `CompositePane` Invariants (`ARCH-R02`)
- **Private struct fields**: All fields on [`WindowManager`](../src/wm/mod.rs) and [`CompositePane`](../src/pane/mod.rs) are private. External modules (`app`, `api`, `input`, `ui`, `colon_command`) interact strictly through methods.
- **Single pane removal path (`WindowManager::remove_pane`)**: Closing the focused pane (`:close` / `Alt+W`), reaping exited child shells (`reap_exited_panes`), and disconnecting remote fleet nodes (`disconnect_fleet_node`) all call [`WindowManager::remove_pane`](../src/wm/mod.rs). This guarantees `zoomed_pane`, `split_tree`, `z_stack`, and `scrollback` stay synchronized even when a zoomed pane exits.
- **Single command submission path (`CompositePane::submit_command`)**: Pressing `Enter` in the floating prompt (`submit_prompt`), broadcasting commands (`broadcast_command`), and JSON-RPC `pane.exec` all route through [`CompositePane::submit_command`](../src/pane/mod.rs), enforcing read-only lock checks (`🔒 PROT`), `running_command` banner updates, scrollback reset, `VtParser` command block recording, and `HistoryStore` persistence in one place.
- **Exclusive `Modal` & `Focus` enums**:
  - [`Modal`](../src/wm/mod.rs) (`None`, `Help { scroll }`, `Inspector { scroll }`, `Palette { query, selected }`, `Menubar { category, item }`) makes simultaneous conflicting workspace modals unrepresentable.
  - [`Focus`](../src/wm/mod.rs) (`Quick` vs `Pane(PaneId)`) eliminates the `PaneId 0` sentinel collision in mouse hit-testing.

### 3.2 Unified `Action` Command Vocabulary & Declarative Keymap (`ARCH-R01`)
- [`crate::action::Action`](../src/action.rs) is the single typed command enum across the entire application.
- [`WindowManager::apply(&mut self, action: Action) -> Result<AppAction>`](../src/wm/mod.rs) is the single executor.
- [`src/input/keymap.rs`](../src/input/keymap.rs) defines declarative tables:
  - [`DEFAULT_LEADER_BINDINGS`](../src/input/keymap.rs): Maps post-leader keys (`"`, `-`, `h`, `F2`, `%`, `|`, `\`, `v`, `z`, `F5`, `o`, `n`, `Tab`, `F6`, `p`, `BackTab`, `1..9`, `w`, `M`, `q`, `Space`, `:`, `?`, `i`, `x`, `L`, `}`, `{`, `t`, `b`, `s`, `e`, `g`, `c`, `[`, `f`, `l`, `/`, `N`, `P`, `H`, `R`, `Up`, `Down`, `+`, `]`, `PageUp`, `PageDown`) to `Action`.
  - [`DEFAULT_GLOBAL_BINDINGS`](../src/input/keymap.rs): Maps global workspace shortcuts (`F1`..`F10`, `Alt+<key>`, `Ctrl+P`, `Ctrl+\`, `Ctrl+Space`) to `Action`.
  - [`DEFAULT_PANE_BINDINGS`](../src/input/keymap.rs): Maps intra-pane `F3` and `Alt+<key>` shortcuts to `Action`.
- **Dual-Prefix Leader (`Ctrl+B` + `Ctrl+X`) with `$TMUX` Auto-Fallback**:
  - [`is_leader_key_with_tmux`](../src/input/keymap.rs) accepts **both `Ctrl+B`** (`tmux`/`herdr` muscle memory) **and `Ctrl+X`** (classic DOS/Borland) when running standalone (`!inside_tmux`).
  - When `$TMUX` is detected (`inside_tmux == true`), `Ctrl+B` is not intercepted so it passes straight through to the outer `tmux` server, while `Ctrl+X` remains active for Bilterm.
- **Held-`Ctrl` Leader Normalization & `asciinema` Harmony (`REC-02`)**:
  - Holding `Ctrl` across a leader sequence (`Ctrl+X Ctrl+V`, `Ctrl+B Ctrl+Z`, `Ctrl+X Ctrl+E`, `Ctrl+B Ctrl+N`, `Ctrl+B Ctrl+P`) strips `KeyModifiers::CONTROL` on the follow-up key and dispatches the matching leader `Action` (while double-tapping `Ctrl+X Ctrl+X` or `Ctrl+B Ctrl+B` in raw mode still sends literal `0x18` / `0x02` to the PTY).
  - When a PTY wrapper like `asciinema rec` or `script` disables `ECHO` on its outer pseudo-terminal without entering `DECSET 1049` `ALT_SCREEN` or displaying a password prompt, pressing any prompt-mode toggle (`F4`, `Alt+E`, `F12`, `Ctrl+G`, `Ctrl+X e/t/b/s`, `:mode float/fixed`) sets `FloatingPromptState::force_command_prompt = true`, restoring the floating command prompt while [`CompositePane::is_active_canvas_password_prompt`](../src/pane/mod.rs) continues to lock out the prompt whenever `[sudo] password` or `Password:` appears on the canvas.

### 3.3 Whole-Workspace Asciicast v2 (`.cast`) Compositor Frame Recorder (`REC-02`)
- [`WorkspaceRecorder`](../src/vt/recording.rs) records the entire composited `ratatui::buffer::Buffer` produced by [`WorkspaceWidget::render`](../src/ui/mod.rs) on every frame.
- Each frame is diffed cell-by-cell against the previous `Buffer` (skipping wide-char continuation cells and emitting minimal `CUP` `\x1b[row;colH` + 24-bit/256/ANSI SGR transitions) and appended as standard `[elapsed_secs, "o", "..."]` JSON lines under `{"version":2,"width":W,"height":H,...}`.
- Commands:
  - `:record start [path]` / `:record workspace [path]` / `Ctrl+X R` — Records the entire multi-pane workspace (splits, z-order stacks, floating prompts, Borland menus, 3270 cross-hairs, modals).
  - `:record pane [path]` — Records only the active pane's raw PTY output stream.
  - `:record stop` — Finalizes and flushes the `.cast` file (defaulting to a `0600` file inside the per-user `0700` runtime directory).

### 3.4 Security Boundaries (`SEC-R01..04`)
- **Private runtime directory (`src/secure_fs.rs`)**: All temporary files (shell integration rc scripts, history exports, default `.cast` recordings, SSH `ControlPath` sockets, default headless Unix socket) live inside a verified `0700` UID-owned directory (`$XDG_RUNTIME_DIR/bilterm` or `/tmp/bilterm-<uid>`) and are created with `O_CREAT | O_EXCL | O_NOFOLLOW` mode `0600`.
- **Authenticated RPC & Snapshot Confinement (`src/api/`)**: `--headless` defaults to a `0600` Unix socket with kernel peer-UID verification; non-loopback TCP requires `--allow-remote` + `--token-file <0600-path>` verified via constant-time `auth.login`. RPC/MCP `workspace.snapshot` `ppm_path` is confined to `$XDG_STATE_HOME/bilterm/snapshots` (or `--snapshot-dir`), rejecting path traversal and symlinks.
- **SSH Option-Injection Guard (`src/remote/mod.rs`)**: `validate_destination` rejects leading `-`, whitespace, and control characters, and `build_ssh_args` / `build_ssh_tmux_cc_args` always place `--` before the destination.

---

## 4. End-to-End `Action` & Input Dispatch Pipeline

```mermaid
flowchart LR
    Key["KeyEvent (crossterm)"] --> Norm["normalize_macos_option_key"]
    Norm --> ModalCheck{"Active Modal?\n(Help/Inspector/Palette/Menubar)"}
    ModalCheck -- Yes --> ModalHandler["handle_wm_modal_key"]
    ModalCheck -- No --> LeaderPend{"wm.is_leader_pending()?"}
    LeaderPend -- Yes --> LeaderTbl["DEFAULT_LEADER_BINDINGS\n(+ held-Ctrl normalization)"]
    LeaderPend -- No --> UserBind{"config.toml [keybindings]\nor Ctrl+B / Ctrl+X Leader?"}
    UserBind -- Match --> Apply["WindowManager::apply(Action)"]
    UserBind -- None --> RawCheck{"RawPtyPassthrough?\n(ALT_SCREEN / !ECHO / F12)"}
    RawCheck -- Yes --> PtyEnc["encode_key_to_pty_bytes_full -> PTY"]
    RawCheck -- No --> GlobalTbl["DEFAULT_GLOBAL_BINDINGS\n(F1..F10, Alt+key, Ctrl+P, Ctrl+\\)"]
    GlobalTbl --> ColonEnter[":colon Enter /DEFAULT_PANE_BINDINGS"]
    LeaderTbl --> Apply
    ColonEnter --> Apply
    RPC["JSON-RPC wm.command /\nPlaybook colon step"] --> ParseColon["parse_colon_command"] --> Apply
```

---

## 5. Inspectability, Observability & Live Debugging Playbook

### 5.1 Live In-App `<F9>` 3270 Terminal Inspector Modal
Press **`F9`**, **`Ctrl+X i`**, **`Ctrl+B i`**, or run **`:inspector`** inside any Bilterm session to open the live **3270 Terminal Inspector** overlay, which displays:
- Pane ID, dimensions `(rows, cols)`, scrollback usage (`used / capacity` lines & bytes), and global `ScrollbackGovernor` memory budget.
- Line-discipline telemetry (`effective_routing_mode`, `echo_enabled`, `canonical_mode`, `foreground_pgid`, `shell_pgid`, `osc133_phase`, `force_command_prompt`, `manual_raw_override`).
- Active DECSET modes (`?1` DECCKM, `?47/?1047/?1049` ALT_SCREEN, `?1000..1007` Mouse, `?2004` Bracketed Paste, `?2026` Sync Output, `?2031` Color Scheme, `?2048` In-Band Resize) and Kitty keyboard protocol flags.
- Current cursor cell inspection (Unicode codepoint `U+XXXX`, UTF-8 bytes, display width, fg/bg colors, SGR modifiers).

### 5.2 Structured File Logging (`bilterm.log`)
Because TUI rendering owns `stdout`/`stderr`, Bilterm writes structured `tracing` logs to `$XDG_STATE_HOME/bilterm/bilterm.log` (or `/tmp/bilterm-<uid>/bilterm.log`):
```bash
RUST_LOG=bilterm=debug cargo run
# In a second terminal:
tail -f "${XDG_STATE_HOME:-$HOME/.local/state}/bilterm/bilterm.log"
```

### 5.3 Live External Introspection via `bilterm ctl` (JSON-RPC 2.0)
You can inspect or drive a headless or live daemon from another terminal or script:
```bash
# Start headless daemon on private 0600 Unix socket ($XDG_RUNTIME_DIR/bilterm/ctl.sock)
cargo run -- --headless &

# Query full workspace topology, pane states, modes, and z-order
cargo run -- ctl workspace.status

# Read active pane screen lines, cursor coordinates, OSC 133 command blocks, and 3270 hints
cargo run -- ctl pane.read '{"pane_id":1}'

# Execute any Action via :colon syntax
cargo run -- ctl wm.command '{"command":":split v"}'

# Query or scale workspace font size dynamically (Phase 25 FONT-01)
cargo run -- ctl workspace.fontsize '{"action":"150%"}'

# Drive the Tektronix 4010/4014 & modern graphical canvas (Phase 26 TEK-01, CANV-01)
cargo run -- ctl canvas.command '{"command":"line 0 0 120 60"}'
cargo run -- ctl canvas.status
```

### 5.4 Deterministic Headless Reproduction in Rust Tests
Every component has a zero-PTY deterministic constructor for fast unit/reproduction tests:
- `CompositePane::new_headless(id, rows, cols)` — Full VT100 + `FloatingPromptState` without spawning a child OS process.
- `WindowManager::new_headless(rows, cols)` — Multi-pane split tree + z-stack + modals in headless mode.
- `AutomationControlPlane::new_headless(rows, cols)` — Full JSON-RPC 2.0 + MCP + Playbook harness in memory.

---

## 6. How to Add a New Command, Keybinding, or Feature

Because of the Phase 22 unified `Action` architecture, adding a new workspace or pane operation requires touching only **4 places** (enforced at compile time by exhaustive `match` statements):

1. **Add variant to [`Action`](../src/action.rs)** in `src/action.rs`.
2. **Parse `:colon` syntax in [`parse_colon_command`](../src/colon_command/parse.rs)** in `src/colon_command/parse.rs` (if the action has a `:command` form).
3. **Implement execution in [`execute_wm_colon_command`](../src/colon_command/execute_wm.rs) or [`execute_colon_command`](../src/colon_command/execute.rs)** in `src/colon_command/execute_wm.rs` / `execute.rs` (called by `WindowManager::apply`).
4. **Bind default key chord (optional)** in [`DEFAULT_LEADER_BINDINGS`](../src/input/keymap.rs), [`DEFAULT_GLOBAL_BINDINGS`](../src/input/keymap.rs), or [`DEFAULT_PANE_BINDINGS`](../src/input/keymap.rs) in `src/input/keymap.rs`, and add the variant to `every_action_is_reachable_from_keymap_colon_or_rpc` in `src/action.rs`.

---

## 6b. Automation API and command parity (Phase 49)

- **Every action has a colon command.** All keybindings, palette entries, RPC and MCP calls lower to `Action` and run through `WindowManager::apply`. `src/command_registry.rs::catalog()` joins Tab completion (`COLON_COMMANDS`), the Ctrl+X palette (`PALETTE_CATALOG`), F1 help and `EXTRA` (descriptions and alias targets) into one read-only catalog that backs `:commands` and `bilterm://commands`. The parser stays hand-written. Drift tests fail when a completion command has no description, an alias target is missing, names are duplicated, a palette example head is not in the catalog, or the parser does not know a catalog command; `src/action.rs` tests cover that every `Action` has a colon form and keymap entries round-trip.
- **Transports** (all run the same `AutomationControlPlane::dispatch_method`; method list: `bilterm::api::RPC_METHODS`):
  1. JSON-RPC 2.0 over a Unix socket (default) or `--headless --listen` TCP; `bilterm ctl [-t FILE] [ENDPOINT] <METHOD> [JSON_PARAMS]` (the endpoint may be omitted).
  2. Loopback-only HTTP + SSE: `bilterm --headless --http 127.0.0.1:PORT` (`POST /rpc`, `GET /events`, `GET /health`, bearer token file written 0600 to the private runtime dir, Host/Origin checks, size limits).
  3. MCP over stdio (`bilterm --mcp`): 13 tools (including `bilterm_find`, a read-only search over `find.*`, and `bilterm_agents_list` / `bilterm_agents_demo`) plus resources `bilterm://state|config|commands|pane/<id>`. Error codes: JSON-RPC dispatch failures are `-32000` (parse `-32700`, invalid request `-32600`); MCP uses `-32602` for missing params/unknown tool and `-32601` for unknown methods.
- **Write gate.** Over RPC/HTTP/MCP/`pane.send_keys`, config-mutating commands (`:set`, `:config save`, `:appearance save`) are refused unless `[api] allow_config_write = true` in `config.toml` (default `false`); the `[api]` section is never settable over the API. Reads (`:get`, `:state`, `:config list`, `:commands`) always work and return structured `data` in the `wm.command` result. Persistence only happens on explicit `:config save` / `:appearance save`.
- **Conformance.** `tests/api_conformance.rs` drives one request table through direct dispatch, TCP, HTTP and MCP (identical results, error shapes, write gate), sweeps every runnable non-alias catalog example through `wm.command` (explicit skip list with reasons), and checks `RPC_METHODS` against the dispatcher source and `parse_ctl_args`.
- **Checklist: adding a command so everything stays in sync.**
  1. `Action` variant (`src/action.rs`) + parse (`src/colon_command/parse.rs`) + execute (`execute*.rs`).
  2. Add the name (and sub-arguments) to the Tab-completion table (`src/pane/completion.rs`), a palette entry in `PALETTE_CATALOG` and/or a help line; add an `EXTRA` row in `src/command_registry.rs` only if neither has a description (aliases: set the target). A verb with no palette entry needs a `PALETTE_EXCLUSIONS` row with a reason from the closed vocabulary in `src/command_registry.rs`; the palette sweep test fails otherwise.
  3. Make it reachable from keymap, colon or RPC in `every_action_is_reachable_from_keymap_colon_or_rpc`.
  4. Return structured output through `take_command_data` if callers should read it back; config-writing commands must go through `config_write_denied`.
  5. A new RPC method goes in `dispatch_method` AND `RPC_METHODS` (the conformance test fails otherwise); a new MCP tool needs a mapping in `src/api/mcp.rs` and `mcp_tools_list`.
  6. Run `scripts/verify.sh` (one line: `PASS n/n`; on FAIL read `target/verify/<stage>/fail.txt`).

---

## 6c. Adding a floating panel (Phase 51)

The AGENTS list window (`src/wm/agents_list*.rs`, `src/input/agents_list*.rs`, `src/ui/agents_list_ui.rs`, `src/api/rpc_agents.rs`) is the reference. A new floating panel splits the same way, so every surface lowers to the same `Action`s run through `WindowManager::apply` (UNI-R01, AGENTS.md convention 6). Keep each new file near the 800-1000 line soft goal (`ARCH-R04`).

1. **State.** A state struct in its own `src/wm/<panel>.rs` with `active`, `focused`, `pinned`, `docked`, `rect` and `drag`, and `captures_input() = active && focused`. `WindowManager` holds it in a `pub(crate)` field. Only one floating window owns the keyboard: opening one blurs the others, and `focus_and_raise` blurs both.
2. **Vocabulary.** An `Op` enum in `src/wm/<panel>_cmd.rs` with a `colon_form()` inverse, one `Action` variant for it, and a `PanelTarget` for the GUI scale. The `*_apply` match is exhaustive, with no wildcard arm.
3. **Geometry is the single source.** One `src/wm/<panel>_geometry.rs` with `rect`, `layout` and `hit`, in the panel's scaled virtual space (`panel_space`). The renderer and the mouse code both read it; neither computes its own rectangle.
4. **Render.** `src/ui/<panel>_ui.rs` draws only from the layout, in TUI cells and the GUI scaled panel. Sanitise every name with `sanitize_display`.
5. **Keys.** A declarative table in `src/input/<panel>_keys.rs`. The leader key is never consumed, and the toggle chord is checked before the table.
6. **Mouse.** A pure `plan_<panel>_mouse(wm, event, ws)` that returns a plan (actions, drag bookkeeping, swallow flag), plus a thin applier that calls `WindowManager::apply`. Test the plan, not the applier.
7. **Dock.** `set_panel_docked` is the only writer of the `docked` flag. Release the dock on every manual geometry change and on close, and add the panel to `DockPanel` and the `:arrange` hook (`src/wm/panel_dock.rs`).
8. **Colon, API and palette.** Parse and execute the colon form. For RPC, add the method to `dispatch_method` and to `RPC_METHODS` (`src/api/rpc_agents.rs` is the pattern); for MCP, add a tool mapping in `src/api/mcp.rs`. Add a palette entry or a `PALETTE_EXCLUSIONS` row, a help row and a completion row.
9. **Docs.** Document every verb in the user guide. The doc gate (`scripts/check_phase50_doc_coverage.sh`) reads the `:agents` and `:drawer` completion rows automatically, so a new verb there must appear in chapter 11 as `:<cmd> <verb>`. Chapter 11's "Surface uniformity" table records the path for each surface, and any exception with its reason.

## 6d. Persistent SSH + tmux link (Phase 51.1)

Phase 51.1 introduces persistent remote `tmux -CC` sessions over SSH (`:ssh-tmux`), replacing the local-shell placeholder with a real PTY transport, an interactive connection phase, a pure supervisor state machine with exponential backoff, bi-directional I/O pumps, and topology hydration.

### Module Map

- **`src/tmux/link.rs`**: Core types and traits (`LinkState`, `LinkTarget`, `LinkSummary`, `LinkExitSummary`, `LinkBanner`, `LinkEvent`, `LinkCmd`, `ReconnectPolicy`, `LinkDriver`, `LinkConnector`).
- **`src/tmux/reconnect.rs`**: Pure clock-injected `LinkMachine` state machine, deterministic capped exponential backoff (`backoff_delay_ms`: 1, 2, 4, 8, 16, 30s; max 20 attempts), exit classification (`classify_exit`), and banner text formatting (`link_banner_text`).
- **`src/tmux/splitter.rs`**: `ControlLineSplitter` separating raw initial terminal text (SSH prompts, host-key questions) and the DCS sentinel (`ESC P 1000 p`) from line-oriented `tmux -CC` control streams (`%begin`, `%output`, etc.).
- **`src/tmux/transport.rs`**: `PtyLinkDriver` and `PtyLinkConnector` implementing child process spawning via `portable-pty`, reading PTY bytes, and writing stdin.
- **`src/tmux/hydrate.rs`**: `ReplyCorrelator` (FIFO block correlation), `HydratePlanner` (300 ms post-greeting settle, `refresh-client` and `capture-pane` planning), `capture_pane_command`, and `render_capture`.
- **`src/wm/tmux_link.rs`**: Top-level link lifecycle on `WindowManager` (`connect_ssh_tmux`, `tick_tmux_link`, `link_reconnect`, `link_disconnect`, `tmux_link_summary`, `tmux_link_forget`), single-link guard, and structured `bilterm::link` tracing.
- **`src/wm/tmux_io.rs`**: Three bi-directional I/O pumps (`pump_tmux_input`, `pump_tmux_outbound`, `pump_tmux_replies`), pane reconciliation (`reap_removed_tmux_panes_and_sync_fleet`), and orphan tagging.
- **`src/wm/pane_hold.rs`**: `reap_or_hold_exited` holding exited remote panes with status banners and `respawn_active_pane`.
- **`src/colon_command/parse_link.rs` & `execute_link.rs`**: Grammar and execution for `:ssh-tmux reconnect|disconnect|info|prefill` and `link_summary_json`.
- **`src/input/link_mouse.rs`**: Interactive banner key routing (`Enter`/`r`, `x`, `d`) and left-click handling on banner rows.
- **`src/api/rpc_link.rs`**: JSON-RPC `remote.link` method and honest `remote.connect_tmux` / `remote.connect_ssh` result formatting.

### State Machine & Transitions

The link supervisor is a pure finite state machine (`LinkMachine` in `src/tmux/reconnect.rs`) driven by `LinkEvent` and timestamps:

| State | Transition Trigger | Next State | Actions Generated |
|-------|--------------------|------------|-------------------|
| `Idle` | `ConnectRequested(target)` | `Connecting` | `[SpawnTransport(target)]` |
| `Connecting` | `ControlStarted` (`ESC P 1000 p` seen) | `Connected` | `[MarkConnected, UpdateBanners]` |
| `Connecting` | Process exit / error | `Backoff` or `Failed` | `[KillTransport, UpdateBanners]` |
| `Connected` | Disconnect / EOF / transient `%exit` | `Backoff` | `[KillTransport, UpdateBanners]` |
| `Connected` | Clean `%exit` / remote session ended | `Ended` | `[KillTransport, UpdateBanners]` |
| `Backoff` | Timer reaches `retry_at_ms` | `Connecting` | `[SpawnTransport(target)]` |
| `Backoff` | User disconnect (`:ssh-tmux disconnect`) | `Detached` | `[KillTransport, MarkDetached]` |
| `Backoff` | Attempt count exceeds 20 | `Failed` | `[MarkFailed, UpdateBanners]` |
| Any | `ReconnectRequested` (`Ctrl+Alt+R`, `Enter`) | `Connecting` | `[KillTransport, SpawnTransport(target)]` |

### Tick Processing Order (`tick_tmux_link`)

Each frame in `tick_tmux_link(now_ms)` executes operations in strict sequence:
1. **Driver Poll**: Polls the link transport driver to drain incoming raw bytes through `ControlLineSplitter`.
2. **Control Line Processing**: Feeds completed control lines to `poll_tmux_control()`.
3. **Input Pump (`pump_tmux_input`)**: Flushes user keyboard input to `send-keys -H` chunks or raw PTY stdin.
4. **Outbound Pump (`pump_tmux_outbound`)**: Flushes queued bridge commands (`split-window`, `resize-pane`) once greeting block has ended.
5. **Hydration Pump (`pump_tmux_replies`)**: After 300 ms settle delay, executes `capture-pane` queries and hydrates screen cells via `render_capture`.
6. **Supervisor Tick**: Calls `link_machine.tick(now_ms)` to handle retry deadlines and state transitions.
7. **Banner Updates**: Refreshes link banners on active and mapped panes.

### Headless Testing Architecture

The entire subsystem is designed to be fully testable without network access, real SSH servers, or macOS targets:
- **`WindowManager::new_headless()`**: Instantiates an isolated in-memory window manager without a host TTY.
- **`TmuxControlSubprocess::from_channels()`**: Injects mock `cmd_tx` and `line_rx` channels to simulate arbitrary `tmux -CC` control responses (`%begin`, `%output`, `%layout-change`, `%exit`).
- **Mock `LinkConnector` & `LinkDriver`**: Implement the `LinkConnector` trait to simulate process spawning, stream delays, or network failures in tests (`wm.set_link_connector(Box::new(mock))`).
- **Clock Injection**: Time is explicitly passed as `now_ms: u64` to `wm.tick_tmux_link(now_ms)` and `LinkMachine::tick(now_ms)`. No test depends on `tokio::time::sleep`, `std::thread::sleep`, or real wall clocks.

### Test Suites

- `tests/phase51_1_link_seam.rs`: Architectural compilation seam, action lowering, and stub validation.
- `tests/phase51_1_ssh_argv.rs`: Frozen SSH argv generation, login shell wrapping, and argument redaction.
- `tests/phase51_1_transport.rs`: PTY link driver, control line splitter, and sentinel detection.
- `tests/phase51_1_reconnect.rs`: Pure `LinkMachine` state machine, backoff calculation, and exit classification.
- `tests/phase51_1_bridge.rs`: Bridge unseeding, pane adoption, and `%exit` sanitization.
- `tests/phase51_1_hydrate.rs`: `ReplyCorrelator` FIFO block parsing and `HydratePlanner` capture generation.
- `tests/phase51_1_hold.rs`: Exited shell retention with diagnostic banners and respawn-in-place.
- `tests/phase51_1_labels.rs`: Honest restore labels and drawer badge filtering.
- `tests/phase51_1_catalog.rs`: Tab completion, palette discovery, and menu bar parity.
- `tests/phase51_1_input.rs`: Banner hotkeys, mouse clicks, and raw password passthrough safety.
- `tests/phase51_1_api.rs`: `remote.link` RPC method and `bilterm_remote_link` MCP tool conformance.
- `tests/phase51_1_link.rs`: Full headless link lifecycle, reconnection supervisor, and fake-clock timing.
- `tests/phase51_1_io.rs`: Bi-directional I/O pumps (`input`, `outbound`, `replies`), pane hydration, and resize synchronization.

---

## 6e. Session & Window Management (Phase 53, `SES-01..05`, `UNI-R01`)

Session and window topology mutations dispatch through typed actions lowering to `WindowManager::apply`:
- `Action::WindowManage(WindowManageOp)`:
  - `Move { window_id, target_session, switch }`: Moves a window (composite pane) preserving live PTY, scrollback, prompt, and running commands (`SES-01`).
  - `Rename { window_id, new_name }`: Renames window with deterministic deduplication and validation (`SES-02`).
  - `Select(WindowSelectTarget)`: Non-modal window multi-selection (`Toggle`, `Range`, `All`, `None`, `Invert`, `Explicit`) (`SES-03`).
  - `Bulk(BulkWindowOp)`: Atomic all-or-nothing operations across selection (`Move`, `Close` honoring exit guards, `RenamePrefix`, `Tile`, `Pin`) (`SES-03`).
  - `Undo`: Single-step topology undo restoring layout, names, and selections (`SES-04`).
- `Action::SessionManage(SessionManageOp)`:
  - `Rename { old_name, new_name }`: Renames active or named session, updating SessionTree hierarchy (`SES-02`).
  - `MoveOrder { name, target_pos }`: User-defined session ordering (`SES-02`).
  - `Undo`: Session-level topology undo (`SES-04`).

Surface Uniformity (`UNI-R01`):
- **Keyboard**: Leader chords (`Ctrl+X m`, `Ctrl+X r`), Navigator keys (`Space` toggle selection, `Shift+Up/Down` range, `Ctrl+A` select all, `m` move, `r` rename, `x` close / bulk close, `u` undo).
- **Mouse**: Navigator `Ctrl+Click` multi-select toggle, right-click context menu (`Move to session...`, `Rename`, `Bulk move...`, `Bulk close`, `Bulk tile`).
- **Colon**: Full `:window <move|rename|select|bulk|undo>` and `:session <rename|move|undo>` grammar.
- **Control Plane**: JSON-RPC endpoints (`window.move`, `window.rename`, `session.rename`, `window.select`, `window.bulk`, `window.undo`), MCP tool `bilterm_window_manage`, and `bilterm ctl` CLI forwarding.

---

## 6f. The Traffic Monitor window (Phase 64-03)

`src/traffic/**` is the std-only metrics leaf (64-01); `src/wm/traffic_monitor*.rs`, `src/ui/traffic_monitor_{ui,gui}.rs`,
`src/input/traffic_monitor{,_keys}.rs`, `src/colon_command/parse_traffic.rs` and `src/api/rpc_traffic.rs` are the window
and its surfaces, cloned from the CLIPBOARD window structure (6c). Every operation is a `TrafficMonitorOp` executed through
`WindowManager::apply(Action::TrafficMonitor(op))`; `colon_form()` is the exact inverse of the parser (round-trip test).
Rect, layout and hit-testing live only in `traffic_monitor_geometry.rs` (`TrafficLayout::compute`), used by both renderers and
the mouse planner. `WindowManager::traffic_group_json` is the single JSON builder behind `:traffic show`, `traffic.snapshot`
and MCP `bilterm_traffic`. Toggle chords: `Ctrl+Alt+T` (reserved global, works in raw passthrough) and Leader `$` (`#` is
ArrangeGrid). API callers are refused `reset|enable|classify|oia` unless `[api] allow_config_write = true`.
`scripts/check_phase64_doc_coverage.sh` (run by `tests/phase64_traffic_surfaces.rs`) keeps chapter 13 in sync with the
`:traffic` completion row.

### Phase 70 Mission Control HUD, turn replay and watchdog (HUD-01..04)

`src/wm/agents_hud_cmd.rs` holds the three op enums (`HudOp`, `ReplayOp`, `WatchdogOp`) wrapped as `Action::AgentsHud`,
`Action::AgentsReplay`, `Action::AgentsWatchdog`; `colon_form()` is the inverse of `src/colon_command/parse_agents_hud.rs`.
`src/wm/agents_hud.rs` (state `AgentsHudState`, ops, the shared `agent_approve_pane`) and `agents_hud_feed.rs` (poll-tick
feed) own the behaviour; they wrap the standalone models `mission_control_hud.rs` (tiles, hit-test, Buffer renderer),
`turn_replay.rs` (per-pane `TurnReplayLog`) and `agent_watchdog.rs`. The window is drawn by `src/ui/agents_hud_ui.rs`
(the TUI and GUI share the cells) and driven by `src/input/agents_hud.rs`, whose pure functions `agents_hud_key_action` /
`hud_mouse_action` map a key or click to an `Action` (leader `*`, since every letter and F-key chord was taken and `A`
is the AGENTS list; in-window `a p d z r Esc`, arrows, PgUp/PgDn; right-click a tile = replay).

- **No resize, ever.** Opening, refreshing and drawing only read pane state; `CompositePane::resize_events()` is the
  counter the tests assert on. The refresh is capped by `MIN_REFRESH_MS` and the tile page by `MAX_TILES_PER_PAGE`.
- **Approve** runs `WindowManager::agent_approve_pane`, the same function as RPC `agent.approve`: it needs the Blocked state,
  and a click carries the tile's generation token (bumped on every state change), so a stale tile is refused. Panes in a
  no-echo prompt are masked (no preview, no replay lines). **Diff** reports a status line only: Phase 69 has a `swarm`
  library but no `:swarm review` surface to open yet (documented gap). **Prompt** focuses the pane's own prompt.
- **Replay** is fed by agent-state transitions (Working starts a turn, Idle/Done/Errored ends it; the prompt text comes from
  `agent.prompt` or the last command block). The view owns no PTY handle and swallows plain keys.
- **Watchdog** `[agents.watchdog]` (`enabled`, `stall_secs`, `loop_window`, `loop_repeat`, `alert = ["oia","toast","event"]`,
  `cooldown`) is parsed in `src/config/agents.rs` (`AgentsSection` keeps `[agents.<name>]` profiles beside it) and validated
  at load. Alerts go to the pane status line (`oia`, wins over `toast`), a timed toast, and/or the RPC `events.poll` queue
  (`watchdog_alert`). It never kills a pane.
- **RPC / MCP / CLI:** `agent.hud` (snapshot; optional `op` lowers to the same Action), `agent.replay` (turn list, one turn
  by `turn_id`, optional view `op`), `agent.watchdog`; MCP tool `bilterm_agent_control` (`action` = the method suffix, it also
  reaches `agent.approve|prompt|last_turn|wait` and `pane.*` / `workspace.*`); CLI `bilterm agent hud|replay|watchdog`.
  `tests/phase70_agent_swarm_parity.rs` is the `UNI-R01` matrix (Phases 66/67/70 live; Phases 68/69 are `#[ignore]`d gaps),
  `tests/phase70_hud_wiring.rs` the behaviour tests.

Adding a metered boundary (Plan 64-02 hooks): open a `TrafficTap` cell with the right `Origin` and `Role`
(`Wire` for physical links, `Payload` for decoded pane streams, `Both` only for a local PTY), count with `rx`/`tx` at the
single byte boundary, register the hook in `HOOK_MANIFEST` in `tests/phase64_layer_dag.rs`, and never count the same bytes
on both a Wire and a Payload cell of the same view.

---

## 7. Verification & CI Conservation Gates

Always run these 4 checks locally before committing or pushing:

```bash
# 1. Full unit, integration, snapshot, and security test suite
cargo test

# 2. Zero-warning Clippy lint gate across all targets
cargo clippy --all-targets -- -D warnings

# 3. Handle_wm_key_event <= 80 lines gate
test $(awk '/^pub fn handle_wm_key_event/,/^}/' src/input/mod.rs | wc -l) -le 80

# 4. Global 1200-line backstop (soft target ~800-1000)
awk 'ENDFILE{if(FNR>1200){print FILENAME,FNR; e=1}} END{exit e}' $(find src -name '*.rs')
```
