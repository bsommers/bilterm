---
gsd_state_version: 1.0
milestone: v2.6.0
milestone_name: UI Modernization (Phases 43-48): design system, display modes, new looks; Legacy looks unchanged
current_phase: 46
current_phase_name: shell-display-modes
status: in_progress
stopped_at: 46-06 done (Zen/Minimal modes); 46-06b resolved (no code); next 46-07 navigator
last_updated: "2026-11-02"
last_activity: 2026-11-02
last_activity_desc: Phase 46 steps 46-01..06 committed (font default, :appearance, fresh-install Obsidian, :mode, ShellLayout, Zen/Minimal); GSD plans backfilled for 43-45
progress:
  total_phases: 48
  completed_phases: 44
  total_plans: 101
  completed_plans: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-31)

**Core value:** A responsive composite terminal pane (movable/resizable floating command prompt over a full-PTY output canvas that scrolls underneath) that can be split horizontally/vertically, stacked/shuffled in z-order via keyboard or mouse, styled with classic IBM 3270 mainframe & DOS Borland Turbo Vision or Non-Retro Modern Minimal / Liquid Glass aesthetics, and integrated seamlessly with `tmux` and the persistent `Bilterm` (`Herdr` + TRON `MCP`) multiplexer daemon.
**Current focus:** Milestone `v2.6.0` UI Modernization — Phase 46 in progress (43, 44 done; 45 done except `:appearance` profile management, see `phases/45-design-system/GAPS.md`). Standing rule: add new looks, take NOTHING away from Legacy.

## Current Position

Phase: 42 (modern-liquid-glass-per-window-theme-cursor-and-ssh-hardening) — COMPLETE
Plan: All `v2.4.x`–`v2.5.2` plans (`39-01..02`, `40-01..03`, `41-01..03`, `42-01..03`) complete
Status: Phases 1–42 complete (`42/42` Phases across `v1.0`–`v2.5.2`).
Last activity: 2026-10-31 — Completed Phase 42 (`v2.5.2`): Non-Retro Modern Minimal (`--modern`, `:modern`) and Liquid Glass (`--liquid-glass`, `:liquid`) UI modes, per-window vs. global `:theme` and `:cursor` (`:cursor cursor-color`), and `:ssh` parsing, `<Tab>` completion (`~/.ssh/config` hosts), `<F1>` help, and SSH login `PROMPT_COMMAND` echo fix.

Progress: [██████████] 100% (42/42 Phases)

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
