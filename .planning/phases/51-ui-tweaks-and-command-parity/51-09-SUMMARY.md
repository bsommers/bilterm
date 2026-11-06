---
phase: 51-ui-tweaks-and-command-parity
plan: 09
subsystem: wm
tags: [tiling, panel-dock, agents-list, agents-traffic, UIT-04]
requires: ["51-02"]
provides:
  - "panel_dock.rs: pre_dock_area, dock_base_area, dock_layout, subtract_docked_panels, panel_dock_slot, dock_panels_on_arrange, set_panel_docked (all real)"
  - "AgentsWindowState.docked with release on close, SetRect, Size, Maximize (and so Move/Resize)"
affects: [51-05, 51-06, 51-07, 51-12]
key-files:
  created:
    - tests/phase51_panel_tiling.rs
  modified:
    - src/wm/panel_dock.rs
    - src/wm/agents_window.rs
    - src/wm/agents_geometry.rs
    - tests/phase51_seams.rs
decisions:
  - "slot comes from the live workspace (ShellLayout) and ignores the passed ws position, so callers using the already reduced effective area still get the slot"
  - "stack-grid floating rects are rebuilt inside dock_panels_on_arrange after the dock decision (layout_profile.rs untouched)"
  - "unequal remainder cells of the strip are left unused so slots are exactly equal"
status: complete
plan_head_before: 0d52cae2f4fc6ee98d42a2ff788b91c9621eed06
plan_head_after: 7059fd8a9d139e2324a4d1140179a6df392a6e98
commits: 2
actuals:
  tokens: 22000
  tasks: 2
  commits: 2
---

# Phase 51 Plan 09: Panel tiling Summary

`:arrange` / `:auto-arrange` / `:layout grid|columns|rows|main-side|main-top|stack-grid` now dock the open agents list and AGENT TRAFFIC window into a live-derived strip (right strip `clamp(w/3,28,48)` when the workspace is >= 100 wide, else a bottom strip `max(6,h/3)`), switch an active MCP console to Push, and re-flow the panes; with no panel open nothing changes.

Commits: `9f68ccc` (panel_dock + traffic window flag and release paths), `7059fd8` (tests, seam tests updated to the real hooks).

## Gates

- `cargo test -q --test phase51_panel_tiling`: 18 passed. `--lib wm::panel_dock`: 5 passed.
- `--test phase51_seams --test phase50_agents_window --test phase50_agents_nonmodal --test panel_pin_scale --test layout_profiles_and_autosave --test phase3_window_manager --test drawer_classic_parity --test phase51_tracer`: all pass.
- `cargo clippy -q --all-targets -- -D warnings`: clean. `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`. `scripts/verify.sh`: `PASS 80/80 51s`.
- `git diff --exit-code HEAD -- layout_profile.rs drawer.rs input/agents_window.rs agents_list_geometry.rs Cargo.toml Cargo.lock`: clean. `agents_window.rs` 1177 lines (+9), `panel_dock.rs` 327.

## Deviations from Plan

**1. [Rule 1] 51-02 seam tests updated.** `phase51_seams.rs` asserted the stub behavior (arrange hook never docks, `set_panel_docked` never reflows). Three assertions now assert the real behavior. File not in `files_modified`; committed with the tests.

**2. [Constraint] `Not enough room to dock panels` is not visible after `:arrange`.** The hook sets the status, but the `:arrange` / `:layout <pattern>` executor in `layout_profile.rs` (forbidden) overwrites the pane status with `Auto-arranged ...` right afterwards. Flags stay off and panels float correctly; the message shows on the direct `set_panel_docked` path (tested there). Follow-up: let `layout_profile.rs` append a `dock_notice`.

**3. [Rule 2] Closed-panel dock refusal sets status `Open the panel before docking it`** (plan said "with the status" without wording).

**4. [Rule 3] `subtract_dock_rect` is private to `drawer.rs`;** a 6-line copy lives in `panel_dock.rs` (`subtract_edge`), guarded by the `pre_dock_area_matches_effective` drift test.

**5. stack-grid RISK handled.** `auto_arrange_windows` computes the floating rects before the hook; the hook now re-derives them from `split_tree` over the final area (`regrid_floating_windows`, only when every pane has a custom geometry, i.e. stack-grid). Test `stack_grid_floating_rects_account_for_docks` fails without it (verified).

## Notes for other plans

- **51-05:** `close_agents_list` / dismiss / float must call `set_panel_docked(DockPanel::AgentsList, false)` (it does not yet); otherwise a closed list leaves the flag set and the panes are not re-flowed until the next resize. Closing an undocked list is unaffected (a closed panel never reserves space).
- **51-06/07:** the renderer and mouse handler pass the real `shell.workspace`; `agents_list_rect` intersects the slot with the passed `ws`. If a caller passes `current_workspace_area().1` (already minus the strip) the list slot is clipped away and it falls back to floating. Use the real workspace or `dock_base_area()` (pub, added here).

## Known Stubs

None.

## Threat Flags

None. T-51-22 (saturating math, 20x6 floor, `strip_rules_right_bottom_and_floor` incl. 0x0 and u16::MAX), T-51-23 (`arrange_autosave_ignores_docked_flag`, `dock_flag_not_in_config`), T-51-24 (pane sizes asserted on dock and release), T-51-SC (no dependency change) mitigated.

## Self-Check: PASSED

Files exist; commits `9f68ccc`, `7059fd8` are ancestors of HEAD.
