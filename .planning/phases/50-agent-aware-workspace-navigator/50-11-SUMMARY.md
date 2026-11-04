---
phase: 50-agent-aware-workspace-navigator
plan: 11
subsystem: ui
tags: [gui, pixel-canvas, vector-graphics, agents-graph, navigator-icons, anti-aliasing]
status: complete

requires:
  - phase: 50-09
    provides: "agents_pane_rects (graph rect shared by renderer, hit-tests and the GUI pass)"
  - phase: 50-16
    provides: "PanelTarget::Agents scale and the renderer's scaled-panel block"
  - phase: 50-04
    provides: "navigator_dot_cells (single source of dot positions)"
provides:
  - "GUI agents graph as one opaque 1:1 RGBA texture (id 950_000): curved AA stroked edges, flow particles, state glow, hi-DPI and panel-scale aware"
  - "Navigator state dots as vector icons (ids 960_000 + row index): rotating arc, pulsing triangle, ring, disc, cross"
  - "graph_raster::{draw_agent_graph_hires, draw_state_icon, render_graph_labels, stroke_polyline, bezier_point, node_px_hires}"
  - "WindowManager::tick_agent_ui is pub; keeps the 120 ms cadence while an edge flowed within 2 s"
affects: [50-13, 50-12, 50-14]

actuals:
  tokens: 60000
  tasks: 3
  commits: 3
plan_head_before: 87b329e4cf392b5d13e812779dfa9191d2bef3a4
plan_head_after: edd1853697b7f72a7a5e3485eacdb38e900948fd

tech-stack:
  added: []
  patterns:
    - "Texture pass runs on the finished cell buffer, clears covered cells, pushes opaque 1:1 textures; the renderer sorts agents textures last so pane canvases cannot paint over them"
    - "Per-pixel max coverage over polyline segments (no double-blended joints) for stroked edges"
    - "Const unit-vector / pulse tables instead of runtime trigonometry"

key-files:
  created:
    - src/ui/agents_gui.rs
    - tests/phase50_gui_render.rs
    - tests/snapshots/phase50_gui_render__render_gui_static_frame.snap
  modified:
    - src/ui/graph_raster.rs
    - src/ui/mod.rs
    - src/ui/renderer.rs
    - src/wm/agent_state.rs
    - src/wm/traffic.rs

key-decisions:
  - "No vector crate (tiny-skia not added): Cargo.toml / Cargo.lock untouched"
  - "A texture over u16::MAX per axis or 16 Mpx is skipped (braille/dot fallback stays) instead of being allocated or down-scaled, because 1:1 is required (T-50-26)"
  - "Textures are skipped when an overlay (:mcp console, modal, menu dropdown, quick terminal) covers them; icons also when the agents window covers them"
  - "Agents textures are moved after pane canvas textures with a stable sort in the renderer"
---

# Phase 50 Plan 11: GUI vector graphics for the agents graph and Navigator icons Summary

In GUI mode the agents graph is now one opaque 1:1 hi-DPI RGBA texture with anti-aliased curved edges, three flow particles per active edge and a state-coloured glow per node, and the Navigator sidebar state dots are smooth vector icons, with no new dependency.

## Tasks

| Task | Name | Commit |
|------|------|--------|
| 1 | Tracer: opaque 1:1 graph texture, braille cells cleared, labels on top | d6db68e |
| 2 | Curved AA strokes, particles, glow, hi-DPI, scaled window, tick cadence | d7ab0fc |
| 3 | Navigator state icons with working/blocked animation | edd1853 |

## Verification

- `scripts/verify.sh`: PASS 57/57 (full run, also the targeted run from the plan: clippy, unit, phase50_gui_render, phase46_display_modes, high_res_pixel_and_ssh_canvas, v040_v050_gpu_ghostty_tmux_graphics, drawer_classic_parity).
- `scripts/check_classic_goldens.sh`: GOLDENS OK 11 (classic output unchanged).
- `phase50_gui_render`: 15 tests (tracer, static-frame snapshot, curve/AA, particles, glow, hi-DPI, scaled window, size cap, tick cadence, icons pushed/shapes/animation, classic pushes none, TUI path, scaled drawer icons).
- `grep -Ec "\.(sin|cos|powf|atan2)\(" src/ui/graph_raster.rs` = 0; `git diff Cargo.toml Cargo.lock` clean; `src/ui/renderer.rs` is 875 lines.
- Human check (X11 `--gui` visual smoothness and animation feel) not performed: needs a desktop session.

## Deviations from Plan

**1. [Rule 3 - Blocking] `WindowManager::tick_agent_ui` made `pub`** (was `pub(crate)`) so the cadence test can live in the integration test file as specified; added `TrafficBus::latest_at()` (src/wm/traffic.rs, not in the plan's file list) so the 2 s "edge still flowing" check is O(1) per poll. Commit d7ab0fc.

**2. [Plan detail] Blocked pulse uses a 12-step table (`tick % 12`) instead of 6-step.** With `tick % 6`, ticks 0 and 6 are identical, which would contradict the plan's own `render_gui_navigator_icons_animate_working_only` assertion (Blocked changes between tick 0 and tick 6). Commit edd1853.

**3. [Plan detail] Oversize textures are skipped, not clamped.** Clamping the dimensions would break the 1:1 rule; the 16 Mpx budget was added on top of `u16::MAX` because an in-range 60000 x 30000 texture would still allocate gigabytes.

**4. [Scope] `ratatui_color_to_rgba` made `pub(crate)`** in renderer.rs to resolve icon backgrounds from the cell the dot sits on.

Otherwise the plan was executed as written.

## Known Stubs

None.

## Threat Flags

None. T-50-26 mitigated (`render_gui_texture_size_capped`, at most one graph texture and one icon per visible dot), T-50-27 mitigated (id ranges documented at the top of `src/ui/agents_gui.rs`), T-50-SC mitigated (no Cargo changes).

## Self-Check: PASSED

Commits d6db68e, d7ab0fc and edd1853 exist on gsd/phase-50-navigator; created files present.
