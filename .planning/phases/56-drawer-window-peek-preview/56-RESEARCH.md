# Phase 56: Drawer Window Peek & Live Preview Pane — Research

**Researched:** 2026-11-07
**Requirements:** `DRW-PEEK-01`, `DRW-PEEK-02`, `DRW-PEEK-03`
**Mode:** Codebase Architecture & Non-Destructive Compositor Inspection

## 1. Current Drawer Scroll, Highlight, and Selection Behavior

### 1.1 Classic & Navigator Drawer State (`src/wm/drawer.rs`, `src/wm/drawer/navigator.rs`)
- `WorkspaceDrawer` (`src/wm/drawer.rs:146-175`) maintains:
  - `pub open: bool` (whether the drawer panel is visible),
  - `pub selected_index: usize` (currently highlighted row index in `drawer_rows()`),
  - `pub scroll_offset: usize` (viewport scroll offset into `drawer_rows()`),
  - `pub collapsed_sessions: BTreeSet<String>` and `pub collapsed_groups: BTreeSet<String>`.
- `WindowManager::drawer_rows(&self) -> Vec<DrawerRow>` (`src/wm/drawer.rs:348-353`) dispatches on `self.config.drawer.style`:
  - `DrawerStyle::Classic`: returns `classic_drawer_rows()`, containing `DrawerRowKind::Pane(id)` for every active `PaneId`, optionally `DrawerRowKind::QuickTerminal`, and `DrawerRowKind::SavedSession(name)` for each session in `self.window_manage.saved_sessions`.
  - `DrawerStyle::Navigator`: returns `navigator_drawer_rows()`, paired with `navigator_drawer_rows_with_meta(&self) -> (Vec<DrawerRow>, Vec<NavigatorRowMeta>)` (`src/wm/drawer/navigator.rs:176-184`), emitting hierarchical `NavigatorLevel::{Group, Session, Window}` rows.
- Currently in `WalkCtx::walk` (`src/wm/drawer/navigator.rs:212-234`), child `Window` (`DrawerRowKind::Pane(id)`) rows are emitted only when `is_active && !collapsed` (i.e. only for `self.history_session`). Background live sessions in `self.window_manage.background_sessions` do not yet expand their child `PaneId` rows in the Navigator tree.

### 1.2 Current Keyboard & Mouse Scrolling Behavior (`src/input/modal.rs`, `src/input/drawer_navigator.rs`, `src/input/mouse_wm.rs`)
- **Keyboard in Classic Drawer (`ModalState::Drawer` in `src/input/modal.rs:502-565`)**:
  - `Up` / `k`, `Down` / `j`, `PageUp`, `PageDown` call `self.drawer_select_prev()`, `self.drawer_select_next()`, or `self.drawer_scroll_by(±5)` (`src/wm/drawer/ops.rs:58-87`).
  - These methods update `self.drawer.selected_index` and auto-adjust `self.drawer.scroll_offset`, **without** changing `self.active_pane_id` or `self.z_stack`.
  - Pressing `Enter` or `Space` calls `self.drawer_activate_selected()`, which commits focus (`focus_pane(id)` + `unminimize(id)`, or `switch_session(&name)`) and closes the drawer.
- **Keyboard in Navigator Drawer (`src/input/drawer_navigator.rs:42-120`)**:
  - `Up` / `k`, `Down` / `j`, `PageUp`, `PageDown` emit `Action::DrawerNavigate(DrawerNavOp::{Prev, Next, Scroll(±5)})`, moving `self.drawer.selected_index` across group, session, and window rows without activating them.
  - `Enter` emits `Action::DrawerNavigate(DrawerNavOp::Activate)` (`src/wm/drawer/nav.rs:88-127`), which activates the highlighted pane or switches to the highlighted session.
- **Mouse Wheel & Hover (`src/input/drawer_navigator.rs:144-160`, `src/input/mouse_wm.rs:835-945`)**:
  - In `Navigator` mode (`src/input/drawer_navigator.rs:152-158`), `ScrollUp` / `ScrollDown` inside `drawer_rect` calls `apply_drawer_nav(DrawerNavOp::Scroll(-1/+1))` (`step * 3`), which advances `self.drawer.selected_index`.
  - In `Classic` vertical mode (`src/input/mouse_wm.rs:867-875`), `ScrollUp` / `ScrollDown` currently calls `self.drawer.wheel_scroll(±1, rows.len(), visible_rows)`, which scrolls `scroll_offset` without moving `selected_index` (and does not open the drawer if it wasn't focused).
  - Left-clicking a row in either drawer immediately activates the row (`focus_pane` or `switch_session`).

### 1.3 Why Highlighting Alone Is Insufficient
When a user scrolls down the drawer list (`Up`/`Down`/`j`/`k` or mouse wheel), `self.drawer.selected_index` highlights each row in turn, but the user cannot see the contents of that window if:
1. **Stacked Mode Occlusion (`WorkspaceLayoutMode::Stacked`)**: The highlighted window is buried underneath the active top-of-Z-stack window or overlaps other cascades.
2. **Minimized Windows**: The highlighted window is minimized (`self.z_stack.is_minimized(id)`), so `WorkspaceWidget` skips rendering it entirely (`src/ui/mod.rs:365-367`, `467-469`).
3. **Background Live Sessions (`self.window_manage.background_sessions`)**: The highlighted item is a live background session (or a window inside a background session) parked in `WindowManageState::background_sessions` (`src/wm/window_manage.rs:33-46`) while another session is active.
4. **Cold Saved Sessions (`SavedWorkspaceSession`)**: The highlighted item is a saved session snapshot on disk (`~/.config/bilterm/sessions/<name>.toml` or in-memory test store) that has not yet been spawned.
5. **Distant Tiled Split / Zoomed Active Pane**: When another pane is zoomed (`self.zoomed_pane.is_some()`), all non-zoomed split panes are hidden from the workspace canvas.

## 2. Non-Destructive Rendering of Live & Snapshot Windows into a Floating Preview Pane

### 2.1 Zero-Mutation / Zero-`SIGWINCH` Live `CompositePane` Rendering
- `CompositePaneWidget` (`src/ui/mod.rs:28-235`) takes an immutable reference `pane: &'a CompositePane` and renders into any target `ratatui::layout::Rect` on `buf: &mut Buffer`:
  - It calls `pane.vt_state()` (`Mutex<VtState>` lock for screen grid read) without mutating `CompositePane` fields (`prompt`, `vt_worker`, `line_discipline`, `mode`, `focus`) or sending `MasterPty::resize` (`SIGWINCH`).
  - In `CompositePaneWidget::render` (`src/ui/mod.rs:144-169`), cell iteration automatically clips to the widget's `canvas: Rect` (`let render_rows = (canvas.height as usize).min(vt_rows); let render_cols = (canvas.width as usize).min(vt_cols);`).
  - Passing `focused: false` suppresses the active cross-hair cursor shader and active focus ring while preserving full ANSI colors, scrollback state, OIA status bar, and Borland/Modern window border.
- Therefore, **any live `&CompositePane`** can be rendered into a floating `preview_rect: Rect` with **zero side effects**:
  1. **Active-session pane (`DrawerRowKind::Pane(id)` in active session)**: `self.panes.get(&id)` — even if minimized, occluded in `Stacked` mode, or hidden behind a zoomed pane.
  2. **QuickTerminal (`DrawerRowKind::QuickTerminal`)**: `&self.quick_terminal.pane` — even when `self.quick_terminal.visible == false`.
  3. **Live Background Session (`DrawerRowKind::SavedSession(name)` or background `Pane(id)` where `self.window_manage.background_sessions.contains_key(&name)`)**:
     - `BackgroundSessionState` (`src/wm/window_manage.rs:33-46`) holds `pub panes: BTreeMap<PaneId, CompositePane>`, `pub active_pane_id: PaneId`, `pub z_stack: ZStack`, and `pub layout_tree: SplitNode`.
     - Its PTY reader threads continue updating each background `CompositePane`'s `VtState` in real time.
     - Looking up `bg.panes.get(&pane_id)` (or `bg.panes.get(&bg.active_pane_id)`) yields a live `&CompositePane` that `CompositePaneWidget` can render directly into the preview pane!
  4. **Navigator Background Session Child Windows**:
     - By extending `WalkCtx::walk` in `src/wm/drawer/navigator.rs` to also emit child `DrawerRowKind::Pane(id)` rows (with `NavigatorRowMeta.session_name = Some(node.name.clone())`) when an uncollapsed session exists in `self.window_manage.background_sessions`, users can expand a background session in the Navigator drawer, scroll through its individual open windows, preview each live window, and press `Enter` or click the preview to switch to that session and focus that specific `PaneId`.

### 2.2 Cold Saved-Session Snapshot Preview (`SavedWorkspaceSession`)
- When a highlighted `DrawerRowKind::SavedSession(name)` is **not** in `background_sessions` (cold session on disk or in-memory test snapshot), `self.load_session_snapshot_any(&name)` (`src/wm/window_manage.rs:367-374`) loads its `SavedWorkspaceSession` (`src/wm/session.rs:58-87`), which contains:
  - `panes: Vec<SavedPaneState>` (`id`, `cwd`, `custom_title`, `scrollback_tail: Vec<String>`, `command_history: Vec<String>`, `minimized`),
  - `active_pane_id: u32`, `layout_mode`, `theme`.
- To avoid disk I/O on every 60fps render frame, `DrawerPreviewState` on `WorkspaceDrawer` caches `cached_session_snapshot: Option<(String, SavedWorkspaceSession)>`, refreshing it only when the previewed session name changes.
- The preview renderer draws a structured Borland/Modern preview card showing the saved session's active pane (or window list summary + `scrollback_tail` lines + `cwd` + window count) inside `preview_rect`.

### 2.3 TUI & GUI Compositor Layering (`src/ui/mod.rs`, `src/ui/renderer.rs`)
- **TUI (`WorkspaceWidget::render` in `src/ui/mod.rs:675-686`)**:
  - `render_workspace_drawer(self.wm, ws_area, buf)` runs at line 678 after tiled/stacked panes, float overlays, and `QuickTerminal`, and before top/context menus and modal dialogs.
  - Rendering `drawer_preview::render_drawer_preview_overlay(self.wm, ws_area, buf)` immediately after `render_workspace_drawer(self.wm, ws_area, buf)` ensures the floating preview pane sits cleanly above the workspace canvas (with `occlude_rect` + drop shadow / Liquid Glass halo) while never obscuring context menus or modal dialogs (`Help`, `CommandPalette`, `ConfirmClose`).
- **GUI (`GuiWorkspaceRenderer::render_workspace_with_cell_size` in `src/ui/renderer.rs:546-578`)**:
  - The GUI renderer runs `WorkspaceWidget` into an offscreen `ratatui::buffer::Buffer` and then composites per-pane scaled sub-buffers unless a cell is covered by an overlay (`is_overlay_cell` at line 569).
  - Adding `let drawer_preview_rect = wm.drawer_preview_rect(ws_area);` to `is_overlay_cell` (`|| drawer_preview_rect.is_some_and(|r| r.contains(pos))`) guarantees that the GUI renderer preserves the preview overlay cells and never overwrites them with underlying scaled pane pixels or smooth-scroll subcell offsets.

### 2.4 Non-Destructive Invariants (`ARCH-R02`)
- Opening, updating, or dismissing the Peek Preview Pane **MUST NOT** mutate:
  - `self.active_pane_id`,
  - `self.z_stack` order or minimized flags (`self.z_stack.is_minimized(id)`),
  - `self.zoomed_pane`,
  - `self.history_session` or `self.window_manage.background_sessions`,
  - Any `CompositePane` internal state (`prompt`, `vt_worker`, `line_discipline`, `mode`, `focus`).
- Only **committing** the preview (clicking inside `drawer_preview_rect`, pressing `Enter` while previewing/focused in drawer, or executing `Action::DrawerPreview(DrawerPreviewOp::Commit)`) performs the focus transition:
  - Active-session `Pane(id)`: `self.z_stack.unminimize(id)`, `self.focus_pane(id)`, clear `drawer.preview`, and close/unfocus the drawer.
  - `QuickTerminal`: open/focus `QuickTerminal`, clear `drawer.preview`, and close/unfocus the drawer.
  - Background/Saved `Session(name)` (with optional target `pane_id`): `self.switch_session(&name)`, optionally `self.focus_pane(pane_id)` + `self.z_stack.unminimize(pane_id)`, clear `drawer.preview`, and close/unfocus the drawer.
