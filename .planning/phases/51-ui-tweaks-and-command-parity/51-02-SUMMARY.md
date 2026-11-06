---
phase: 51-ui-tweaks-and-command-parity
plan: 02
subsystem: wm
tags: [seam, geometry, hit-test, panel-dock, agents-demo, UIT-01, UIT-03, UIT-04]
requires: ["51-01"]
provides:
  - "agents_list_geometry.rs: ListChip, ListBadge, ListHit, AgentsListRects, agents_list_rect/layout/hit (FROZEN)"
  - "panel_dock.rs: DockPanel + identity stubs (pub, see deviations)"
  - "agents_demo.rs: private AgentsDemoState, AgentsDemoSummary, clear_agents_demo, agents_demo_summary"
  - "three hooks: subtract_docked_panels, dock_panels_on_arrange, clear_agents_demo"
affects: [51-03, 51-05, 51-06, 51-07, 51-09, 51-10]
key-files:
  created:
    - src/wm/agents_list_geometry.rs
    - src/wm/panel_dock.rs
    - src/wm/agents_demo.rs
    - tests/phase51_seams.rs
  modified:
    - src/wm/agents_list.rs
    - src/wm/mod.rs
    - src/wm/drawer.rs
    - src/wm/layout_profile.rs
    - src/wm/session_manager.rs
decisions:
  - "geometry shift-clamps a manual rect into the workspace (keeps its size, floors 20x6) instead of intersecting"
  - "Pin badge reuses agents_pin_badge_x (13-char AGENT TRAFFIC title), so a 24-wide window has no Pin"
status: complete
base_sha: 4a298815dc56e827ada823b0a3dfb51db2d4d983
plan_head_before: 4a298815dc56e827ada823b0a3dfb51db2d4d983
plan_head_after: 2a5b4519248c276c8822966080c924ea34da748b
commits: 2
actuals:
  tokens: 16000
  tasks: 2
  commits: 2
---

# Phase 51 Plan 02: Geometry/hit seam and identity hooks Summary

One frozen, tested geometry source (`agents_list_rect/layout/hit`) for the list window plus three identity hooks (tiling x2, demo reset) so wave 3 can build against stable signatures.

Commits: `aca82a0` (Task 1 geometry + panel_dock stub), `2a5b451` (Task 2 hooks + demo seam).

## Gates

- `cargo test -q --test phase51_seams`: 10 passed (6 geometry + 4 hook tests). `--test phase51_tracer`: 10 passed.
- `scripts/verify.sh` (full): `PASS 79/79 65s`.
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
- `git diff --exit-code HEAD -- Cargo.toml Cargo.lock`: clean.
- Acceptance greps: `pub fn agents_list_hit` in geometry = 1; `fn agents_list_rect` in agents_list.rs = 0; `subtract_docked_panels` in drawer.rs = 1; `dock_panels_on_arrange` in layout_profile.rs = 1; `clear_agents_demo` in layout_profile.rs = 1 and session_manager.rs = 1; `agents_demo` in mod.rs = 3, `pub.*agents_demo:` = 0.
- Lines: agents_list_geometry.rs 267; layout_profile.rs 981 (+2); agents_window.rs 1168 (untouched); mod.rs 918; agents_list.rs 197.

## Deviations from Plan

**1. [Rule 3 - Blocking] Dock stubs and demo helper are `pub`, not `pub(crate)`.** Integration tests (`tests/`) cannot call `pub(crate)` items, and the plan's own tests (and 51-09's) need `subtract_docked_panels`, `panel_dock_slot`, `dock_panels_on_arrange`, `set_panel_docked`. `agents_demo` field stays private (ARCH-R02). Added `#[doc(hidden)] pub fn seed_agents_demo_for_tests` for the same reason (cfg(test) is invisible to integration tests); 51-03 may delete it.

**2. [Doc] Plan wrote `area = ...; area` for the drawer hook.** Implemented as the tail expression `self.subtract_docked_panels(area)` (same behavior, one fewer line).

**3. Test substitution.** Plan says `:agents list pin` is reflected in `agents_list_summary`; `Pin` is still a 51-05 no-op arm, so the test asserts open/focus/scale/row_count only.

## Decisions / facts for wave 3

- The seam constants `LIST_MIN_WORKSPACE`, `LIST_DEFAULT_WIDTH` moved from `agents_list.rs` to the geometry file; added `LIST_MIN_WINDOW = (20, 6)` and `LIST_CHIP_LABELS`.
- Manual `rect` handling: size clamped to `[min(20,ws.w) x min(6,ws.h) .. ws]`, position clamped so the window is wholly inside `ws` (shifted, not shrunk). A zero or 1x1 stored rect yields 20x6.
- Docked: `panel_dock_slot(ws, AgentsList)` result is intersected with `ws`; if empty it falls back to manual/default placement.

## What wave 3 may rely on (exact seams)

| Plan | May rely on | Must not |
|------|-------------|----------|
| 51-03 demo | `wm.agents_demo` private field of type `agents_demo::AgentsDemoState { endpoints, spawned }` (crate-private struct, `pub(crate)` fields), `agents_demo_summary()`, `clear_agents_demo()`; hooks already run in `create_new_session` / `apply_saved_session_snapshot`; may delete `seed_agents_demo_for_tests` and `AgentsDemoSummary.running` is `!endpoints.is_empty() \|\| !spawned.is_empty()` (keep or redefine in its own file) | edit `wm/mod.rs`, `session_manager.rs`, `layout_profile.rs` |
| 51-05 core | `agents_list_geometry` (frozen); `agents_list_apply` still has the grouped no-op arm (exhaustive, no wildcard); `set_panel_docked(DockPanel::AgentsList, bool) -> bool` for Dock/float and every manual-geometry release path (never write `docked` directly); `agents_list_layout(ws).visible_rows` for Page/scroll clamping (ws = `panel_virtual_area(real_ws, PanelTarget::AgentsList)`) | edit the geometry file, `LIST_*` constants |
| 51-06 render | `agents_list_layout(ws)` for all regions, `badges`, `chips` (`LIST_CHIP_LABELS` gives chip text; chips left-to-right from `footer.x`, one blank cell between), header/footer are zero-size when the window is under 3 inner rows; `agents_pin_badge_x` semantics: Pin badge is a 1-cell glyph at `badges[Pin]`; `render_agents_list` currently still calls `agents_list_rect` (works unchanged) | compute its own rect |
| 51-07 input | `agents_list_hit(ws, col, row)` with `(mouse, ws) = panel_space(mouse, real_ws, PanelTarget::AgentsList)`; `ListHit::{Badge(Pin\|Minimize\|Dock\|Close), Border(edge), Chip, Row(id), Header, Body, Footer, Title, Outside}`; precedence badges > border > title > header > chips > footer > rows > body; `Row` only when `scroll + (row - rows_area.y)` is an existing filtered row (so a click on the empty remainder is `Body`, not `Row`) | add its own hit math |
| 51-09 tiling | `panel_dock.rs` stubs to replace (signatures frozen; all `pub`); hooks live: `effective_workspace_area` ends with `self.subtract_docked_panels(area)` (area is real cells; called for `compute_workspace_rects`, `arrange_workspace_area`), `auto_arrange_windows` calls `dock_panels_on_arrange()` right before `resize_all(vp)`; NOTE the `stack-grid` / `float-grid` branch computes `arrange_workspace_area()` (and so the dock subtraction) BEFORE the hook runs, so docks flipped by the hook are not reflected in that branch's floating rects unless 51-09 handles it (e.g. dock decision made earlier or recompute); the geometry file already consumes `panel_dock_slot(ws, AgentsList)` when `docked` | edit `drawer.rs` / `layout_profile.rs` / geometry file |
| 51-10 API | `agents_list_summary()` (51-01) and `agents_demo_summary()` (this plan), both `&self` | |

## Known Stubs

- `panel_dock.rs` bodies (identity; 51-09 owns), `seed_agents_demo_for_tests` (test helper; 51-03 may delete). Plan-assigned seams; no ledger entries.

## Threat Flags

None new. T-51-04 mitigated (saturating math, clamp, `None` under 24x6, `rect_bounds`/`hit_table` tests incl. 24x6 and 200x60, zero-size manual rect, 20x6 and 1x1 scan in `badges_hidden_when_narrow`); T-51-05 mitigated (`clear_demo_hook_runs` covers `create_new_session` and the snapshot restore via `switch_layout_profile`).

## Self-Check: PASSED

Created files exist; commits `aca82a0` and `2a5b451` are ancestors of HEAD.
