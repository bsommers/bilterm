# Phase 56: Drawer Window Peek & Live Preview Pane — Contract

**Phase:** 56 (`56-drawer-window-peek-preview`)
**Requirements:** `DRW-PEEK-01`, `DRW-PEEK-02`, `DRW-PEEK-03`
**Architecture Governance:** `ARCH-R01` (Unified `Action` Dispatch), `ARCH-R02` (Encapsulated State Invariants), `ARCH-R04` (Module Size Ceiling), `UNI-R01` (Surface Uniformity)

---

## 1. Requirement Specifications

### `DRW-PEEK-01`: Transient Peek Preview on Drawer Scroll / Highlight
1. When the workspace drawer is open (`self.drawer.open == true`), scrolling through or highlighting window/session rows in either `DrawerStyle::Classic` or `DrawerStyle::Navigator` (`Up`/`Down`/`j`/`k`/`PageUp`/`PageDown`, mouse wheel over the drawer, mouse hover over drawer rows when preview is enabled, or explicit preview toggle `v` / `Alt+P` / `:drawer preview`) pops up a floating **Peek Preview Pane** over the workspace canvas showing the highlighted item's live terminal canvas (or saved-session snapshot) without selecting/focusing it.
2. **Non-Destructive Guarantee (`ARCH-R02`)**:
   - While peeking, `self.active_pane_id`, `self.z_stack` order, `self.z_stack.is_minimized(id)`, `self.zoomed_pane`, `self.history_session`, `self.window_manage.background_sessions`, and PTY window sizes (`SIGWINCH`) **MUST NOT** change.
   - Occluded windows in `WorkspaceLayoutMode::Stacked`, minimized windows, hidden non-zoomed panes, `QuickTerminal`, live windows in `self.window_manage.background_sessions`, and cold `SavedWorkspaceSession` snapshots on disk are all previewable.
3. **Navigator Background Session Window Expansion**:
   - In `DrawerStyle::Navigator`, expanding a session that resides in `self.window_manage.background_sessions` lists its live child `DrawerRowKind::Pane(id)` rows (tagged with `session_name: Some(session_name)` in `NavigatorRowMeta`) so the user can scroll across individual open windows of background sessions and preview each one live.

### `DRW-PEEK-02`: Click or Enter Commits Focus; Esc or Blur Dismisses Cleanly
1. **Commit Focus**:
   - **Mouse Click on Preview Pane**: Left-clicking anywhere inside `drawer_preview_rect(ws_area)` (except the `[×]` close button on the preview header) commits focus to the previewed window/session (`WindowManager::commit_drawer_preview()`), closes the preview and drawer, and routes focus to the target window.
   - **Keyboard `Enter` in Drawer**: Pressing `Enter` while a drawer item is highlighted/previewed activates and focuses the target window/session (including focusing a specific `PaneId` inside a background session when a background window row is selected) and closes the preview and drawer.
   - **Close Badge Click / `Esc`**: Clicking the `[×]` badge on the preview header or pressing `Esc` when a preview is active dismisses the preview (`DrawerPreviewOp::Off`) without changing the active window or session.

### `DRW-PEEK-03`: `UNI-R01` Surface Uniformity
Every operation on the Drawer Peek Preview MUST lower to `Action::DrawerPreview(DrawerPreviewOp)` via `WindowManager::apply`:
- **Keyboard**:
  - `Alt+P` (`DEFAULT_GLOBAL_BINDINGS`) and `Ctrl+B P` (`DEFAULT_LEADER_BINDINGS`): `Action::DrawerPreview(DrawerPreviewOp::Toggle)` — opens the drawer if closed, and toggles the live peek preview pane for the currently highlighted drawer row.
  - Inside Drawer (`Classic` and `Navigator`):
    - `Up`/`Down`/`j`/`k`/`PageUp`/`PageDown` move `selected_index` and automatically activate/refresh the live Peek Preview (`self.drawer.preview.active = true` when `self.config.drawer.auto_preview_on_scroll` is true, which defaults to `true`).
    - `v` or `Space` (in Navigator or when toggling preview): toggles `Action::DrawerPreview(DrawerPreviewOp::Toggle)`.
    - `Enter`: commits the previewed/selected item into focus (`DrawerPreviewOp::Commit` / `drawer_activate_selected`).
    - `Esc`: if `self.drawer.preview.active` is `true`, first dismisses the preview (`DrawerPreviewOp::Off`) or closes drawer if preview is already off.
- **Mouse**:
  - Scrolling the mouse wheel (`ScrollUp` / `ScrollDown`) over `drawer_rect` in **both** `Classic` and `Navigator` styles moves `self.drawer.selected_index` and pops up / updates the Peek Preview Pane (`self.drawer.preview.active = true`) without stealing active workspace pane focus.
  - Moving/hovering over drawer rows when `self.drawer.preview.active` is `true` updates `selected_index` and the preview target in real time.
  - Clicking the `[👁 Preview]` button in the Navigator footer or Classic header toggles `Action::DrawerPreview(DrawerPreviewOp::Toggle)`.
  - Clicking inside `drawer_preview_rect` commits focus to the previewed target (`Action::DrawerPreview(DrawerPreviewOp::Commit)`); clicking `[×]` on the preview title bar dismisses it (`Action::DrawerPreview(DrawerPreviewOp::Off)`).
- **Colon Commands (`:drawer preview` / `:peek`)**:
  - `:drawer preview` or `:drawer preview toggle` or `:peek` or `:peek toggle` → `Action::DrawerPreview(DrawerPreviewOp::Toggle)`
  - `:drawer preview on` or `:peek on` → `Action::DrawerPreview(DrawerPreviewOp::On)`
  - `:drawer preview off` or `:peek off` → `Action::DrawerPreview(DrawerPreviewOp::Off)`
  - `:drawer preview commit` or `:peek commit` → `Action::DrawerPreview(DrawerPreviewOp::Commit)`
  - `:drawer preview pane <id>` or `:peek pane <id>` → `Action::DrawerPreview(DrawerPreviewOp::Pane(PaneId(id)))`
  - `:drawer preview session <name>` or `:peek session <name>` → `Action::DrawerPreview(DrawerPreviewOp::Session(name))`
- **Command Palette, Top Menu (`Windows` menu), Help Overlay, JSON-RPC (`drawer.preview`), MCP (`bilterm_drawer_preview`), and `bilterm ctl`**:
  - Full parity covered by `every_action_is_reachable_from_keymap_colon_or_rpc` and `tests/phase56_drawer_preview_uniformity.rs`.

---

## 2. Data Structures & State Contract

### 2.1 `DrawerPreviewOp` (`src/action.rs`)
```rust
/// Operations for the Drawer Window Peek & Live Preview Pane (`DRW-PEEK-01..03`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawerPreviewOp {
    /// Enable the floating peek preview for the currently highlighted drawer row.
    On,
    /// Dismiss the floating peek preview without changing focus or session.
    Off,
    /// Toggle the floating peek preview on or off.
    Toggle,
    /// Commit focus to the currently previewed window/session and close the preview.
    Commit,
    /// Explicitly preview a specific open window by `PaneId`.
    Pane(PaneId),
    /// Explicitly preview a saved or background session by name.
    Session(String),
}
```
Added to `pub enum Action` in `src/action.rs`:
```rust
    /// Control the Drawer Window Peek & Live Preview Pane (`DRW-PEEK-01..03`).
    DrawerPreview(DrawerPreviewOp),
```

### 2.2 `DrawerPreviewTarget` & `DrawerPreviewState` (`src/wm/drawer.rs`)
```rust
/// Resolved target displayed inside the floating Drawer Peek Preview Pane (`DRW-PEEK-01`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawerPreviewTarget {
    /// Live pane in the currently active workspace session.
    ActivePane(PaneId),
    /// The Quake/Guake drop-down QuickTerminal pane.
    QuickTerminal,
    /// Live pane inside a warm background session (`window_manage.background_sessions`).
    BackgroundPane {
        session_name: String,
        pane_id: PaneId,
    },
    /// Cold saved session snapshot on disk (`SavedWorkspaceSession`).
    SavedSession {
        session_name: String,
    },
}

/// Ephemeral state for the Drawer Window Peek & Live Preview Pane (`DRW-PEEK-01..03`).
#[derive(Debug, Clone, Default)]
pub struct DrawerPreviewState {
    /// Whether the floating peek preview overlay is currently visible.
    pub active: bool,
    /// Optional explicit target override (set by `:drawer preview pane <id>` or `session <name>`);
    /// when `None`, the preview tracks the currently highlighted drawer row (`selected_index`).
    pub override_target: Option<DrawerPreviewTarget>,
    /// Cached cold session snapshot `(session_name, snapshot)` to prevent per-frame disk I/O.
    pub cached_session_snapshot: Option<(String, crate::wm::session::SavedWorkspaceSession)>,
}
```
- `WorkspaceDrawer` gains `pub preview: DrawerPreviewState`.
- `DrawerConfig` (`src/wm/drawer.rs`) gains:
  ```rust
  /// Automatically pop up the floating peek preview when scrolling/navigating the drawer (`DRW-PEEK-01`).
  #[serde(default = "default_auto_preview_on_scroll")]
  pub auto_preview_on_scroll: bool,
  ```
  where `default_auto_preview_on_scroll() -> bool { true }`.
- `NavigatorRowMeta` (`src/wm/drawer/navigator.rs`) gains:
  ```rust
  /// For `NavigatorLevel::Window` rows belonging to a background session, the owning session name.
  pub session_name: Option<String>,
  ```

---

## 3. Geometry & Rendering Contract (`src/wm/drawer.rs`, `src/ui/drawer_preview.rs`)

### 3.1 `WindowManager::drawer_preview_rect(&self, area: Rect) -> Option<Rect>`
- Returns `Some(Rect)` if and only if `self.drawer.preview.active` is `true` and `self.resolve_drawer_preview_target().is_some()` and `area.width >= 24 && area.height >= 8`.
- **Dock-Aware Positioning**:
  - When `self.drawer.open` is `true` and `let Some(d_rect) = self.drawer_rect(area)`:
    - **`DrawerPosition::Left`**: Placed immediately to the right of the drawer (`x = d_rect.right().saturating_add(1)`), width `((area.width.saturating_sub(d_rect.width + 2)) * 65 / 100).clamp(28, 96)`, vertically centered (`height = (area.height * 75 / 100).clamp(10, area.height.saturating_sub(2))`).
    - **`DrawerPosition::Right`**: Placed immediately to the left of the drawer (`x = d_rect.x.saturating_sub(preview_w + 1)`), same width/height clamping.
    - **`DrawerPosition::Top`**: Placed immediately below the top drawer (`y = d_rect.bottom().saturating_add(1)`), horizontally centered.
    - **`DrawerPosition::Bottom`**: Placed immediately above the bottom drawer (`y = d_rect.y.saturating_sub(preview_h + 1)`), horizontally centered.
  - When `self.drawer.open` is `false` (e.g. triggered directly via `:drawer preview pane <id>`): centered inside `area` (`65%` width × `75%` height, clamped to `area`).

### 3.2 Visual Chrome (`src/ui/drawer_preview.rs`)
- Calls `occlude_rect(buf, preview_rect, ...)` and renders Borland/Modern drop shadow or Liquid Glass halo matching `self.config.visual`.
- Outer double-line/rounded frame with top header banner:
  - Left title: ` 👁 PEEK PREVIEW: #<id> <title> [<status>] ` (e.g. `[MINIMIZED]`, `[STACKED-OCCLUDED]`, `[BG SESSION: <name>]`, `[SAVED SNAPSHOT]`, or `[LIVE]`).
  - Right badges: `[⏎/Click: Focus] [×]` (where clicking `[×]` at `rect.right() - 4..rect.right() - 1` dismisses the preview, and clicking anywhere else inside `preview_rect` commits focus).
- Inner body (`preview_rect.inner(Margin::new(1, 1))`):
  - For `ActivePane(id)`, `QuickTerminal`, and `BackgroundPane { session_name, pane_id }`: renders the live `&CompositePane` via `CompositePaneWidget::with_config(pane, false, &pane_cfg)` so the full ANSI screen, scrollback, and OIA status bar are visible live.
  - For `SavedSession { session_name }`: renders the cached `SavedWorkspaceSession` summary header (session name, layout mode, pane count, active pane CWD) and the active saved pane's `scrollback_tail` lines.
- GUI Compositor (`src/ui/renderer.rs`): `wm.drawer_preview_rect(ws_area)` is included in `is_overlay_cell` so GUI pixel compositing preserves the preview overlay.
