# Phase 50: herdr-Style Left Sidebar (Agent-Aware Workspace Navigator) - Research

**Researched:** 2026-11-03
**Domain:** Rust / ratatui 0.30 TUI multiplexer (Bilterm): slide-out drawer re-layout, per-pane agent-state heuristics, internal traffic-event bus, resizable agent-traffic zoom window (graph + snoop), TUI braille and GUI RGBA-texture rendering
**Confidence:** HIGH on codebase seams and integration traps (every cited value was read this session); MEDIUM on heuristic thresholds and per-agent screen patterns (assumed, need user confirmation)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Scope split with Phase 51**
- **D-01:** Phase 50 builds the UI on a small internal traffic-event bus (who sent what to whom, with timestamps). Today it is fed by `:mcp broadcast` and control-API/MCP calls; Phase 51's `agent.send` etc. plug into the same bus. Graph and snoop work from day one. — **Reversibility:** costly — the event schema becomes a contract shared with Phase 51 and the API events (`agent.message`).
- **D-02:** The drawer's new layout is `[drawer] style = "classic" | "herdr"`; `classic` stays byte-identical (Legacy parity, snapshot-proven). Default stays `classic` until the user signs off.

**Agent state**
- **D-03:** State (working / blocked / idle / done / errored) is derived from heuristics on existing signals (OSC 133 phase, output activity, exit code, running command, waiting-for-input prompt patterns) plus optional per-agent profiles (`[agents.<name>]` for known tools such as claude, codex, gemini) carrying their own working/blocked patterns. Explicit `agent.state` reports (Phase 51) override heuristics.
- **D-04:** Per-workspace count summary and a jump-to-next-blocked key stay as in the roadmap success criteria.

**Glyphs and comms indicators**
- **D-05:** Unicode glyphs with an ASCII fallback set (for limited terminals and Legacy looks). In GUI mode the same semantic states are drawn as smooth vector icons with colour and subtle animation. — **Reversibility:** reversible.
- **D-06:** Each row shows a pairing glyph plus the partner's short name (pair), or a fan-out glyph with a count (fan-out); the glyph animates while traffic flows. Enter/click on it opens the zoom window focused on that edge.

**Traffic zoom window**
- **D-07:** Opened by key, mouse click, or command (`:agents zoom`), and from the comms glyph on a row. Behaves like the MCP window: drag-resize edges, pin, Ctrl+wheel scale (independent scale), Esc closes, adjustable splitter. Graph on one side, conversation on the other. Keys and mouse/scroll are equally capable.
- **D-08:** Layout is automatic: layered or force-directed, with stable node positions so nodes don't jump. TUI: arrow-key selection cursor over nodes/edges. GUI: nodes can be dragged. (Claude's discretion on the exact algorithm per mode.)
- **D-09:** Snoop view is read-only follow + filter (by agent, pair, keyword) + history scroll + copy, plus a "send as me" compose line that injects a message into the selected edge. Injection goes through the Phase 51 permission gate (`[api] allow_agent_messaging`), rate limits, loop guard and logging; without Phase 51 enabled the compose line is disabled. — **Reversibility:** costly — injection adds an outward-facing write path with security implications.

**Rendering**
- **D-10:** TUI uses box-drawing/braille graph rendering. GUI uses smooth, high-resolution vector graphics built on the existing pixel canvas (`pixel_canvas`, `hires_subcell`): anti-aliased curved edges, animated flow particles along edges, node glow for state, hi-DPI aware, good graph layout algorithms. No new heavy dependency without sign-off (a vector crate such as tiny-skia is an option the plan must justify against cross-platform builds).

### Claude's Discretion
Exact layout algorithm per mode, glyph choices within the semantic set, animation timings, splitter defaults, key bindings (must not collide with existing keys), and event-bus buffer sizes.

### Deferred Ideas (OUT OF SCOPE)
None - discussion stayed within phase scope. (Agent messaging itself stays in Phase 51.)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FLT-01 | The slide-out drawer offers a `herdr`-style left sidebar (`[drawer] style = "herdr"`): workspace > session > pane tree, per-pane agent-state dots (working / blocked / idle / done / errored), attention counts and jump-to-next-blocked, keyboard- and mouse-driven, Legacy output unchanged when not selected. (REQUIREMENTS.md:194) | Sections "Drawer integration seams", "AgentState detector", "Keys/mouse", "Classic parity proof", "Validation Architecture". D-01, D-06..D-10 (bus + zoom window) have **no requirement ID** - see Open Question 1. |
</phase_requirements>

## Summary

The existing drawer is a single code path: `WindowManager::drawer_rows()` (src/wm/drawer.rs:402) builds a flat `Vec<DrawerRow>`, `render_workspace_drawer` (src/ui/drawer.rs:147) draws it, and `selected_index` is an index into that same vector for **every** key and mouse handler (modal.rs:334-492, mouse_wm.rs:672-886). The safest herdr implementation is therefore: add `DrawerConfig.style`, make `drawer_rows()` dispatch on style (classic body untouched), reuse `DrawerRowKind::SessionHeader`/`Pane` for the new tree rows so all index-based helpers keep working, and branch once at the top of the renderer and once at the top of the mouse/key handlers. All new logic goes into new modules because `src/wm/drawer.rs` is 1015 lines, `src/input/modal.rs` 925, `src/input/mouse_wm.rs` 887, `src/colon_command/execute_wm.rs` 853, `src/wm/mod.rs` 865 against a hard test backstop of 1200 [VERIFIED: tests/session_lessons_and_regression_suite.rs:375 `line_count <= 1200`].

CONTEXT.md is considerably larger than the roadmap's 3-plan sidebar scope: it adds a traffic-event bus, a resizable zoom window with an agent graph and snoop view, and a GUI vector renderer. These are all locked, so research covers them, but only FLT-01 exists as a requirement. The honest picture of "agent-to-agent traffic" today is thin: the only real sources are input broadcast (`wm.broadcast_command`, src/input/mod.rs:651-653), `:mcp broadcast` (daemon `broadcast_log: Vec<(String, String)>`, src/daemon/mod.rs:160) and API calls such as `pane.send_keys`/`pane.exec`. No agent addressing exists until Phase 51, so the graph is star-shaped (one external sender node fanning out to panes) until then and tests must inject synthetic events.

No new dependency is needed. The existing `PixelCanvasBuffer` already has anti-aliased lines, cubic Beziers, SDF circles and a braille/sextant/octant cell flusher, and all three GUI backends (X11, Win32, macOS) already paint `GuiFrameSurface.images` RGBA textures. One raster function can feed both modes: flush to braille cells in the TUI, push one `GuiImageTexture` in the GUI. Do not add tiny-skia.

**Primary recommendation:** Land a committed classic-drawer golden snapshot set FIRST (none exists today), then add `style` + a pure `AgentState` classifier + herdr rows/renderer (FLT-01), then the traffic bus + `AgentsWindow`, then the GUI texture pass; keep every new `Action` reachable per the 6-step command checklist, and ship the compose line disabled because `[api] allow_agent_messaging` does not exist yet.

## Project Constraints (from CLAUDE.md / AGENTS.md)

- Read `AGENTS.md` and `docs/DEVELOPER_AND_AGENT_GUIDE.md`; verification is `scripts/verify.sh` (one line; on FAIL read only `target/verify/<stage>/fail.txt`). Always pass `-q` to cargo. [VERIFIED: AGENTS.md, CLAUDE.md]
- `ARCH-R04`: files target ~800-1000 lines, hard backstop 1200 (tested). `ARCH-R01`: `handle_wm_key_event` in `src/input/mod.rs` must stay `<= 80` lines (tested); every user-triggerable command lowers to `Action` and runs via `WindowManager::apply`; every new `Action` variant must be added to `every_action_is_reachable_from_keymap_colon_or_rpc` (src/action.rs:457).
- `ARCH-R02`: `WindowManager`/`CompositePane` internal fields stay private; pane removal only via `WindowManager::remove_pane(id)`.
- `SEC-R01..04`: runtime files go through `secure_fs` (`0700` dirs, `0600` atomic files). The helper lives at `src/secure_fs.rs` (CLAUDE.md/AGENTS.md say `src/util/secure_fs.rs`; that path does not exist) [VERIFIED: src/secure_fs.rs:1-60]. Phase 50 should persist nothing to disk (in-memory bus), so no secure_fs use is expected.
- GSD workflow: edits happen through a GSD command; subagents never commit (ROADMAP execution-model note).
- Harmony rule (ROADMAP Phase 50): add new paths; remove/rewire nothing; `F7`, `:drawer ...`, Legacy looks, every existing key keep working.
- Project skills: no `.claude/skills` or `.agents/skills` directory exists in the repo [VERIFIED: ls returned nothing].

## Architectural Responsibility Map

This is a single-process Rust TUI/GUI, so "tiers" are the crate's layered DAG (`vt/config/theme/tmux` -> `pane` -> `wm` -> `action/colon_command/input` -> `api/app` -> `lib/main`, [VERIFIED: AGENTS.md architecture section]).

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| AgentState classification + hysteresis | `wm` (new `agent_state.rs`) | `pane` (read-only signals) | Needs pane signals and a per-pane history; must be testable headless and callable from `poll_all_ptys`. |
| `[drawer] style`, `[agents.<name>]` profiles | `wm::drawer` (`DrawerConfig`) / `config` | - | `DrawerConfig` lives in `wm` and is embedded in `BiltermConfig` (src/config/mod.rs:133). |
| Herdr row model (tree, counts, fold) | `wm` (new `drawer_herdr.rs`) | session tree (`SessionTree.collapsed`) | `drawer_rows()` is the single index source for selection. |
| Herdr rendering (TUI cells) | `ui` (new `drawer_herdr.rs`) | - | Mirrors `render_workspace_drawer`. |
| Key/mouse handling | `input` (new `drawer_herdr.rs`, `agents_window.rs`) | `wm` helpers | `handle_wm_modal_key`/`handle_drawer_mouse_event` delegate; handlers lower to `Action`. |
| Traffic-event bus | `wm` (new `traffic.rs`) | `api` reads via cursor | The `api` layer is ABOVE `wm`; `AutomationEvent` lives in `src/api/playbook.rs:14` so the bus cannot live there. |
| Agents zoom window state (rect, pin, scale, splitter, selection) | `wm` (new `agents_window.rs`) | `ui` | Same pattern as `McpConsoleState` on `WindowManager`. |
| Graph layout (stable positions) | `wm` (pure, deterministic) | - | Pure data; unit-testable. |
| Graph raster (TUI braille / GUI RGBA) | `ui` (new `graph_raster.rs`) | `ui::pixel_canvas`, `ui::hires_subcell` | One draw routine, two sinks. |
| GUI texture compositing | `ui::renderer::GuiWorkspaceRenderer` | per-OS `gui_font/*` | Images already painted by all 3 backends. |

## Standard Stack

### Core (all already in Cargo.toml - no installs)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.30.0 | Cell buffer rendering, `Rect`/`Buffer` | Project stack; used by every renderer here [VERIFIED: Cargo.toml `ratatui = "0.30.0"`] |
| crossterm | 0.29.0 | `KeyEvent`/`MouseEvent` types for handlers | Project stack [VERIFIED: Cargo.toml] |
| unicode-width | 0.2.0 | Column-width-aware truncation of rows/labels | Already a dep; needed for 24-30 col rows [VERIFIED: Cargo.toml] |
| serde + toml | 1.0.219 / 0.8.20 | `[drawer] style`, `[agents.<name>]` | `DrawerConfig` already derives `Serialize, Deserialize` with `#[serde(default)]` [VERIFIED: src/wm/drawer.rs:89-91] |
| insta | 1.42.2 (dev) | Classic-parity and TUI graph snapshots | Existing snapshot pattern `buffer_to_snapshot_string` + `insta::assert_snapshot!` [VERIFIED: Cargo.toml dev-deps; tests/phase3_window_manager.rs:44] |
| tempfile | 3.19.1 (dev) | Hermetic state dirs in tests | existing |

### In-repo building blocks to reuse (not libraries)
| Module | Reuse for |
|--------|-----------|
| `ui::pixel_canvas::PixelCanvasBuffer` (`draw_line_aa`, `draw_bezier_aa`, `draw_circle_aa`, `draw_rect_aa`, `blend_pixel`) | Edges, nodes, particles, glow (src/ui/pixel_canvas.rs:134-260) |
| `ui::hires_subcell` `flush_truecolor_subcells_to_ratatui(CanvasSubcellMode::Braille, ...)` | TUI braille graph (src/ui/hires_subcell.rs:113) |
| `ui::design::DesignTokens::from_palette` | State colours: fields `success`, `warning`, `danger`, `accent`, `text_muted` (src/ui/design/tokens.rs:27-41) |
| `ui::glyphs::GlyphPack::Ascii` | ASCII fallback selector (D-05) - no new setting needed (src/ui/glyphs.rs:7-17) |
| `input::mouse_wm::{detect_border_resize_edge, apply_rect_edge_resize}` and `WmMouseDrag` | Drag-resize/move of the zoom window |
| `WindowManager::{step_scale, set_scale, scale_pct, panel_virtual_area, panel_space}` | Independent panel scale |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-written PixelCanvas graph raster | tiny-skia 0.12.0 | Pure-Rust, OK verdict from the legitimacy seam, but adds a dependency + MSRV/compile-time cost to a project whose canvas already draws AA lines/Beziers/circles; the only gaps are stroke width, glow and a capsule - ~100 lines in a new `graph_raster.rs`. **Recommend NOT adding** (D-10 requires sign-off). |
| Hand-rolled ~30-node force layout | `fdg`, `forceatlas2` crates | Overkill for <= ~30 nodes; adds deps. Deterministic fixed-iteration Fruchterman-Reingold seeded from node-id hash is sufficient. |
| Substring/wildcard pattern matching for profiles | `regex` crate | `regex` is NOT a direct dependency (only transitive) [VERIFIED: Cargo.toml has no `regex`; Cargo.lock lists it transitively]. Case-insensitive substring match covers D-03. |

**Installation:** none. `cargo build -q` only.

**Version verification:** `cargo search tiny-skia --limit 2` returned `tiny-skia = "0.12.0"`; legitimacy seam verdict OK (published 2020-07-04, ~1.85M weekly downloads, repo github.com/linebender/tiny-skia, not deprecated). The package name came from CONTEXT.md/training, not Context7, so it is `[ASSUMED]` until the user signs off - and it is not recommended anyway.

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| tiny-skia | crates.io | ~6 yrs (2020-07-04) | ~1.85M/wk | github.com/linebender/tiny-skia | OK | NOT RECOMMENDED (alternative only; `[ASSUMED]` name provenance) |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none
**New packages recommended for install:** none. If the user later insists on tiny-skia, the planner must add a `checkpoint:human-verify` task before `cargo add`.

## Architecture Patterns

### System Architecture Diagram

```
 PTY output ──> CompositePane/VtParser ──┐
 (OSC133, bytes, title, notifications,   │  read-only signals
  alt-screen, !ECHO, exit code,          ▼
  running_command, visible rows)   AgentTracker::update(now)  <── called from WindowManager::poll_all_ptys
                                         │  (pure classify() + hysteresis + "seen" flag; profiles)
                                         ▼
 [drawer].style ─┬─ classic ──> drawer_rows() classic body (UNCHANGED) ──> render_workspace_drawer (UNCHANGED)
                 └─ herdr  ───> herdr_rows(): SessionTree roots/descendants + live panes + counts + fold
                                   │  (reuses DrawerRowKind::SessionHeader / Pane; adds depth + agent meta)
                                   ▼
        keys (modal.rs drawer.focused block) ──> herdr key fn ──> Action::* ──> WindowManager::apply
        mouse (handle_drawer_mouse_event) ──────> herdr mouse fn ─┘
                                   │ comms glyph / :agents zoom / leader chord
                                   ▼
 Traffic sources ──> TrafficBus.record(event)            AgentsWindow (pin, rect, scale, splitter, selection, filter)
  - broadcast_command (input broadcast)  ring buffer ───> graph model (nodes=panes+external senders, edges=pairs)
  - execute_mcp_command broadcast        (cap 512)         │ stable layout (deterministic)
  - rpc pane.send_keys / pane.exec                         ├─ TUI: PixelCanvasBuffer -> braille flush into Buffer
  - Phase 51 agent.send (later)                            └─ GUI: same canvas -> GuiImageTexture (no blend, alpha>0 copy)
                                                           snoop list (filter, scroll, copy) + compose line (DISABLED in Phase 50)
```

### Recommended Project Structure (all new files; each well under ~800 lines)
```
src/wm/agent_state.rs      # AgentState, AgentSignals, classify(), AgentTracker (per-pane history, hysteresis, seen)
src/wm/drawer_herdr.rs     # impl WindowManager { herdr_rows, jump_next_attention, fold toggle, counts }
src/wm/traffic.rs          # TrafficEvent, TrafficKind, TrafficBus (ring, seq cursor, drain_since)
src/wm/agents_window.rs    # AgentsWindowState + graph model + deterministic layout + snoop filter
src/config/agents.rs       # AgentProfile ([agents.<name>]) - serde, skip_serializing_if empty
src/ui/drawer_herdr.rs     # render_herdr_drawer (TUI cells), glyph tables (unicode/ascii)
src/ui/agents_ui.rs        # zoom window chrome + snoop pane + status line (cell renderer)
src/ui/graph_raster.rs     # draw_agent_graph(&mut PixelCanvasBuffer, &GraphScene, tick) + capsule/glow helpers
src/input/drawer_herdr.rs  # herdr key + mouse handling called from the classic handlers
src/input/agents_window.rs # zoom window keys + mouse (drag/resize/splitter/wheel)
tests/drawer_classic_parity.rs   # Wave 0 goldens (committed BEFORE any src change)
tests/phase50_herdr_drawer.rs    # detector, rows, keys, mouse, commands
tests/phase50_agents_window.rs   # bus, layout, zoom window, snoop, GUI texture
```
`WindowManager` stores `pub(crate)` fields (`agents: AgentTracker`, `traffic: TrafficBus`, `agents_window: AgentsWindowState`) next to `mcp_console` (src/wm/mod.rs:138) and exposes accessors; child modules of `wm` can read the private fields directly (same as `wm/drawer.rs` does with `self.panes`).

### Pattern 1: Style dispatch with an untouched classic body
**What:** `drawer_rows()` becomes a 4-line dispatcher; the existing body is renamed `classic_drawer_rows()` byte-for-byte. Renderer: `if dcfg.style == DrawerStyle::Herdr { return render_herdr_drawer(...) }` placed after the shared chrome (shadow/occlusion/frame) is drawn or before it - decide in plan, but the classic branch must run the identical statements in identical order.
**When to use:** every shared entry point (`drawer_rows`, `render_workspace_drawer`, `handle_drawer_mouse_event`, `handle_wm_modal_key` drawer block, `drawer_rect` width).
**Why:** the classic proof is "same statements, same order" + a golden snapshot; a rewrite of the shared body would risk subtle drift.

### Pattern 2: Pure classifier + stateful tracker
**What:** `classify(&AgentSignals, &AgentProfile?, now) -> AgentState` is a pure function (table-driven unit tests); `AgentTracker` owns `BTreeMap<PaneId, History>` (last byte count, last activity `Instant`, current state, `since`, `seen`), and `update(&mut self, panes, now: Instant)` is called from `poll_all_ptys` right next to `update_drawer_telemetry()` (src/wm/mod.rs:700). Take `now: Instant` as a parameter exactly like `maybe_rebalance_scrollback(now)` (src/wm/mod.rs:701) so tests use synthetic time and never sleep.

**Signals available today (all read this session):**
| Signal | Accessor |
|--------|----------|
| OSC 133 phase | `pane.vt().osc133_phase()` -> `Option<Osc133Phase>` |
| Running command text | `pane.prompt().running_command` (`Option<String>`, src/pane/prompt_editor.rs:73) |
| Last command + exit code | `pane.vt().command_blocks()` -> `CommandBlockRecord { command, exit_code, duration_ms, .. }` (src/vt/mod.rs:39-45) |
| Output activity | `pane.vt().total_bytes_processed()` (monotonic usize) |
| Password / !ECHO | `pane.is_password_prompt()`, `pane.is_active_canvas_password_prompt()` |
| Alt screen | `pane.vt().is_alternate_screen()` |
| Notifications (OSC 9/777) | `pane.vt().notifications()` (`TerminalNotification { title, body }`) |
| Process exit | `pane.has_exited()`, `pane.held_exited_shell()` -> `HeldExitedShellState { exit_code, banner_message }` |
| Visible text for patterns | `pane.vt().row_text(row)` + `pane.vt().size()` |
There is NO foreground-process-name accessor - only `foreground_pgid` (src/pty/line_discipline.rs:33) - so agent identity must come from the command text / title, not `/proc`. Do not add platform-specific `/proc` or `sysctl` lookups.

**Recommended rule table (thresholds are `[ASSUMED]`, Claude's discretion):**
| State | Condition (first match wins) |
|-------|------------------------------|
| Errored | `has_exited`/held shell with non-zero code, or last block/`CommandFinished(Some(c))` with `c != 0` and not yet `seen` |
| Done | exited/finished with code 0 and not yet `seen` (pane not focused when it finished); becomes Idle when focused ("seen") |
| Blocked | `is_password_prompt()`, or profile `blocked` pattern in last N rows, or unread notification on an unfocused pane |
| Working | foreground command running (`running_command.is_some()` or phase `CommandExecuted`) AND bytes changed within `working_hold` (~1.5 s), or profile `working` pattern visible |
| Idle | everything else (shell at prompt, or running command silent beyond `working_hold`) |
Add hysteresis (minimum dwell ~500 ms per state) to stop dot flicker; scan only the last ~8 visible rows, and only when `total_bytes_processed` changed or every ~250 ms (the tick runs every 16 ms: src/app.rs:56 `Duration::from_millis(16)`).

### Pattern 3: Reuse `DrawerRowKind`, add metadata instead of variants
`DrawerRowKind` is `SessionHeader(String) | LayoutGroup(WorkspaceLayoutMode) | Pane(PaneId) | QuickTerminal | FleetNode(String)` [VERIFIED: src/wm/drawer.rs:113-119]. Represent workspace and session rows as `SessionHeader(name)` and panes as `Pane(id)`; add fields to `DrawerRow` (`depth: u8`, `agent: Option<AgentRowMeta>`). `DrawerRow` struct literals exist only in `drawer.rs` (six `rows.push(DrawerRow {` sites at lines 423, 437, 497, 513, 534, 567) [VERIFIED: grep], so adding fields means six one-line edits (+6 lines, file stays ~1021 < 1200). Existing helpers (`drawer_activate_selected`, `selected_drawer_pane_id`, `drawer_close_selected_pane`, `drawer_zoom_selected_pane`, `drawer_move_selected_session`) then work unchanged on herdr rows. No new enum variants means no exhaustive-match fallout in `ui/drawer.rs`.

### Pattern 4: Tree mapping (workspace > session > pane)
The session tree is `SessionTree { nodes: Vec<SessionTreeNode> }` where node has `name, kind (Independent|Rooted|Subtree), parent, children, collapsed, frozen, pane_count, active_commands` [VERIFIED: src/wm/session_tree.rs:28-50]. Only the **active** session has live panes (`self.panes`, `drawer_rows()` renders other sessions as `[SAVED]` headers). Recommended mapping:
- Workspace = session-tree root (`parent == None`); Session = its descendants (indented by depth, arbitrary nesting); Pane = live panes under the active session only.
- Non-active sessions show a `pane_count` rollup and no state dots (no live panes exist). Per-workspace state counts aggregate live panes only; saved subtrees show `-`. (Open Question 2: user may want last-known counts.)
- Fold/expand: reuse `SessionTree::toggle_fold(name)` / `collapsed` ("drawer fold state", src/wm/session_tree.rs:319-327). Note it also changes the MCP `tree` output because `render_herdr_tree_lines` honours `collapsed` (src/wm/session_tree.rs:386,410).
- A root with no children collapses the middle level (panes appear directly under the workspace) so the "one line per pane" criterion holds in the common single-session case.

### Pattern 5: Zoom window = standalone panel state (MCP pattern) with Modal-style drag
`McpConsoleState` carries `active, pinned, focused, size_override, h_scroll, layout_mode, edge` [VERIFIED: src/mcp_console/mod.rs:252-277] and is rendered last in `WorkspaceWidget` (src/ui/mod.rs:727). **CONTEXT D-07 says the MCP window has "drag-resize edges"; it does not.** MCP size is only set by `:mcp size` (`size_override`, src/wm/v2_runtime.rs:817-821) and mouse handling only does badge/row/scroll clicks (src/input/mouse_wm.rs:107-176). Drag-move/resize exists for **modals** (`WmMouseDrag::MoveModal`/`ResizeModal`, mouse_wm.rs:218-228, 631-650) and panes. Therefore:
- Do NOT make the zoom window a `Modal` variant (Modal is exclusive and always captures the keyboard; "pin" needs the MCP-style `pinned/focused` split).
- Create `AgentsWindowState` with `rect: Option<Rect>` (+ maximized flag) clamped like `set_modal_rect` (src/wm/z_stack.rs:477-493), reuse `detect_border_resize_edge`/`apply_rect_edge_resize`/`detect_title_bar_badge_click` (all `pub(super)` in `mouse_wm.rs` - the new input module must live inside `input` to reach them, or widen to `pub(crate)`), and add two `WmMouseDrag` variants (`MoveAgents`, `ResizeAgents`). `WmMouseDrag` is matched only in `mouse_wm.rs` [VERIFIED: grep of `MoveModal|ResizeModal` shows no other consumers outside z_stack.rs/mouse_wm.rs].
- Mouse precedence: `handle_wm_mouse_event` currently calls `handle_drawer_mouse_event` -> `handle_wm_window_drag_event` -> `handle_modal_mouse_event` (src/input/mouse.rs:677-685). Insert the zoom-window handler BEFORE the drawer handler (it floats above the drawer) and make the active drag arm run first.
- Key precedence in `handle_wm_modal_key`: zoom window (when focused) first, then the drawer block (modal.rs:334), then MCP (modal.rs:496).
- Add `PanelTarget::Agents` for the independent scale. `PanelTarget` is matched exhaustively in `scale_setting`/`set_scale` (src/ui/typography.rs:203-216) and `execute_wm.rs:850`, plus the renderer's scale pass (src/ui/renderer.rs:569-575, 701-731) - all need an `Agents` arm.

### Pattern 6: One raster routine, two sinks
Render into `PixelCanvasBuffer::new(w_px, h_px, [bg opaque])`; TUI: `canvas.flush_truecolor_subcells_to_ratatui(CanvasSubcellMode::Braille, graph_rect, buf)`; GUI: push `GuiImageTexture { id, x_px, y_px, width_px, height_px, pixel_width, pixel_height, rgba_bytes }`. Facts that shape this:
- The flusher only writes into cells whose symbol is `" "` (src/ui/hires_subcell.rs `if cell.symbol() != " " { continue; }`), so draw node labels into the buffer first and they survive; clear the graph rect to spaces before flushing each frame.
- `GuiFrameSurface::rasterize_rgba` and all three GUI presenters paint quads, then `images` (copy-only-if-alpha>0, **no blending**, nearest-neighbour), then glyphs [VERIFIED: src/ui/renderer.rs:253-295; src/ui/gui_font/linux_x11.rs:443-461; win32_gdi.rs:462; macos_ct.rs:540]. Consequences: (a) make the canvas opaque (fill with the window bg) or AA edges alias/halo; (b) render at 1:1 (`pixel_width == width_px`) to avoid nearest-neighbour shimmer; (c) cell glyphs paint ON TOP of the texture, so labels can stay as cells; (d) the cell buffer under the texture must be cleared in GUI mode or half-block/braille cells would paint over the graph.
- Implement the GUI pass inside `GuiWorkspaceRenderer::render_workspace_with_cell_size` right after the cell loop (where pane canvas textures are pushed, src/ui/renderer.rs:750-800): clear the graph rect in `cell_buf` before converting cells to quads/glyphs, then push one texture. This avoids touching `WorkspaceWidget` (a public struct with a public `wm` field, src/ui/mod.rs:567-569).
- Determinism: `f32` basic ops are IEEE-deterministic, but `sin/cos/powf` can differ across libm builds. Keep layout and the static (tick = 0) graph free of trig where possible and snapshot only the static frame; assert animation semantically.

### Anti-Patterns to Avoid
- **Rewriting the shared classic render/rows body "to share code":** breaks byte-identical proof; add branches instead.
- **Adding a Modal variant for the zoom window** (see Pattern 5).
- **Putting traffic types in `api/playbook.rs`:** wrong layer direction; `wm` cannot import `api`.
- **Using the hard-coded header/footer x-ranges for herdr:** see Pitfall 3.
- **Adding keys to `:state` JSON:** pinned by test (Pitfall 5).
- **Emoji in herdr rows** (the classic rows use `📡 🔑 🔔 🌐`): double-width/variation-selector glyphs break 24-30 col alignment; use single-width BMP symbols.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| AA lines/curves/circles | A new rasterizer or vector crate | `PixelCanvasBuffer::{draw_line_aa, draw_bezier_aa, draw_circle_aa, blend_pixel}` | Already tested and used by GUI + SSH paths; gaps (stroke width, glow, capsule) are small additions in `graph_raster.rs` |
| TUI sub-cell graphics | Custom braille encoder | `flush_truecolor_subcells_to_ratatui(CanvasSubcellMode::Braille, ..)` | exists, mask->glyph tables included |
| Window drag/resize hit-testing | New edge detection | `detect_border_resize_edge`, `apply_rect_edge_resize` | identical semantics to all other windows ("resizable like all other windows") |
| Per-panel scale | New scale plumbing | `PanelTarget` + `step_scale/scale_pct/panel_space/panel_virtual_area` | Only add the `Agents` variant |
| Command/palette/help sync | Ad-hoc command strings | The 6-step "adding a command" checklist (AGENTS.md "Automation API and command parity") + drift tests | Drift tests fail otherwise |
| Config get/set for `[drawer] style` | Custom setter | Serde field + `dotted::set_key` (`:set drawer.style herdr` works automatically) | `dotted` round-trips through `toml::Value` (src/config/dotted.rs:28-34) |
| Column-width truncation | `chars().take(n)` | `unicode-width` (already a dep) | wide chars/ellipsis at 24-30 cols |
| Terminal-safe text for untrusted strings | Passing titles/bodies straight to `set_stringn` | One `sanitize_display(&str)` helper (strip C0/C1 controls, bidi overrides U+202A-202E/U+2066-2069, cap length) used by sidebar rows AND snoop view | Pane titles and agent messages are attacker-influenced |
| Force-directed layout crate | `fdg`/`forceatlas2` | ~100-line deterministic Fruchterman-Reingold with fixed iterations, seeded from node-id hash, warm-started from previous positions | <= ~30 nodes; determinism + no new dependency |

**Key insight:** every hard sub-problem here (raster, window chrome, scale, config, command catalog) already has a project-local answer; the risk is in wiring and in keeping Legacy output identical, not in missing libraries.

## Runtime State Inventory

Not a rename/refactor/migration phase - section omitted per template. (Adding `[drawer] style` and `[agents.*]` is additive; see Pitfall 6 for the saved-config side effect.)

## Common Pitfalls

### Pitfall 1: No classic-drawer golden exists, so "snapshot-proven parity" is currently unprovable
**What goes wrong:** `tests/snapshots/` holds 13 files (composite_pane, phase2, phase3, phase4) and none cover the drawer [VERIFIED: ls tests/snapshots]. Existing drawer tests assert substrings only. If code changes first, the "before" is lost.
**How to avoid:** Wave 0 task, before any `src/` edit: add `tests/drawer_classic_parity.rs` rendering `WorkspaceWidget` via `TestBackend` + `buffer_to_snapshot_string` for a matrix (suggested 8-10): Left/Right/Top/Bottom edge on legacy; Left on `modern_mode`; Left on `liquid_glass` (2-row cards, `drawer_vertical_layout_metrics` step 2 only when `inner_height >= 14` and items fit, src/ui/drawer.rs:30); maximized; overlay mode; with saved sessions; with `fleet` node if constructible. Generate with `INSTA_UPDATE=always cargo test -q --test drawer_classic_parity` ONCE on the untouched tree, commit the `.snap` files, then forbid edits (`git diff --exit-code tests/snapshots/drawer_classic_parity__*` is a final gate). Insta default (`auto`) writes `.snap.new` and fails when no snapshot exists, so missing goldens fail loudly rather than pass.
**Determinism rules:** do NOT call `poll_all_ptys()` (spinner = `total_bytes % 10 + tick_counter`, src/wm/drawer.rs:253-257); hold the env-lock + temp `XDG_STATE_HOME`/`XDG_CONFIG_HOME` pattern from `tests/api_config_commands.rs:14-25` because `list_sessions()` reads the saved-session directory; do not use recording or `std::env::temp_dir()` paths in rendered text.
**Warning signs:** snapshot contains absolute paths, a home dir, or a changing spinner glyph.

### Pitfall 2: `drawer_rows()` is the shared index space
**What goes wrong:** `selected_index`, `drawer_select_next/prev`, `drawer_scroll_by`, `sync_drawer_selection_to_active_pane`, `drawer_hscroll_by`, and the mouse hit-test all call `drawer_rows()`. If herdr rows live in a separate list, each call site needs a branch and one missed site gives off-by-N clicks.
**How to avoid:** Pattern 1 + Pattern 3 (single dispatching `drawer_rows()`). Keep `selected_index` semantics.

### Pitfall 3: Hard-coded click zones in the classic mouse handler misfire on a narrow herdr drawer
**What goes wrong:** `handle_drawer_mouse_event` hit-tests the header and footer by absolute column ranges tuned to the classic text: row `dr.y+1` with `rel_x >= 38` creates a session, `rel_x >= 31 || rel_x >= dr.width-8` creates a window, and the footer row `dr.height-2` uses `win_end/sess_end/move_end/par_end` thresholds (src/input/mouse_wm.rs:751-815). In a 24-30 column herdr drawer the `dr.width-8` rule makes a plain click on the second header row open a new window; the footer thresholds map to different glyphs.
**How to avoid:** Branch to a herdr mouse function BEFORE those blocks (after the title-bar badge and edge-resize handling, which are geometry-based and safe to share). Add tests clicking every header/footer cell of a 26-col herdr drawer and asserting no pane/session is created.

### Pitfall 4: Layout metrics are shared with the mouse handler
**What goes wrong:** `drawer_vertical_layout_metrics(cfg, inner_height, item_count)` returns step 2 for liquid glass; the mouse handler calls it directly in three places (mouse_wm.rs:822, 851, 867). Herdr must be strictly one row per item (success criterion 1).
**How to avoid:** Add `WindowManager::drawer_metrics(inner_h, rows_len)` that returns the classic function result for Classic and `(1, 1, 1)`-style herdr metrics for Herdr, and switch the three call sites to it (mechanical; identical results in classic). Renderer uses the same function.

### Pitfall 5: `:state` and config-list tests are exact
**What goes wrong:** `tests/config_commands_edge.rs:196` asserts the exact `:state` key list `["active_pane_id", "appearance", "display_mode", "drawer_visible", "layout_mode", "pane_count"]`. Adding agent counts to `state_json` breaks it. `:config list` only asserts `> 20` keys (config_commands_edge.rs:181-186), so new `drawer.style` is safe.
**How to avoid:** Expose agent state through a NEW command/RPC data path (`:agents list` -> `take_command_data`) rather than `:state`.

### Pitfall 6: Serialized-config side effects
**What goes wrong:** `DrawerConfig` derives `Serialize`; a new `style` field will appear in `:config save` output and in `get_key` listings. An `agents: BTreeMap` field would emit an empty `[agents]` table.
**How to avoid:** Always serialize `style` (discoverable; `:get drawer.style` works) but mark `agents` with `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`. Use `#[serde(rename_all = "kebab-case")]` on `DrawerStyle` like the sibling enums (`classic`/`herdr`). Verify `Config: N keys` tests still hold.

### Pitfall 7: Width default
**What goes wrong:** `DrawerConfig::default().width_cols` is 34 and `resize_drawer` clamps to 16..=100 (src/wm/drawer.rs:105, 950). Success criterion wants ~24-30 columns by default for herdr, but `width_cols` is shared with classic (changing it changes classic).
**How to avoid:** Add `herdr_width_cols: u16` (default 28) used by `drawer_rect` only when style is Herdr, and make `resize_drawer`/mouse edge-drag write the field matching the active style. (Alternative: set 28 once on the first switch to herdr; rejected because it silently mutates config.)

### Pitfall 8: Animation needs a redraw trigger
**What goes wrong:** `poll_all_ptys` returns `true` only on real changes (src/wm/mod.rs:659-703) and the result of `update_drawer_telemetry()` is ignored, so spinners/flow particles advance only when something else redraws.
**How to avoid:** Return `true` at a throttled cadence (~100-150 ms) ONLY when (style == Herdr and drawer visible and any pane is Working) or (zoom window open and traffic active). Keep classic returning exactly what it returns today (no extra redraws). All GUI loops already gate on this return value (`app.poll_all_ptys()` in gui_window.rs:565, gui_macos.rs:674, linux_x11.rs:766).

### Pitfall 9: GUI plumbing gaps that CONTEXT assumes are working
**What goes wrong (all `[VERIFIED: grep over src/ and tests/ this session]`; re-run before relying):**
1. `WindowManager::set_gui_surface(true)` is called only in `GuiWorkspaceRenderer::resize_workspace_for_pixel_window` (src/ui/renderer.rs:490), and that function has **no call sites** in `src/`; only tests set it (tests/phase46_display_modes.rs:154,194). So `scale_pct()` returns 100 in the real X11/Win32/macOS loops and the existing drawer/MCP panel scale is effectively a no-op at runtime ("applies in the GUI only" note is never satisfied).
2. The X11 loop turns Ctrl+wheel into `Action::FontZoomIn/Out` before it reaches the panel handler (linux_x11.rs:730-735) and builds every mouse event with `KeyModifiers::empty()` (lines 693-712), so `ctrl_wheel_scale_delta` (mouse_wm.rs:96-105), Shift+wheel horizontal scroll and Alt-drag cannot trigger in the X11 GUI.
**How to avoid:** Detect GUI mode for the zoom texture from the renderer being `GuiWorkspaceRenderer` (not from `wm.gui_surface`). Treat panel-scale-in-real-GUI as a pre-existing gap: either fix it here (set `gui_surface` in the three runners; pass modifiers through; small but cross-cutting, only partially testable headless) or explicitly scope it out with user agreement (Open Question 4). Roadmap 50-03 already lists a "GUI scale/pin check", so the plan should include a task that proves scale via `set_gui_surface(true)` tests AND documents the runtime gap.

### Pitfall 10: Event-name mismatch with the existing API
**What goes wrong:** Existing `AutomationEvent` JSON `type` values are snake_case (`command_submitted`, `echo_lock_changed`, ...) [VERIFIED: src/api/playbook.rs:14-44 and docs/API.md "events.poll"], while ROADMAP/CONTEXT name the new events dotted (`agent.message`, `agent.state`). The Phase 50 bus schema becomes a shared contract (D-01, costly).
**How to avoid:** Keep the internal `TrafficEvent` independent of JSON; give it a `kind()` string and `seq` cursor; define the API mapping (and pick one naming style) in the 50-01 spec for user sign-off before Phase 51 builds on it.

### Pitfall 11: Compose line gate does not exist
**What goes wrong:** D-09 requires `[api] allow_agent_messaging`, but `ApiConfig` has exactly one field, `allow_config_write` [VERIFIED: src/config/dotted.rs:15-20], and the `[api]` section is never settable over the API.
**How to avoid:** Phase 50 renders the compose line disabled ("requires agent messaging, Phase 51") and adds NO injection path. Provide the seam only: `trait`/fn pointer or `Option<...>` hook that Phase 51 fills. Adding the key now without rate limits/loop guard/logging would ship an unguarded write path.

### Pitfall 12: Key collisions inside the focused drawer
**What goes wrong:** `handle_wm_modal_key` already consumes (when `drawer.focused`): `q F7 F11 _ - * [ ] M F5 J K N b B > < u U P s S r R : j k h l g G n + c w v | % x z d m p a A t T L space Enter Esc Tab Left Right PageUp PageDown Home End Delete`, plus Shift/Alt+Up/Down and Alt+Left/Right (src/input/modal.rs:342-491). Unused letters include `e f i o y C D E F H I O Q V W X Y Z` and symbols like `!`, `@`, `#`.
**How to avoid:** Suggested bindings (Claude's discretion, re-verify with a keymap-uniqueness test): in-drawer `!` = jump to next blocked/attention; `e` = fold/expand the selected workspace/session row (keep Enter = focus pane / switch session exactly as classic, because `drawer_activate_selected` is shared); leader chord `!` = jump-to-attention (works with the drawer closed) and leader `a` = open the agents zoom window. Leader table currently uses `" - h % | \ V z Z F5 o O n Tab BackTab p v ] 1-9 w W d D F7 F11 M q ` ~ F10 Space : ? F1 i I F9 x X L { } t T b B s S e E m F4 g G F12 c C [ F8 r f F l / N P H R j k < > J K + = 0` [VERIFIED: src/input/keymap.rs:14-136 read this session] - `!` and `a` do not appear. The herdr key function is a NEW function called from the `if wm.drawer().focused` block (so `handle_wm_key_event` stays <= 80 lines and `modal.rs` stays < 1000).

### Pitfall 13: `Action` derives `Eq`
`Action` is `#[derive(Clone, Debug, PartialEq, Eq)]` (src/action.rs:14): no `f32` payloads. Scale/splitter values must be integers (percent as `u16`). New variants need entries in the exhaustive samples and `colon_expr` match inside `every_action_is_reachable_from_keymap_colon_or_rpc` (src/action.rs:457-...), the completion table row (`(":drawer", &["toggle", ...])`, src/pane/completion.rs:483), and a description via `HELP_LINES`/`PALETTE_CATALOG`/`EXTRA` (drift tests in src/command_registry.rs:155-195).

### Pitfall 14: Untrusted text in rows and snoop
Pane titles come from OSC 0/2 emitted by whatever runs in the pane; message bodies will come from other agents. Unsanitised control sequences stored in `Buffer` cells can be replayed to the host terminal. Use the single `sanitize_display` helper (Don't Hand-Roll) and cap lengths before any layout math.

## Code Examples

Verified in-repo patterns the plan can reference (values quoted from files read this session):

### Existing drawer selection/activation contract (reuse, don't fork)
```rust
// Source: src/wm/drawer.rs:112-119 (verbatim variants)
pub enum DrawerRowKind {
    SessionHeader(String),
    LayoutGroup(WorkspaceLayoutMode),
    Pane(PaneId),
    QuickTerminal,
    FleetNode(String),
}
```

### Style enum + config field (skeleton; follows sibling enums)
```rust
// Source pattern: src/wm/drawer.rs:26-34 (#[serde(rename_all = "kebab-case")], #[default])
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrawerStyle {
    #[default]
    Classic,
    Herdr,
}
// DrawerConfig (src/wm/drawer.rs:89-97) gains: pub style: DrawerStyle, pub herdr_width_cols: u16
// and Default (src/wm/drawer.rs:99-109) gains: style: DrawerStyle::Classic, herdr_width_cols: 28
```

### Classifier signature (pure, testable)
```rust
// Skeleton (new). Osc133Phase variants verbatim from src/pty/line_discipline.rs:3-12:
// PromptStart, CommandStart, CommandExecuted, CommandFinished(Option<i32>)
pub enum AgentState { Working, Blocked, Idle, Done, Errored }
pub struct AgentSignals {
    pub phase: Option<Osc133Phase>,
    pub running_command: bool,
    pub bytes_changed_ms_ago: Option<u64>,
    pub echo_locked: bool,
    pub exited_code: Option<i32>,
    pub unread_notification: bool,
    pub seen: bool,
    pub tail_text: Vec<String>, // last N visible rows, sanitised
}
pub fn classify(s: &AgentSignals, profile: Option<&AgentProfile>) -> AgentState { /* table above */ }
```

### Headless fixtures that already work for detector tests
```rust
// Source: tests/slide_out_drawer_and_telemetry.rs:138-160 (pattern)
let mut wm = WindowManager::new_headless(26, 110);
let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
wm.pane_mut(p2).unwrap().process_pty_output(b"output stream bytes\r\n\x1b]133;D;42\x07"); // errored
wm.pane_mut(1).unwrap().process_pty_output(b"\x1b]9;Alert1\x07");                        // notification
wm.pane_mut(3).unwrap().submit_command("cargo test").unwrap();                           // running command
```

### TUI graph sink (existing API)
```rust
// Source: src/ui/hires_subcell.rs:113 signature; CanvasSubcellMode::Braille src/pane/canvas_scene.rs:17-20
canvas.flush_truecolor_subcells_to_ratatui(CanvasSubcellMode::Braille, graph_rect, buf);
```

### GUI graph sink (existing struct)
```rust
// Source: src/ui/renderer.rs:180-190 GuiImageTexture fields (verbatim):
// id: u32, x_px: u32, y_px: u32, width_px: u32, height_px: u32,
// pixel_width: u16, pixel_height: u16, rgba_bytes: Vec<u8>
surface.images.push(GuiImageTexture { id: 950_000, x_px, y_px, width_px: w, height_px: h,
    pixel_width: w as u16, pixel_height: h as u16, rgba_bytes: canvas.rgba });
// existing ids in use: 900_000 + pane id (renderer.rs:778) -> pick a disjoint range
```

### Panel mouse-space + scale (existing)
```rust
// Source: src/input/mouse_wm.rs:677-684
let (mouse, workspace_area) = wm.panel_space(mouse, workspace_area, PanelTarget::Drawer);
if let Some(delta) = ctrl_wheel_scale_delta(&mouse) && /* inside rect */ { wm.step_scale(PanelTarget::Drawer, delta); }
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| herdr (reference tool): agent detection by process/screen, blue dot on finish in background | Same semantic set, derived here from OSC 133/activity/exit code/patterns | - | herdr docs describe states blocked/working/idle (+ "done"/blue dot when finished in a background workspace) and "zero config" detection `[CITED: betterstack.com/community/guides/ai/herdr-ai-agent.md; herdr.dev search summary]`. Internals (process name vs screen) are NOT documented in what was fetched - treat as `[ASSUMED]`; CONTEXT says herdr was "described by the user, not read" and roadmap 50-01 asks for a user-confirmed capability list. |
| Classic drawer rows: spinner + sparkline + badges, emoji-heavy | Compact tree + state dot + counts | this phase | additive via `[drawer] style` |

**Deprecated/outdated:** nothing is removed; classic stays.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | herdr's left side shows a workspaces section with an aggregate state dot per workspace and an agents list for the selected workspace; states blocked/working/idle/done (blue = finished in background) | State of the Art | Layout mismatch vs user's mental model; roadmap 50-01 requires a user-confirmed list anyway |
| A2 | Threshold values: `working_hold` ~1.5 s, min dwell ~500 ms, scan last ~8 rows every ~250 ms | Pattern 2 | Flicker or laggy states; tunable constants, low blast radius |
| A3 | Per-agent screen patterns (e.g. "esc to interrupt", "Do you want to proceed", "(y/n)") for claude/codex/gemini | Pattern 2 / profiles | False blocked/working states; user must supply/confirm profile defaults; ship profiles empty-by-default or clearly marked |
| A4 | Agents like claude/codex repaint a spinner while working and are silent when idle, so "running command + recent output = Working, silent = Idle" is a usable heuristic | Pattern 2 | Long silent builds look Idle; accepted limitation |
| A5 | Default herdr width 28 columns | Pitfall 7 | Cosmetic; roadmap says "about 24-30" |
| A6 | Leader chords `!` and `a`, in-drawer `!`/`e` are free and acceptable | Pitfall 12 | Collisions; re-verify with a keymap uniqueness test at plan time |
| A7 | Mapping workspace = session-tree root, session = descendant, panes only for the active session | Pattern 4 | Wrong tree semantics; Open Question 2 |
| A8 | Single-width BMP glyphs (`⠋ ▲ ○ ● ✗` and ASCII `* ! - + x`) render as 1 column in the user's terminals | Glyphs | Alignment drift; selector is `GlyphPack::Ascii` fallback |
| A9 | tiny-skia 0.12.0 is a viable but unnecessary option | Standard Stack | None (not recommended) |
| A10 | `ratatui` `Buffer::set_stringn` does not strip control characters, so sanitising is required | Pitfall 14 | If it does sanitise, helper is harmless redundancy; not verified in ratatui source this session |

## Open Questions

1. **Requirement coverage for the bus + zoom window + GUI graph (D-01, D-06..D-10).**
   - Known: only FLT-01 (sidebar) exists; ROADMAP lists 3 plans; CONTEXT locks a much larger scope.
   - Unclear: whether the user wants new IDs (e.g. FLT-02 bus+zoom window, FLT-03 GUI vector graph) and a re-planned wave structure.
   - Recommendation: planner proposes FLT-02/FLT-03 in REQUIREMENTS.md/ROADMAP.md for sign-off and splits into roughly: 50-01 spec + classic goldens (Wave 0/1) -> 50-02 AgentState + config + herdr rows -> 50-03 herdr render/keys/mouse/commands + parity -> 50-04 traffic bus + AgentsWindow (TUI) -> 50-05 GUI texture/scale/pin + docs. Do not silently drop locked decisions.
2. **What are "workspace" and "session" in this codebase's tree?** Only the live session has panes. Recommendation in Pattern 4; ask whether saved sessions should show last-known state counts (needs a small cache written when switching away).
3. **Default per-agent profiles.** Ship none (heuristics only) or ship claude/codex/gemini defaults whose strings are unverified (A3)? Recommend ship none plus documented examples; user supplies patterns.
4. **GUI runtime gaps (Pitfall 9).** Fix `set_gui_surface`/modifier pass-through in the three runners now, or scope out? Affects whether "Ctrl+wheel scale works in GUI" can truthfully be claimed.
5. **Event schema naming** (snake_case vs dotted) and API exposure timing (Pitfall 10).
6. **Traffic sources are thin until Phase 51.** Confirm the graph may ship showing external sender nodes ("you", "api", "mcp") and broadcast fan-out only; add an `:agents demo`-free approach (tests inject events directly).
7. **Compose line:** confirm "disabled stub only" for Phase 50 (Pitfall 11).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | build/test | yes | cargo/rustc 1.90.0 (rust-toolchain.toml `channel = "1.90"`) | - |
| `scripts/verify.sh` | verification gate | yes | baseline `PASS 51/51 52s` (`--status`) | - |
| Node (GSD tools) | planning tooling | yes | - | - |
| X11 / Win32 / macOS display | manual GUI check | not probed | - | Headless `GuiFrameSurface` + `rasterize_rgba()`/`export_ppm()` tests; GUI visual check is manual-only |

**Missing dependencies with no fallback:** none for automated work.
**Missing dependencies with fallback:** real GUI window verification (use headless raster tests; mark visual check as human-verify).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust 1.90) + `insta` 1.42.2 snapshots + `ratatui::backend::TestBackend` |
| Config file | none (Cargo-native); `scripts/verify.sh` stages one per `tests/*.rs` (auto-discovered) |
| Quick run command | `scripts/verify.sh --only t:drawer_classic_parity t:phase50_herdr_drawer t:phase50_agents_window` |
| Full suite command | `scripts/verify.sh` (baseline PASS 51/51; new files become `t:<name>` stages automatically) |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FLT-01 | Classic output byte-identical (all edges/looks) | snapshot | `cargo test -q --test drawer_classic_parity` | No - Wave 0 |
| FLT-01 | `[drawer] style` parses/serialises; `:set drawer.style herdr`; default classic | unit/integration | `cargo test -q --test phase50_herdr_drawer style_` | No - Wave 0 |
| FLT-01 | Tree rows (workspace>session>pane), one line per pane, fold/expand, focused marker, width 24-30 | integration | `cargo test -q --test phase50_herdr_drawer tree_` | No - Wave 0 |
| FLT-01 | `classify()` table: working/blocked/idle/done/errored, hysteresis, seen, profiles | unit (synthetic `Instant`) | `cargo test -q --test phase50_herdr_drawer state_` | No - Wave 0 |
| FLT-01 | Per-workspace counts + jump-to-next-blocked (wraps, no-op when none) | integration | `cargo test -q --test phase50_herdr_drawer attention_` | No - Wave 0 |
| FLT-01 | Keyboard parity (j/k/arrows/Enter/new/rename/close, `:drawer` equivalents all `Action`) | integration | `cargo test -q --test phase50_herdr_drawer keys_` and `cargo test -q --lib action::tests` | No - Wave 0 / existing lib tests |
| FLT-01 | Mouse parity (click focus, wheel, Ctrl+wheel scale, edge-drag resize, header/footer no misfire) | integration | `cargo test -q --test phase50_herdr_drawer mouse_` | No - Wave 0 |
| FLT-01 | Pin (`Fixed`) + scale with herdr style (`set_gui_surface(true)` as in phase46) | integration | `cargo test -q --test phase50_herdr_drawer pin_scale_` | No - Wave 0 |
| D-01 | Bus ring cap, seq cursor, sources (broadcast, mcp broadcast, rpc) | unit/integration | `cargo test -q --test phase50_agents_window bus_` | No - Wave 0 |
| D-07..D-09 | Zoom window open/close/pin/resize/splitter/scale/snoop filter/compose disabled | integration | `cargo test -q --test phase50_agents_window window_` | No - Wave 0 |
| D-08 | Layout deterministic + stable on insertion | unit | `cargo test -q --test phase50_agents_window layout_` | No - Wave 0 |
| D-10 | TUI braille graph non-empty; GUI texture pushed, cleared cells, opaque canvas | integration | `cargo test -q --test phase50_agents_window render_` | No - Wave 0 |
| (all) | New `Action`s reachable; catalog/completion drift; file-size backstop; `:state` keys unchanged | existing | `scripts/verify.sh --only unit t:session_lessons_and_regression_suite t:config_commands_edge` | Yes |

### Sampling Rate
- **Per task commit:** `scripts/verify.sh --quick` (clippy + unit) plus the touched `t:` stage via `--only`.
- **Per wave merge:** `scripts/verify.sh` (full).
- **Phase gate:** full suite green, `git diff --exit-code tests/snapshots/drawer_classic_parity__*` empty, then `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `tests/drawer_classic_parity.rs` + committed `tests/snapshots/drawer_classic_parity__*.snap` generated on the UNCHANGED tree (`INSTA_UPDATE=always` once).
- [ ] `tests/phase50_herdr_drawer.rs`, `tests/phase50_agents_window.rs` (skeletons with the fixture helpers above, env-lock + temp XDG pattern from `tests/api_config_commands.rs:14-25`).
- [ ] A keymap/drawer-key uniqueness test for the new bindings (none verified to exist; grep found no duplicate-key test).
- Framework install: none needed.

## Security Domain

`security_enforcement` is enabled (`.planning/config.json`: `"security_enforcement": true`, `"security_asvs_level": 1`, `"security_block_on": "high"`).

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | - (no new auth surface) |
| V3 Session Management | no | - |
| V4 Access Control | yes | Compose/injection path stays disabled and gated behind Phase 51 `[api] allow_agent_messaging`; API write gate pattern (`config_write_denied`) is the model |
| V5 Input Validation | yes | `sanitize_display()` for pane titles, agent names, message bodies; length caps before layout; profile patterns are plain substrings (no regex ReDoS surface) |
| V6 Cryptography | no | - |
| V7 Error handling & logging | yes | Do not log keystroke content; redact preview when target pane is `!ECHO`/password prompt |
| V8 Data protection | yes | Bus is in-memory only (no files in Phase 50); if persistence is ever added it must use `src/secure_fs.rs` (`0600`/`0700`) |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Terminal escape/bidi injection via OSC titles or message bodies rendered in the sidebar/snoop | Tampering / Spoofing | `sanitize_display` strip C0/C1 + U+202A-202E/U+2066-2069 + length cap (A10 notes ratatui behaviour unverified) |
| Secret exposure through traffic snoop (passwords sent via `pane.send_keys`/broadcast into prompts) | Information disclosure | Bus events carry kind/byte-count and a short sanitised preview; never record content when `is_password_prompt()`; panes in `!ECHO` lock excluded from previews |
| Unbounded memory from event flood | Denial of service | Ring buffer cap (match `AutomationControlPlane::push_event` 512, src/api/rpc.rs:234-239), per-event body cap, filter history cap |
| Outward write path from snoop compose line (prompt injection / agent ping-pong) | Elevation of privilege | Disabled in Phase 50; Phase 51 gate + rate limit + loop guard + logging before enabling |
| Prompt-injection content displayed to a human or copied | Spoofing | Read-only display, explicit "copy" action only, sanitised |
| Config-write via API for `drawer.style` | Tampering | Already covered: config-mutating commands refused over RPC/HTTP/MCP unless `[api] allow_config_write` (AGENTS.md "Write gate") |

## Sources

### Primary (HIGH confidence - files read this session)
- `/home/bill/src/ai/bilterm/src/wm/drawer.rs` (full), `src/ui/drawer.rs` (full), `src/input/modal.rs:320-560`, `src/input/mouse_wm.rs:1-260,620-887`, `src/input/mouse.rs:640-720`
- `src/mcp_console/mod.rs:245-320,510-575`, `src/wm/z_stack.rs:1-70,380-520`, `src/ui/typography.rs:60-260`, `src/ui/renderer.rs:140-345,434-520,520-851`, `src/ui/mod.rs:560-740`
- `src/wm/session_tree.rs:1-80,315-431`, `src/wm/mod.rs:95-140,640-720`, `src/config/dotted.rs:1-120`, `src/colon_command/execute_config.rs:40-100`, `src/action.rs:380-540`, `src/input/keymap.rs:14-306`
- `src/api/rpc.rs`, `src/api/playbook.rs:14-60`, `src/daemon/mod.rs:80-100,420-450`, `src/pty/line_discipline.rs:1-25`, `src/ui/design/tokens.rs:24-42`, `src/ui/pixel_canvas.rs:1-300`, `src/ui/hires_subcell.rs:1-232`, `src/ui/gui_font/linux_x11.rs:424-470,690-790`
- `Cargo.toml`, `.planning/config.json`, `.planning/REQUIREMENTS.md:186-204`, `.planning/ROADMAP.md:751-792`, `50-CONTEXT.md`, `AGENTS.md`, `CLAUDE.md`
- `scripts/verify.sh --status` -> `PASS 51/51 52s`; `cargo --version`/`rustc --version` -> 1.90.0

### Secondary (MEDIUM confidence)
- [Herdr: Terminal Multiplexer with Built-in AI Agent State Awareness (Better Stack)](https://betterstack.com/community/guides/ai/herdr-ai-agent.md) - states, two-layer sidebar, zero-config detection (no glyph/keys/width detail)
- [herdr agent guide](https://herdr.dev/agent-guide.md), [herdr on docs.rs](https://docs.rs/herdr), GitHub results (maxjendrall/herdr, naaive/herdr, ttbug/herdr) - search summary only; several projects share the name, not individually verified

### Tertiary (LOW confidence)
- Training-knowledge claims about agent TUI behaviour (A3, A4) - tagged `[ASSUMED]`

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies; everything verified in Cargo.toml and source.
- Architecture: HIGH - seams, line counts, handler order and GUI compositing facts all read directly; MEDIUM for the tree mapping (A7) and tiering of FLT-02/03.
- Pitfalls: HIGH for code-derived ones (1-9, 12-13); MEDIUM for 10, 14 (design-time risks).
- Heuristics/patterns: LOW-MEDIUM - assumed thresholds and agent strings need user confirmation.

**Research date:** 2026-11-03
**Valid until:** 2026-12-03 (stable internal code; re-run the Pitfall 9 greps and the `drawer.rs`/`modal.rs` line counts before planning if other phases merge first)
