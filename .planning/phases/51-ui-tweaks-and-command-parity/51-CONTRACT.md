# Phase 51 Frozen Contract (UIT-01..04, UNI-R01)

Authored by the planner from 51-CONTEXT.md (D-01..D-04), 51-RESEARCH-A/B/C and AGENTS.md conventions 1-6. Every Phase 51 plan reads this file. Plans 51-01 and 51-02 (the seam) turn sections 2-3 into code; plans 51-03..51-10 build against the seam and may not change a name, signature or key here. A needed change goes to the orchestrator, not into a plan.

Wave map (GSD numbering starts at 1; the brief's "Wave 0 seam" is waves 1-2):

| Wave | Plans | Executor |
|------|-------|----------|
| 1 | 51-01 tracer + command contract | orchestrator only |
| 2 | 51-02 geometry, dock and demo seams | orchestrator only |
| 3 | 51-03 demo stop, 51-04 palette parity, 51-05 list core, 51-06 list render, 51-07 list input, 51-08 list commands and menu, 51-09 tiling, 51-10 RPC/MCP | 8 parallel subagents, disjoint files |
| 4 | 51-11 docs, 51-12 uniformity parity matrix | 2 parallel subagents |
| 5 | 51-13 final gate | orchestrator |

## 1. Decisions (from 51-CONTEXT.md "Planning defaults")

- **D-01 demo stop.** Re-running `:agents demo` REPLACES the prior demo events (and topology pairs); `:agents demo stop` closes only panes the demo spawned, removes `[demo]` events, purges the demo topology pairs, and is allowed over RPC/MCP with no new gate. Demo events are recorded unredacted (`record_traffic_with(.., false, ..)`) so the prefix match works.
- **D-02 palette.** Explicit palette entries for fixed sub-verb sets (incl. `drawer style navigator|classic`, `drawer rows 1|2`); exclusions live in `PALETTE_EXCLUSIONS` with a reason; the 13 `:window theme <preset>` entries are skipped (picker covers them: reason `reachable-via-picker`). `src/input/modal.rs` Enter-prefill behavior is NOT changed (follow-up needing sign-off).
- **D-03 agents list.** Separate `Action::AgentsList(AgentsListOp)` and `PanelTarget::AgentsList`; `AgentsOp::List` is kept unchanged at parse level and its executor also opens the window; default rows = every tracked pane (`filter agents` narrows to detected agents); global toggle chord `Ctrl+Alt+N` (verified free by grep of `DEFAULT_GLOBAL_BINDINGS`, `DEFAULT_PANE_BINDINGS`, `reserved_global_action`; plan 51-07 re-checks and, only on a real conflict, falls back to `Ctrl+Alt+G` and records it).
- **D-04 tiling.** Only the `:arrange` / `:layout grid|columns|rows|main-side|main-top|stack-grid|arrange` family (`auto_arrange_windows`) docks open panels (agents list + agents traffic into a dock strip; MCP console via its existing Push layout). `:layout tile|stack|cycle`, `Alt+M`, and the drawer's persisted Push/Overlay config are untouched. With no panel open the output is byte-identical.
- **UNI-R01 (AGENTS.md convention 6).** Every capability below lists its mouse path, its key sequence working in GUI and TUI, its colon + palette + help + RPC/HTTP/MCP/`bilterm ctl` path, all lowering to `Action` via `WindowManager::apply`, with parity tests (plan 51-12 consolidates; per-plan tests cover their slice). Deliberate exceptions are in section 9.

## 1b. Sequencing assumption (Phase 50.1)

The ROADMAP inserts Phase 50.1 (Unified Registry and Find: `:find`, `:finder`, `find.*` RPC, `bilterm_find` MCP tool, View-menu find entries, `Ctrl+Alt+F`, its own doc gate script) AHEAD of Phase 51, so it executes first. Phase 51 plans therefore re-grep every line number and every count at execution time and tolerate its additions: the pinned MCP tool count is read from the tests and raised by two (51-10), the menu tables are compared against their state at the start of 51-08, the palette sweep (51-04) also covers the `:find` / `:finder` verbs (a missing entry becomes a palette entry or a `PALETTE_EXCLUSIONS` row with a reason, in 51-04's own files), and `scripts/check_phase50_doc_coverage.sh` is left alone by 50.1 and extended only by 51-13. No Phase 51 key collides with its chords (`Ctrl+Alt+F`; Find has no leader chord, leader `?` is HelpToggle). The leader chords `A`, `y`, `Y` were verified free in `DEFAULT_LEADER_BINDINGS`, and 51-07's uniqueness test reads the leader table live.

## 2. Frozen types and signatures

All new `pub` items are re-exported through their module only (no `lib.rs` change). Existing visibility conventions: window state fields `pub(crate)`, demo state private.

### 2.1 `src/wm/agents_list_cmd.rs` (created by 51-01, never edited afterwards)
- `ListFilter { All, Agents }` (`Default` = `All`, `Copy`).
- `AgentsListOp` (`Clone, Debug, PartialEq, Eq`):
  `Open`, `Close`, `Toggle`, `Dismiss`, `Blur`, `Step(i16)`, `First`, `Last`, `Page(i16)`, `SelectPane(PaneId)`, `Jump(Option<PaneId>)`, `Zoom(Option<PaneId>)`, `Pin(Option<bool>)`, `ScaleStep(i16)`, `Filter(Option<ListFilter>)`, `Dock(Option<bool>)`, `Move { dx: i16, dy: i16 }`, `Resize { dw: i16, dh: i16 }`, `SetRect { x: u16, y: u16, w: u16, h: u16 }`, `Size(Option<(u16, u16)>)`, `Scroll(i16)`.
  `None` on `Pin`/`Filter`/`Dock` means toggle. Absolute scale reuses `Action::PanelScaleOf(PanelTarget::AgentsList, n)`.
- `impl AgentsListOp { pub fn colon_form(&self) -> String }` returns the canonical `:agents list ...` string of section 3.
- `Action::AgentsList(AgentsListOp)` in `src/action.rs`, colon example `:agents list toggle`.
- `AgentsOp::DemoStop` in `src/wm/agents_cmd.rs`, colon form `:agents demo stop`.
- `PanelTarget::AgentsList` in `src/ui/typography.rs`; `WindowManager::agents_list_scale_pct` (`pub(crate)`, init 100); `panel_name` -> `"Agents list"`; `:panel scale n` sets it too.

### 2.2 `src/wm/agents_list.rs` (created by 51-01; owned by 51-05 afterwards)
- `AgentsListState { active, pinned, focused: bool, rect: Option<Rect>, docked: bool, selected: Option<PaneId>, scroll: usize, filter: ListFilter, drag: Option<ListDrag> }`, `Default`, `captures_input() = active && focused`, `blur()`.
- `ListDrag { Move { grab_dx: u16, grab_dy: u16 }, Resize { edge: WindowResizeEdge, initial: Rect } }` (bookkeeping only, like `AgentsDrag`).
- `AgentsListRow { pane_id: PaneId, name: String, state: AgentState, since_ms: u64, is_agent: bool, is_active_pane: bool }`.
- `AgentsListSummary { active, pinned, focused, docked: bool, filter: ListFilter, selected: Option<PaneId>, row_count: usize, scale_pct: u16 }`.
- `impl WindowManager`: `agents_list()`, `agents_list_mut()`, `open_agents_list()` (sets active + focused, blurs the traffic window), `close_agents_list()`, `agents_list_rows(&self) -> Vec<AgentsListRow>` (pane-id order, filter applied, derived on demand), `agents_list_apply(&mut self, op: &AgentsListOp) -> Option<String>` (exhaustive `match`, no wildcard; returns an optional status message), `agents_list_summary(&self) -> AgentsListSummary`.
- `WindowManager.agents_list: AgentsListState` is `pub(crate)` (precedent: `agents_window`).
- Only one of the traffic window and the list window owns the keyboard: `open_agents_list` blurs the traffic window and `open_agents_window` blurs the list; `focus_and_raise` and `shuffle_deck_*` blur both.

### 2.3 `src/wm/agents_list_geometry.rs` (created by 51-02; FROZEN afterwards, bug reports go to the orchestrator)
- `ListChip { Filter, Dock, Zoom }`, `ListBadge { Pin, Minimize, Dock, Close }`.
- `ListHit { Outside, Title, Badge(ListBadge), Border(WindowResizeEdge), Header, Row(PaneId), Chip(ListChip), Body, Footer }`.
- `AgentsListRects { outer, inner, title_row, header, rows_area, footer: Rect, visible_rows: usize, badges: Vec<(ListBadge, Rect)>, chips: Vec<(ListChip, Rect)> }`.
- `impl WindowManager`: `agents_list_rect(&self, ws: Rect) -> Option<Rect>` (`None` when inactive or the workspace is under 24x6; default 36 columns wide (never wider than the workspace), height `min(rows + 4, 70% of ws height)` with a floor of 6, right-aligned one column in from the right edge, one row below the top; `rect` override clamped into `ws`; `docked` uses `panel_dock_slot`), `agents_list_layout(&self, ws: Rect) -> Option<AgentsListRects>`, `agents_list_hit(&self, ws: Rect, col: u16, row: u16) -> ListHit`. All coordinates are in the panel's own (scaled virtual) space: callers pass `panel_virtual_area` / `panel_space` results exactly like the traffic window.
- Title: `AGENTS`. Title badges follow the same cell formulas as `detect_title_bar_badge_click` (trio `[_][□][X]`) and `agents_pin_badge_x`: `Minimize` -> `Dismiss`, `Dock` (the zoom position) -> `Dock(None)`, `Close` -> `Close`, `Pin` -> `Pin(None)`. Footer chips (clickable): `[filter]` `[dock]` `[zoom]`.

### 2.4 `src/wm/panel_dock.rs` (created by 51-02 as identity stubs; owned by 51-09 afterwards)
- `DockPanel { AgentsList, AgentsTraffic }`.
- `impl WindowManager` (all `pub(crate)`): `subtract_docked_panels(&self, area: Rect) -> Rect` (real cells, identity stub), `panel_dock_slot(&self, ws: Rect, panel: DockPanel) -> Option<Rect>` (`ws` and the result are in the panel's own space; `None` stub), `dock_panels_on_arrange(&mut self)` (no-op stub), `set_panel_docked(&mut self, panel: DockPanel, on: bool) -> bool` (the ONLY place that flips a `docked` flag: returns whether it changed, refuses `on` for an inactive panel or when there is no room, and re-flows the panes with `resize_all`; every release path (manual geometry, close, `float`) calls it with `false` and never writes the flag directly; the 51-02 stub flips the `AgentsList` flag only, `AgentsTraffic` returns `false`, no reflow).
- Hooks added by 51-02: last line of `effective_workspace_area` (`wm/drawer.rs`) calls `subtract_docked_panels`; `auto_arrange_windows` (`wm/layout_profile.rs`) calls `dock_panels_on_arrange()` immediately before `resize_all`.

### 2.5 `src/wm/agents_demo.rs` (created by 51-02; owned by 51-03 afterwards)
- Private `AgentsDemoState { endpoints: Vec<PaneId>, spawned: Vec<PaneId> }` field `agents_demo` on `WindowManager` (ARCH-R02: private).
- `AgentsDemoSummary { running: bool, endpoints: Vec<PaneId>, spawned: Vec<PaneId> }`; `WindowManager::agents_demo_summary(&self)`, `WindowManager::clear_agents_demo(&mut self)`.
- Hooks added by 51-02: `clear_agents_demo()` in `create_new_session` (`wm/session_manager.rs`) and `apply_saved_session_snapshot` (`wm/layout_profile.rs`) because pane ids can be reused there.
- `execute_agents_topology::demo_stop(wm)` is a seam stub (status `No agent demo to stop`); 51-03 replaces it.

### 2.6 Seam-owned hook rows (never touched by a wave-3 plan)
`src/action.rs`, `src/wm/mod.rs`, `src/wm/drawer.rs` (the one hook line), `src/ui/typography.rs`, `src/colon_command/execute_wm.rs`, `src/colon_command/mod.rs`, `src/colon_command/parse_agents.rs`, `src/ui/mod.rs`, `src/wm/agents_cmd.rs`, `src/wm/session_manager.rs`, `src/wm/agents_list_cmd.rs`, `src/wm/agents_list_geometry.rs`. A wave-3 plan that needs an edit here reports it.

## 3. Colon grammar (frozen; implemented in `parse_agents_list.rs` by 51-01)

`:agents list ...` (aliases `ls` for `list`; `status` stays data-only `List`):

| Colon | Action |
|-------|--------|
| `:agents`, `:agents list`, `:agents ls`, `:agents status` | `Agents(AgentsOp::List)` (executor also opens the window, D-03) |
| `:agents list open` / `close` / `toggle` / `dismiss` / `blur` | `AgentsList(Open / Close / Toggle / Dismiss / Blur)` |
| `:agents list select up` / `down` | `Step(-1)` / `Step(1)` |
| `:agents list select first` / `last` | `First` / `Last` |
| `:agents list select page-up` / `page-down` | `Page(-1)` / `Page(1)` |
| `:agents list select <pane-id>` (`#3` accepted) | `SelectPane(id)` |
| `:agents list jump [pane-id]` | `Jump(None / Some(id))` |
| `:agents list zoom [pane-id]` | `Zoom(None / Some(id))` |
| `:agents list pin` / `unpin` / `pin toggle` | `Pin(Some(true))` / `Pin(Some(false))` / `Pin(None)` |
| `:agents list scale +n` / `-n` | `ScaleStep(±n)` |
| `:agents list scale <75..200>[%]` | `PanelScaleOf(PanelTarget::AgentsList, n)` |
| `:agents list filter all` / `agents` / `toggle` | `Filter(Some(All))` / `Filter(Some(Agents))` / `Filter(None)` |
| `:agents list dock` / `dock toggle` / `float` | `Dock(Some(true))` / `Dock(None)` / `Dock(Some(false))` |
| `:agents list move <dx> <dy>` | `Move` |
| `:agents list resize <dw> <dh>` | `Resize` |
| `:agents list rect <x> <y> <w> <h>` | `SetRect` |
| `:agents list size <cols>x<rows>` / `size auto` | `Size(Some((c, r)))` / `Size(None)` |
| `:agents list scroll <±n>` | `Scroll(n)` |
| `:agents demo stop` | `Agents(AgentsOp::DemoStop)` |

Anything else under `:agents list` or `:agents demo` is a usage `Err` (no panic, no partial parse). `colon_form()` is the inverse for every variant (`Pin(None)` -> `:agents list pin toggle`).

Completion row `(":agents", ...)` gains these entries (51-04): `list open`, `list close`, `list toggle`, `list dismiss`, `list blur`, `list select `, `list jump`, `list zoom`, `list pin`, `list unpin`, `list scale `, `list filter `, `list dock`, `list float`, `list move `, `list resize `, `list rect `, `list size `, `list scroll `, `demo stop`. (No brackets in entries: `scripts/check_phase50_doc_coverage.sh` regex.)

## 4. Keys (GUI and TUI)

| Key | Action | Table | Plan |
|-----|--------|-------|------|
| `Ctrl+Alt+N` (and `Ctrl+Alt+Shift+N`) | `AgentsList(Toggle)` | `DEFAULT_GLOBAL_BINDINGS` + `reserved_global_action` (raw panes) | 51-07 |
| leader `A` | `AgentsList(Toggle)` | `DEFAULT_LEADER_BINDINGS` | 51-07 |
| leader `y` | `Agents(Demo)` | `DEFAULT_LEADER_BINDINGS` | 51-07 |
| leader `Y` | `Agents(DemoStop)` | `DEFAULT_LEADER_BINDINGS` | 51-07 |

Leader chords work in GUI and TUI with no modifier-protocol dependence (the terminal path for hosts that swallow `Ctrl+Alt+N`). In-window keys (list focused; table `AGENTS_LIST_BINDINGS` in `src/input/agents_list_keys.rs`; the leader key is never consumed; every other key is swallowed while focused):

| Key | Action |
|-----|--------|
| Up, `k` / Down, `j` | `Step(-1)` / `Step(1)` |
| PageUp / PageDown | `Page(-1)` / `Page(1)` |
| Home / End | `First` / `Last` |
| Enter | `Jump(None)` |
| `z` | `Zoom(None)` |
| `f` | `Filter(None)` |
| `*` | `Pin(None)` |
| `[` / `]` | `ScaleStep(-10)` / `ScaleStep(10)` |
| Shift+arrows | `Move` by 1 cell |
| Alt+arrows | `Resize` by 1 cell |
| `d` | `Dock(None)` |
| Esc | `Dismiss` |
| `Ctrl+Alt+N` while focused | `Toggle` (closes it, like `Ctrl+Alt+A` for the traffic window) |

## 5. Mouse (GUI and TUI; handler `src/input/agents_list.rs`, called before the traffic-window handler at `mouse.rs`, so the list is the top-most panel)

All coordinates go through `panel_space(.., PanelTarget::AgentsList)`.

| Gesture | Action |
|---------|--------|
| click a row (not selected) | `SelectPane(id)` (an unfocused window first gets `Open`, which retakes focus) |
| click the selected row again | `Jump(Some(id))` |
| right-click a row | `Zoom(Some(id))` |
| title badge `Pin` / `Minimize` / `Dock` / `Close` | `Pin(None)` / `Dismiss` / `Dock(None)` / `Close` |
| footer chip `[filter]` / `[dock]` / `[zoom]` | `Filter(None)` / `Dock(None)` / `Zoom(None)` |
| drag the title | `SetRect` per motion (clears `docked`) |
| drag a border or corner | `SetRect` per motion via `apply_rect_edge_resize` |
| click header, body or footer inside an unfocused window | `Open` |
| press outside while focused | `Blur`, then the event passes through |
| wheel inside | `Scroll(∓3)` |
| Ctrl+wheel inside | `ScaleStep(±10)` via `ctrl_wheel_scale_delta` |

Menu bar (the mouse path for global open, and for the rest of the phase's capabilities), added by 51-08 to `MENUBAR_CATEGORIES`: View: `Agents List (Ctrl+Alt+N)` -> `:agents list toggle`, `Agent Demo (Ctrl+X y)` -> `:agents demo`, `Stop Agent Demo (Ctrl+X Y)` -> `:agents demo stop`; Window: `Tile Windows & Panels` -> `:arrange grid`.

## 6. RPC / HTTP / MCP / ctl (plan 51-10; write gate: none, these are window and demo operations, `[api] allow_config_write` not involved)

| Surface | Name | Params | Lowers to |
|---------|------|--------|-----------|
| RPC / HTTP `POST /rpc` / `bilterm ctl` | `agents.list` | `{"op"?: string, "args"?: [string or int] or one whitespace-separated string}`; `op` absent or `list` = data read (also opens, D-03); otherwise one of `open close toggle dismiss blur select jump zoom pin unpin scale filter dock float move resize rect size scroll` | `:agents list [op args...]` -> `parse_colon_command` -> `apply_with_policy(.., Confined)` |
| RPC / HTTP / ctl | `agents.demo` | `{"stop"?: bool}` | `:agents demo` / `:agents demo stop` |
| MCP tool | `bilterm_agents_list` -> `agents.list`, `bilterm_agents_demo` -> `agents.demo` | same | same (tool count plus two: pinned in three existing tests that 51-10 updates) |

Token hygiene: `op` must be in the closed set above; each arg token matches `[A-Za-z0-9+\-_.#%:]` and is at most 32 chars, at most 8 args; anything else is `invalid agents.list parameter` (no second command can ride in an argument). Result: the `wm.command` result object plus `"agents_list": {active, pinned, focused, docked, filter, selected, row_count, scale_pct}` or `"demo": {running, endpoints, spawned}`. `wm.command` and `bilterm_colon_command` keep working for every verb (existing path, unchanged).

## 7. Palette, help, completion (plan 51-04 is the single owner of `PALETTE_CATALOG`, `HELP_LINES`, the `(":agents", ..)` and `(":drawer", ..)` completion rows)

Palette entries for the new verbs: `:agents demo`, `:agents demo stop`, `:agents list` (exists), `list open`, `list close`, `list toggle`, `list dismiss`, `list blur`, `list pin`, `list unpin`, `list select down|up|first|last`, `list jump`, `list zoom`, `list filter agents|all`, `list dock`, `list float`, `list scale +10|-10`, `list move -1 0`, `list resize 1 0`, `list size auto`, `list scroll -3` (each a runnable, parsing example; the `rect` form is excluded as `numeric`, like the traffic window's). Help rows: one `HELP_LINES` row per `AgentsListOp` verb, one for `:agents demo stop`, plus two summary rows `Agents list keys` and `Agents list mouse` in the style of the `Navigator drawer keys/mouse` rows. The palette guard (51-04) enumerates every `COLON_COMMANDS` probe and requires a palette entry (same parsed `Action`) or a `PALETTE_EXCLUSIONS` row with a reason from the vocabulary: `free-text`, `host-or-path`, `entity-id`, `numeric`, `combinatorial`, `destructive`, `reachable-via-picker`, `short-alias`.

## 8. Tiling contract (plan 51-09)

`auto_arrange_windows` -> `dock_panels_on_arrange()`: every active eligible panel gets `docked = true` BEFORE `resize_all`; eligible = agents list, AGENT TRAFFIC window; the MCP console's `layout_mode` is set to Push; the drawer is untouched. A dock strip: workspace width >= 100 -> right strip of `clamp(ws.width/3, 28, 48)` columns with panels stacked vertically at equal height; narrower -> bottom strip of `max(6, ws.height/3)` rows with panels side by side; skipped (status message, panels stay floating) when the leftover pane area would be under 20x6. The strip is recomputed from the live workspace on every call (no stored absolute rects), so terminal resize keeps the tile. Docked slots are real cells; `panel_dock_slot` converts to the panel's scaled virtual space (`virtual = real * 100 / scale`). Any manual geometry (`Move`, `Resize`, `SetRect`, `Size`, drag, `float`) clears `docked`; closing the panel frees the strip. `set_panel_docked(AgentsTraffic, ..)` is reachable only from arrange (the traffic window has no dock verb; its existing geometry verbs clear it).

## 9. UNI-R01 capability matrix and exceptions

Legend: M = mouse, K = key (GUI+TUI), C = colon, P = palette, H = help row, R = RPC/HTTP/ctl, T = MCP. "wm.command" always works for every colon verb.

| Capability | M | K | C | P / H | R / T |
|------------|---|---|---|-------|-------|
| UIT-01 demo (start) | menu View > Agent Demo | leader `y` | `:agents demo` | 51-04 | `agents.demo`, `bilterm_agents_demo` (51-10) |
| UIT-01 demo stop | menu View > Stop Agent Demo | leader `Y` | `:agents demo stop` | 51-04 | `agents.demo {"stop":true}` |
| UIT-02 palette coverage | click an entry in the palette | `Ctrl+P`, `Alt+Space`, leader `Space`, then type | `:palette`, `:commands` | the table itself | `bilterm://commands` resource lists `palette_exclusion` per command (51-04); `wm.command ":commands"` |
| UIT-03 list open/close/toggle | menu View > Agents List; badge Close | `Ctrl+Alt+N`, leader `A` | `:agents list open|close|toggle`, bare `:agents list` | 51-04 | `agents.list` |
| list dismiss/blur | badge Minimize; click outside | Esc | `dismiss`, `blur` | 51-04 | `agents.list` |
| list select/step/page/first/last | click row; wheel scrolls | Up/Down/`j`/`k`/PgUp/PgDn/Home/End | `select ...` | 51-04 | `agents.list` |
| list jump | click selected row | Enter | `jump [id]` | 51-04 | `agents.list` |
| list zoom (open traffic window on pane) | right-click row; chip `[zoom]` | `z` | `zoom [id]` | 51-04 | `agents.list` |
| list pin/unpin | badge Pin | `*` | `pin|unpin|pin toggle` | 51-04 | `agents.list` |
| list scale | Ctrl+wheel | `[`, `]` | `scale +n|-n|<75..200>` | 51-04 | `agents.list` |
| list filter | chip `[filter]` | `f` | `filter all|agents|toggle` | 51-04 | `agents.list` |
| list move/resize/rect/size | drag title / border | Shift+arrows / Alt+arrows | `move resize rect size` | 51-04 | `agents.list` |
| list scroll | wheel | PgUp/PgDn (as `Page`) | `scroll ±n` | 51-04 | `agents.list` |
| UIT-04 dock/float the list | badge Dock, chip `[dock]` | `d` | `dock`, `float`, `dock toggle` | 51-04 | `agents.list` |
| UIT-04 tile everything (`:arrange`) | menu Window > Tile Windows & Panels | none (exception 4) | `:arrange grid|columns|rows|main-side|main-top|stack-grid|float-grid` | 51-04 (`:arrange` entries) | `wm.command ":arrange grid"` |

Exceptions (each recorded as `uniformity_exceptions` in the owning plan and asserted by name in 51-12):
1. `SelectPane(id)`, `Jump(Some(id))`, `Zoom(Some(id))` with an explicit pane id: no dedicated key (arrows + Enter act on the selection); reason `entity-id`.
2. `Size`, `SetRect`: no key (numeric); mouse is the edge/title drag, which lowers to `SetRect`; reason `numeric`.
3. Explicit-value toggle forms (`pin`/`unpin`, `filter all|agents`, `dock`/`float`): key and mouse send the toggle form (`None`); explicit forms are colon/RPC/palette only (same convention as Phase 50 `*` -> `Pin(None)`).
4. `:arrange <pattern>`: lowers to `Action::LayoutProfileCommand(String)`, which is not const-constructible and cannot sit in the const `KeyBinding` tables; key path is the palette (`Ctrl+P`, type `arrange`); follow-up: a const `Arrange(ArrangePattern)` Action in a later phase.
5. `Dismiss`: the mouse twin is the Minimize badge; `Page`: mouse twin is the wheel (`Scroll`).
6. `:agents list status` is data-only `List`: no key or mouse.

## 10. File ownership (disjoint within a wave)

Wave 3 (parallel). `Cargo.toml` and `Cargo.lock` are never touched in this phase.

| Plan | Exclusive files |
|------|-----------------|
| 51-03 demo stop | `src/wm/agents_demo.rs`, `src/wm/traffic.rs`, `src/wm/agents_topology.rs`, `src/colon_command/execute_agents_topology.rs`, `tests/phase51_demo_stop.rs` |
| 51-04 palette | `src/ui/help_catalog.rs`, `src/pane/completion.rs`, `src/command_registry.rs`, `src/api/mcp_resources.rs`, `tests/phase51_palette_parity.rs` |
| 51-05 list core | `src/wm/agents_list.rs`, `src/wm/agent_state.rs`, `tests/phase51_agents_list_core.rs` |
| 51-06 list render | `src/ui/agents_list_ui.rs`, `src/ui/agents_gui.rs`, `src/ui/renderer.rs`, `tests/phase51_agents_list_render.rs` |
| 51-07 list input | `src/input/agents_list_keys.rs`, `src/input/agents_list.rs`, `src/input/mod.rs`, `src/input/keymap.rs`, `src/input/modal.rs`, `src/input/mouse.rs`, `src/input/gui_route.rs`, `src/input/tests.rs`, `tests/phase51_agents_list_input.rs` |
| 51-08 list commands | `src/colon_command/parse_agents_list.rs`, `src/colon_command/execute_agents_list.rs`, `src/colon_command/execute_agents.rs`, `src/ui/modals.rs`, `tests/phase51_agents_list_cmd.rs` |
| 51-09 tiling | `src/wm/panel_dock.rs`, `src/wm/agents_window.rs`, `src/wm/agents_geometry.rs`, `tests/phase51_panel_tiling.rs` |
| 51-10 RPC/MCP | `src/api/rpc.rs`, `src/api/rpc_agents.rs`, `src/api/mcp.rs`, `src/api/mod.rs`, `tests/phase51_api_parity.rs`, `tests/v110_v120_v130_automation_api_mcp_playbooks.rs`, `tests/api_mcp_resources.rs`, `tests/api_error_paths.rs` |

Wave 4: 51-11 docs (`docs/user-guide/11-navigator-sidebar-and-agent-traffic.md`, `docs/user-guide/06-keyboard-mouse-and-command-reference.md`, `docs/user-guide/index.md`, `docs/API.md`, `docs/DEVELOPER_AND_AGENT_GUIDE.md`); 51-12 (`tests/phase51_uniformity.rs`, `tests/phase50_agents_window.rs`). Wave 5: 51-13 (planning artifacts, `scripts/check_phase50_doc_coverage.sh`, residual integration fixes).

`AGENTS.md` is not edited by any plan (the coordinator has uncommitted edits there); its file map is a follow-up for the orchestrator.

## 11. Rules for parallel executors

1. Edit only the files your plan lists. Never `git add -A`; `git add` your files by path and commit with `type(51-NN): ...`. Never revert or reformat another plan's file.
2. The tree is shared (`workflow.use_worktrees` is false). The crate must compile after each of your saves: build against the seam's stubs, never against another wave-3 plan's output. A compile error in a file you do not own: wait and retry once, then report; do not fix it.
3. Task checks use `cargo test -q --test <your_test>`; the closing check of a plan uses `scripts/verify.sh --only ...` only when run alone (it rewrites `target/verify/summary.tsv`). Recommended: the orchestrator runs each wave-3 plan in its own git worktree with its own `CARGO_TARGET_DIR` and merges (file sets are disjoint, so merges are conflict-free).
4. New logic goes in the new files named here; hooks in hot files stay at most 5 lines. Soft file goal 800-1000 lines, hard backstop 1200 (`tests/session_lessons_and_regression_suite.rs`). `agents_window.rs` is at 1167 lines: 51-09 may add at most 25.
5. Legacy output: `scripts/check_classic_goldens.sh` must print `GOLDENS OK 11` and `cargo test -q --test drawer_classic_parity` must pass after any plan touching rendering, geometry or the drawer.
6. Untrusted text (agent names come from running commands and titles) is rendered through `sanitize_display`.

## 12. Multi-source coverage audit (planner)

| Source | ID | Item | Plan(s) | Status |
|--------|----|------|---------|--------|
| GOAL | - | undo `:agents demo` | 51-01, 51-03 | COVERED |
| GOAL | - | palette entry for every colon verb | 51-04 | COVERED |
| GOAL | - | `:agents list` floating window that tiles | 51-01, 51-02, 51-05..51-09 | COVERED |
| REQ | UIT-01 | demo stop | 51-01, 51-03, 51-10 | COVERED |
| REQ | UIT-02 | palette parity, test fails on a new verb | 51-04 | COVERED |
| REQ | UIT-03 | list window: pin, key, mouse, verbs, TUI+GUI | 51-01, 51-02, 51-05..51-08, 51-10 | COVERED |
| REQ | UIT-04 | tiling includes list (and traffic, MCP) | 51-02, 51-09 | COVERED |
| REQ | UNI-R01 | mouse/key/colon/palette/help/API per capability + parity tests | all, 51-12 | COVERED |
| CONTEXT | D-01 | demo replace, unredacted, topology purge, no new gate | 51-03, 51-10 | COVERED |
| CONTEXT | D-02 | `PALETTE_EXCLUSIONS`, skip window-theme presets, no modal.rs change | 51-04 | COVERED |
| CONTEXT | D-03 | separate Action/PanelTarget, List also opens, all-panes default, Ctrl+Alt+N | 51-01, 51-05..51-08 | COVERED |
| CONTEXT | D-04 | arrange family only, no-panel byte-identical | 51-02, 51-09 | COVERED |
| RESEARCH A | - | probe-based parity test, exclusion vocabulary, anti-rot guards, EXTRA canonical cleanup, fixed sets enumerated, `arrange` + drawer sub-verbs in `COLON_COMMANDS`, tier-2 parser scan | 51-04 | COVERED |
| RESEARCH A | - | Enter-prefill UX change in `modal.rs` | - | EXCLUDED (D-02: separate sign-off, listed as follow-up) |
| RESEARCH B | - | tracking, rect, hit, render TUI+GUI, key/mouse tables, dock strip, scaled-slot conversion, tick redraw, blur hooks, GUI seams (renderer, agents_gui, gui_route) | 51-01, 51-02, 51-05..51-09 | COVERED |
| RESEARCH C | - | state in private field, spawned-only close, `retain_events`, `remove_pair`, UI reference cleanup, replace-on-rerun, API allowed + docs, test plan 1-12, stale phase comments | 51-01, 51-03, 51-10, 51-11 | COVERED |
| RESEARCH C | A1 | child shell terminated on pane drop | 51-03 reads `impl Drop for PtySession` and uses the `:close-pane` path | COVERED (verified in-plan) |
