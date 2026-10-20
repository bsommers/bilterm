# Phase 1: Core PTY, VT100 Engine & Single CompositePane Prototype - Research

**Researched:** 2026-10-19
**Status:** Complete
**Requirements Covered:** `PTY-01`, `PTY-02`, `PTY-03`, `PANE-01`

---

## 1. Phase 1 Technical Architecture

Phase 1 establishes the foundational Cargo workspace/crate (`bilterm`) and proves the core invariant of Bilterm: **a single `CompositePane` where the child shell's PTY `OutputCanvas` spans 100% of the pane's usable height (`pane_rect.height.saturating_sub(1)`) while the `FloatingCommandPrompt` renders on top as a pure compositor overlay (`ratatui::widgets::Clear` + bordered prompt widget) without ever resizing the PTY (`SIGWINCH`) when the prompt moves or hides**.

### Module Layout (`src/`)
- `src/lib.rs` — Public library root exporting `pty`, `vt`, `pane`, `input`, `ui`, `app`, and `error` modules so unit, snapshot (`insta`), and headless PTY integration tests in `tests/` can test every subsystem directly.
- `src/main.rs` — Binary entrypoint installing `color-eyre` + terminal restoration panic/signal hooks (`RestoreTerminalGuard`), initializing non-blocking file tracing (`bilterm.log`), and running the `tokio` async event loop.
- `src/error.rs` — Structured error types (`BiltermError`) and `TerminalRestoreGuard` ensuring raw mode, `DisableMouseCapture`, and `LeaveAlternateScreen` are always restored on drop, panic, or `SIGTERM`/`SIGINT`/`SIGHUP`.
- `src/pty/mod.rs` — Cross-platform PTY manager (`PtyHandle`, `PtyEvent`, `PtyConfig`) built on `portable-pty 0.9.0`. Spawns the configured shell (`$SHELL`, `bash`, `zsh`, `fish`, or Windows `powershell.exe`/`cmd.exe`), exposes `resize(rows, cols)` (computing a geometry change guard so `resize` is a no-op if `(rows, cols)` has not changed, and tracking `sigwinch_count` for verification), writes bytes to the PTY master, and runs a dedicated `std::thread` / `tokio::task::spawn_blocking` reader loop feeding a bounded `mpsc` channel.
- `src/pty/termios_ inspect.rs` (or `src/pty/line_discipline.rs`) — Three-signal line-discipline inspector (`LineDisciplineState`, `InputRoutingMode`):
  1. **Signal 1 (`OSC 133`)**: Semantic shell prompt state parsed from the VT stream (`PromptReady` on `OSC 133;A`/`B`, `CommandExecuting` on `OSC 133;C`, `CommandFinished(exit_code)` on `OSC 133;D`).
  2. **Signal 2 (`tcgetpgrp` / Foreground Process Group)**: On Unix (`#[cfg(unix)]`), queries `libc::tcgetpgrp(fd)` against the shell's process group ID (`shell_pgid`) to detect when a foreground child process (`sudo`, `ssh`, `python`, `vim`) owns the terminal.
  3. **Signal 3 (`termios` `ECHO` / `ICANON`)**: On Unix (`#[cfg(unix)]`), queries `libc::tcgetattr(fd)` on the PTY master/slave FD to check `(c_lflag & libc::ECHO) == 0` (password prompt such as `sudo` or `ssh`) and `(c_lflag & libc::ICANON) == 0` (raw/cbreak interactive mode). Also provides deterministic mock/override injection for unit tests across all platforms.
- `src/vt/mod.rs` — Virtual Terminal state machine (`VtScreen`, `VtParser`, `CellStyle`, `Osc133State`). Uses `vt100-ctt 0.17.1` (and custom `OSC 133` + `DECSET 47/1047/1049` pre-scanner/tracker) to maintain:
  - Primary screen + scrollback buffer (`scrollback_len`, `set_scrollback_offset`)
  - Alternate screen state (`is_alternate_screen()` tracking `\x1b[?1049h`/`l`, `\x1b[?1047h`/`l`, `\x1b[?47h`/`l`)
  - Full 16-color, 256-color, and 24-bit TrueColor (`Color::Rgb`) + `BOLD`, `ITALIC`, `UNDERLINED`, `REVERSED` cell attributes
  - `OSC 133` semantic prompt markers (`\x1b]133;A`, `B`, `C`, `D`)
- `src/pane/mod.rs` — `CompositePane` state and geometry:
  - Holds `PaneId`, `VtParser`, optional `PtyHandle`, `FloatingPromptState` (text buffer, cursor position, `PromptAnchor::Top` [IBM 3270 default] / `Bottom` / `Floating { y_offset }`, height, manual visibility toggle), and `LineDisciplineState`.
  - Computes `canvas_rect(area)` = `Rect { x: area.x, y: area.y, width: area.width, height: area.height.saturating_sub(1) }`.
  - Computes `prompt_rect(canvas_rect)` clamped within `canvas_rect`.
  - Computes `effective_routing_mode()`: returns `InputRoutingMode::RawPtyPassthrough` whenever:
    - `vt.is_alternate_screen()` is true (`DECSET 47/1047/1049`), OR
    - `line_discipline.echo_disabled` is true (`!ECHO` password prompt), OR
    - `line_discipline.foreground_subcommand` is true (`fg_pgid != shell_pgid` or `OSC 133;C`), OR
    - `prompt.manual_raw_override` is true (`Ctrl+G` / `F12` toggle);
    otherwise returns `InputRoutingMode::CommandPrompt`.
  - Computes `is_prompt_visible()`: `effective_routing_mode() == InputRoutingMode::CommandPrompt && !prompt.manually_hidden`.
- `src/ui/mod.rs` — Ratatui `Widget` / `StatefulWidget` compositor for `CompositePane`:
  - **Pass 1 (`OutputCanvas`)**: Renders all visible rows/cols of `VtParser::screen()` into `canvas_rect` of `ratatui::buffer::Buffer`.
  - **Pass 2 (`FloatingCommandPrompt`)**: If `pane.is_prompt_visible()` is true, invokes `ratatui::widgets::Clear.render(prompt_rect, buf)` and renders the bordered IBM 3270 / Borland-styled floating command prompt (`╔═╗` double border, title `═══╣ BILTERM 3270 // CMD ╠═══`, current input line, and software cursor).
  - **Pass 3 (`OiaStatusBar`)**: Renders the bottom 1-row Operator Information Area (`READY` vs `X SYSTEM` / `ALT SCR` / `!ECHO LOCK`, `INPUT: CMD BOX` vs `RAW PTY`, shell name, and cursor `ROW/COL`).
- `src/input/mod.rs` — Keyboard event encoder and router:
  - Intercepts global hotkeys (`Ctrl+Q` quit, `Ctrl+G` or `F12` toggle raw PTY / floating prompt visibility, `PageUp`/`PageDown` scrollback in `CommandPrompt` mode).
  - In `InputRoutingMode::CommandPrompt`: routes printable characters, `Backspace`, `Delete`, `Left`, `Right`, `Home`, `End`, `Ctrl+U`, `Ctrl+K`, `Ctrl+A`, `Ctrl+E` to `FloatingPromptState`; on `Enter`, writes `format!("{}\n", line)` to the PTY master and clears the prompt buffer.
  - In `InputRoutingMode::RawPtyPassthrough`: encodes `crossterm::event::KeyEvent` into standard VT100/xterm byte sequences and writes them directly to the PTY master without touching or echoing into `FloatingPromptState`.

---

## 2. Pitfalls & Mitigations Applied in Phase 1

1. **Zero `SIGWINCH` on Prompt Move/Hide (`PANE-01`)**:
   - `CompositePane::resize_area(new_area)` calculates `canvas_rows = new_area.height.saturating_sub(1)` and `canvas_cols = new_area.width`. It only invokes `pty.resize(canvas_rows, canvas_cols)` and `vt.set_size(canvas_rows, canvas_cols)` when `(canvas_rows, canvas_cols) != self.last_canvas_size`. Moving or hiding the floating prompt never changes `canvas_size` and emits zero `SIGWINCH` signals.
2. **Alternate Screen Detection (`PTY-02`)**:
   - `vt100-ctt` tracks `screen().alternate_screen()` for `DECSET 47`, `1047`, and `1049`. We also test all three sequences (`\x1b[?1049h`/`l`, `\x1b[?1047h`/`l`, `\x1b[?47h`/`l`) explicitly in unit tests.
3. **Password Prompts (`sudo`/`ssh`) & Interactive Sub-REPLs (`PTY-03`)**:
   - Before routing a keystroke in `CompositePane`, `refresh_line_discipline()` checks `OSC 133` state + POSIX `tcgetpgrp` + `tcgetattr` (`!ECHO` / `!ICANON`). If `!ECHO` is active, `effective_routing_mode()` immediately switches to `RawPtyPassthrough`, hiding the floating command box and sending bytes straight to the PTY so passwords are never echoed into the floating editor buffer.
4. **Panic & Signal Terminal Restoration (`PTY-01`)**:
   - `TerminalRestoreGuard` tracks whether raw mode and alternate screen were entered and restores them idempotently on `Drop` and inside the `color_eyre` panic hook.

---

## 3. Validation Architecture

### Test Infrastructure
- **Framework**: `cargo test` (built-in Rust test harness) + `insta 1.42+` (`ratatui::backend::TestBackend` ASCII/style snapshot tests)
- **Quick run command**: `cargo test --lib`
- **Full suite command**: `cargo test --all-targets`
- **Linter command**: `cargo clippy --all-targets -- -D warnings`

### Verification Coverage Map
| Requirement | Automated Test Verification |
|-------------|-----------------------------|
| `PTY-01` | Headless `portable-pty` integration test spawning `/bin/sh`, executing `printf 'HELLO_BILTERM_%d\n' 42`, reading VT screen output, and testing `TerminalRestoreGuard` panic/drop cleanup |
| `PTY-02` | Unit + integration tests feeding `\x1b[?1049h`, `\x1b[?1047h`, `\x1b[?47h` and `\x1b[?1049l` to `CompositePane`, asserting `is_prompt_visible() == false` during alt-screen and automatic restoration on exit, plus `Ctrl+G` manual toggle |
| `PTY-03` | Unit + PTY tests verifying 3-signal detection (`OSC 133;C`, `fg_pgid != shell_pgid`, `termios !ECHO` for `sudo` password prompt) switches `effective_routing_mode()` to `RawPtyPassthrough` and never stores password characters in `FloatingPromptState` |
| `PANE-01` | `TestBackend` + `insta` snapshot tests verifying `OutputCanvas` spans `height - 1` rows underneath `FloatingCommandPrompt` (`widgets::Clear` occlusion) and proving `sigwinch_count` stays unchanged when moving or hiding the floating prompt |
