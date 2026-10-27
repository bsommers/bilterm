---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Herdr Daemon, Session Trees, Mainframe Paging, Open Color Themes & TRON MCP Root Terminal
current_phase: 29
current_phase_name: smooth-and-block-page-mode-scrolling
status: planning_complete_ready_for_phase_29
stopped_at: Completed v1.12.0 features (mouse/keyboard cut-copy-paste, Powerline/PUA font rendering, bil;*term icons, persistent scrollable Herdr Drawer) and scoped Milestone v2.0 (Phases 29-34)
last_updated: "2026-10-27"
last_activity: 2026-10-27
last_activity_desc: Scoped Milestone v2.0 (Phases 29-34: Smooth/Block-Page Scrolling, Exit Guard, iTerm2/Open Color & Font Engine, Herdr Daemon, Session Trees & Crash-Safe Journaling, TRON MCP Root Terminal)
progress:
  total_phases: 34
  completed_phases: 28
  total_plans: 64
  completed_plans: 52
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-27)

**Core value:** A responsive composite terminal pane (movable/resizable floating command prompt over a full-PTY output canvas that scrolls underneath) that can be split horizontally/vertically, stacked/shuffled in z-order via keyboard or mouse, styled with classic IBM 3270 mainframe & DOS Borland Turbo Vision aesthetics, and integrated seamlessly with `tmux` and the persistent `Bilterm` (`Herdr` + TRON `MCP`) multiplexer daemon.
**Current focus:** Milestone `v2.0` (`Phases 29–34`) — Starting with **Phase 29: Smooth Scrolling & Mainframe/Timeshare Block/Page-Mode Scroller (`SCRL-01`, `SCRL-02`)**.

## Current Position

Phase: 29 (smooth-and-block-page-mode-scrolling) — READY TO PLAN / EXECUTE
Plan: `29-01-PLAN.md` & `29-02-PLAN.md` scoped in `ROADMAP.md`
Status: Phases 1–28 complete (`47/47` v1 requirements complete); Phases 29–34 (`14` v2.0 requirements: `SCRL-01..02`, `SAFE-01..02`, `THM-01..02`, `DAEM-01..02`, `TREE-01..02`, `PERS-01..02`, `MCP-01..02`) scoped and dependency-sorted.
Last activity: 2026-10-27 — Completed mouse & keyboard cut/copy/paste with cyan visual selection highlights, pixel-perfect Powerline (`U+E0A0..=U+E0D7`) & Nerd Font PUA geometric rendering, cross-platform `"bil;*term"` icons (`icns`/`ico`/`png`/`svg`), persistent in-place scrollable `Herdr` Drawer, and Milestone `v2.0` (`Phases 29–34`) roadmap & requirements.

Progress: [████████░░] 82% (28/34 Phases)

## Performance Metrics

**Velocity:**

- Total plans completed: 15
- Average duration: 0 min
- Total execution time: 0.0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 3 | - | - |
| 2 | 3 | - | - |
| 3 | 3 | - | - |
| 4 | 3 | - | - |
| 5 | 3 | - | - |

**Recent Trend:**

- Last 5 plans: none
- Trend: Stable

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Init]: Composite Window Pane (`OutputCanvas` + `FloatingPrompt` + `OiaStatusBar`) is the atomic layout unit across all H/V splits and z-order stacks.
- [Init]: Child PTY dimensions stay fixed to full `OutputCanvas` rect so moving/resizing/hiding `FloatingPrompt` never triggers `SIGWINCH` reflow storms.
- [Init]: Three-signal input routing (`OSC 133` + `tcgetpgrp` + `termios !ECHO` + `DECSET 1049`) prevents `sudo` password echo leaks and interactive REPL lockouts.
- [Init]: Custom `occlude_rect` buffer pass sanitizes split 2-column wide graphemes at floating prompt, stacked pane, Guake drop-down overlay, and Turbo Vision drop-shadow boundaries.

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| *(none)* | | | |

## Session Continuity

Last session: 2026-10-27 09:40
Stopped at: Installed v1.11.1 on macOS (`~/.local/bin/bilterm` + `~/Applications/Bilterm.app`) and verified 248/248 tests passing
Resume file: .planning/CHECKPOINT.md

## Quick Tasks Completed

| Slug | Date | Status | Summary |
|------|------|--------|---------|
| `20261020-tab-completion-and-smooth-floating-prompt` | 2026-10-20 | complete ✓ | Tab completion (`Tab`/`Shift+Tab`) + smooth floating prompt over `tail -f /dev/random` |
| `20261020-split-prompt-borders-shadows-vt-torture-coverage` | 2026-10-20 | complete ✓ | Non-overlapping split command/output panes (`SplitTop`/`SplitBottom`), 8 outline styles, 4 themeable DOS drop shadows, full VT control sequence compliance, and `cargo llvm-cov` (`81.83%` lines) + torture/stress suite |
| `20261020-prompt-width-scroll-running-cmd-history-help-docs` | 2026-10-20 | complete ✓ | Resizable/scrollable prompt, running command banner, rich OIA status bar (`R:C` + `OUT:`), history popup & session file persistence/temp export, full-screen cross-hairs, `<F1>` help modal, and updated user manual |
| `20261022-macos-keybindings-leader-and-option-normalization` | 2026-10-22 | complete ✓ | Universal `Ctrl+X` Leader prefix key, macOS `Option` Unicode symbol normalization (`˙`, `√`, `∫`, `ƒ`, etc.), Readline word navigation (`Alt+B`/`Alt+F`/`Alt+D`/`Alt+Backspace`), `Ctrl+\` Quake toggle, and `F1..F10`/`Alt+<key>` passthrough in `RawPtyPassthrough`/`ALT_SCREEN` |
