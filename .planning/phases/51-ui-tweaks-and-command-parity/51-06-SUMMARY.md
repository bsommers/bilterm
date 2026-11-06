---
phase: 51-ui-tweaks-and-command-parity
plan: 06
subsystem: ui
tags: [render, agents-list, gui, scaled-panel, UIT-03]
requires: ["51-02"]
provides:
  - "render_agents_list: full TUI window drawn from agents_list_layout"
  - "GUI scaled-panel block, occluder masks and texture overlap for the list"
affects: [51-07, 51-12]
key-files:
  created:
    - tests/phase51_agents_list_render.rs
  modified:
    - src/ui/agents_list_ui.rs
    - src/ui/renderer.rs
    - src/ui/agents_gui.rs
decisions:
  - "selected row is reverse video; its state dot keeps the state colour on the bar colour (Navigator convention)"
  - "scroll is drawn exactly as stored (not clamped) so drawn rows equal the rows agents_list_hit maps"
  - "an unscaled list is re-pushed after the scaled traffic window in the GUI so it stays above it"
status: complete
plan_head_before: 0d52cae2f4fc6ee98d42a2ff788b91c9621eed06
plan_head_after: 00742129603a2710cf314282efc4f69851ade8cd
commits: 1
actuals:
  tokens: 22000
  tasks: 2
  commits: 1
---

# Phase 51 Plan 06: AGENTS list render Summary

The AGENTS list window renders in the TUI and in the GUI at any panel scale: Borland frame with pin badge and min/dock/close trio, `STATE NAME PANE SINCE` header, one coloured-dot row per pane, selection and active-pane marker, scroll indicator, empty state and footer chips at exactly the geometry rects.

Commit: `0074212` (both tasks together; the GUI hooks are small and the tests are one file).

## Deltas from plan text (as instructed by 51-01/51-02 summaries)

- Uses `agents_list_layout`, `badges`, `chips` and `LIST_CHIP_LABELS`; `render_agents_list` no longer calls `agents_list_rect`.
- Name sanitising is double: rows already come sanitised from `agents_list_rows`, and the renderer runs `sanitize_display` again before drawing.
- Name column degrade: `SINCE` dropped, then `PANE`; `[docked]` title hint only when the title has room before the leftmost badge.

## Gates

- `cargo test -q --test phase51_agents_list_render`: 13 passed (8 TUI + 5 GUI).
- `cargo test -q --test phase51_tracer --test phase50_agents_window --test drawer_classic_parity --test phase50_gui_render --test phase50_gui_runtime --test linux_session_regression_and_gui_scaling --test phase51_seams`: all ok (10, 11, 70, 15, 7, 10, 6 tests).
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
- Cargo.toml, Cargo.lock, src/ui/mod.rs untouched; no `.snap` files; clippy clean on touched files.
- Lines: agents_list_ui.rs 348, renderer.rs 926 (+25).

## Deviations from Plan

None in behavior. Tasks were committed as one commit instead of two (Task 2 hooks depend on the Task 1 renderer; one cohesive diff).

## Known Stubs

None.

## Threat Flags

None. T-51-15 (names sanitised, `render_sanitises_names`) and T-51-16 (only `visible_rows` rows formatted, saturating math, clipped writes, `render_tiny_workspaces_do_not_panic`) mitigated.

## Self-Check: PASSED

Created and modified files exist; commit `0074212` is an ancestor of HEAD.
