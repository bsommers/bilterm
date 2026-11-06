# Phase 54 Frozen Contract: Session Packing Viewer & Drag-and-Drop (VIEW-01..04, UNI-R01)

Authored for Phase 54 of Bilterm under `AGENTS.md` conventions (`ARCH-R01` unified Action dispatch, `ARCH-R02` encapsulated state on `WindowManager`, `ARCH-R04` modular source files <= 800-1000 lines, `UNI-R01` surface uniformity, `SEC-R01..04` private filesystem operations via `secure_fs`).

Every Phase 54 plan reads this contract. Plans 54-01, 54-02, and 54-03 build their respective modules against these frozen definitions.

## Wave Map

| Wave | Plan | Scope | Primary Files |
|------|------|-------|---------------|
| 1 | `54-01-PLAN.md` | Pure treemap packing algorithm (`VIEW-01`) & property tests | `src/wm/session_pack_layout.rs`, `tests/phase54_packing_layout.rs` |
| 2 | `54-02-PLAN.md` | Non-modal `SESSION MAP` window (`VIEW-02`), dual TUI & GUI rendering, chrome badges, `:sessions map` / `:packing` grammar | `src/wm/session_map.rs`, `src/wm/session_map_cmd.rs`, `src/ui/session_map_ui.rs`, `src/ui/session_map_gui.rs`, `tests/phase54_session_map_render.rs` |
| 3 | `54-03-PLAN.md` | Mouse drag-and-drop (`VIEW-03`), keyboard pickup/drop (`VIEW-04`), Navigator integration, RPC/MCP parity & E2E tests | `src/input/session_map.rs`, `src/input/session_map_keys.rs`, `src/input/drawer_navigator.rs`, `tests/phase54_drag_drop_parity.rs` |

---

## 1. Architectural Decisions

- **D-01 Pure, Deterministic Layout Engine (`VIEW-01`)**:
  - The packing layout is a 100% pure function: `pack_sessions_treemap(area: Rect, sessions: &[SessionPackInput]) -> SessionPackLayout`.
  - It performs no I/O, no terminal queries, and mutates no global or window-manager state.
  - Given identical input slices and bounding rect, output is bit-for-bit identical across runs, platforms, and architectures.
  - Adapts squarified treemap partitioning with an explicit terminal character cell aspect ratio factor ($k \approx 2.0$, width:height) to yield visually balanced rectangular boxes on standard text grids.
- **D-02 Non-Modal First-Class Utility Window (`VIEW-02`)**:
  - `SESSION MAP` follows the proven `AgentsList` (`src/wm/agents_list.rs`) and `FontsWindow` (`src/wm/fonts_window.rs`) pattern.
  - It is NOT a modal overlay: it floats above the composite terminal panes in the UI rendering pass.
  - It owns keyboard focus ONLY when `SessionMapState::focused == true` (`state.captures_input()`).
  - Terminal PTY output, background command execution, and live agent traffic under the window continue unhindered.
- **D-03 Scalable Dual Rendering (TUI Box/Braille & GUI Vector Textures)**:
  - **TUI (Ratatui `Buffer`)**: Clean Borland/Turbo Vision double-line frame (`╔═╗`/`║`), box-drawing container boundaries (`┌─┐`/`│`/`└─┘`), braille/half-block sub-cell separators, and sanitized text labels.
  - **GUI (`--gui` mode)**: Reuses Phase 50's `graph_raster` / `PixelCanvasBuffer` / `GuiImageTexture` infrastructure with dedicated texture ID `SESSION_MAP_TEXTURE_ID = 970_000`. Renders anti-aliased vector borders, soft container glows, drop-target indicator pulses, and high-resolution typography without introducing external GUI dependencies.
  - Scaled independently via `PanelTarget::SessionMap` (50%..300% zoom) through `[` / `]` keys or `Ctrl+Wheel`.
- **D-04 Window Chrome & Docking Integration**:
  - Title bar features interactive chips: `[•]` / `[o]` Pin chip, `[□]` Dock chip, and `[×]` Close button.
  - Pinning (`pinned: true`, toggled via `*` or `Alt+P`) preserves the window across pane clicks and blur events; unpinned closes on `Esc` or external click.
  - Docking (`docked: true`, toggled via `d` or `[□]`) embeds the packing map into the side tiling strip via `panel_dock.rs` (`DockPanel::SessionMap`).
- **D-05 Unified Drag-and-Drop Across Viewer and Navigator (`VIEW-03`)**:
  - Windows can be dragged between session containers inside the `SESSION MAP` window AND between session nodes in the Navigator drawer (`src/ui/drawer/navigator.rs`).
  - Active drag maintains explicit pointer grab bookkeeping (`SessionMapDrag::WindowDrag { window_id, source_session, grab_offset }`).
  - Hovering detects the candidate session container; valid drop targets are highlighted with accent borders (`[DROP HERE]`); invalid targets (source session or non-session regions) display warning/disabled styling.
  - Releasing mouse over a valid target session lowers to Phase 53's undoable move action (`Action::WindowMove { window_id, target_session_id }`).
  - `Esc` or releasing outside valid targets immediately cancels the drag cleanly with user status feedback.
- **D-06 Complete Keyboard Pickup/Drop Equivalents (`VIEW-04`, `UNI-R01`)**:
  - Every mouse drag action has an exact keyboard counterpart in the focused viewer:
    - `Space` / `Enter`: pick up the selected window (`holding: Some(WindowPickup)`).
    - `h`/`j`/`k`/`l` / Arrow keys / `Tab`: navigate target sessions, highlighting candidate drop targets.
    - `Enter`: drop held window into target session (triggers `Action::WindowMove`).
    - `Esc`: cancel pickup, returning window to source without mutation.
  - Complete `:sessions map` colon command family with full autocomplete, help catalog entries, and keybindings in `src/input/keymap.rs`.
- **D-07 Strict Invariants & Graceful Degradation**:
  - Zero-panic invariant: Degenerate areas (`width == 0`, `height == 0`, tiny boxes `< 4x2`) degrade gracefully to compact counts (`[3w]`), glyphs, or blank cells.
  - Strict containment: Every window rect is strictly contained within its session's `inner_rect`; every session is contained within `area`.
  - Zero overlap: Sibling session rectangles and sibling window rectangles have mutually disjoint cell interiors.
  - Display sanitization: All session names and window titles pass through `sanitize_display` (`src/vt/mod.rs`), stripping ANSI escapes and control characters before rendering.

---

## 2. VIEW-01: Pure Squarified Treemap Layout Specification

### 2.1 Data Structures (`src/wm/session_pack_layout.rs`)

```rust
use ratatui::layout::Rect;
use crate::pane::PaneId;

/// Input node describing a window inside a session for packing.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowPackInput {
    pub window_id: PaneId,
    pub title: String,
    pub is_active: bool,
    pub weight: f64,
}

/// Input node describing a session container (can be nested).
#[derive(Clone, Debug, PartialEq)]
pub struct SessionPackInput {
    pub session_id: String,
    pub name: String,
    pub is_active: bool,
    pub weight: f64,
    pub windows: Vec<WindowPackInput>,
    pub children: Vec<SessionPackInput>,
}

/// Computed layout box for an individual window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowLayoutBox {
    pub window_id: PaneId,
    pub title: String,
    pub rect: Rect,
    pub is_active: bool,
    pub display_label: String,
    pub degraded: bool,
}

/// Computed layout box for a session container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionLayoutBox {
    pub session_id: String,
    pub name: String,
    pub outer_rect: Rect,
    pub inner_rect: Rect,
    pub header_rect: Rect,
    pub is_active: bool,
    pub windows: Vec<WindowLayoutBox>,
    pub children: Vec<SessionLayoutBox>,
    pub degraded: bool,
}

/// Full layout result computed by the treemap engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPackLayout {
    pub area: Rect,
    pub sessions: Vec<SessionLayoutBox>,
    pub degraded: bool,
}
```

### 2.2 Algorithm & Aspect Ratio Correction

1. **Aspect Ratio Metric**:
   Terminal characters have a height approximately $2.0 \times$ their width. To achieve visually balanced boxes, the aspect ratio penalty for a cell rectangle of width $w$ and height $h$ with character aspect factor $k = 2.0$ is:
   $$\rho(w, h) = \max\left(\frac{w}{k \cdot h}, \frac{k \cdot h}{w}\right)$$
2. **Squarified Partitioning**:
   - For a container rectangle $R$, partition candidate items along the shorter visual axis.
   - Add items to a row/strip while adding the item improves or preserves $\rho(w, h)$.
   - When adding an item worsens the ratio, fix the current row and recurse on the remaining bounding rectangle.
   - For identical weights, tie-break deterministically using lexical comparison on `session_id` and `window_id`.
3. **Hierarchy & Margins**:
   - Each session container claims a border: outer frame width 1 cell on all sides (`inner_rect.x = outer_rect.x + 1`, `inner_rect.y = outer_rect.y + 2` to account for header).
   - If session contains nested child sessions, partition `inner_rect` between direct windows and child session containers according to aggregate weights.
4. **Degradation Thresholds**:
   - `area.width < 10 || area.height < 4`: Whole layout marks `degraded = true`.
   - `outer_rect.width < 6 || outer_rect.height < 3`: Session degrades to summary glyph/badge `[S: Nw]` with no internal window boxes.
   - `window.rect.width < 5 || window.rect.height < 2`: Window degrades to numeric ID or single letter.
   - `window.rect.width == 0 || window.rect.height == 0`: Clamped to empty rect `Rect::default()`, no text rendered.

---

## 3. VIEW-02: Non-Modal `SESSION MAP` Window & Dual Rendering

### 3.1 Window State (`src/wm/session_map.rs`)

```rust
use ratatui::layout::Rect;
use crate::pane::PaneId;
use crate::wm::WindowResizeEdge;
use crate::wm::session_pack_layout::SessionPackLayout;

/// Active pointer drag gesture inside the session map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionMapDrag {
    /// Moving the floating window: grab offset from window origin.
    TitleMove { grab_dx: u16, grab_dy: u16 },
    /// Resizing the floating window borders.
    BorderResize { edge: WindowResizeEdge, initial: Rect },
    /// Dragging a window between sessions.
    WindowDrag {
        window_id: PaneId,
        source_session: String,
        grab_x: u16,
        grab_y: u16,
        current_x: u16,
        current_y: u16,
        target_session: Option<String>,
    },
}

/// Keyboard holding state for pick-up/drop operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowPickup {
    pub window_id: PaneId,
    pub source_session: String,
    pub target_session: String,
}

/// State of the SESSION MAP non-modal window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionMapState {
    pub active: bool,
    pub pinned: bool,
    pub focused: bool,
    pub docked: bool,
    pub rect: Option<Rect>,
    pub selected_session: usize,
    pub selected_window: Option<PaneId>,
    pub holding: Option<WindowPickup>,
    pub drag: Option<SessionMapDrag>,
    pub cached_layout: Option<SessionPackLayout>,
}

impl SessionMapState {
    pub fn captures_input(&self) -> bool {
        self.active && self.focused
    }
}
```

### 3.2 Operations & Colon Grammar (`src/wm/session_map_cmd.rs`)

```rust
use crate::pane::PaneId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionMapOp {
    Open,
    Close,
    Toggle,
    Dismiss,
    Blur,
    Focus,
    Pin(Option<bool>),
    Dock(Option<bool>),
    ScaleStep(i16),
    SelectNextSession,
    SelectPrevSession,
    SelectNextWindow,
    SelectPrevWindow,
    SelectWindow(PaneId),
    Pickup(Option<PaneId>),
    SetTarget(String),
    Drop,
    Cancel,
    Move { dx: i16, dy: i16 },
    Resize { dw: i16, dh: i16 },
    SetRect { x: u16, y: u16, w: u16, h: u16 },
}
```

Colon grammar:
- `:sessions map` / `:packing` (toggle open/close)
- `:sessions map open` / `close` / `toggle` / `dismiss` / `blur`
- `:sessions map pin [on|off|toggle]`
- `:sessions map dock [on|off|toggle]`
- `:sessions map scale <+10|-10|val>`
- `:sessions map pickup [window-id]`
- `:sessions map target <session-id>`
- `:sessions map drop`
- `:sessions map cancel`

### 3.3 TUI & GUI Rendering Specifications

- **TUI Renderer (`src/ui/session_map_ui.rs`)**:
  - Outlines: `render_borland_frame_styled` using `cfg.window_outline`.
  - Top header: `SESSION MAP (<N> sessions, <M> windows)` with title badges `[s:Scale] [•] [□] [×]`.
  - Subheader status row: `Focused: <session_name> | Holding: <window_title or None> | Key: Space=Pickup Enter=Drop Esc=Cancel`.
  - Session containers: Rendered with single-line box drawing `┌─┐`/`│`/`└─┘`. If active session, rendered with `palette.title_fg` and bold text.
  - Window boxes: Rendered inside session containers with distinct background tint, showing sanitized window title and active indicator `*`.
  - Drop Target Highlight: If `holding.is_some()` or `drag.is_some()`, the candidate target session container border is drawn with high-contrast blinking/bold accent (`tokens.accent`).
- **GUI Vector Renderer (`src/ui/session_map_gui.rs`)**:
  - Reuses `src/ui/graph_raster.rs` and `PixelCanvasBuffer`.
  - Texture ID: `pub const SESSION_MAP_TEXTURE_ID: u32 = 970_000;`.
  - Paints antialiased rounded rectangles for session containers, subtle drop shadows, smooth window cards, and vector text labels.
  - GUI panel scaling: mapped through `PanelTarget::SessionMap` in `src/ui/typography.rs`.

---

## 4. VIEW-03: Mouse Drag-and-Drop Lifecycle & Navigator Integration

### 4.1 Viewer Hit Testing & Planning (`src/input/session_map.rs`)

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionMapHit {
    Outside,
    Border(WindowResizeEdge),
    TitleBar,
    CloseBadge,
    DockBadge,
    PinBadge,
    ScaleBadge,
    SessionContainer(String),
    WindowBox { window_id: PaneId, session_id: String },
}
```

1. **Mouse Down (`MouseEventKind::Down(MouseButton::Left)`)**:
   - Hit on `WindowBox { window_id, session_id }`:
     - Initiates `SessionMapDrag::WindowDrag { window_id, source_session: session_id, grab_x, grab_y, current_x, current_y, target_session: None }`.
     - Event is swallowed (`swallow = true`).
   - Hit on `TitleBar`: Starts `TitleMove`.
   - Hit on `Border(edge)`: Starts `BorderResize`.
   - Hit on chips: Dispatches `Pin`, `Dock`, `Close`, `ScaleStep`.
2. **Mouse Drag (`MouseEventKind::Drag(MouseButton::Left)`)**:
   - Updates `current_x`, `current_y`.
   - Hit-tests underlying position against session container rectangles.
   - If hovered over session container $S_{target}$ where $S_{target} \neq S_{source}$:
     - `target_session = Some(S_{target})`.
   - Otherwise, `target_session = None`.
3. **Mouse Up (`MouseEventKind::Up(MouseButton::Left)`)**:
   - If `target_session == Some(target)`:
     - Lowers to `Action::WindowMove { window_id, target_session_id: target }`.
     - Resets drag to `None`.
     - Displays status feedback: `"Moved window #<id> to session '<target>'"`.
   - If `target_session.is_none()` or dropped outside:
     - Resets drag to `None` (safe no-op).
     - Displays status feedback: `"Drop cancelled"`.
4. **Escape Key (`Esc`)**:
   - If active drag exists, `Esc` resets drag to `None` without mutation.

### 4.2 Navigator Tree Drag Integration (`src/input/drawer_navigator.rs`)

- Dragging a window node in the Navigator drawer (`src/ui/drawer/navigator.rs`) initiates the same cross-session drag protocol.
- Dragging over another session node in the tree highlights that session node.
- Releasing over the target session node dispatches `Action::WindowMove { window_id, target_session_id }`.

---

## 5. VIEW-04: Keyboard Equivalents & Surface Parity

### 5.1 In-Window Declarative Keymap (`src/input/session_map_keys.rs`)

When `SessionMapState::captures_input() == true`, key events are looked up in `SESSION_MAP_BINDINGS`:

| Key | Mode / Modifier | Operation (`SessionMapOp`) | Description |
|-----|-----------------|----------------------------|-------------|
| `k` / `Up` | Normal | `SelectPrevWindow` | Navigate to previous window |
| `j` / `Down` | Normal | `SelectNextWindow` | Navigate to next window |
| `h` / `Left` | Normal | `SelectPrevSession` | Navigate to previous session |
| `l` / `Right` | Normal | `SelectNextSession` | Navigate to next session |
| `Tab` | Normal | `SelectNextSession` | Cycle session container focus |
| `BackTab` | Shift+Tab | `SelectPrevSession` | Reverse cycle session containers |
| `Space` / `m` | Normal | `Pickup(None)` | Pick up selected window |
| `Enter` | Normal (Holding) | `Drop` | Drop held window into target session |
| `Enter` | Normal (Not Holding) | `Pickup(None)` | Pick up selected window |
| `Esc` | Holding | `Cancel` | Cancel pickup; return window to source |
| `Esc` | Not Holding (Unpinned) | `Dismiss` | Close session map window |
| `Esc` | Not Holding (Pinned) | `Blur` | Release keyboard focus to panes |
| `*` / `Alt+P` | Normal | `Pin(None)` | Toggle pinned open state |
| `d` | Normal | `Dock(None)` | Toggle docked into tiling strip |
| `[` / `]` | Normal | `ScaleStep(-10)` / `+10` | Zoom / scale viewer panel |
| `Shift+Arrows` | Shift | `Move { dx, dy }` | Move window position by 1 cell |
| `Alt+Arrows` | Alt | `Resize { dw, dh }` | Resize window rect by 1 cell |

### 5.2 Uniformity Matrix (`UNI-R01`)

Every session map capability MUST be reachable across all four surfaces:

| Capability | Mouse Surface | Keyboard Surface | Colon Command | Automation API (RPC/MCP) |
|------------|---------------|------------------|---------------|--------------------------|
| Open / Toggle | Click status/menu chip | Leader `S`, `Ctrl+Alt+M` | `:sessions map`, `:packing` | `session.map_toggle` |
| Close / Dismiss | Click `[×]` button | `Esc` | `:sessions map close` | `session.map_close` |
| Pin / Unpin | Click `[•]` / `[o]` chip | `*` / `Alt+P` | `:sessions map pin toggle` | `session.map_pin` |
| Dock / Float | Click `[□]` chip | `d` | `:sessions map dock toggle` | `session.map_dock` |
| Pick up Window | Mouse down on window | `Space` / `m` | `:sessions map pickup <id>` | `window.pickup` |
| Target Session | Mouse hover container | Arrow keys / `Tab` | `:sessions map target <id>` | `window.target` |
| Drop Window | Mouse up over container | `Enter` | `:sessions map drop` | `window.drop` / `window.move` |
| Cancel Drag | Press `Esc` or drop outside | `Esc` | `:sessions map cancel` | `window.cancel_move` |
| Scale Step | `Ctrl+Wheel` / `[s:Scale]` | `[` / `]` | `:sessions map scale <pct>` | `session.map_scale` |

---

## 6. Safety Boundaries & Invariant Guarantees

1. **State Encapsulation (`ARCH-R02`)**:
   - `SessionMapState` is a private encapsulated field on `WindowManager`.
   - Mutated exclusively via `wm.apply(Action::SessionMap(op))` or `wm.apply(Action::WindowMove { .. })`.
2. **Deterministic Layout Assurance**:
   - Squarified algorithm must never divide by zero or encounter floating point `NaN`/`Inf`.
   - All weights clamped to `weight.max(1.0)`.
   - Output coordinates clamped to terminal bounding box via `saturating_add` and `.min(area.right())`.
3. **Display Sanitization (`SEC-R01..04`)**:
   - Session names and window titles retrieved from untrusted processes/commands are passed through `sanitize_display` before inclusion in layout display labels or TUI cell symbols.
4. **Move Action Lowering & Undo Safety**:
   - Valid drops lower directly to Phase 53's `Action::WindowMove`.
   - The move operation is recorded in the session journal as a single undoable transaction (`:undo` restores the window to its original session and position).
   - Moving the last window out of a session does NOT delete or orphan the session unless explicitly requested.

---

## 7. Verification & Test Plan

1. **Unit & Property Tests (`tests/phase54_packing_layout.rs`)**:
   - `test_packing_determinism`: Identical input slices produce bit-for-bit identical `SessionPackLayout`.
   - `test_packing_containment`: Every window rect $\subseteq$ session inner rect $\subseteq$ outer rect $\subseteq$ bounding area.
   - `test_packing_no_overlap`: All sibling rectangles satisfy $A \cap B = \emptyset$.
   - `test_packing_nested_sessions`: Correct hierarchical indentation and containment for sub-sessions.
   - `test_packing_degraded_bounds`: Tested on `0x0`, `1x1`, `5x2`, `10x3`, and `200x60` grids without panics.
2. **Rendering Tests (`tests/phase54_session_map_render.rs`)**:
   - `test_tui_render_snapshot`: Golden snapshot of session map in standard Ratatui buffer.
   - `test_gui_vector_render`: Verifies `draw_session_map_vector` writes to `PixelCanvasBuffer` and assigns `SESSION_MAP_TEXTURE_ID`.
   - `test_badge_hit_coordinates`: Exact coordinate alignment between rendered badge spans and mouse hit-test rects.
3. **Drag-and-Drop & Parity Tests (`tests/phase54_drag_drop_parity.rs`)**:
   - `test_mouse_drag_drop_success`: Dragging window across containers triggers `Action::WindowMove` and updates session hierarchy.
   - `test_mouse_drag_drop_cancel`: Dropping outside or pressing `Esc` leaves state unchanged.
   - `test_keyboard_pickup_drop_parity`: Proves `Space` + arrow + `Enter` produces identical state mutations to mouse drag-drop, colon commands, and JSON-RPC calls.
   - `test_navigator_drag_drop`: Proves dragging window in Navigator tree triggers identical move action.
