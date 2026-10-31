---
gsd_state_version: 1.0
milestone: v2.5.0
milestone_name: 2D Rectangular Block Select/Highlight/Copy & Full Per-Pane Session Shell, Running Command & CWD Persistence
current_phase: 41
current_phase_name: rectangular-select-and-per-pane-session-process-cwd-persistence
status: milestone_v2_5_0_complete
stopped_at: Released v2.5.0 (2D Rectangular Select/Highlight/Copy + Full Per-Pane Session Shell, Running Software/Command & CWD Persistence across every session)
last_updated: "2026-10-31"
last_activity: 2026-10-31
last_activity_desc: Released v2.5.0 with 2D rectangular block selection (:rect, :select, :copy rect, Alt+R, Alt+LeftDrag) and full per-pane session shell, running command, and CWD persistence
progress:
  total_phases: 41
  completed_phases: 41
  total_plans: 89
  completed_plans: 89
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-31)

**Core value:** A responsive composite terminal pane (movable/resizable floating command prompt over a full-PTY output canvas that scrolls underneath) that can be split horizontally/vertically, stacked/shuffled in z-order via keyboard or mouse, styled with classic IBM 3270 mainframe & DOS Borland Turbo Vision aesthetics, and integrated seamlessly with `tmux` and the persistent `Bilterm` (`Herdr` + TRON `MCP`) multiplexer daemon.
**Current focus:** Milestone `v2.5.0` (`Phases 39–41`) — **COMPLETE (`41/41` Phases)**.

## Current Position

Phase: 41 (rectangular-select-and-per-pane-session-process-cwd-persistence) — COMPLETE
Plan: All `v2.4.x`–`v2.5.0` plans (`39-01..02`, `40-01..03`, `41-01..03`) complete
Status: Phases 1–41 complete (`41/41` Phases across `v1.0`–`v2.5.0`).
Last activity: 2026-10-31 — Completed Phase 41 (`v2.5.0`): 2D rectangular block select, highlight & copy (`:rect`, `:select`, `:copy rect`, `Alt+R`, `Alt+LeftDrag`, `:rect mode`, `:rect <r1> <c1> <r2> <c2>`) and full per-pane session persistence of layout, shell (`shell`, `shell_args`), running software/commands (`running_command`), and working directory (`cwd`) with auto-relaunch (`session_restore_processes`).

Progress: [██████████] 100% (41/41 Phases)

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

Last session: 2026-10-28 20:01
Stopped at: Released v2.1.0 (Phases 35-36), pulled macOS/Claude Code/agy hardening (7817cd2), synced docs, ready for items 3 & 4 after restart
Resume file: .planning/CHECKPOINT.md

## Quick Tasks Completed

| Slug | Date | Status | Summary |
|------|------|--------|---------|
| `20261020-tab-completion-and-smooth-floating-prompt` | 2026-10-20 | complete ✓ | Tab completion (`Tab`/`Shift+Tab`) + smooth floating prompt over `tail -f /dev/random` |
| `20261020-split-prompt-borders-shadows-vt-torture-coverage` | 2026-10-20 | complete ✓ | Non-overlapping split command/output panes (`SplitTop`/`SplitBottom`), 8 outline styles, 4 themeable DOS drop shadows, full VT control sequence compliance, and `cargo llvm-cov` (`81.83%` lines) + torture/stress suite |
| `20261020-prompt-width-scroll-running-cmd-history-help-docs` | 2026-10-20 | complete ✓ | Resizable/scrollable prompt, running command banner, rich OIA status bar (`R:C` + `OUT:`), history popup & session file persistence/temp export, full-screen cross-hairs, `<F1>` help modal, and updated user manual |
| `20261022-macos-keybindings-leader-and-option-normalization` | 2026-10-22 | complete ✓ | Universal `Ctrl+X` Leader prefix key, macOS `Option` Unicode symbol normalization (`˙`, `√`, `∫`, `ƒ`, etc.), Readline word navigation (`Alt+B`/`Alt+F`/`Alt+D`/`Alt+Backspace`), `Ctrl+\` Quake toggle, and `F1..F10`/`Alt+<key>` passthrough in `RawPtyPassthrough`/`ALT_SCREEN` |
