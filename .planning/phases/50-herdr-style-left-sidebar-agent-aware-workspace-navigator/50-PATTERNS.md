# Phase 50: herdr-Style Left Sidebar - Pattern Map

**Mapped:** 2026-11-03
**Files analyzed:** 21 (13 new src, 8 modified src, 3 new tests; see table)
**Analogs found:** 21 / 21 (all analog paths verified git-tracked)

Line numbers are from the tree at commit 0915792. Re-grep before relying on them.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match |
|---|---|---|---|---|
| `src/wm/drawer.rs` (mod: `DrawerStyle`, `style`, `herdr_width_cols`, `DrawerRow` fields, dispatcher) | model/config | CRUD | itself (`DrawerEdge` / `DrawerConfig`, lines 26-109) | exact |
| `src/wm/drawer_herdr.rs` (new) | service | transform | `src/wm/drawer.rs::drawer_rows` (398+) | role-match |
| `src/wm/agent_state.rs` (new) | service | event-driven/transform | `update_drawer_telemetry` call site `src/wm/mod.rs:700` + `spinner_for` in drawer.rs | role-match |
| `src/wm/traffic.rs` (new) | model | pub-sub (ring buffer) | `AutomationEvent` `src/api/playbook.rs:14` (shape only; wrong layer) | partial |
| `src/wm/agents_window.rs` (new) | model/state | request-response | `McpConsoleState` `src/mcp_console/mod.rs:252-277` | role-match |
| `src/config/agents.rs` (new) | config | CRUD | `DrawerConfig` serde pattern | role-match |
| `src/ui/drawer_herdr.rs` (new) | component | request-response | `src/ui/drawer.rs::render_workspace_drawer` (146+) | exact |
| `src/ui/agents_ui.rs` (new) | component | request-response | `src/ui/mcp_ui.rs` | role-match |
| `src/ui/graph_raster.rs` (new) | utility | transform | `PixelCanvasBuffer` + `flush_truecolor_subcells_to_ratatui` (`src/ui/hires_subcell.rs:113`) | role-match |
| `src/ui/renderer.rs` (mod: GUI texture pass) | service | transform | pane-canvas texture push, lines 750-800 | exact |
| `src/ui/typography.rs` (mod: `PanelTarget::Agents`) | config | CRUD | `PanelTarget::Mcp` arms, lines 203-216 | exact |
| `src/input/drawer_herdr.rs` (new) | controller | event-driven | `handle_wm_modal_key` drawer block (`modal.rs:334+`) and `handle_drawer_mouse_event` (`mouse_wm.rs:672+`) | exact |
| `src/input/agents_window.rs` (new) | controller | event-driven | modal drag/resize in `mouse_wm.rs:218-228, 631-650` | role-match |
| `src/input/mouse_wm.rs`, `modal.rs`, `mouse.rs` (mod: delegations, `WmMouseDrag::MoveAgents/ResizeAgents`) | controller | event-driven | existing arms | exact |
| `src/wm/z_stack.rs` or `agents_window.rs` (rect clamp) | utility | transform | `active_modal_rect`/`set_modal_rect` `z_stack.rs:470-495` | exact |
| `src/action.rs`, `src/colon_command/*`, `src/pane/completion.rs`, `src/command_registry.rs` (mod) | route | request-response | existing `Drawer*` actions and `:drawer` rows | exact |
| `src/wm/mod.rs` (mod: fields, `poll_all_ptys` hook) | service | event-driven | `update_drawer_telemetry()` at line 700 | exact |
| `tests/drawer_classic_parity.rs` (new, FIRST) | test | snapshot | `tests/phase3_window_manager.rs:44` + `tests/api_config_commands.rs:10-25` | role-match |
| `tests/phase50_herdr_drawer.rs` (new) | test | request-response | `tests/slide_out_drawer_and_telemetry.rs:138-160`, `tests/panel_pin_scale.rs` | exact |
| `tests/phase50_agents_window.rs` (new) | test | request-response | `tests/panel_pin_scale.rs`, `tests/phase46_display_modes.rs:154,194` | role-match |

## Pattern Assignments

### `src/wm/drawer.rs` (config additions)

**Enum pattern** (lines 26-34, copy for `DrawerStyle`):
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrawerEdge { #[default] Left, Right, Top, Bottom }
```
**Config struct** (lines 89-109): `#[serde(default)] pub struct DrawerConfig { edge, visibility, layout, width_cols: u16, height_rows: u16 }` with a manual `Default` (width 34). Add `style: DrawerStyle` (default Classic) and `herdr_width_cols: u16` (28). Agent profiles: `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`.

**Row model** (lines 112-131): `DrawerRowKind { SessionHeader(String), LayoutGroup(..), Pane(PaneId), QuickTerminal, FleetNode(String) }`. `DrawerRow` has 10 fields; six `rows.push(DrawerRow {` literals exist (about lines 423, 437, 497, 513, 534, 567). Add `depth: u8` and `agent: Option<AgentRowMeta>` there. Do not add enum variants.

**Dispatcher** (line 402): rename the body of `pub fn drawer_rows(&self)` to `classic_drawer_rows` byte-for-byte; the new `drawer_rows` dispatches on `self.drawer_config().style`. Drawer.rs is about 1015 lines, so put the herdr logic in `drawer_herdr.rs`.

### `src/ui/drawer_herdr.rs` (renderer)

**Analog:** `src/ui/drawer.rs`

**Imports** (lines 3-18):
```rust
use ratatui::buffer::Buffer; use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::wm::{DrawerLayoutMode, DrawerRow, DrawerRowKind, WindowManager, inset_window_frame};
```
**Entry pattern** (lines 147-190): `render_workspace_drawer(wm, workspace_area, cfg, palette, glyphs, buf)`; `let Some(drawer_rect) = wm.drawer_rect(workspace_area) else { return };` then overlay shadow/halo, `occlude_rect(buf, drawer_rect)`, glass surface, title, `render_borland_frame_styled`. Add one branch `if dcfg.style == DrawerStyle::Herdr { return render_herdr_drawer(...) }`; the classic statements stay identical and in order. Metrics: `drawer_vertical_layout_metrics(cfg, inner_height, item_count) -> (header, footer, rows_per_item)` (line 21) is shared with the mouse handler; add `wm.drawer_metrics()` returning `(1,1,1)`-style for herdr. State colours from `ui::design::DesignTokens::from_palette` (fields `success, warning, danger, accent, text_muted`); ASCII fallback via `ui::glyphs::GlyphPack::Ascii`. Use `unicode-width` for truncation and a single `sanitize_display` helper for titles.

### `src/input/drawer_herdr.rs` (keys + mouse)

**Key delegation** (`src/input/modal.rs:334-345`): the focused-drawer block starts with the leader checks, then `match key.code`. Insert `if herdr { return herdr_key(wm, key) }` after the leader guard, so `handle_wm_key_event` stays <= 80 lines.
```rust
if wm.drawer().focused {
    if wm.is_leader_pending() { return Ok(None); }
    let inside_tmux = crate::tmux::TmuxEnv::detect().is_some();
    if super::keymap::is_leader_key_with_tmux(key, &wm.config().leader_key, inside_tmux) { return Ok(None); }
    match key.code { KeyCode::F(7) | KeyCode::Char('q') => wm.toggle_drawer(), ...
```
Handlers lower to actions: `let _ = wm.apply(crate::action::Action::DrawerSize(0));`. Free keys: `!` (next blocked), `e` (fold); leader `!`, `a` (zoom). Add a keymap-uniqueness test.

**Mouse** (`src/input/mouse_wm.rs:672-700`): `handle_drawer_mouse_event` begins with panel-scale remap and Ctrl+wheel:
```rust
let (mouse, workspace_area) = wm.panel_space(mouse, workspace_area, PanelTarget::Drawer);
if let Some(delta) = ctrl_wheel_scale_delta(&mouse)
    && wm.drawer_rect(workspace_area).is_some_and(|r| rect_contains(r, mouse.column, mouse.row))
{ wm.step_scale(PanelTarget::Drawer, delta); return Some(AppAction::Handled); }
```
Branch to the herdr mouse fn after badge/edge-resize handling and BEFORE the hard-coded header/footer column hit zones (lines ~751-815; `rel_x >= 38`, `dr.width-8`). Call sites of `drawer_vertical_layout_metrics` in mouse_wm.rs: ~822, 851, 867.

### `src/wm/agents_window.rs` (zoom window state)

**Analog:** `McpConsoleState` (`src/mcp_console/mod.rs:252-277`): fields `active, pinned, focused, size_override, h_scroll, layout_mode, edge` plus a doc-commented pin rule ("Pinned windows stay open and stop capturing the keyboard unless `focused`"). Copy the `pinned/focused/captures_input()` semantics (exercised by `tests/panel_pin_scale.rs`). Add `rect: Option<Rect>`, `maximized`, `splitter_pct: u16`, selection, filter. Integer percents only (`Action` derives `Eq`).

**Rect clamp** (`src/wm/z_stack.rs:470-495`): min 20x5, clamp w/h into workspace, clamp x/y to `max_x/max_y`; `set_modal_rect` resets maximized. Copy for `set_agents_rect`.

### `src/input/agents_window.rs` (drag/resize)

**Analog:** `src/input/mouse_wm.rs:218-228` and `625-650`:
```rust
if mouse.row == modal_outer.y {
    match detect_title_bar_badge_click(wm, modal_outer, mouse.column) {
        Some(0 | 2) => wm.close_modal(), Some(1) => wm.toggle_maximize_modal(),
        _ => wm.set_wm_mouse_drag(Some(WmMouseDrag::MoveModal { grab_dx: mouse.column.saturating_sub(modal_outer.x), grab_dy: 0 })),
    }
} else if let Some(edge) = detect_border_resize_edge(modal_outer, mouse.column, mouse.row) {
    wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeModal { edge, initial_rect: modal_outer }));
}
// drag arm
WmMouseDrag::ResizeModal { edge, initial_rect } => {
    let resized = apply_rect_edge_resize(initial_rect, edge, mouse.column, mouse.row, workspace_area);
    wm.set_modal_rect(resized, workspace_area); Ok(Some(AppAction::Handled)) }
```
Helpers are `pub(super)` in `input`, so keep the new module under `src/input/` (or widen). Do NOT make a `Modal` variant (MCP does not drag-resize either; RESEARCH Pattern 5). Precedence: zoom handler before `handle_drawer_mouse_event` in `src/input/mouse.rs:677-685`; zoom key handling before the drawer block in `handle_wm_modal_key`.

### `src/ui/typography.rs` (`PanelTarget::Agents`)

Lines 203-216: exhaustive matches add an arm per target:
```rust
PanelTarget::Drawer => self.drawer_scale_pct,
PanelTarget::Mcp => self.mcp_scale_pct,   // + Agents => self.agents_scale_pct
```
Clamp 75..=200. Also update `execute_wm.rs:~850` and renderer scale pass (`renderer.rs:~569-575, 701-731`).

### `src/ui/graph_raster.rs` and GUI pass in `src/ui/renderer.rs`

**TUI sink:** `canvas.flush_truecolor_subcells_to_ratatui(CanvasSubcellMode::Braille, rect, buf)` (`hires_subcell.rs:113`); only writes into cells whose symbol is `" "`, so draw labels first and clear the rect each frame.

**GUI sink** (`renderer.rs:~770-800`, copy exactly):
```rust
surface.images.push(GuiImageTexture {
    id: 900_000 + (pane_snap.id as u32),
    x_px: u32::from(rect.x) * u32::from(eff_cw), y_px: u32::from(rect.y) * u32::from(eff_ch),
    width_px: pix.width, height_px: pix.height,
    pixel_width: pix.width.min(u32::from(u16::MAX)) as u16,
    pixel_height: pix.height.min(u32::from(u16::MAX)) as u16,
    rgba_bytes: pix.rgba,
});
```
Use a disjoint id range (e.g. 950_000). Canvas must be opaque and rendered 1:1 (images are copied with alpha>0, no blending). Use `PixelCanvasBuffer::{draw_line_aa, draw_bezier_aa, draw_circle_aa}`. Detect GUI by renderer type, not `wm.gui_surface`.

### `src/wm/mod.rs` (tracker hook)

`poll_all_ptys` tail (lines ~695-702):
```rust
self.update_drawer_telemetry();
self.maybe_rebalance_scrollback(Instant::now());
changed
```
Add `self.agents.update(&self.panes, now)` here, taking `now: Instant` as a parameter (like `maybe_rebalance_scrollback`) so tests use synthetic time. Return `true` only on a throttled cadence when herdr + Working or zoom + traffic is active; classic return value is unchanged.

### Tests

**Parity goldens** (`tests/drawer_classic_parity.rs`): env isolation from `tests/api_config_commands.rs:10-25`:
```rust
static ENV_LOCK: Mutex<()> = Mutex::new(());
fn with_temp_xdg<T>(f: impl FnOnce(&std::path::Path) -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ... std::env::set_var("XDG_CONFIG_HOME", &dir); std::env::set_var("XDG_STATE_HOME", &dir); ...
```
Render with `TestBackend` + `buffer_to_snapshot_string` + `insta::assert_snapshot!` (pattern at `tests/phase3_window_manager.rs:44`). Do not call `poll_all_ptys()` (spinner nondeterminism). Commit before any `src/` edit.

**Herdr/zoom tests** (`tests/panel_pin_scale.rs:1-40`): `fn run(wm, cmd)` via `parse_colon_command(cmd).unwrap().unwrap(); wm.apply(c)`; `WindowManager::new_headless(30, 100)`; `handle_wm_key_event` / `handle_wm_mouse_event`. For scale tests use `set_gui_surface(true)` (`tests/phase46_display_modes.rs:154,194`). Detector fixtures (`tests/slide_out_drawer_and_telemetry.rs:138-160`): `pane_mut(id).process_pty_output(b"...\x1b]133;D;42\x07")`, `submit_command("cargo test")`.

## Shared Patterns

### Everything lowers to Action
**Source:** `src/action.rs` (Action derives `Clone, Debug, PartialEq, Eq`, no f32). New variants must be added to `every_action_is_reachable_from_keymap_colon_or_rpc` (~line 457), completion table (`src/pane/completion.rs:~483` `(":drawer", &[...])`), and help/palette drift tests (`src/command_registry.rs:155-195`). Follow the 6-step checklist in AGENTS.md. Do not extend `:state` JSON (exact-key test at `tests/config_commands_edge.rs:196`); use a new `:agents list` command data path.

### File size gates
`src/**/*.rs` target ~800-1000, hard 1200 (`tests/session_lessons_and_regression_suite.rs:375`). Over-budget now: drawer.rs 1015, modal.rs 925, mouse_wm.rs 887, execute_wm.rs 853, wm/mod.rs 865. All new logic goes in new modules.

### Untrusted text
One `sanitize_display(&str)` (strip C0/C1, bidi overrides, cap length) used by sidebar rows and snoop view.

### Classic parity
Add branches; never edit shared classic bodies. Golden `.snap` files committed first and gated with `git diff --exit-code tests/snapshots/drawer_classic_parity__*`.

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `src/wm/traffic.rs` | model | pub-sub ring buffer | No in-wm event bus; only `AutomationEvent` (`src/api/playbook.rs:14`) in the higher `api` layer. Plan from RESEARCH (ring cap 512, seq cursor, `kind()` string). |
| Force-directed layout in `agents_window.rs` | utility | transform | No graph layout exists; use RESEARCH (fixed-iteration Fruchterman-Reingold, node-id-hash seeded). |
| Compose line (disabled stub) | component | request-response | `[api] allow_agent_messaging` does not exist (`ApiConfig` only has `allow_config_write`, `src/config/dotted.rs:15-20`). |

## Metadata

**Analog search scope:** `src/wm`, `src/ui`, `src/input`, `src/mcp_console`, `src/config`, `tests/`
**Files scanned:** about 20 (targeted excerpts only)
**Notes:** `secure_fs` is at `src/secure_fs.rs` (not `src/util/`); Phase 50 persists nothing. The second page of RESEARCH.md (lines 447-539) was not read.
**Pattern extraction date:** 2026-11-03
