# Phase 1: Core PTY, VT100 Engine & Single CompositePane Prototype - Context

**Gathered:** 2026-10-19
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped via workflow.skip_discuss)

<domain>
## Phase Boundary

As a terminal user, I want to launch Bilterm into an interactive shell inside a full-height PTY output canvas with an overlaid floating command prompt, automatic `DECSET 47/1047/1049` alternate-screen prompt hiding, and safe `sudo`/REPL input routing, so that terminal output scrolls smoothly underneath the prompt without `SIGWINCH` geometry thrashing or password/TUI corruption.

**Requirements covered:** `PTY-01`, `PTY-02`, `PTY-03`, `PANE-01`

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — discuss phase was skipped per user setting. Use ROADMAP phase goal, success criteria, and `.planning/research/` architecture/stack conventions to guide decisions:
- **VT100 / ANSI Engine**: Use `vt100-ctt` (`0.17.1`) / `vte` wrapped behind a clean `TerminalBackend` / `VtEngine` trait and parser so `DECSET 47/1047/1049` alternate screen mode, `OSC 133` semantic prompt markers (`A`, `B`, `C`, `D`), cursor position, TrueColor/256-color/16-color attributes, and scrollback ring buffer are tracked deterministically.
- **PTY Management (`portable-pty 0.9.0` + `tokio`)**: Spawn the user's shell (`$SHELL` or `/bin/sh` on Unix, `powershell.exe`/`cmd.exe` on Windows) inside a `portable-pty` pseudoterminal sized to 100% of the `OutputCanvas` dimensions (`pane_height - 1` status bar row, `pane_width`). Use a dedicated `tokio::task::spawn_blocking` reader loop feeding a bounded `tokio::sync::mpsc::channel` to prevent blocking the async TUI event loop.
- **Zero-`SIGWINCH` Floating Overlay (`PANE-01`)**: Render the full-height `OutputCanvas` first into the `ratatui::buffer::Buffer`, then clear the `FloatingCommandPrompt` rectangle using `ratatui::widgets::Clear` and render the bordered floating prompt on top, followed by the bottom 1-row OIA status bar. Moving or hiding the floating prompt never sends `SIGWINCH` (`MasterPty::resize`) to the child PTY.
- **Three-Signal Line-Discipline & Password/REPL Detection (`PTY-03`)**: Combine:
  1. `OSC 133` semantic shell state (`A`/`B` PromptReady vs `C` CommandExecuting vs `D` CommandFinished),
  2. Foreground process group inspection (`tcgetpgrp` on POSIX master/slave FD vs shell PID),
  3. Termios `ECHO` / `ICANON` inspection (`tcgetattr` checking `!ECHO` for `sudo`/`ssh` password prompts and `!ICANON` for raw sub-REPLs).
  When `ALT_SCREEN` is active, or `fg_pgid != shell_pgid`, or `!ECHO` is detected, or manual raw toggle (`Ctrl+G` / `F12`) is engaged, automatically switch `InputRoutingMode` to `RawPtyPassthrough` (hiding or dimming the floating prompt and never echoing password keystrokes in cleartext).
- **Panic & Signal Restoration**: Install `color-eyre` panic hooks and signal cleanup guards so `DisableMouseCapture`, `LeaveAlternateScreen`, and `disable_raw_mode()` always execute on exit or panic.

</decisions>

<code_context>
## Existing Code Insights

Greenfield Rust repository (`Cargo.toml` + `src/` to be created in Plan 01-01). Comprehensive stack and architecture blueprints exist in `.planning/research/STACK.md`, `.planning/research/ARCHITECTURE.md`, and `.planning/research/PITFALLS.md`.

</code_context>

<specifics>
## Specific Ideas

Refer to `.planning/ROADMAP.md` Phase 1 success criteria and `.planning/REQUIREMENTS.md` (`PTY-01`, `PTY-02`, `PTY-03`, `PANE-01`).

</specifics>

<deferred>
## Deferred Ideas

Multi-line `tree-sitter-bash` highlighting, `nucleo-matcher` fuzzy history, triple-modality prompt drag/pin/resize, H/V splits, z-order stacking, Guake quick terminal, Borland Turbo Vision chrome/config, and `tmux` integration are scheduled for Phases 2–5.

</deferred>
