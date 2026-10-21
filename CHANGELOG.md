# Changelog

All notable changes to **Bilterm** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
