---
phase: 50-agent-aware-workspace-navigator
plan: 09
subsystem: input
tags: [agent-traffic, window-chrome, mouse, hit-test, colon-command, keymap]
status: complete

requires:
  - phase: 50-08
    provides: "AGENTS_WINDOW_BINDINGS, agents_window_action, handle_agents_window_key, wm.-allow-list gate"
  - phase: 50-07
    provides: "AGENT TRAFFIC window state, GraphLayout (pinned map), graph_raster"
provides:
  - "src/wm/agents_geometry.rs: agents_pane_rects / agents_hit (single geometry source), set_agents_rect (clamped, min 40x12), move/resize/size/maximize/splitter ops, pin/unpin/nudge node, grab mode, agents_select_edge"
  - "AgentsDrag + AgentsWindowState.drag/grab_mode + WindowManager::set_agents_drag"
  - "13 new AgentsOp variants with :agents move|resize|rect|size|max|split|select <key>|edge <a> <b>|grab|nudge|node-pin|node-unpin"
  - "src/input/agents_window.rs::handle_agents_window_mouse (hit-test -> Action -> wm.apply)"
  - "AgentsKeyScope::Grab and window geometry / node rows in AGENTS_WINDOW_BINDINGS"
affects: [50-16, 50-10, 50-11, 50-12, 50-14]

actuals:
  tokens: 30000
  tasks: 3
  commits: 3
plan_head_before: a0f4152cdb464dba905700d9848e2c5ee042db5c
plan_head_after: 6647f54c382fed91f8982b66e5dd93f99413a2fc

tech-stack:
  added: []
  patterns:
    - "Drag bookkeeping (AgentsDrag) stored with set_agents_drag; every drag step lowers to SetRect / Split / PinNode"
    - "Geometry computed once (agents_pane_rects) and shared by renderer, hit-test and mouse handler"

key-files:
  created:
    - src/wm/agents_geometry.rs
  modified:
    - src/wm/agents_window.rs
    - src/wm/agents_cmd.rs
    - src/wm/mod.rs
    - src/wm/z_stack.rs
    - src/input/agents_window.rs
    - src/input/keymap.rs
    - src/input/mouse.rs
    - src/ui/agents_ui.rs
    - src/ui/graph_raster.rs
    - src/ui/help_catalog.rs
    - src/colon_command/parse_agents.rs
    - src/colon_command/execute_agents.rs
    - src/pane/completion.rs
    - tests/phase50_agents_window.rs

key-decisions:
  - "Top-row corners resize, the rest of the top row is the title bar (same convention as modal windows)"
  - "Node pin position is stored as permille; a drag treats the pointer as the label cell (label sits one row below the dot) so the hit-test round-trips"
  - "Unpinned window swallows clicks anywhere in the workspace (modal-like) but lets menubar / F-key bar clicks through; a pinned window releases the keyboard on any outside press and lets the click continue"

requirements-completed: []
---

# Phase 50 Plan 09: Agents Window Chrome and Node Gestures Summary

The AGENT TRAFFIC window now moves, resizes, maximizes, closes and splits like other windows, and its nodes and edges can be clicked, dragged, grabbed and unpinned; every key, mouse and colon path lowers to `Action::Agents(..)` and `scripts/verify.sh` passes 56/56 with `GOLDENS OK 11`.

## What was built

- **Geometry** (`src/wm/agents_geometry.rs`, new): `agents_pane_rects` (outer, inner, title row, graph, splitter column, conversation pane) feeds `render_agents_window`, `agents_hit` and the mouse handler. `agents_hit` returns `Title`, `Border(edge)`, `Splitter`, `Node(key)` (label or dot cell), `Edge(a, b)` (cell within one cell of the sampled bezier) and `Graph`/`Snoop`.
- **Ops** (reached only through `:agents`): clamped `set_agents_rect`, `move`, `resize`, `size`, `max`, `split` (20..=80), `pin`, `unpin`, `nudge` (one terminal cell in permille), `grab`, `agents_select_edge`.
- **Mouse** (`handle_agents_window_mouse`): title drag, edge/corner resize, splitter drag, badge maximize/close, node select / second-click activate / drag-pin, edge select, right-click unpin, pinned outside-click `Dismiss`. In-window events never reach drawer or panes.
- **Keys**: Shift+arrows move, Alt+arrows resize, `m`, `<`/`>`, `g`, `u`; `AgentsKeyScope::Grab` rows (arrows nudge; `g`, Enter, Esc drop).
- **Commands, completion, help**: all 13 new verbs have parse (with usage errors), executor, completion and `HELP_LINES` rows, plus palette rows.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Window handler placed before the active-pane frame-badge check, not immediately before the drawer handler**
- **Found during:** Task 1
- **Issue:** `handle_wm_mouse_event` runs `pane_actions::action_at` on the active pane first; a click on a pane badge hidden beneath the window would have triggered a pane action, breaking "mouse events over the window never reach the panes".
- **Fix:** call `handle_agents_window_mouse` right after the pills-bar check. To keep the menubar and F-key bar usable, an unpinned window only swallows outside clicks that lie inside the workspace rect.
- **Files modified:** src/input/mouse.rs
- **Commit:** de74d71

**2. [Rule 3 - Blocking] Geometry ops live in a new file `src/wm/agents_geometry.rs`**
- **Issue:** adding them to `agents_window.rs` (650 lines) would have exceeded the ~1000-line soft target (ARCH-R04). `AgentsDrag` and the state fields stay in `agents_window.rs`.
- **Files modified:** src/wm/mod.rs (one `pub mod` line)
- **Commit:** ea1a230

**3. [Rule 3 - Blocking] `label_cell` made `pub` in `src/ui/graph_raster.rs`** (visibility only) so the node hit-test uses the exact label cells the renderer draws.

**4. [Rule 1 - Bug] 50-08 key tests adapted to the new scope**
- `keymap_agents_window_keys_unique` now permits the documented Grab-scope Esc override of the Window Esc row; `window_keys_match_colon_forms` turns grab on in setup for Grab-scope rows and compares a richer `Observed` (rect, maximized, splitter, grab, pinned).

**5. Commit granularity:** the three tasks' implementation shared files, so the work was committed as geometry/commands, input/keys, and tests (each builds) instead of strictly one commit per task.

**6. Top edge:** only the four corners of the top row resize; the top-edge cells are the title bar (modal convention), so "any edge" is covered by corners plus the other three edges.

## Known limitation

- The `:mcp` console is drawn above the agents window, but the agents handler sees mouse events first, so an MCP panel overlapping the window gets no clicks in the overlap. Not addressed (outside this plan).

## Verification

- `cargo test --test phase50_agents_window`: 35 passed (26 `window_`, 1 `parity_`).
- `wm.` allow-list gate on `src/input/agents_window.rs`: prints nothing.
- `scripts/verify.sh`: `PASS 56/56`; `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
- `src/input/mouse_wm.rs`, `Cargo.toml`, `Cargo.lock` untouched; keymap.rs is 854 lines.

## Known Stubs

None.

## Threat Flags

None. T-50-21 (clamping, parse limits) and T-50-22 (swallowed events) are covered by `window_rect_clamped_to_workspace`, `window_colon_size_forms`, `window_colon_node_forms` and the tracer test.

## Self-Check: PASSED

Files and commits verified: src/wm/agents_geometry.rs present; commits ea1a230, de74d71, 6647f54 are ancestors of HEAD.
