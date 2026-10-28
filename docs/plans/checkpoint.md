# Bilterm Session Checkpoint (`v2.1.0` + macOS Post-Merge Hardening)

- **Timestamp**: 2026-10-28T13:40:30-04:00
- **Branch & Commit**: `main` at `14181e0` (pushed to `origin/main`)
- **Installed Binary**: `/Users/bill/.local/bin/bilterm` & `/Users/bill/Applications/Bilterm.app` (`bilterm 2.1.0`)

## 1. Completed in This Session
1. **Merged Linux (`PR #2`) & macOS (`PR #3`) Sessions (`v2.1.0`, Phases 1–36)**:
   - Smooth Sub-Cell & Kinetic Velocity Scrolling (`PaneScrollMode::Smooth`) + IBM 3270 / VM/CMS Block/Page-Mode Scroller (`PaneScrollMode::BlockPage`, `MORE...`/`HOLDING` OIA badges, `PF7`/`PF8`/`PA1`/`PA2`/`Space`/`Enter`/`b`/`c` paging) (`tests/smooth_and_mainframe_page_scrolling.rs`).
   - Session Introspection & Regression Test Suite (`docs/LESSONS_LEARNED.md`, `tests/session_lessons_and_regression_suite.rs`).
   - Live background OS daemon (`bilterm ls` / `bilterm sessions`, `--daemon-serve`, per-session `0700`/`0600` Unix socket) & Interactive TRON MCP Console (`:mcp`, `Up`/`Down`/`Enter`/click row actions, Float/Push toggle).
2. **Fixed Claude Code (`claude`) & Antigravity CLI (`agy`) Runtime Issues (`dff0232`, `14181e0`)**:
   - **Claude Code Startup/Exit Crash Fix (`src/vt/mod.rs`, `src/vt/scan.rs`)**: Initialized `kitty_kbd_stack_main` and `kitty_kbd_stack_alt` to `vec![0]` and guarded `CSI < u` / `CSI = u` against empty stacks so `claude` popping Kitty Keyboard Protocol flags before pushing never panics at `src/vt/scan.rs:170`.
   - **Kitty Keyboard Protocol Shifted-Char Fix (`src/input/encode.rs`)**: Printable `KeyCode::Char(c)` with only `KeyModifiers::SHIFT` (`'M'`, `':'`, `'>'`, `'\''`) now sends raw UTF-8 bytes under disambiguate mode (`CSI > 1 u`) instead of `\x1b[<codepoint>;2u`, preventing garbled typing in `agy` and `claude`.
   - **Inline Raw-Mode TUI Prompt Submission (`src/pane/mod.rs`)**: Submitting from the `FloatingCommandPrompt` into a non-canonical raw-mode TUI with bracketed paste (`?2004h`, such as `agy` or `claude`) now wraps the payload in `\x1b[200~...\x1b[201~\r` (ending in `\r` `0x0D` instead of `\n` `0x0A`).
   - **GUI Viewport Resize Sync (`src/ui/gui_macos.rs`, `src/ui/gui_window.rs`)**: `app.wm.resize_all(desired_vp)` is called before every GUI redraw so toggling Fixed/Docked prompt mode (`F4` / `Alt+S`) or opening the `F7` drawer immediately updates the child PTY's `canvas_rect` dimensions (`SIGWINCH`).
   - **macOS GUI Zsh Login Profile & UTF-8 Locale (`src/pty/mod.rs`, `src/pty/shell_integration.rs`, `src/clipboard.rs`, `src/ui/gui_font/macos_ct.rs`)**: Sourced `~/.zshenv` and `~/.zprofile` in addition to `~/.zshrc` inside the `ZDOTDIR` wrapper, guaranteed `LANG=en_US.UTF-8` when `LANG`/`LC_ALL` are unset in `Bilterm.app`, and added MacRoman fallback decoding for `CFStringCreateWithBytes`.

## 2. Verification State
- `cargo +stable clippy -q --all-targets -- -D warnings`: `0` warnings.
- Unit & integration test suites passing (`301` tests total, including new regression tests in `src/vt/tests.rs`, `src/input/tests.rs`, and `src/pane/tests.rs`).
- All `src/**/*.rs` files `<= 800` lines (`ARCH-R04`) and `src/input/mod.rs::handle_wm_key_event` `<= 80` lines (`ARCH-R01`).

## 3. Exact Next Step (When Resuming)
Resume from `.planning/CHECKPOINT.md` / `docs/plans/checkpoint.md` to tackle cross-platform Windows verification, prebuilt release artifacts, or the next feature milestone.
