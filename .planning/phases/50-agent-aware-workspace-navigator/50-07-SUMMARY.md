---
phase: 50-agent-aware-workspace-navigator
plan: 07
subsystem: wm-ui
tags: [agent-traffic, graph, layout, braille, selection, tui]
status: complete

requires:
  - phase: 50-05
    provides: "TrafficBus / TrafficEndpoint read through WindowManager::traffic()"
  - phase: 50-03
    provides: "WindowManager::agent_state(id)"
provides:
  - "AgentsWindowState, NodeKey, GraphScene/SceneNode/SceneEdge, GraphSelection, GraphDir, GraphLayout in src/wm/agents_window.rs"
  - "WindowManager::{agents_window, agents_window_mut, open_agents_window, close_agents_window, agents_window_rect, agents_graph_scene, agents_select_node, agents_select_direction, agents_cycle_edge}"
  - "crate::ui::agents_ui::render_agents_window (frame, graph pane, events pane)"
  - "crate::ui::graph_raster::{GraphColours, draw_agent_graph, render_graph_tui, node_px, curve_controls}"
affects: [50-08, 50-09, 50-10, 50-11, 50-12]

actuals:
  tokens: 13000
  tasks: 2
  commits: 2
plan_head_before: 04d1bdd27506abf9b5e555351898028efcd4e5a7
plan_head_after: d380428f76a4f28c3d19bd96f3b076a9dca4d53e

tech-stack:
  added: []
  patterns:
    - "Floating window state copies McpConsoleState semantics (captures_input = active && (!pinned || focused)); not a Modal variant"
    - "Layout cache behind a Mutex inside GraphLayout so the &WindowManager render path stays idempotent per topology"
    - "Transparent PixelCanvasBuffer + labels written before the braille flush"

key-files:
  created:
    - src/wm/agents_window.rs
    - src/ui/agents_ui.rs
    - src/ui/graph_raster.rs
    - tests/phase50_agents_window.rs
    - tests/snapshots/phase50_agents_window__render_tui_graph_snapshot_static.snap
  modified:
    - src/wm/mod.rs
    - src/ui/mod.rs

key-decisions:
  - "Edges are undirected, one per unordered endpoint pair with from < to by key; direction stays visible in the events list"
  - "GraphLayout holds a Mutex cache (previous positions plus topology signature): an unchanged topology returns cached positions, a changed one warm-starts (20 iterations, T0 0.05) and known nodes drift at most 0.06"
  - "Above 64 nodes (only possible with more than 64 live panes) the O(n^2) force layout is skipped for the grid placement"
  - "Edge colours: idle border colour, success colour for active traffic (newest 3 bus events), accent for the selected edge (drawn 2 px thick)"

requirements-completed: [FLT-01, FLT-02]

coverage:
  - id: D1
    description: "Opening the window renders AGENT TRAFFIC with a braille graph, node labels and the newest bus events; closing removes it"
    requirement: FLT-02
    verification:
      - kind: unit
        ref: "tests/phase50_agents_window.rs window_tracer_render_open_close"
        status: pass
    human_judgment: false
  - id: D2
    description: "Layout is bit-for-bit deterministic, stable on insertion, bounded and capped at 64 nodes with live panes kept"
    requirement: FLT-02
    verification:
      - kind: unit
        ref: "tests/phase50_agents_window.rs layout_deterministic_bit_for_bit, layout_stable_on_insertion, layout_respects_bounds_and_cap"
        status: pass
    human_judgment: false
  - id: D3
    description: "Selection moves to the nearest node by direction and cycles incident edges with wrap"
    requirement: FLT-02
    verification:
      - kind: unit
        ref: "tests/phase50_agents_window.rs graph_select_nearest_in_direction, graph_cycle_incident_edges"
        status: pass
    human_judgment: false
  - id: D4
    description: "Selected node label is reversed and the selected edge is accent coloured; static frame snapshot is stable"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_agents_window.rs render_selection_highlight, render_tui_graph_snapshot_static"
        status: pass
    human_judgment: true
    rationale: "Whether the graph reads well (spacing, bend, colours) is a visual judgement"
---

# Phase 50 Plan 07: Agent Traffic Window Summary

**Floating AGENT TRAFFIC window: bus-driven node graph (deterministic Fruchterman-Reingold layout, braille Bezier edges, state-coloured labelled nodes) next to a newest-at-bottom events list, with a direction/edge selection model, all reachable through the `WindowManager` API.**

## Performance

- Tasks: 2 (1 tracer, 1 tdd auto); files: 5 created, 2 modified
- Commits: `ef671bf` (tracer), `d380428` (layout, selection, curves, snapshot)

## Accomplishments

- `src/wm/agents_window.rs`: window state (`captures_input` same rule as the `:mcp` console, no `Modal` variant), scene aggregation from the bus plus one node per live pane (labels `you`, `#<id>`, `@<name>` through `sanitize_display(.., 12)`), node cap 64 keeping live panes, default rect 80% x 70% (min 40 x 12), nothing below 30 x 10.
- Layout: FNV-1a seeded placement next to a placed neighbour, 60 cold / 20 warm iterations, pinned nodes fixed, positions clamped to [0.05, 0.95], only `+ - * / sqrt`.
- Selection: `agents_select_node`, `agents_select_direction` (dot-product filter, nearest, key tie-break; false and unchanged when nothing lies that way), `agents_cycle_edge` (sorted by other endpoint, wraps).
- `render_agents_window` is called right before `render_mcp_console_overlay`, so it draws above the drawer and below the `:mcp` console.

## Deviations from Plan

**1. [Rule 1 - Bug] Transparent canvas background instead of "opaque background"**
- **Found during:** Task 1
- **Issue:** `flush_truecolor_subcells_to_ratatui` treats a fully opaque cell as a two-colour cluster; with an opaque background every empty cell would flush as a full braille block.
- **Fix:** canvas pre-filled with the window background colour at alpha 0, so only drawn pixels become braille cells.
- **Files modified:** src/ui/graph_raster.rs

**2. [Rule 1 - Bug] Active edge colour distinct from the accent**
- **Found during:** Task 2 (`render_selection_highlight`)
- **Issue:** in the default palette `border_focus == accent`, so a selected edge was indistinguishable from active traffic.
- **Fix:** active edges use the `success` token; the selected edge is accent and two pixels thick.
- **Files modified:** src/ui/graph_raster.rs

**3. [Rule 2 - Missing critical] Layout cache and drift limit**
- The plan's `agents_graph_scene(&self)` renders from `&WindowManager`, so warm-start memory sits behind a `Mutex` in `GraphLayout`; an unchanged topology returns cached positions (no wobble between frames), and known nodes are limited to 0.06 drift per re-layout so the 0.08 stability bound holds by construction. Above 64 nodes the grid placement replaces the O(n^2) layout (T-50-19).

Tests were written together with the implementation for Task 2 (the selection tests were observed failing only as compile errors before the API existed).

## Known Stubs

None. `tick` in `draw_agent_graph` is `0` for the TUI static frame; the animated halo for `tick > 0` is consumed by the GUI backend (50-11).

## Threat Flags

None. T-50-18: labels and previews go through `sanitize_display` (12 / 160 chars) and are width-truncated; T-50-19: node cap 64, fixed iteration counts, grid fallback above the cap; T-50-SC: `git diff Cargo.toml Cargo.lock` clean.

## Verification

- `scripts/verify.sh --only clippy unit t:phase50_agents_window t:phase50_traffic t:drawer_classic_parity t:panel_pin_scale t:mcp_interactive_navigation`: `PASS 56/56`
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `tests/phase50_agents_window.rs`: 8 passed, snapshot stable over repeated runs
- Acceptance greps: `render_agents_window` in src/ui/mod.rs 1, `pub mod agents_window` in src/wm/mod.rs 1, `Modal::Agents` 0, `.sin/.cos/.powf/.atan2` in agents_window.rs 0

## Next Phase Readiness

50-08 binds `:agents zoom|close`, leader `a`, Esc and the arrow/Tab/Enter keys to the `WindowManager` methods above; 50-09 adds chrome drag/resize/splitter and fills `GraphLayout::pinned`; 50-10 turns the events pane into the snoop view; 50-11 reuses `draw_agent_graph` for the GUI.

## Self-Check: PASSED

Created files exist; commits ef671bf and d380428 are ancestors of HEAD.
