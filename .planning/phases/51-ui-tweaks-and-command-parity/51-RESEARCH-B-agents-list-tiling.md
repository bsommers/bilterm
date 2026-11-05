# Phase 51 Research B: Agents-list window (UIT-03) and tiling (UIT-04)

**Researched:** 2026-11-05 (read-only; no source modified)
**Confidence:** HIGH for current-state facts (all read this session, cited file:line); MEDIUM for the recommended design (a design, not an existing fact).
Provenance: `[V path:line]` = opened and read this session. `[A]` = assumption, needs a decision or a grep before use.

## Project Constraints (from CLAUDE.md / AGENTS.md)
- Every `src/**/*.rs` targets ~800-1000 lines; hard test backstop 1200 (`tests/session_lessons_and_regression_suite.rs:375-376`, `tests/vt_worker_and_snapshot_seam.rs:208`). `handle_wm_key_event` stays <= 80 lines (ARCH-R01).
- Every key/mouse/colon path lowers to `crate::action::Action` and runs through `WindowManager::apply` (`src/colon_command/execute_wm.rs`). WM/pane internals stay private (ARCH-R02).
- Harmony rule (ROADMAP Phase 51): add paths, never rewire; Legacy output byte-identical (11 goldens, `scripts/check_classic_goldens.sh`).
- Verify with `scripts/verify.sh` (one-line PASS/FAIL).
- Docs gate: `scripts/check_phase50_doc_coverage.sh` requires every `:agents` completion-row verb to appear in `docs/user-guide/11-navigator-sidebar-and-agent-traffic.md` as `:agents <verb>` [V scripts/check_phase50_doc_coverage.sh:1-40].

---

## 1. The existing window-tiling function

**It is `WindowManager::auto_arrange_windows(pattern)`** [V src/wm/layout_profile.rs:238-318], reached by `:arrange [pattern]` / `:auto-arrange` and `:layout grid|columns|rows|main-side|main-top|stack-grid|arrange [pattern]` [V src/colon_command/parse.rs:180-186, 191-215] -> `ColonCommand::LayoutProfileCommand("arrange <pat>")` -> `layout_profile.rs:869-895`. The default arm (`grid`/`tile`/`equal`/unknown) sets `layout_mode = Tiled` and rebuilds `split_tree = build_balanced_split_tree(&ids)` [V layout_profile.rs:301-306]. It ends with `resize_all(vp)` and an autosave [V :309-315].

Separate from it: `:layout tile|stack|cycle`, `Alt+M`, `Ctrl+X M` only flip `layout_mode` (`set_layout_mode` wm/mod.rs:378, `toggle_layout_mode` wm/mod.rs:785) and keep existing split ratios / z-rects. Treat `auto_arrange_windows` as "the tiling function"; do NOT hook the mode toggle (it would clobber manual geometry). Flag as decision D3.

**What it tiles:** only `CompositePane`s: `ids = self.ordered_pane_ids()` [V :241], a `SplitTree` (Tiled) or `ZStack` rects (Stacked).

**What it ignores:** every floating panel. Geometry flows `compute_workspace_rects(area)` [V src/wm/mod.rs:801-830] -> `effective_workspace_area(area)` [V src/wm/drawer.rs:388-402], which subtracts only (a) the drawer when `config.drawer.layout == Push` and (b) the MCP console when `active && layout_mode == Push`:

```rust
if self.config.drawer.layout == DrawerLayoutMode::Push && let Some(dr) = self.drawer_rect(area) { area = subtract_dock_rect(area, self.config.drawer.edge, dr); }
if self.mcp_console.active && self.mcp_console.layout_mode == DrawerLayoutMode::Push && let Some(mr) = self.mcp_console.panel_rect(area) { ... }
```

So: drawer and MCP console already have a "docked/tiled" mode (Push) but `arrange` never switches them to it; the AGENT TRAFFIC window (`agents_window_rect`, wm/agents_window.rs:610-636) has no dock mode at all and always overlays. `arrange_workspace_area` (layout_profile.rs:218-236) also calls `effective_workspace_area`.

### How a floating panel should join tiling (recommendation)
Add a **dock strip** that `auto_arrange_windows` activates, in a NEW file `src/wm/panel_dock.rs`:
- Panels eligible: agents-list window (new), AGENT TRAFFIC window. MCP console: arrange sets `layout_mode = Push` (reuses existing dock path, no new geometry). Drawer: left unchanged (config-level Push, persisted; listed as deliberate exclusion, D4).
- Each eligible panel gets `docked: bool`. `auto_arrange_windows` sets `docked = true` for every `active` eligible panel BEFORE `resize_all`; with no active panel it changes nothing (goldens safe).
- `panel_dock_strip(raw_ws) -> Option<(Rect strip, Vec<(PanelId, Rect)> slots)>` is a pure function of live state: wide workspace (>= 100 cols [A]) -> right strip of `clamp(ws.width/3, 28, 48)` cols, panels stacked vertically with equal height; narrow -> bottom strip, `max(6, ws.height/3)` rows, panels side by side. Skip docking (status message) if the pane area left would be < 20x6 [A thresholds].
- `effective_workspace_area` gains one line after the MCP block: `area = self.subtract_docked_panels(area);`. Because the strip is recomputed from the live workspace every call, terminal resize keeps the tile (no stored absolute rect).
- A docked panel's rect getter (`agents_list_rect`, `agents_window_rect`) returns its slot; geometry is defined in REAL cells. Panels run in a scaled virtual grid (`panel_virtual_area`, ui/typography.rs:271-281), so the getter must convert `virtual = real * 100 / scale` (inverse of `scaled_panel_real_rect`, typography.rs:90-111) or a 125% panel will not fill its slot.
- Any manual `SetRect`/`Move`/`Resize`/`Size`, drag, or `:agents list float` clears `docked` (panel floats again; strip released). Closing the panel (`active=false`) releases the strip automatically.
- Mouse hit-testing needs no change for panes (they already use `compute_workspace_rects`).

---

## 2. How the floating pinnable panels are built today

### Common shape
| | Drawer | MCP console | AGENT TRAFFIC window |
|---|---|---|---|
| State | `wm.drawer`, `config.drawer` (`DrawerState`, wm/drawer.rs) | `McpConsoleState` [V src/mcp_console/mod.rs:252-275] | `AgentsWindowState` [V src/wm/agents_window.rs:384-405] |
| "Pin" | `*` toggles `visibility` Fixed/OnDemand [V src/input/modal.rs:365-371] | `pinned` + `focused`; `captures_input = active && (!pinned \|\| focused)` [V mcp_console/mod.rs:417-418] | `pinned` + `focused`; `captures_input = active && focused` [V agents_window.rs:547-549]; pin only changes what Esc does |
| Rect | `drawer_rect(ws)` wm/drawer.rs:362-385 | `panel_rect(ws)` mcp_console/mod.rs:516-575 | `agents_window_rect(ws)` agents_window.rs:610-636 (default 80%x70% centred, `rect: Option<Rect>` override, `maximized`) |
| Scale | `PanelTarget::Drawer` | `PanelTarget::Mcp` | `PanelTarget::Agents` |
| Key routing | modal.rs `wm.drawer().focused` block | modal.rs:511-535 | modal.rs:334-341 -> `input/agents_window.rs:34-70` |
| Mouse | `handle_drawer_mouse_event` mouse_wm.rs:677+ | `handle_modal_mouse_event` mouse_wm.rs:107+ | `input/agents_window.rs:90-150`, called FIRST at `input/mouse.rs:606` |

### AGENT TRAFFIC window (the template to copy)
- **State:** `pub(crate) agents_window: AgentsWindowState` on `WindowManager` (wm/mod.rs:136, init :216); fields `active, pinned, focused, rect, maximized, splitter_pct, selection, layout, drag, grab_mode, pane_focus, snoop`; `Default` at agents_window.rs:524-541.
- **Lowering:** all ops are `Action::Agents(AgentsOp)` (action.rs:181); executor `execute_agents_command` (colon_command/execute_agents.rs:15-61; window ops in `execute_window_op` :95-176; wired at execute_wm.rs:835). `AgentsOp` is `src/wm/agents_cmd.rs:8-114`; variants relevant here: `Close`, `Toggle`, `Dismiss`, `Blur`, `Select(GraphDir)`, `Activate`, `Move`, `Resize`, `SetRect`, `Size`, `Maximize`, `Pin(Option<bool>)`, `ScaleStep(i16)`. Absolute scale reuses `Action::PanelScaleOf(PanelTarget, u16)` (parse_agents.rs:234-243; action.rs:73).
- **Keys:** declarative table `AGENTS_WINDOW_BINDINGS` (input/keymap.rs:355-415) looked up by `agents_window_action(scopes, key)` (keymap.rs:432); `handle_agents_window_key` only resolves scopes and `wm.apply`s (input/agents_window.rs:34-70). Leader key is never consumed; global toggle chord `Ctrl+Alt+A` (keymap.rs:181-182) also closes the focused window (agents_window.rs:40-43) and is in `reserved_global_action` for raw-passthrough panes (keymap.rs:483-488). Gate in modal.rs:334-341: only when no workspace modal is open and no leader pending.
- **Mouse:** `handle_agents_window_mouse` works in the window's own scaled space via `wm.panel_space(mouse, ws, PanelTarget::Agents)` (input/agents_window.rs:99); `agents_hit(ws, col, row)` -> `AgentsHit::{Outside,Title,Border(edge),...}` (wm/agents_geometry.rs:34-44, 168-+); title pin badge via `agents_pin_badge_x` (ui/agents_ui.rs:37-46); maximize/close via `detect_title_bar_badge_click` (input/mouse_wm.rs:12-30, returns 0/1/2 = min/zoom/close); drag = `AgentsDrag` bookkeeping only, each motion lowers to `AgentsOp::SetRect` (input/agents_window.rs:225-245); resize uses `apply_rect_edge_resize` (mouse_wm.rs:56); a press outside while focused lowers `AgentsOp::Blur` and the event passes on (agents_window.rs:111-117); a click inside an unfocused window lowers `AgentsOp::Zoom(None)` to retake focus (:129-132); Ctrl+wheel = `AgentsOp::ScaleStep` via `ctrl_wheel_scale_delta` (mouse_wm.rs:96). The shared geometry helper `agents_pane_rects` is the single source for renderer + mouse (agents_geometry.rs:~222-245).
- **Scale:** `PanelTarget {Drawer, Mcp, Agents}` (ui/typography.rs:79-84); fields `*_scale_pct` on `WindowManager` (wm/mod.rs:144-146, init :220-221); `scale_setting/set_scale/set_panel_scale_pct/step_scale/scale_pct/panel_virtual_area/panel_space` (typography.rs:232-295); `scale_pct` returns 100 unless `gui_surface` (typography.rs:266). Exhaustive matches on `PanelTarget` exist at typography.rs:232-256 and execute_wm.rs:861-866 (`panel_name`).
- **Focus release hooks:** `focus_and_raise`, `shuffle_deck_next/prev` call `self.agents_window.blur()` (wm/mod.rs:737-765). A new window must be blurred there too.
- **Render (TUI):** `render_agents_window(wm, ws, cfg, palette, glyphs, buf)` (ui/agents_ui.rs:272-343): shadow/halo, `occlude_rect`, fill, `render_borland_frame_styled(rect, title, state.captures_input(), ...)`, pin badge glyph `pin_badge_glyph`. Called from `WorkspaceWidget` at ui/mod.rs:720-722, only `if scale_pct(Agents) == 100`, drawn below modals and the MCP overlay (ui/mod.rs:741).
- **Render (GUI):** GUI turns the cell buffer into quads/glyphs (renderer.rs ~520-800). At scale != 100 the panel is rendered into a scratch `Buffer::empty(vws)` and blitted by `draw_panel(&scratch, vr, scale)` (renderer.rs:749-780; agents block :770-780). Per-panel occluder masks: `real_rect(...)` for `drawer_rect`, `mcp_rect`, `agents_overlay_rect` (renderer.rs:534-556), `is_overlay_cell` (:566-575), `cell_in_scaled_panel` (:590-598). GUI textures (graph, Navigator icons) must not paint over overlapping windows: `overlay_rects` in ui/agents_gui.rs:113-123 (+ `agents_window_pixels` :127-135). Wheel routing `over_scalable_panel` in input/gui_route.rs:50-58 must learn each new scalable panel.
- **Redraw:** `tick_agent_ui` (wm/agent_state.rs:465-482) returns early with only `traffic` when the Navigator drawer is not visible; a new window showing live state needs its own `changed` return there (otherwise its rows go stale).

---

## 3. Current `:agents list` and agent state

- Parse: bare `:agents`, `:agents list|ls|status` all -> `AgentsOp::List` [V src/colon_command/parse_agents.rs:187-191]. Executor `list(wm)` [V execute_agents.rs:229-298] refreshes `update_agent_states` if never run, builds a JSON blob (`agents[]` with `pane_id,name,state,since_ms,profile,reported,partners[]`, `counts`, `traffic`) via `set_command_data`, and sets ONE status message `"Agents: N working · ..."`. No window.
- Contract tests: `tests/phase50_agent_state.rs:38,48,339,346`, `tests/phase50_traffic.rs:51` run `:agents list` and assert on data; `docs/API.md:883-885` documents it as read-only data. `tests/phase50_agents_window.rs:509-519,598,755-790` enforce, for every `AgentsOp`, a parseable colon form (`parsed == Action::Agents(op)`), a completion-row verb, a `HELP_LINES` row, and for every `AGENTS_WINDOW_BINDINGS` row a colon twin. **Adding variants to `AgentsOp` breaks `agents_op_colon_form`'s exhaustive match** (it has no wildcard).
- Row source: `agent_statuses()` -> `Vec<AgentStatus {pane_id, name, state, since_ms, profile: Option<String>, reported}>` in pane-id order [V wm/agent_state.rs:138-147, 397-411]; `agent_counts()` :415; `agent_state(id)` :393. `AgentState {Working, Blocked, Idle, Done, Errored}` with `as_str()` and `is_attention()` [V agent_state.rs:33-58]. `name` = `identity_token(id, running_command, title)` (default `#<id>`) [V agent_state.rs:353, PaneHistory::new]. Every tracked pane has a state (plain shells read `Idle`); a *detected* agent has `profile.is_some() || reported`. State colours: `dot_color(state, &DesignTokens)` (ui/drawer/navigator.rs:164-172); glyph sets `NavigatorGlyphs` (navigator.rs ~150).
- Jumping to a pane: `wm.focus_and_raise(id)` (wm/mod.rs:737); the existing `activate()` does it then `dismiss()` (execute_agents.rs:200-224).

---

## 4. Recommended design: agents-list window

### Decisions
- **D1 `:agents list` behaviour.** Keep `AgentsOp::List` parse unchanged and make its executor *additionally* open the list window (focused). RPC/`ctl` data contract and all existing tests stay green; zero churn in `phase50_agents_window.rs`. Side effect: RPC `:agents list` (and bare `:agents`, `status`) pops a window in a live UI. Alternative (not recommended): new variant + edit exhaustive test.
- **D2 Separate op family**, not `AgentsOp` growth: new `Action::AgentsList(AgentsListOp)` so `AgentsOp` parity tests, `AGENTS_WINDOW_BINDINGS` and plan 51-A's `AgentsOp::DemoStop` stay untouched.
- **D5 Independent scale:** new `PanelTarget::AgentsList` (spec lists "scale"; drawer/MCP/traffic each scale independently).
- Rows = every tracked pane (same population as Navigator); `filter agents` shows only detected agents (`profile||reported`). Sort by pane id. Selection keyed by `PaneId` (survives re-sorts and pane churn; drop to nearest if the pane closes). [A: confirm default filter with user; "listing only agents" means no `user/api/mcp/session` nodes, which panes-only already guarantees.]

### State (`src/wm/agents_list.rs`)
```rust
pub struct AgentsListState {
    pub active: bool, pub pinned: bool, pub focused: bool,
    pub rect: Option<Rect>,          // None = default (right-aligned, 36x(min(rows+4, 70% ws)))
    pub docked: bool,                // set by auto_arrange; cleared by manual geometry
    pub selected: Option<PaneId>, pub scroll: usize,
    pub filter: ListFilter,          // All | Agents
    pub drag: Option<ListDrag>,      // Move{grab_dx} | Resize{edge, initial} (bookkeeping only)
}
```
`captures_input = active && focused`; `blur()`; `open()` focuses it and blurs the traffic window (and vice versa) so only one window owns the keyboard. `WindowManager` accessors `agents_list()/agents_list_mut()`, `agents_list_rows()` (derived on demand from `agent_statuses()` + filter; no cache), `agents_list_rect(ws)`, `agents_list_layout(ws) -> Option<AgentsListRects{outer,inner,title_row,header,rows_area,footer}>` and `agents_list_hit(ws,col,row) -> ListHit {Outside,Title,PinBadge,Border(edge),Row(PaneId),Header,Footer}` (single source for renderer + mouse, like `agents_pane_rects`/`agents_hit`).

### Ops (`src/wm/agents_list_cmd.rs`, contract defined in wave 0)
`AgentsListOp`: `Open`, `Close`, `Toggle`, `Dismiss` (Esc: close if unpinned, release keyboard if pinned), `Blur`, `Step(i16)` (Up/Down), `First`, `Last`, `Page(i16)`, `SelectPane(PaneId)` (click), `Jump(Option<PaneId>)` (Enter/double-click: `focus_and_raise`, then dismiss unless pinned), `Zoom(Option<PaneId>)` (bridge to `AgentsOp::Zoom(Some(id))`: open the traffic window on that node), `Pin(Option<bool>)`, `ScaleStep(i16)` (absolute stays `PanelScaleOf(AgentsList, n)`), `Filter(ListFilter)`, `Dock(Option<bool>)` (`float` = undock), `Move{dx,dy}`, `Resize{dw,dh}`, `SetRect{x,y,w,h}`, `Size(Option<(u16,u16)>)`, `Scroll(i16)`.

Colon surface (all under `:agents list ...`; bare `:agents list` = open, D1): `close`, `toggle`, `pin [toggle]`, `unpin`, `select up|down|first|last|<pane-id>`, `jump [pane-id]`, `zoom [pane-id]`, `scale <75..200|+n|-n>`, `filter all|agents`, `dock|float`, `move dx dy`, `resize dw dh`, `rect x y w h`, `size CxR|auto`, `blur`. Parse hook goes before the `list|ls|status` arm in `parse_agents.rs:189-191`, only when `parts.len() > 2`.

### Keys (declarative table in NEW `src/input/agents_list_keys.rs`; `keymap.rs` is 909 lines)
Up/Down/`k`/`j` step, PageUp/PageDown, Home/End, Enter jump, `z` zoom to traffic window, `f` filter toggle, `*` pin, `[`/`]` scale -/+10, Shift+arrows move, Alt+arrows resize, `d` dock/float, Esc dismiss. Same handler contract as `handle_agents_window_key`: look up -> `wm.apply`, swallow other keys while focused, never consume the leader key. Optional global toggle chord: `Ctrl+Alt+L/H/K/J` are taken by `:move`/pane-move (keymap.rs:229-233, help_catalog.rs:259-262); pick a free one after a grep (suggest `Ctrl+Alt+N` [A]), also add menu View entry next to modals.rs:405 and (optionally) `reserved_global_action`.

### Mouse (NEW `src/input/agents_list.rs`)
Called at `input/mouse.rs:604-608` *before* the traffic-window handler (the list is the smaller, top-most panel; document the z-order). In its own `panel_space(.., PanelTarget::AgentsList)`. Events: press outside while focused -> `Blur` and pass through; click inside unfocused -> retake focus (`Open`); row click -> `SelectPane`; click on selected row or double-click -> `Jump`; title pin badge -> `Pin(None)`; zoom/close badges (`detect_title_bar_badge_click`) -> `Size(None)`-style maximize is NOT offered, close -> `Close`; title drag -> `SetRect` (clears `docked`); border/corner drag -> `SetRect` via `apply_rect_edge_resize`; wheel -> `Scroll(+-3)`; Ctrl+wheel -> `ScaleStep` (`ctrl_wheel_scale_delta`); right-click row -> `Zoom(Some(id))`. All gestures lower to `Action::AgentsList`; only `ListDrag` bookkeeping lives in the handler.

### Render
- TUI: new `ui/agents_list_ui.rs::render_agents_list(wm, ws, cfg, palette, glyphs, buf)` modelled on `render_agents_window`: shadow/halo, `occlude_rect`, fill, `render_borland_frame_styled(.., "AGENTS", captures_input, ..)`, pin badge (`pin_badge_glyph`; badge-x fn own to the list title), header `STATE  NAME  PANE  SINCE` + counts line (`agent_counts`), one row per agent: coloured state dot via `dot_color` + `AgentState::as_str`, sanitised name (`sanitize_display`), `#id`, `age_text`, selected row reverse video, `*` marker on the active pane, scroll indicator, footer hints. Called in `ui/mod.rs` next to the traffic window (:720), `if scale_pct(AgentsList) == 100`.
- GUI: text only, **no textures**. Needed seams: (1) `renderer.rs` scaled branch (:770 pattern) draws the list into a scratch grid + `draw_panel(&scratch, vr, scale)`; (2) add `agents_list_rect` to `real_rect`/`is_overlay_cell`/`cell_in_scaled_panel` (renderer.rs:534-598); (3) add list rect to `overlay_rects` and the pixel-rect helper in `agents_gui.rs:113-135` so graph/icon textures do not paint over it; (4) `gui_route.rs:50-58` `over_scalable_panel`; (5) `tick_agent_ui` returns `changed` while `agents_list.active`.

### Tiling participation
See section 1: `docked` flag + `panel_dock.rs`; `Dock(Some(true))` from `:agents list dock` places it in the strip without running a full arrange (calls the same pure function and `resize_all`).

---

## 5. Pitfalls
1. `AgentsOp` exhaustive match in `tests/phase50_agents_window.rs:508-519` and the binding-parity test (:755-790): do not add `AgentsOp` variants for the list window (D2).
2. `agents_window.rs` is 1167 lines (backstop 1200) and `layout_profile.rs` 979, `wm/drawer.rs` 1071, `keymap.rs` 909, `renderer.rs` 901, `mouse_wm.rs` 898: new logic goes in new files; hooks in these files stay <= ~5 lines each (traffic-window `docked` field + default = 2 lines in agents_window.rs).
3. Scaled panels live in a virtual grid: docked slots (real cells) must be converted to virtual; mouse math must go through `panel_space` for the right `PanelTarget`.
4. `effective_workspace_area` is called on every layout/hit-test; keep `subtract_docked_panels` O(1) and side-effect free; no stored absolute rects.
5. `focus_and_raise`/`shuffle_deck_*` must also `blur()` the list window or typing after a jump stays captured.
6. Stale rows: `tick_agent_ui` early return (see section 2 Redraw).
7. Opening the list must `blur` the traffic window and vice versa; both `captures_input` true would double-route keys (modal.rs:334-341 order).
8. Untrusted text: agent names come from running commands/titles; always `sanitize_display` (as `snoop_line_text` does, agents_ui.rs:80-92).
9. New `Action` variant requires entries in the two example tables in `action.rs` (:386 colon-form fallback and :599 sample list) and the UIT-02 palette/exclusion guard (cross-topic: Plan 51-C must know `AgentsList` verbs exist).
10. Doc gate: any new verb added to the `:agents` completion row (`src/pane/completion.rs:479`) must appear in user-guide chapter 11.

---

## 6. Work split (parallel plans, DISJOINT files)

### Wave 0, orchestrator-only seam commit (small, one agent, must land first)
Owns every shared/hot file edit, all as thin stubs with FINAL signatures so waves compile independently:
- `src/action.rs`: `Action::AgentsList(AgentsListOp)` + the two example tables.
- `src/wm/agents_list_cmd.rs`: full `AgentsListOp`, `ListFilter` (final enums = the contract).
- `src/wm/mod.rs`: `pub mod agents_list; pub mod panel_dock;`, field `agents_list`, `agents_list_scale_pct`, blur calls (:737-765).
- `src/wm/agents_list.rs`, `src/wm/panel_dock.rs`, `src/ui/agents_list_ui.rs`, `src/input/agents_list.rs`, `src/input/agents_list_keys.rs`, `src/colon_command/{parse,execute}_agents_list.rs`: stub files with final pub signatures (`agents_list_rect/layout/hit/rows`, `panel_dock_slot`, `subtract_docked_panels`, `render_agents_list`, `handle_agents_list_key/mouse`).
- `src/ui/typography.rs` + `execute_wm.rs`: `PanelTarget::AgentsList` arms; `execute_wm.rs:835` arm for `Action::AgentsList`.
- One-line hook calls: `modal.rs` (~:337), `mouse.rs` (~:606), `gui_route.rs:55`, `ui/mod.rs:720`, `renderer.rs` (:534-598, :770), `agents_gui.rs:113`, `wm/drawer.rs:401`, `layout_profile.rs:~310`, `parse_agents.rs:189`, `execute_agents.rs` `List` arm.

### Wave 1, five plans in parallel (each owns its files exclusively)
| Plan | Owns (exclusive) | Delivers |
|---|---|---|
| **51-B1 core** | `src/wm/agents_list.rs`, `src/wm/agent_state.rs` (tick tweak only), `tests/phase51_agents_list_core.rs` | state, rows/filter/selection, rect+layout+hit, open/close/pin/blur/jump/zoom/scale/geometry methods, headless unit tests |
| **51-B2 render** | `src/ui/agents_list_ui.rs`, `src/ui/agents_gui.rs`, `src/ui/renderer.rs`, `src/ui/mod.rs`, `tests/phase51_agents_list_render.rs` | TUI + GUI (scaled panel path, occluders, snapshot via `buffer_to_snapshot_string`), Legacy goldens untouched |
| **51-B3 input** | `src/input/agents_list.rs`, `src/input/agents_list_keys.rs`, `src/input/gui_route.rs`, `src/input/modal.rs`, `src/input/mouse.rs`, `tests/phase51_agents_list_input.rs` | key table + handler, mouse handler incl. drag/resize/wheel/badges, Esc/pin semantics, non-modal pass-through tests in the style of `tests/phase50_agents_nonmodal.rs` |
| **51-B4 commands/docs** | `src/colon_command/parse_agents_list.rs`, `src/colon_command/execute_agents_list.rs`, `src/colon_command/parse_agents.rs`, `src/colon_command/execute_agents.rs`, `src/pane/completion.rs`, `src/ui/help_catalog.rs`, `src/ui/modals.rs`, `docs/user-guide/11-*.md`, `docs/API.md`, `tests/phase51_agents_list_cmd.rs` | parse/execute every verb, `:agents list` also opens window, completion + help + menu, docs gate green |
| **51-B5 tiling** | `src/wm/panel_dock.rs`, `src/wm/layout_profile.rs`, `src/wm/drawer.rs`, `src/wm/agents_window.rs`, `src/wm/agents_geometry.rs`, `tests/phase51_panel_tiling.rs` | dock strip, `auto_arrange_windows` integration, traffic-window `docked`, MCP -> Push on arrange, undock on manual geometry; tests: arrange with no panels is unchanged, with list/traffic open panes+panels do not overlap, resize keeps tiling, scale 125% fills slot |

Cross-plan conflicts avoided by: B1 owns the list getters but calls `panel_dock_slot` (stub returns `None` until B5); B2/B3 consume B1's stub signatures; B4 never edits `AgentsOp`/`agents_cmd.rs` (Plan 51-A owns that for `DemoStop`).

### Wave 2 (orchestrator): merge, `scripts/verify.sh`, `scripts/check_phase50_doc_coverage.sh`, `scripts/check_classic_goldens.sh`, UIT-02 palette guard reconciliation with Plan 51-C.

## 7. Open questions for the user / orchestrator
1. D1: is it acceptable that RPC/`ctl` `:agents list` now also opens a window (alternative: window only for interactive `:agents list`, data-only for `status`)?
2. D3: should `:layout tile` (mode switch) also dock panels, or only the arrange family?
3. D4: leave the drawer's persisted Push/Overlay config untouched by arrange? (recommended yes)
4. Default list filter: all tracked panes (recommended) vs detected agents only.
5. Free global chord for toggling the list window (needs grep; `Ctrl+Alt+N` unverified).
