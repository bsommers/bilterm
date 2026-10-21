# Changelog

All notable changes to **Bilterm** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
