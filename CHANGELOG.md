# Changelog

All notable changes to **Bilterm** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v1.3.0] - 2026-10-21

### Added
- **Structured Automation Event Subscription Bus & Declarative Robot Playbooks (`AUTO-03`, `src/api/mod.rs`, `src/main.rs`)**:
  - `AutomationEvent` tracks `command_submitted`, `command_completed` (with `OSC 133;D` exit code and duration), `trigger_matched`, `notification_received` (`OSC 9`/`OSC 777`), `echo_lock_changed`, and `alternate_screen_changed` via `events.poll`.
  - `Playbook` (`bilterm --playbook <file.toml>` / `playbook.run`) executes declarative multi-step TOML automation playbooks (`colon`, `exec`, `send_keys`, `feed_output`, `assert_screen_contains`, `assert_trigger_fired`, `assert_exit_code`, `export_ppm`).

## [v1.2.0] - 2026-10-21

### Added
- **Native Model Context Protocol (MCP) Server for AI Agents (`AUTO-02`, `bilterm --mcp`, `src/api/mod.rs`, `src/main.rs`)**:
  - Zero-wrapper JSON-RPC 2.0 MCP (`2024-11-05`) server over `stdio` exposing `initialize`, `ping`, `tools/list`, and `tools/call` for 6 agent-native tools: `bilterm_workspace_status`, `bilterm_exec_command`, `bilterm_inspect_pane`, `bilterm_send_keys`, `bilterm_colon_command`, and `bilterm_run_playbook`.

## [v1.1.0] - 2026-10-21

### Added
- **JSON-RPC 2.0 Automation Control Plane, `--headless` Daemon & `bilterm ctl` CLI (`AUTO-01`, `src/api/mod.rs`, `src/main.rs`)**:
  - `AutomationControlPlane`, `run_headless_tcp_server` (`bilterm --headless --listen <addr>`), and `send_rpc_request` (`bilterm ctl <endpoint> <method> [params]`) exposing 100% of Bilterm's workspace, pane, `:colon` command, key-chord, and snapshot capabilities over JSON-RPC 2.0 with built-in `!ECHO LOCK` and `🔒 PROT` safety guards.

## [v1.0.0] - 2026-10-21

### Added
- **CLI Shell Completions, Manpage Generator & Nix Flake Packaging (`PKG-01`, `src/main.rs`, `docs/bilterm.1`, `flake.nix`)**:
  - Added `bilterm --completions bash|zsh|fish` and `bilterm --man` CLI flags alongside the standalone `roff(7)` manual page (`docs/bilterm.1`) and reproducible Nix flake (`flake.nix`).
  - Promoted crate version to `1.0.0` with full documentation synchronization across `README.md`, `docs/`, and `docs/user-guide/`.

## [v0.7.0] - 2026-10-21

### Added
- **`OSC 9` / `OSC 777` Desktop Notifications & OIA Bell Telemetry (`NTF-01`, `src/vt/mod.rs`, `src/ui/mod.rs`, `src/pane/colon_command.rs`)**:
  - Parses iTerm2 `OSC 9;<message>ST` and rxvt-unicode `OSC 777;notify;<title>;<body>ST` notifications alongside `BEL` (`0x07`), displaying a live `🔔 <count>:<title>` badge in the IBM 3270 OIA status bar and clearing via `:notifications clear` (`:notify clear`).
- **Custom Output Highlight Triggers (`TRG-01`, `src/config/mod.rs`, `src/ui/mod.rs`, `src/pane/colon_command.rs`)**:
  - Highlights user-defined pattern matches across the `OutputCanvas` in real time via `:trigger add <pattern>`, `:trigger clear`, or `triggers = [...]` in `config.toml`.
- **Asciicast v2 (`.cast`) Terminal Session Recorder (`REC-01`, `src/vt/mod.rs`, `src/ui/mod.rs`, `src/pane/colon_command.rs`)**:
  - Records live PTY output streams into standard Asciicast v2 JSONL `.cast` files (`:record start [path]` / `:record stop`) with a live `⏺ REC` indicator in the OIA status bar.

## [v0.6.0] - 2026-10-21

### Added
- **Interactive Fuzzy Command Palette (`CMD-01`, `Ctrl+P` / `Alt+Space` / `:palette`, `src/wm/mod.rs`, `src/input/mod.rs`, `src/ui/mod.rs`)**:
  - Borland Turbo Vision modal command palette powered by `nucleo-matcher` that searches and executes all `:colon` commands, switches panes, and applies themes with instant fuzzy filtering.
- **Custom Keybinding Remapping Engine (`CFG-02`, `src/config/mod.rs`, `src/input/mod.rs`)**:
  - Supports `[keybindings]` in `~/.config/bilterm/config.toml` (e.g. `"Ctrl+T" = ":split v"`) via `format_key_chord` and `execute_colon_command` dispatch.
- **Retro CRT Phosphor Scanline Shader (`CRT-01`, `:crt [on|off]`, `src/config/mod.rs`, `src/ui/mod.rs`, `src/ui/renderer.rs`)**:
  - Optional even-row scanline luminance modulation across both the Ratatui TUI compositor and the native `GuiFrameSurface` RGBA rasterizer.

## [v0.5.0] - 2026-10-21

### Added
- **Bi-Directional `tmux -CC` Session Daemon & Layout Topology Bridge (`TMUX-03`, `src/tmux/mod.rs`)**:
  - `parse_tmux_layout_panes()` parses `tmux` layout strings (`<checksum>,<WxH>,<X>,<Y>{...}[...]`) and automatically hydrates 1:1 local `CompositePane` instances on `%layout-change`.
  - Handles `%window-pane-changed`, `%window-close` / `%unlinked-window-close`, and `%exit [reason]`, and routes `FloatingCommandPrompt` submissions back to `tmux` via `TmuxControlBridge::route_prompt_submission` (`send-keys -t %<id> -H <hex>`), `queue_split`, and `queue_resize_client`.
  - `TmuxControlSubprocess::spawn_attach_or_new` spawns `tmux -CC new-session -A -s <session>` with asynchronous `stdin`/`stdout` Tokio channels.
- **Kitty Graphics Protocol (`APC G`) & DEC Sixel (`DCS q`) Support (`GFX-01`, `src/vt/mod.rs`, `src/ui/mod.rs`, `src/ui/renderer.rs`)**:
  - Decodes Kitty Graphics Protocol (`\x1b_G<k=v,...>;<base64>\x1b\\`) queries (`a=q` -> `\x1b_Gi=<id>;OK\x1b\\`), RGB/RGBA transmissions & placements (`a=t`, `a=T`, `a=p`), and deletions (`a=d`).
  - Decodes DEC Sixel (`\x1bP<params>q<sixel>\x1b\\`) raster attributes (`"Pan;Pad;Ph;Pv`), RGB color registers (`#<reg>;2;<r>;<g>;<b>`), RLE repeat (`!<count><char>`), and 6-pixel vertical bands into RGBA pixel buffers.
  - Renders `TerminalGraphicPlacement` regions onto TUI `OutputCanvas` cells using 24-bit TrueColor Unicode half-blocks (`▀`) and attaches full-resolution `GuiImageTexture` buffers to `GuiFrameSurface` in GPU mode.

## [v0.4.0] - 2026-10-21

### Added
- **Native GPU/GUI Workspace Renderer & Framebuffer Pipeline (`GPU-01`, `src/ui/renderer.rs`, `src/main.rs`)**:
  - `GuiWorkspaceRenderer` and `GuiFrameSurface` implement `WorkspaceFrameRenderer` for `GPUI` / `winit + egui` integration, emitting `GuiQuadPrimitive` backgrounds/underlines, `GuiGlyphRun` text runs, `GuiImageTexture` quads, and a deterministic RGBA / Netpbm `P6` `.ppm` software rasterizer (`--gui-dump <path.ppm>`).
- **`libghostty-vt` C-ABI Vtable & FFI Handle (`GHOSTTY-01`, `src/vt/ghostty_backend.rs`)**:
  - Added `GhosttyCAbiVtable` (`terminal_new`, `terminal_free`, `terminal_vt_write`, `terminal_resize`) and safe `GhosttyCAbiTerminal` wrapper alongside the `--features ghostty-vt` binding.

## [v0.3.5] - 2026-10-21

### Changed
- **Living Documentation & User Guide Synchronization (`README.md`, `docs/architecture.md`, `docs/user-guide/`)**:
  - Synchronized all documentation tables, architecture diagrams, keyboard/mouse/`:command` references (`F9`, `Alt+F`, `Alt+L`, `Alt+/`, `Alt+C`, `:inspector`, `:hints`, `:lock`, `:broadcast`, `:menubar`, `:session`, `:search`, `:links`, `:copy`), and multi-pane session guides with `v0.2.0`–`v0.5.0`.

## [v0.3.0] - 2026-10-21

### Added
- **Live 3270 Terminal Inspector Modal (`BEL-01`, `F9` / `:inspector`, `src/vt/mod.rs`, `src/ui/mod.rs`, `src/input/mod.rs`)**:
  - Real-time Borland Turbo Vision diagnostic overlay inspecting the focused `CompositePane`'s active `DECSET` modes (`?1049`, `?2004`, `?1006`, `?1007`, `?2026`, `?2027`, `?2031`, `?2048`), `DECCKM`/`DECBKM`, Kitty keyboard protocol flags (`CSI > flags u`), cursor coordinates, cell-under-cursor grapheme + `U+XXXX` codepoints, extended SGR underline/overline state, 3-signal line-discipline status, and a bounded 16-entry ring buffer of recent CSI/OSC escape sequences.
- **`OSC 9;4` OIA Progress Bar Gauge (`BEL-02`, `src/vt/mod.rs`, `src/ui/mod.rs`)**:
  - Parses ConEmu/Ghostty `\x1b]9;4;<state>;<pct>(\x07|\x1b\\)` progress sequences (`Normal`, `Error`, `Paused`, `Indeterminate`, and reset `0`) and renders a live 8-cell block progress bar (`[████░░░░] 50%`, `![████░░░░] 50%`, `⏸[████░░░░] 50%`, `[▓▓░░░░▓▓] BUSY`) inside the IBM 3270 Operator Information Area (OIA) status bar.
- **Color Scheme Change Notifications (`BEL-03`, `DECSET ?2031` & `CSI ? 996 n`, `src/vt/mod.rs`, `src/wm/mod.rs`, `src/pane/colon_command.rs`)**:
  - Replies to `CSI ? 996 n` color-scheme DSR queries with `CSI ? 997 ; 1 n` (dark) or `CSI ? 997 ; 2 n` (light) computed from the active theme background luminance, and automatically dispatches unsolicited `CSI ? 997 ; 1|2 n` notifications to all panes with `DECSET ?2031` enabled whenever `:theme` changes.
- **Unicode Grapheme Cluster Mode (`FND-07`, `DECSET ?2027`, `src/ui/mod.rs`)**:
  - Preserves multi-codepoint Zero-Width Joiner (`U+200D`) and Variation Selector-16 (`U+FE0F`) grapheme clusters on the `OutputCanvas` when `DECSET ?2027` is active.
- **QuickSelect 3270 Field Hints (`ECO-01`, `Alt+F` / `:hints`, `src/vt/mod.rs`, `src/pane/mod.rs`, `src/ui/mod.rs`, `src/input/mod.rs`)**:
  - Scans the visible `OutputCanvas` for URLs (`https://`, `http://`, `file://`), file paths (`/path`, `./path`, `../path`, `~/path`, `dir/file`), IPv4 addresses (`a.b.c.d[:port]`), and Git commit SHAs (`7..40` hex chars), overlays high-contrast `[a]..[z]` jump badges, and inserts the selected token into the floating command prompt while yanking via `OSC 52`.
- **Workspace Layout & Session Serialization (`ECO-02`, `:session save <name>` / `:session restore <name>` / `:session list`, `src/wm/mod.rs`, `src/pane/colon_command.rs`)**:
  - Saves and restores complete multi-pane workspace topologies (`WorkspaceLayoutMode`, `SplitTree`, `ZStack` z-order, zoom state, pane titles, working directories, `PromptAnchor`, prompt dimensions, read-only locks, and command histories) to declarative TOML files in `$XDG_STATE_HOME/bilterm/sessions/<name>.toml`.
- **Interactive Top Borland Turbo Vision Menu Bar (`ECO-03`, `:menubar [on|off]`, `src/ui/mod.rs`, `src/input/mod.rs`, `src/config/mod.rs`)**:
  - Optional 1-row top pull-down menu bar (`≡ System  Window  Prompt  View  Theme  Tmux  Help`) with mouse click activation, keyboard arrow navigation, and Borland shadow-framed pull-down menus.
- **Read-Only Protected 3270 Field Lock & Multi-Pane Input Broadcast (`ECO-04`, `Alt+L` / `:lock` & `:broadcast`, `src/pane/mod.rs`, `src/wm/mod.rs`, `src/input/mod.rs`, `src/ui/mod.rs`)**:
  - Per-pane Read-Only lock (`Alt+L` / `:lock [on|off]`) displays a `🔒 PROT` OIA badge and blocks accidental shell command submissions or raw PTY keystrokes while still permitting `:colon` commands.
  - Synchronized Multi-Pane Input Broadcast (`:broadcast [on|off]`) displays a `📡 BCAST` badge and fans out submitted commands to all unlocked workspace panes simultaneously.

## [v0.2.5] - 2026-10-21

### Added
- **Renderer-Agnostic `GridSnapshot` & `CellSnapshot` Seam (`ARCH-01`, `src/vt/snapshot.rs`, `src/vt/mod.rs`)**:
  - Point-in-time `GridSnapshot`, `RowSnapshot`, `CellSnapshot`, `CursorSnapshot`, `UnderlineKind`, and `RgbTriplet` structures that decouple the UI rendering layer (`Ratatui`, `GPUI`, or `winit + egui`) from the underlying virtual terminal state machine.
- **Dedicated Emulation Worker Thread Pipeline (`ARCH-02`, `src/vt/worker.rs`, `src/vt/ghostty_backend.rs`)**:
  - `VtWorkerHandle` and `SharedGridSnapshot` spawn a dedicated OS worker thread (`bilterm-vt-worker`) paired with a `portable-pty` reader pipe (`bilterm-pty-pipe`), feeding raw VT/ANSI byte chunks into `libghostty-vt` (`GhosttyVtEngine` behind `--features ghostty-vt`) or `VtParser`, answering terminal queries (`DSR`, `CPR`, `DA1`, `DECRPM`, `OSC 4/10/11/12`, `XTGETTCAP`) directly on the worker thread, and publishing atomic `GridSnapshot` frames to the UI layer.
- **Abstract UI & Window Layer Renderer Seam (`ARCH-03`, `src/ui/renderer.rs`)**:
  - `WorkspaceFrameSnapshot`, `PaneFrameSnapshot`, and `WorkspaceFrameRenderer` (`RatatuiWorkspaceRenderer`) allow `Ratatui`, Zed's `GPUI`, or `winit + egui` to render the entire multi-pane workspace (`SplitTree`, `ZStack`, `QuickTerminal`, `FloatingPrompt`, `OiaStatusBar`) from immutable snapshots.
- **Soft-Wrapped Scrollback Reflow (`FND-08`), Extended SGR Attributes (`FND-09`) & In-Band Size Reports (`VAL-05`, `src/vt/mod.rs`)**:
  - Reflows soft-wrapped primary-screen lines when horizontal pane columns shrink or expand (`VtParser::set_size`), while leaving Alternate Screen (`?1049`) grids un-reflowed.
  - Parses extended SGR sub-parameter underlines (`4:0` none, `4:1` single, `4:2` double, `4:3` curly, `4:4` dotted, `4:5` dashed), `SGR 53`/`55` overline, and `SGR 58;2;r;g;b` / `58:2::r:g:b` RGB underline colors.
  - Emits in-band resize notifications (`CSI 48 ; rows ; cols ; 0 ; 0 t`) when `DECSET ?2048` is enabled.
- **`vttest` / `esctest` & 17-Session Torture Suite (`tests/vttest_esctest_fuzz_torture.rs`, `tests/vt_worker_and_snapshot_seam.rs`)**:
  - Added 1-byte/3-byte/7-byte split-boundary escape sequence fuzzing, parameter bomb tests, 16-workspace-pane + 1-Quake-panel simultaneous tiling/stacking/shuffling stress tests, and live `portable-pty` -> `VtWorkerHandle` -> `GridSnapshot` integration tests (74 total tests passing).

## [v0.2.0] - 2026-10-21

### Added
- **Paste Injection Sanitization & Unsafe Multi-Line Paste Guard (`FND-01`, `src/vt/mod.rs`, `src/input/mod.rs`, `src/ui/mod.rs`)**:
  - `sanitize_paste_bytes()` strips dangerous C0/DEL control bytes (`NUL`, `ESC`, `DEL`, `VINTR 0x03`, `VQUIT 0x1C`, `VSUSP 0x1A`, `VWERASE 0x17`, `VLNEXT 0x16`) from all clipboard pastes.
  - `is_unsafe_paste()` intercepts unbracketed multi-line pastes and embedded `\x1b[201~` bracketed-paste breakout payloads in `INPUT: RAW PTY` mode and opens a Borland Turbo Vision `[!] UNSAFE PASTE GUARD` confirmation modal (`y`/`Enter` to confirm, `n`/`Esc` to abort).
- **Mouse Alternate Scroll (`DECSET ?1007`, `FND-02`, `src/vt/mod.rs`, `src/input/mod.rs`)**:
  - Tracks `\x1b[?1007h`/`l` (enabled by default) and translates mouse wheel scroll events on the `OutputCanvas` into `Up`/`Down` arrow keys (`\x1bOA`/`\x1bOB` or `\x1b[A`/`\x1b[B`) when in Alternate Screen (`?1049`) without mouse tracking enabled (`less`, `man`, `git log`).
- **Modern Terminal Protocol Queries (`DECRPM`, Kitty Keyboard `CSI u`, Dynamic `OSC 4/10/11/12`, `XTGETTCAP` & `DECRQSS`, `FND-03..06`, `src/vt/mod.rs`, `src/input/mod.rs`)**:
  - `DECRPM` (`CSI ? Pd $ p` & `CSI Pd $ p`) mode state reporting (`CSI ? Pd ; Ps $ y`) for `?1`, `?7`, `?25`, `?47`, `?67`, `?1000..1007`, `?1047`, `?1049`, `?2004`, `?2026`, `?2027`, `?2031`, `?2048`.
  - Per-screen Kitty Keyboard Protocol flag stacks (`CSI > flags u`, `CSI < count u`, `CSI = flags ; mode u`, `CSI ? u`) and `CSI u` key encoding (`encode_key_to_pty_bytes_full`).
  - Dynamic palette color query responders for `OSC 4`, `OSC 10` (fg), `OSC 11` (bg), and `OSC 12` (cursor) (`\x1b]10;rgb:RRRR/GGGG/BBBB\x1b\\`).
  - `XTGETTCAP` (`DCS + q <hex> ST` -> `DCS 1 + r <hex>=<hex> ST` for `TN`, `Co`, `RGB`, `Tc`, `Smulx`, `Setulc`, `Ms`) and `DECRQSS` (`DCS $ q <pt> ST`).
- **Zero-Config Shell Integration Auto-Injection (`VAL-01`, `src/pty/shell_integration.rs`, `src/pty/mod.rs`)**:
  - Automatically injects `OSC 133;A/B/C/D;<exit>` and `OSC 7` working directory reporting hooks into interactive `bash` (`--rcfile`), `zsh` (`ZDOTDIR`), and `fish` (`XDG_DATA_DIRS`) sessions without modifying user dotfiles.
- **Command Exit-Code Badges & Execution Duration Telemetry (`VAL-06`, `src/vt/mod.rs`, `src/ui/mod.rs`)**:
  - Records per-command execution duration (`duration_ms`) and exit code (`exit_code`) on `CommandBlockRecord` and renders `✓ 0 [12ms]` / `✗ 1 [45ms]` telemetry badges in the floating prompt's `LAST CMD` banner.
- **Output Canvas Scrollback Search & Match Highlighting (`VAL-02`, `Alt+/` / `:search <query>`, `Alt+n` / `Alt+N`)**:
  - Case-insensitive search across the `OutputCanvas` grid with bright yellow/amber match cell highlighting and `next`/`prev` navigation.
- **`OSC 8` Explicit Hyperlinks & URL Auto-Detection (`VAL-03`, `Ctrl+Click` / `:links`)**:
  - Parses explicit `OSC 8` hyperlinks and auto-detects `https://`, `http://`, and `file://` URLs on the `OutputCanvas` with cyan underline styling and `Ctrl+LeftClick` / `:links` `OSC 52` clipboard yanking.
- **Vi Copy Mode & Rectangular Block Selection (`VAL-04`, `Alt+C` / `:copy`, `Alt+LeftDrag`)**:
  - Keyboard-driven Vi Copy Mode (`h/j/k/l/0/$/w/b`, `v` character selection, `V` line selection, `b`/`Ctrl+V` rectangular block selection, `y`/`Enter` yank via `OSC 52` and prompt yank buffer) plus `Alt+LeftDrag` mouse rectangular block selection on the `OutputCanvas`.

## [v0.1.0] - 2026-10-20

### Added
- **Core PTY & VT100/VT220/Xterm Emulation Engine (`src/pty/`, `src/vt/`)**:
  - Cross-platform non-blocking PTY process manager (`portable-pty` + `tokio`) with dynamic `TIOCSWINSZ` resize propagation and bounded 64KB chunk streaming for zero-freeze flood resilience (`tail -f /dev/random`).
  - Full control sequence state machine (`VtEngine`): Alternate Screen (`DECSET ?1049/?1047/?47`), Application Cursor Keys (`DECCKM ?1`), Bracketed Paste (`DECSET ?2004`), SGR/X10/Button/Any Mouse Tracking (`?1000/?1002/?1003/?1006`), Focus Reporting (`?1004`), Synchronized Output (`?2026`), `OSC 133` (`A`/`B`/`C`/`D`) semantic shell prompt markers, `OSC 52` clipboard capture, `OSC 0`/`OSC 2` window titles, `OSC 7` working directory tracking, and `DSR`/`CPR`/`DA1` bidirectional terminal query replies.
  - 3-Signal Line-Discipline Detector (`src/pty/line_discipline.rs`): Combines `DECSET 1049`, `OSC 133`, and `tcgetattr` (`ECHO`/`ICANON`) with printable heuristic fallback so binary output floods never hide or lock out the floating command prompt.
- **Composite Terminal Pane & Floating Command Prompt (`src/pane/`)**:
  - 4-mode Command Prompt Layout (`PromptAnchor`): Floating Top (`Top`), Floating Bottom (`Bottom`), and non-overlapping Split Command/Output Panes (`SplitTop`, `SplitBottom`).
  - Resizable command prompt width (`Alt+]`/`Alt+[`, `:prompt width <cols|full>`, left/right/corner mouse drag) and vertical/horizontal prompt viewport scrolling (`Shift+PageUp`/`Shift+PageDown`, mouse wheel, right-border scrollbar indicator).
  - Running/Last Command indicator banner (`⟳ RUNNING:` / `● LAST CMD:`) displayed inside the command prompt box after submitting a command (`Enter`).
  - Context-aware `<Tab>` completion (`src/pane/completion.rs`) for `$PATH` executables (first token) and relative、`~/`、or directory paths (subsequent tokens) with longest-common-prefix expansion and candidate cycling.
  - Multi-line shell editor with live syntax highlighting, fuzzy history search (`Ctrl+R`), and `OSC 133` command block jump (`Alt+Up`/`Alt+Down`).
- **Persistent Session Command History & History Pop-up (`src/pane/history_store.rs`)**:
  - Automatic per-session history persistence (`$XDG_STATE_HOME/bilterm/history/history-<terminal_id>-<session_id>.log`) with ISO-8601 UTC timestamps, unique session IDs, pane IDs, and configurable file rotation (`history_max_files`, `:history on|off`, `:history max-files <N>`).
  - Temporary shell script export (`:history save [path]`) and interactive Borland-style Command History Pop-up Window (`Alt+P` / `:history`).
- **Multi-Pane Window Manager, Z-Order Deck & Quake Drop-Down (`src/wm/`)**:
  - Binary Split Tree (`SplitNode`) for arbitrary horizontal/vertical composite pane splits (`Alt+H`, `Alt+V`) and directional navigation.
  - Overlapping Z-Order Window Stack (`ZOrderStack`) with cycle-raise, shuffle, and mouse click-to-focus.
  - Persistent Quake/Guake drop-down quick terminal (`F12` / `` Ctrl+` ``).
  - Interactive `<F1>` / `:help` Borland Turbo Vision Quick Reference Help Modal (`Up`/`Down`/`PageUp`/`PageDown` scrollable).
- **Borland Turbo Vision Chrome, IBM 3270 Cross-Hairs & Configuration (`src/ui/`, `src/config/`)**:
  - 5 built-in retro palettes (`borland-classic`, `turbo-vision-dark`, `ibm-3270-green`, `ibm-3270-amber`, `cyberpunk-neon`), 8 border outline styles (`double`, `single`, `rounded`, `thick`, `dotted`, `dashed`, `ascii`, `none`), and 4 DOS drop-shadow styles (`classic`, `solid`, `tinted`, `none`).
  - Software IBM 3270 Cross-Hair Cursor (`Alt+X`, `:crosshair full | pane | off`) with pane-local and full-screen span modes plus 4 caret glyphs (`Block`, `HollowBlock`, `Underscore`, `Bar`).
  - Rich IBM 3270 OIA Status Bar with 4 glyph profiles (`unicode`, `cp437`, `nerdfont`, `ascii`), live `⌖ R:xx C:xxx` cursor coordinates, and `≣ OUT:<lines>L/<bytes>` output length telemetry.
  - Persistent TOML configuration (`~/.config/bilterm/config.toml`) with live `:set` and `:save` commands.
- **Dual-Mode `tmux` Integration (`src/tmux/`)**:
  - Host `tmux` detection (`$TMUX`, `DCS tmux;` passthrough wrapping) and `tmux -CC` control-mode parser (`%output`, `%layout-change`, `%exit`).
- **Testing & Documentation (`tests/`, `docs/`)**:
  - 55 automated unit, integration, `insta` snapshot, and terminal torture/stress tests (`81.83%` line coverage across `src/`).
  - Architecture guide (`docs/ARCHITECTURE.md`), coverage report (`docs/TEST_COVERAGE.md`), and 7-chapter User Manual (`docs/user-guide/`).
