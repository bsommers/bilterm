---
phase: 01-core-pty-vt100-engine-single-compositepane-prototype
verified: 2026-10-20T11:56:00Z
status: passed
score: 11/11 must-haves verified
behavior_unverified: 0
---

# Phase 1: Core PTY, VT100 Engine & Single CompositePane Prototype Verification Report

**Phase Goal:** As a terminal user, I want to launch Bilterm into an interactive shell inside a full-height PTY output canvas with an overlaid floating command prompt, automatic `DECSET 47/1047/1049` alternate-screen prompt hiding, and safe `sudo`/REPL input routing, so that terminal output scrolls smoothly underneath the prompt without `SIGWINCH` geometry thrashing or password/TUI corruption.
**Verified:** 2026-10-20T11:56:00Z
**Status:** passed

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Bilterm spawns an interactive shell process via `portable-pty` and streams live PTY output over a non-blocking channel | ✓ VERIFIED | `src/pty/mod.rs` (`PtySession::spawn`) + `tests/pty_lifecycle.rs` passes |
| 2 | `PtySession::resize` only emits `SIGWINCH` when `(rows, cols)` actually changes, tracking `sigwinch_count` deterministically | ✓ VERIFIED | `src/pty/mod.rs` (`resize`, `sigwinch_count`) + `tests/composite_pane_snapshots.rs#moving_and_hiding_prompt_on_live_pty_pane_emits_zero_sigwinch` |
| 3 | `VtParser` processes VT100/ANSI/TrueColor sequences, tracks scrollback history, and detects `DECSET 47`, `1047`, and `1049` alternate screen transitions | ✓ VERIFIED | `src/vt/mod.rs` (`tracks_decset_1049_1047_and_47_alternate_screen`, `parses_truecolor_and_256_color_cells`, `supports_scrollback_navigation`) |
| 4 | `TerminalRestoreGuard` cleanly restores raw mode, mouse capture, and alternate screen state on drop or panic | ✓ VERIFIED | `src/error.rs` (`TerminalRestoreGuard::restore`, `install_panic_hook`, `Drop`) |
| 5 | Three-signal line-discipline detector (`OSC 133` + `tcgetpgrp` + `termios !ECHO/!ICANON`) automatically switches `InputRoutingMode` to `RawPtyPassthrough` during `sudo`/`ssh` password prompts and interactive sub-REPLs | ✓ VERIFIED | `src/pty/line_discipline.rs` (`inspect_pty_fd`, `requires_raw_passthrough`) + unit & Unix PTY termios tests |
| 6 | Password keystrokes during `!ECHO` mode are sent directly to the PTY and never echoed or stored in `FloatingPromptState` | ✓ VERIFIED | `src/input/mod.rs#password_keystrokes_in_no_echo_mode_never_leak_into_floating_prompt` + `tests/composite_pane_snapshots.rs#snapshot_password_no_echo_lock_hides_prompt_and_protects_input` |
| 7 | `CompositePane` sizes its PTY `OutputCanvas` to 100% of pane height minus 1 status row and emits zero `SIGWINCH` when `FloatingCommandPrompt` moves or hides | ✓ VERIFIED | `src/pane/mod.rs` (`canvas_rect`, `resize_viewport`) + `tests/composite_pane_snapshots.rs` |
| 8 | `DECSET 47/1047/1049` alternate screen automatically hides `FloatingCommandPrompt` and restores it on exit, with `Ctrl+G` / `F12` manual toggle override | ✓ VERIFIED | `src/pane/mod.rs#alternate_screen_and_password_lock_auto_hide_floating_prompt` + `tests/composite_pane_snapshots.rs#snapshot_bottom_pinned_and_alternate_screen_auto_hide` |
| 9 | `CompositePaneWidget` renders `OutputCanvas` across 100% of canvas height, clears the `FloatingCommandPrompt` rect with `ratatui::widgets::Clear`, renders the bordered floating prompt on top, and renders the 1-row OIA status bar at the bottom | ✓ VERIFIED | `src/ui/mod.rs` (`CompositePaneWidget::render`) + `insta` snapshots |
| 10 | When `DECSET 1049` alternate screen or `!ECHO` password mode is active, `FloatingCommandPrompt` is hidden from the rendered frame and the OIA status bar displays `RAW PTY` / `ALT SCR` or `!ECHO LOCK` | ✓ VERIFIED | `tests/snapshots/composite_pane_snapshots__alternate_screen_auto_hide_pane.snap` and `composite_pane_snapshots__password_no_echo_lock_pane.snap` |
| 11 | Bilterm binary (`src/main.rs`) and `App` event loop multiplex PTY output events and Crossterm input events cleanly with `TerminalRestoreGuard` protection | ✓ VERIFIED | `src/app.rs` (`App::run`) and `src/main.rs` |

**Score:** 11/11 truths verified (0 present, behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` | Workspace dependencies | ✓ EXISTS + SUBSTANTIVE | `ratatui`, `crossterm`, `portable-pty`, `vt100-ctt`, `tokio`, `libc`, `insta` |
| `src/error.rs` | `BiltermError` & `TerminalRestoreGuard` | ✓ EXISTS + SUBSTANTIVE | 118 lines, panic hook & RAII drop guard |
| `src/pty/mod.rs` | `PtySession` & deduplicated resize | ✓ EXISTS + SUBSTANTIVE | 220 lines, `sigwinch_count` & reader thread |
| `src/pty/line_discipline.rs` | 3-signal line-discipline inspector | ✓ EXISTS + SUBSTANTIVE | 215 lines, `OSC 133` + `tcgetpgrp` + `tcgetattr` |
| `src/vt/mod.rs` | VT100/ANSI/TrueColor + `OSC 133` engine | ✓ EXISTS + SUBSTANTIVE | 280 lines, `DECSET 47/1047/1049` & `OSC 133` |
| `src/pane/mod.rs` | `CompositePane` & zero-`SIGWINCH` geometry | ✓ EXISTS + SUBSTANTIVE | 310 lines, `PromptAnchor`, `effective_routing_mode` |
| `src/input/mod.rs` | Modal keyboard router & VT100 key encoder | ✓ EXISTS + SUBSTANTIVE | 255 lines, password-safe routing |
| `src/ui/mod.rs` | 3-pass Ratatui `CompositePaneWidget` | ✓ EXISTS + SUBSTANTIVE | 235 lines, `OutputCanvas` + `Clear` + `FloatingCommandPrompt` + `OiaStatusBar` |
| `src/app.rs` | Async `tokio::select!` event loop | ✓ EXISTS + SUBSTANTIVE | 96 lines |
| `src/main.rs` | CLI entrypoint | ✓ EXISTS + SUBSTANTIVE | 35 lines |
| `tests/composite_pane_snapshots.rs` | `TestBackend` + `insta` snapshot tests | ✓ EXISTS + SUBSTANTIVE | 118 lines + 4 `.snap` files |

**Artifacts:** 11/11 verified

## Requirements Coverage

| Requirement | Status | Blocking Issue |
|-------------|--------|----------------|
| `PTY-01`: Cross-Platform Interactive PTY Lifecycle | ✓ SATISFIED | - |
| `PTY-02`: VT100/ANSI Emulation & Alternate Screen Auto-Hide | ✓ SATISFIED | - |
| `PTY-03`: Line-Discipline & Password/REPL Safe Input Routing | ✓ SATISFIED | - |
| `PANE-01`: Composite Window Pane (`OutputCanvas` + Overlaid `FloatingCommandPrompt`) | ✓ SATISFIED | - |

**Coverage:** 4/4 requirements satisfied

## Anti-Patterns Found

None (0 blockers, 0 warnings; `cargo clippy --all-targets -- -D warnings` exits 0).

## Human Verification Required

None — all verifiable items checked programmatically via headless `portable-pty` shells and `ratatui::backend::TestBackend` + `insta` snapshots.

## Gaps Summary

**No gaps found.** Phase 1 goal achieved. Ready to proceed to Phase 2.
