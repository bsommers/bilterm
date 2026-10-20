# Bilterm

## What This Is

Bilterm is a terminal user interface (TUI) shell wrapper and terminal emulator written in Rust using Ratatui. Inspired by classic IBM mainframe terminals (such as the IBM 3270 where the command prompt sits at the top of the screen), Bilterm provides a movable, resizable floating command prompt window that can be pinned to the top or bottom of the viewport or floated freely at any vertical offset while full VT100/ANSI terminal output scrolls seamlessly underneath it.

## Core Value

A responsive, movable, and resizable floating command prompt pane over a live full-PTY terminal canvas where output scrolls directly underneath the command window without losing compatibility with interactive shells and full-screen TUI applications.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Full PTY shell wrapper supporting standard shells (`bash`, `zsh`, `fish`, PowerShell/cmd on Windows) with full VT100/ANSI escape sequence parsing and color rendering
- [ ] Floating command prompt pane that defaults to pinned-top (IBM mainframe style), can be pinned to the bottom, or floated at any vertical offset
- [ ] Resizable command input pane supporting multi-line command editing, syntax highlighting, and command history search
- [ ] Transparent/occluding z-order rendering where terminal scrollback and live output span the full terminal height and scroll directly underneath the floating prompt window
- [ ] Triple-modality positioning and resizing controls: keyboard shortcuts (move up/down, snap top/bottom, resize height), mouse drag-and-drop (border/titlebar move & resize), and in-prompt commands / configuration file settings
- [ ] Automatic Alternate Screen (DECSET 1049) detection that auto-hides the floating command pane when full-screen TUI apps (`vim`, `htop`, `less`) run, restoring the floating prompt on exit, plus a manual toggle hotkey
- [ ] Retro IBM 3270 / mainframe aesthetic options including Operator Information Area (OIA) status bar, classic phosphor green / amber / monochrome themes, and configurable cursor/caret styles (cross-hair cursor spanning rows/columns, solid block, hollow/blink block, underscore, vertical bar)
- [ ] Command block separation and navigation allowing users to jump between previous command invocations and their outputs in scrollback
- [ ] Cross-platform support targeting Linux first, followed by macOS, and Windows (ConPTY)

### Out of Scope

- Custom GPU-accelerated windowing / font rasterizer (e.g., WGPU/OpenGL GUI window) — v1 runs inside standard host terminals via Ratatui + Crossterm so it works everywhere over SSH or local terminals immediately
- Built-in web browser or non-terminal rich media rendering — keep focus on crisp TUI mainframe ergonomics and fast PTY execution

## Context

- **Inspiration**: Classic IBM mainframe terminals (3270 / TSO / ISPF / VM/CMS) where the command entry area is decoupled from scrolling output and commonly positioned at the top of the screen, combined with modern floating window ergonomics.
- **Technical Ecosystem**: Rust + `ratatui` + `crossterm` for TUI rendering and input handling; `portable-pty` (or `nix`/`pty-process`) for cross-platform PTY management; `vt100` or `alacritty_terminal` (`vte`) for virtual terminal state machine and screen buffer emulation; `tui-textarea` or `reedline`/custom editor widget for multi-line command editing.
- **Platform Priority**: Linux is the primary development and verification target (Phase 1+), followed by macOS (POSIX PTY parity), and Windows (ConPTY compatibility).

## Constraints

- **Tech Stack**: Rust + Ratatui — Required by project vision for fast, zero-cost TUI rendering and cross-platform terminal manipulation.
- **Platform Priority**: Linux first, macOS second, Windows third — Ensures rapid iteration on POSIX PTY semantics while keeping cross-platform abstractions (`portable-pty`, `crossterm`) clean from day one.
- **Compatibility**: Must cleanly handle interactive TUI programs (`vim`, `htop`, `ssh`) via Alternate Screen detection and raw input pass-through when the floating command editor is bypassed or auto-hidden.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Full PTY wrapper + VT100 parser (`portable-pty` + `vt100`/`vte`) | Needed to render underlying shell output inside a Ratatui background layer while compositing the floating command widget on top | — Pending |
| Full-height background output canvas with occluding/translucent floating overlay | Matches the user's mental model where output scrolls directly underneath the floating command prompt window | — Pending |
| Auto-hide floating prompt on Alternate Screen (DECSET 1049) + manual toggle | Allows seamless use of `vim`, `htop`, and `less` without the floating command bar intercepting keystrokes meant for full-screen apps | — Pending |
| Support cross-hair cursor + multiple caret styles (block, blink hollow block, underscore) | Authentic IBM 3270 / mainframe terminal feel alongside customizable retro aesthetics | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-10-19 after initialization*
