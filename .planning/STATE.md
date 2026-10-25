---
gsd_state_version: 1.0
milestone: v1.10.1
milestone_name: milestone
current_phase: 28
current_phase_name: architecture-hardening-and-dag-decoupling
status: released_v1_10_1_phase_28_planned
stopped_at: Released v1.10.1 (P0/P1 review remediations, unified automation confinement, dependency audit) and planned Phase 28 (28-01-PLAN.md)
last_updated: "2026-10-25"
last_activity: 2026-10-25
last_activity_desc: Released v1.10.1 with P0/P1 architecture & security remediations and dependency audit; planned Phase 28
progress:
  total_phases: 28
  completed_phases: 27
  total_plans: 52
  completed_plans: 51
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-25)

**Core value:** A responsive composite terminal pane (movable/resizable floating command prompt over a full-PTY output canvas that scrolls underneath) that can be split horizontally/vertically, stacked/shuffled in z-order via keyboard or mouse, styled with classic IBM 3270 mainframe & DOS Borland Turbo Vision aesthetics, and integrated seamlessly with `tmux`.
**Current focus:** `v1.10.1` released; Phase 28 (`28-01-PLAN.md` — P2 Module DAG Decoupling & Dirty-Driven Rendering) planned and ready.

## Current Position

Phase: 28 (architecture-hardening-and-dag-decoupling) — P0/P1 COMPLETE (`v1.10.1`), P2 PLANNED (`28-01-PLAN.md`)
Plan: `28-01-PLAN.md` ready for execution
Status: Phases 1–27 complete (`47/47` requirements shipped through `v1.10.1`); Phase 28 P0/P1 shipped in `v1.10.1`
Last activity: 2026-10-25 — Released `v1.10.1` (`677b668`) with unified `SnapshotPolicy::Confined` (`WindowManager::apply_automation`), live `CanvasResolution` + cached `TIOCGWINSZ`, `ui <-> api` cycle removal, `ARCH-R04` module splits, and `vt100-ctt` dependency pruning.

Progress: [██████████] 100%

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

Last session: 2026-10-25 16:12
Stopped at: Released and pushed v1.10.1 (677b668) with P0/P1 architecture & security remediations; Phase 28 (28-01-PLAN.md) ready
Resume file: .planning/CHECKPOINT.md

## Quick Tasks Completed

| Slug | Date | Status | Summary |
|------|------|--------|---------|
| `20261020-tab-completion-and-smooth-floating-prompt` | 2026-10-20 | complete ✓ | Tab completion (`Tab`/`Shift+Tab`) + smooth floating prompt over `tail -f /dev/random` |
| `20261020-split-prompt-borders-shadows-vt-torture-coverage` | 2026-10-20 | complete ✓ | Non-overlapping split command/output panes (`SplitTop`/`SplitBottom`), 8 outline styles, 4 themeable DOS drop shadows, full VT control sequence compliance, and `cargo llvm-cov` (`81.83%` lines) + torture/stress suite |
| `20261020-prompt-width-scroll-running-cmd-history-help-docs` | 2026-10-20 | complete ✓ | Resizable/scrollable prompt, running command banner, rich OIA status bar (`R:C` + `OUT:`), history popup & session file persistence/temp export, full-screen cross-hairs, `<F1>` help modal, and updated user manual |
| `20261022-macos-keybindings-leader-and-option-normalization` | 2026-10-22 | complete ✓ | Universal `Ctrl+X` Leader prefix key, macOS `Option` Unicode symbol normalization (`˙`, `√`, `∫`, `ƒ`, etc.), Readline word navigation (`Alt+B`/`Alt+F`/`Alt+D`/`Alt+Backspace`), `Ctrl+\` Quake toggle, and `F1..F10`/`Alt+<key>` passthrough in `RawPtyPassthrough`/`ALT_SCREEN` |
