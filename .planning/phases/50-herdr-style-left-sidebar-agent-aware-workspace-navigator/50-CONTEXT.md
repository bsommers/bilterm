# Phase 50: herdr-Style Left Sidebar (Agent-Aware Workspace Navigator) - Context

**Gathered:** 2026-11-03
**Status:** Ready for planning

<domain>
## Phase Boundary

Make the slide-out drawer look and work like herdr's left side: a compact tree (workspace > session > pane) with per-pane state glyphs, per-row pairing/fan-out comms indicators, full keyboard and mouse parity. Add a resizable zoom window showing an agent-to-agent traffic graph plus a conversation (snoop) view. Phase 50 also owns the traffic-event bus the window reads; Phase 51 (agent messaging) plugs into the same bus.

Fixed by the user's brief: glyph status like herdr; arrow- and mouse-friendly; show agent state and pairing/fan-out comms; zoom pop-up with connection/flow graph and snoop; keys and mouse/scroll equal; popup resizable like all other windows; text graphics in text mode, high-resolution smooth graphics in GUI mode.

</domain>

<decisions>
## Implementation Decisions

### Scope split with Phase 51
- **D-01:** Phase 50 builds the UI on a small internal traffic-event bus (who sent what to whom, with timestamps). Today it is fed by `:mcp broadcast` and control-API/MCP calls; Phase 51's `agent.send` etc. plug into the same bus. Graph and snoop work from day one. — **Reversibility:** costly — the event schema becomes a contract shared with Phase 51 and the API events (`agent.message`).
- **D-02:** The drawer's new layout is `[drawer] style = "classic" | "herdr"`; `classic` stays byte-identical (Legacy parity, snapshot-proven). Default stays `classic` until the user signs off.

### Agent state
- **D-03:** State (working / blocked / idle / done / errored) is derived from heuristics on existing signals (OSC 133 phase, output activity, exit code, running command, waiting-for-input prompt patterns) plus optional per-agent profiles (`[agents.<name>]` for known tools such as claude, codex, gemini) carrying their own working/blocked patterns. Explicit `agent.state` reports (Phase 51) override heuristics.
- **D-04:** Per-workspace count summary and a jump-to-next-blocked key stay as in the roadmap success criteria.

### Glyphs and comms indicators
- **D-05:** Unicode glyphs with an ASCII fallback set (for limited terminals and Legacy looks). In GUI mode the same semantic states are drawn as smooth vector icons with colour and subtle animation. — **Reversibility:** reversible.
- **D-06:** Each row shows a pairing glyph plus the partner's short name (pair), or a fan-out glyph with a count (fan-out); the glyph animates while traffic flows. Enter/click on it opens the zoom window focused on that edge.

### Traffic zoom window
- **D-07:** Opened by key, mouse click, or command (`:agents zoom`), and from the comms glyph on a row. Behaves like the MCP window: drag-resize edges, pin, Ctrl+wheel scale (independent scale), Esc closes, adjustable splitter. Graph on one side, conversation on the other. Keys and mouse/scroll are equally capable.
- **D-08:** Layout is automatic: layered or force-directed, with stable node positions so nodes don't jump. TUI: arrow-key selection cursor over nodes/edges. GUI: nodes can be dragged. (Claude's discretion on the exact algorithm per mode.)
- **D-09:** Snoop view is read-only follow + filter (by agent, pair, keyword) + history scroll + copy, plus a "send as me" compose line that injects a message into the selected edge. Injection goes through the Phase 51 permission gate (`[api] allow_agent_messaging`), rate limits, loop guard and logging; without Phase 51 enabled the compose line is disabled. — **Reversibility:** costly — injection adds an outward-facing write path with security implications.

### Rendering
- **D-10:** TUI uses box-drawing/braille graph rendering. GUI uses smooth, high-resolution vector graphics built on the existing pixel canvas (`pixel_canvas`, `hires_subcell`): anti-aliased curved edges, animated flow particles along edges, node glow for state, hi-DPI aware, good graph layout algorithms. No new heavy dependency without sign-off (a vector crate such as tiny-skia is an option the plan must justify against cross-platform builds).

### Claude's Discretion
- Exact layout algorithm per mode, glyph choices within the semantic set, animation timings, splitter defaults, key bindings (must not collide with existing keys), and event-bus buffer sizes.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and requirements
- `.planning/ROADMAP.md` — Phase 50 and 51 detail under "Milestone v2.9 - Agent Fleet"
- `.planning/REQUIREMENTS.md` — FLT-01, AGT-01..03

### Code
- `src/wm/drawer.rs` — `WorkspaceDrawer`, `DrawerRow`, `DrawerConfig`, spinner/sparkline, selection, session move/branch
- `src/ui/drawer.rs` — drawer rendering
- `src/mcp_console/mod.rs`, `src/ui/mcp_ui.rs` — model for a pinnable/resizable/scalable window
- `src/ui/typography.rs` — `PanelTarget`, per-panel scale (the zoom window needs its own target)
- `src/input/mouse_wm.rs`, `src/input/modal.rs` — mouse/key handling patterns, Ctrl+wheel scale
- `src/ui/pixel_canvas.rs`, `src/ui/hires_subcell.rs`, `src/ui/canvas_raster.rs` — GUI vector drawing
- `src/wm/z_stack.rs` — window stacking
- `tests/panel_pin_scale.rs`, `tests/phase46_display_modes.rs` — test patterns

No external specs; herdr was described by the user, not read.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `WorkspaceDrawer` rows with indent tree, `spinner_for`, `sparkline_for`, `drawer_move_selected_session`, branch/reparent.
- Drawer pin (`Fixed`) and independent panel scale from v2.8.0.
- MCP console pin/focus/`size_override` for the zoom window.
- Headless test pattern: `WindowManager::new_headless`, `handle_wm_key_event`, `handle_wm_mouse_event`.

### Established Patterns
- Every command/key/RPC lowers to `Action` and runs via `WindowManager::apply`.
- Fields private; files ~800-1000 lines (drawer.rs already ~1015, so new code goes in new modules).
- Legacy output byte-identical; new looks are additive.

### Integration Points
- Drawer row builder (`drawer_rows`), renderer (`src/ui/renderer.rs` panel scratch drawing), `Action` enum, colon parser, control API events.

</code_context>

<specifics>
## Specific Ideas

- "Like herdr's left side": status glyphs, running agent visible at a glance.
- Zoom window: connections, flows, snoop on conversations, resizable like all other windows.

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within phase scope. (Agent messaging itself stays in Phase 51.)

</deferred>

---

*Phase: 50-herdr-style-left-sidebar-agent-aware-workspace-navigator*
*Context gathered: 2026-11-03*
