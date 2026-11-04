---
phase: 50-agent-aware-workspace-navigator
plan: 16
subsystem: input
tags: [agents-window, pin, panel-scale, gui-renderer, colon-command]
status: complete

requires:
  - phase: 50-09
    provides: "handle_agents_window_mouse, agents_pane_rects, title badges, wm. allow-list gate"
  - phase: 50-08
    provides: "AgentsOp::Zoom / Dismiss, AGENTS_WINDOW_BINDINGS"
provides:
  - "AgentsOp::Pin(Option<bool>) with `*`, pin badge click, :agents pin|unpin|pin toggle"
  - "AgentsOp::ScaleStep(i16) with `[` / `]`, Ctrl+wheel, :agents scale +n|-n"
  - "PanelTarget::Agents + agents_scale_pct; :agents scale <75..200> -> PanelScaleOf; :panel scale sets all three panels"
  - "ui::agents_ui::agents_pin_badge_x / pin_badge_glyph (renderer and hit-test share one placement)"
  - "GUI renderer agents scale block (draw_panel at scale_pct(PanelTarget::Agents))"
affects: [50-10, 50-11, 50-13, 50-14]

actuals:
  tokens: 40000
  tasks: 2
  commits: 2
plan_head_before: 2a8b8ab88c9a86f9a4fa1972d35aad520baa3b0b
plan_head_after: b441fdc808f9ec0d89aadb288a3f49ea71a5dcec

tech-stack:
  added: []
  patterns:
    - "Mouse handler remaps into the panel's virtual cell space with wm.panel_space before hit-testing"
    - "Absolute scale reuses the shared PanelScaleOf Action; relative step is its own AgentsOp"

key-files:
  created: []
  modified:
    - src/ui/typography.rs
    - src/wm/mod.rs
    - src/wm/agents_cmd.rs
    - src/ui/renderer.rs
    - src/ui/agents_ui.rs
    - src/input/agents_window.rs
    - src/input/keymap.rs
    - src/input/mouse_wm.rs
    - src/colon_command/parse_agents.rs
    - src/colon_command/execute_agents.rs
    - src/colon_command/execute_wm.rs
    - src/pane/completion.rs
    - src/ui/help_catalog.rs
    - tests/phase50_agents_window.rs
    - tests/snapshots/phase50_agents_window__render_tui_graph_snapshot_static.snap

key-decisions:
  - "Pin badge sits two cells left of the title badges; width of the three title badges is 9 in every glyph pack, so agents_pin_badge_x(outer) needs no glyph argument"
  - "ASCII pack detected via glyphs.pin_top_bottom.is_ascii() (no new GlyphSet field)"
  - "ctrl_wheel_scale_delta made pub(super) in mouse_wm.rs (needed by the agents mouse handler)"

requirements-completed: []
---

# Phase 50 Plan 16: Agents window pin and independent panel scale Summary

The AGENT TRAFFIC window now pins from a `◆`/`◇` title badge, `*` or `:agents pin|unpin|pin toggle`, and scales independently (`PanelTarget::Agents`) from `[` / `]`, Ctrl+wheel and `:agents scale`, every gesture lowering to an `Action` through `WindowManager::apply`.

## Tasks

| Task | Name | Commit |
|------|------|--------|
| 1 | Pin badge click, `*` and `:agents pin` twins (tracer) | 6e8d6c1 |
| 2 | Independent `PanelTarget::Agents` scale, GUI renderer block, parity test | b441fdc |

## Verification

- `scripts/verify.sh`: PASS 56/56; targeted run (clippy, unit, phase50_agents_window, panel_pin_scale, phase46_display_modes, linux_session_regression_and_gui_scaling, input_mouse_wm_coverage_panels, drawer_classic_parity, api_conformance): PASS 56/56.
- `scripts/check_classic_goldens.sh`: GOLDENS OK 11.
- `wm.` allow-list gate on `src/input/agents_window.rs`: only apply, agents_window, agents_window_rect, agents_pane_rects, agents_hit, panel_space, set_agents_drag, config.
- `src/ui/renderer.rs` is 862 lines. No Cargo changes.
- phase50_agents_window: 44 tests pass (4 `window_tracer_*`, 9 new from this plan incl. `parity_pin_scale_key_mouse_colon_match`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated 50-07 snapshot `render_tui_graph_snapshot_static`**
- **Found during:** Task 1
- **Issue:** The new pin badge changes the agents window title row, so the 50-07 TUI snapshot (agents window only, not a classic golden) no longer matched.
- **Fix:** Accepted the snapshot with the `◇` badge two cells left of `[_][□][X]`.
- **Commit:** 6e8d6c1

**2. [Scope] `ctrl_wheel_scale_delta` visibility** raised to `pub(super)` in `src/input/mouse_wm.rs` (file not in the plan's list) so the agents mouse handler reuses the drawer's Ctrl+wheel rule; `src/colon_command/execute_wm.rs` `panel_name` arm and `src/input/mouse_wm.rs` are the only extra touches.

None otherwise - plan executed as written.

## Known Stubs

None.

## Threat Flags

None. T-50-36 mitigated: absolute values parse only inside 75..=200, `step_scale` clamps (tests `window_colon_scale_absolute_and_relative`, `window_bracket_keys_step_scale`).

## Self-Check: PASSED

Commits 6e8d6c1 and b441fdc exist on gsd/phase-50-navigator; all modified files present.
