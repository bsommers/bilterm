use std::collections::BTreeMap;

use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

use crate::pane::PaneId;
use crate::wm::split_tree::{SplitDirection, SplitTree};
use crate::wm::{WindowManager, WorkspaceLayoutMode};

/// Window border or corner being dragged during an interactive mouse resize.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Active workspace-level mouse drag gesture for window moving or border/split resizing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WmMouseDrag {
    /// Dragging a window's top title bar (`col = new_x + grab_dx`, `row = new_y + grab_dy`).
    MoveWindow {
        pane_id: PaneId,
        grab_dx: u16,
        grab_dy: u16,
    },
    /// Dragging a window border or corner (`◢`) to resize a pane/window in Stacked or Tiled mode.
    ResizeWindow {
        pane_id: PaneId,
        edge: WindowResizeEdge,
        initial_rect: Rect,
    },
    /// Dragging a modal window's top title bar (`Help`, `Inspector`, `Palette`).
    MoveModal {
        grab_dx: u16,
        grab_dy: u16,
    },
    /// Dragging a modal window's border or bottom-right `◢` corner (`Help`, `Inspector`, `Palette`).
    ResizeModal {
        edge: WindowResizeEdge,
        initial_rect: Rect,
    },
    /// Dragging the Quake quick terminal's bottom border or `◢` corner.
    ResizeQuickTerminal,
}

/// Per-window floating position and size override in `WorkspaceLayoutMode::Stacked`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloatingWindowGeometry {
    pub x_offset: Option<u16>,
    pub y_offset: Option<u16>,
    pub width: Option<u16>,
    pub height: Option<u16>,
    #[serde(default)]
    pub minimized: bool,
}

impl FloatingWindowGeometry {
    pub fn is_custom(&self) -> bool {
        self.x_offset.is_some()
            || self.y_offset.is_some()
            || self.width.is_some()
            || self.height.is_some()
            || self.minimized
    }
}

/// Z-Order Deck Stack Manager (`WM-02`).
///
/// Stores panes in ascending z-order (`order[0]` is at the bottom of the deck,
/// `order.last()` is at the top of the deck and holds active focus).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZStack {
    order: Vec<PaneId>,
    #[serde(default)]
    geometries: BTreeMap<PaneId, FloatingWindowGeometry>,
    #[serde(skip)]
    pub modal_rect: Option<Rect>,
    #[serde(skip)]
    pub modal_maximized: bool,
}

impl ZStack {
    pub fn new(initial_pane: PaneId) -> Self {
        Self {
            order: vec![initial_pane],
            geometries: BTreeMap::new(),
            modal_rect: None,
            modal_maximized: false,
        }
    }

    pub fn from_order(order: Vec<PaneId>) -> Self {
        Self {
            order: if order.is_empty() { vec![1] } else { order },
            geometries: BTreeMap::new(),
            modal_rect: None,
            modal_maximized: false,
        }
    }

    pub fn from_order_and_geometries(
        order: Vec<PaneId>,
        geometries: BTreeMap<PaneId, FloatingWindowGeometry>,
    ) -> Self {
        Self {
            order: if order.is_empty() { vec![1] } else { order },
            geometries,
            modal_rect: None,
            modal_maximized: false,
        }
    }

    /// Returns the per-window floating geometry map (`WM-02`).
    pub fn geometries(&self) -> &BTreeMap<PaneId, FloatingWindowGeometry> {
        &self.geometries
    }

    /// Returns panes ordered from bottom (`0`) to top (`len - 1`).
    pub fn order(&self) -> &[PaneId] {
        &self.order
    }

    /// Returns the topmost (active/focused) `PaneId`.
    pub fn top(&self) -> Option<PaneId> {
        self.order.last().copied()
    }

    /// Returns `true` if `pane_id` has a custom position or size override.
    pub fn has_custom_geometry(&self, pane_id: PaneId) -> bool {
        self.geometries
            .get(&pane_id)
            .is_some_and(FloatingWindowGeometry::is_custom)
    }

    /// Returns `true` if any pane in the stack has a custom position or size override.
    pub fn has_any_custom_geometry(&self) -> bool {
        self.geometries.values().any(FloatingWindowGeometry::is_custom)
    }

    /// Clears all custom window position/size overrides, restoring the default Borland cascade.
    pub fn reset_geometries(&mut self) {
        self.geometries.clear();
        self.modal_rect = None;
        self.modal_maximized = false;
    }

    /// Toggles the minimized state of `pane_id`.
    pub fn toggle_minimize(&mut self, pane_id: PaneId) -> bool {
        let entry = self.geometries.entry(pane_id).or_default();
        entry.minimized = !entry.minimized;
        entry.minimized
    }

    pub fn is_minimized(&self, pane_id: PaneId) -> bool {
        self.geometries.get(&pane_id).is_some_and(|g| g.minimized)
    }

    /// Pushes a newly created `PaneId` to the top of the z-order stack.
    pub fn push(&mut self, pane_id: PaneId) {
        self.order.retain(|&id| id != pane_id);
        self.order.push(pane_id);
    }

    /// Removes a closed `PaneId` from the z-order stack.
    pub fn remove(&mut self, pane_id: PaneId) {
        self.order.retain(|&id| id != pane_id);
        self.geometries.remove(&pane_id);
    }

    /// Selectively raises `pane_id` to the top of the z-order stack (`Alt+1..9` or mouse click).
    pub fn raise(&mut self, pane_id: PaneId) -> bool {
        if let Some(pos) = self.order.iter().position(|&id| id == pane_id) {
            let id = self.order.remove(pos);
            self.order.push(id);
            true
        } else {
            false
        }
    }

    /// Shuffles/rotates the deck top-to-bottom (`Alt+Tab` / `F6` / `:deck next`):
    /// sends the current top pane to the bottom of the deck (`index 0`) and promotes the next pane to the top.
    pub fn shuffle_top_to_bottom(&mut self) -> Option<PaneId> {
        if self.order.len() > 1 {
            let top = self.order.pop().unwrap();
            self.order.insert(0, top);
        }
        self.top()
    }

    /// Shuffles/rotates the deck bottom-to-top (`Alt+Shift+Tab` / `:deck prev`):
    /// pulls the bottom-most pane (`index 0`) to the top of the deck.
    pub fn shuffle_bottom_to_top(&mut self) -> Option<PaneId> {
        if self.order.len() > 1 {
            let bottom = self.order.remove(0);
            self.order.push(bottom);
        }
        self.top()
    }

    /// Returns `(1-based rank, total_panes)` where `rank == total_panes` is the topmost pane.
    pub fn rank_of(&self, pane_id: PaneId) -> Option<(usize, usize)> {
        self.order
            .iter()
            .position(|&id| id == pane_id)
            .map(|idx| (idx + 1, self.order.len()))
    }

    /// Computes overlapping Borland Turbo Vision cascade rectangles for each pane in ascending z-order,
    /// applying any user-dragged/resized [`FloatingWindowGeometry`] overrides clamped to `area`.
    pub fn compute_cascade_rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let count = self.order.len();
        if count == 0 {
            return Vec::new();
        }
        if count == 1 && !self.has_custom_geometry(self.order[0]) {
            return vec![(self.order[0], area)];
        }

        let max_step_x = (area.width / 5).clamp(2, 4);
        let max_step_y = (area.height / 6).clamp(1, 2);
        let eff_steps = count.saturating_sub(1).max(1) as u16;
        let total_offset_x = max_step_x * eff_steps;
        let total_offset_y = max_step_y * eff_steps;

        let default_w = area
            .width
            .saturating_sub(total_offset_x)
            .max(20)
            .min(area.width);
        let default_h = area
            .height
            .saturating_sub(total_offset_y)
            .max(6)
            .min(area.height);

        self.order
            .iter()
            .enumerate()
            .map(|(idx, &pane_id)| {
                let geom = self.geometries.get(&pane_id).copied().unwrap_or_default();
                let min_w = 16.min(area.width.max(1));
                let min_h = 5.min(area.height.max(1));
                let w = geom
                    .width
                    .unwrap_or(default_w)
                    .clamp(min_w, area.width.max(1));
                let h = if geom.minimized {
                    3.min(area.height.max(1))
                } else {
                    geom.height
                        .unwrap_or(default_h)
                        .clamp(min_h, area.height.max(1))
                };
                let def_rel_x = (idx as u16) * max_step_x;
                let def_rel_y = (idx as u16) * max_step_y;
                let rel_x = geom
                    .x_offset
                    .unwrap_or(def_rel_x)
                    .min(area.width.saturating_sub(w));
                let rel_y = geom
                    .y_offset
                    .unwrap_or(def_rel_y)
                    .min(area.height.saturating_sub(h));
                (pane_id, Rect::new(area.x + rel_x, area.y + rel_y, w, h))
            })
            .collect()
    }

    /// Returns the current computed `Rect` for `pane_id` inside `area`.
    pub fn rect_for(&self, pane_id: PaneId, area: Rect) -> Option<Rect> {
        self.compute_cascade_rects(area)
            .into_iter()
            .find_map(|(id, r)| (id == pane_id).then_some(r))
    }

    /// Applies minimized state (`[_]`) to `tile_rect` in `Tiled` mode without pinning width/height.
    pub fn apply_tiled_rect(&self, pane_id: PaneId, tile_rect: Rect) -> Rect {
        if let Some(&geom) = self.geometries.get(&pane_id)
            && geom.minimized
        {
            return Rect::new(tile_rect.x, tile_rect.y, tile_rect.width, 3.min(tile_rect.height.max(1)));
        }
        tile_rect
    }

    /// Sets the exact floating `rect` for `pane_id` clamped inside `area`.
    pub fn set_window_rect(&mut self, pane_id: PaneId, rect: Rect, area: Rect) -> Rect {
        let min_w = 16.min(area.width.max(1));
        let min_h = 5.min(area.height.max(1));
        let w = rect.width.clamp(min_w, area.width.max(1));
        let h = rect.height.clamp(min_h, area.height.max(1));
        let rel_x = rect
            .x
            .saturating_sub(area.x)
            .min(area.width.saturating_sub(w));
        let rel_y = rect
            .y
            .saturating_sub(area.y)
            .min(area.height.saturating_sub(h));
        self.geometries.insert(
            pane_id,
            FloatingWindowGeometry {
                x_offset: Some(rel_x),
                y_offset: Some(rel_y),
                width: Some(w),
                height: Some(h),
                minimized: false,
            },
        );
        Rect::new(area.x + rel_x, area.y + rel_y, w, h)
    }

    /// Returns `pane_id`'s current `Rect`, automatically sizing an un-resized full/near-full
    /// cascade window to ~75% of `area` so it has room to move freely across the workspace.
    pub fn prepare_movable_rect(&self, pane_id: PaneId, area: Rect) -> Option<Rect> {
        let mut cur = self.rect_for(pane_id, area)?;
        let geom = self.geometries.get(&pane_id).copied().unwrap_or_default();
        if geom.width.is_none() && cur.width > (area.width * 3) / 4 {
            cur.width = ((area.width * 3) / 4).max(20).min(area.width);
        }
        if geom.height.is_none() && cur.height > (area.height * 3) / 4 {
            cur.height = ((area.height * 3) / 4).max(6).min(area.height);
        }
        Some(cur)
    }

    /// Moves `pane_id`'s floating window by `(dx, dy)` inside `area`.
    pub fn move_window(
        &mut self,
        pane_id: PaneId,
        dx: i16,
        dy: i16,
        area: Rect,
    ) -> Option<Rect> {
        let current = self.prepare_movable_rect(pane_id, area)?;
        let max_x = i32::from(area.x) + i32::from(area.width.saturating_sub(current.width));
        let max_y = i32::from(area.y) + i32::from(area.height.saturating_sub(current.height));
        let new_x = (i32::from(current.x) + i32::from(dx)).clamp(i32::from(area.x), max_x) as u16;
        let new_y = (i32::from(current.y) + i32::from(dy)).clamp(i32::from(area.y), max_y) as u16;
        Some(self.set_window_rect(
            pane_id,
            Rect::new(new_x, new_y, current.width, current.height),
            area,
        ))
    }

    /// Resizes `pane_id`'s floating window by `(dw, dh)` inside `area`.
    pub fn resize_window(
        &mut self,
        pane_id: PaneId,
        dw: i16,
        dh: i16,
        area: Rect,
    ) -> Option<Rect> {
        let current = self.rect_for(pane_id, area)?;
        let min_w = i32::from(16.min(area.width.max(1)));
        let min_h = i32::from(5.min(area.height.max(1)));
        let new_w = (i32::from(current.width) + i32::from(dw))
            .clamp(min_w, i32::from(area.width.max(1))) as u16;
        let new_h = (i32::from(current.height) + i32::from(dh))
            .clamp(min_h, i32::from(area.height.max(1))) as u16;
        Some(self.set_window_rect(pane_id, Rect::new(current.x, current.y, new_w, new_h), area))
    }
}

impl WindowManager {
    /// Returns `true` when workspace panes should render a 1-cell Borland window frame (`╔═╗`).
    pub fn is_framed(&self) -> bool {
        self.panes.len() > 1
            || self.layout_mode == WorkspaceLayoutMode::Stacked
            || self.z_stack.has_any_custom_geometry()
    }

    pub fn z_stack_mut(&mut self) -> &mut ZStack {
        &mut self.z_stack
    }

    pub fn split_tree_mut(&mut self) -> &mut SplitTree {
        &mut self.split_tree
    }

    pub fn wm_mouse_drag(&self) -> Option<WmMouseDrag> {
        self.wm_mouse_drag
    }

    pub fn set_wm_mouse_drag(&mut self, drag: Option<WmMouseDrag>) {
        self.wm_mouse_drag = drag;
    }

    fn current_workspace_area(&self) -> (Rect, Rect) {
        let (rows, cols) = self.default_size;
        let full = Rect::new(0, 0, cols.max(1), rows.max(1));
        let top_off = u16::from(self.config.show_top_menubar && full.height >= 4);
        let bot_off = u16::from(self.config.show_dos_fkey_bar && full.height >= 4 + top_off);
        let ws = Rect::new(
            full.x,
            full.y + top_off,
            full.width,
            full.height.saturating_sub(top_off + bot_off),
        );
        let eff = self.effective_workspace_area(ws);
        (full, eff)
    }

    /// Computes the active modal window's outer `Rect` (`Help`, `Inspector`, or `Palette`),
    /// honoring user mouse/keyboard resize, drag-move, and `[□]` maximize state.
    pub fn active_modal_rect(&self, workspace_area: Rect) -> Option<Rect> {
        let (def_w, def_h) = match self.modal {
            super::Modal::Help { .. } => (
                workspace_area
                    .width
                    .saturating_sub(4)
                    .clamp(20, 78)
                    .min(workspace_area.width),
                workspace_area
                    .height
                    .saturating_sub(2)
                    .clamp(6, 22)
                    .min(workspace_area.height),
            ),
            super::Modal::Inspector { .. } => (
                workspace_area
                    .width
                    .saturating_sub(4)
                    .clamp(24, 76)
                    .min(workspace_area.width),
                workspace_area
                    .height
                    .saturating_sub(2)
                    .clamp(6, 20)
                    .min(workspace_area.height),
            ),
            super::Modal::Palette { .. } => (
                workspace_area
                    .width
                    .saturating_sub(4)
                    .clamp(24, 76)
                    .min(workspace_area.width),
                workspace_area
                    .height
                    .saturating_sub(2)
                    .clamp(6, 14)
                    .min(workspace_area.height),
            ),
            super::Modal::ExitGuard { .. } => (
                workspace_area
                    .width
                    .saturating_sub(4)
                    .clamp(24, 68)
                    .min(workspace_area.width),
                workspace_area
                    .height
                    .saturating_sub(2)
                    .clamp(6, 12)
                    .min(workspace_area.height),
            ),
            super::Modal::None | super::Modal::Menubar { .. } => return None,
        };
        if self.z_stack.modal_maximized {
            return Some(workspace_area);
        }
        if let Some(r) = self.z_stack.modal_rect {
            let min_w = 20.min(workspace_area.width.max(1));
            let min_h = 5.min(workspace_area.height.max(1));
            let w = r.width.clamp(min_w, workspace_area.width.max(1));
            let h = r.height.clamp(min_h, workspace_area.height.max(1));
            let max_x = workspace_area.x + workspace_area.width.saturating_sub(w);
            let max_y = workspace_area.y + workspace_area.height.saturating_sub(h);
            return Some(Rect::new(
                r.x.clamp(workspace_area.x, max_x),
                r.y.clamp(workspace_area.y, max_y),
                w,
                h,
            ));
        }
        let x = workspace_area.x + (workspace_area.width.saturating_sub(def_w)) / 2;
        let y = workspace_area.y + (workspace_area.height.saturating_sub(def_h)) / 2;
        Some(Rect::new(x, y, def_w, def_h))
    }

    /// Sets the custom floating `Rect` for the active modal (`Help`, `Inspector`, `Palette`).
    pub fn set_modal_rect(&mut self, rect: Rect, workspace_area: Rect) -> Rect {
        self.z_stack.modal_maximized = false;
        let min_w = 20.min(workspace_area.width.max(1));
        let min_h = 5.min(workspace_area.height.max(1));
        let w = rect.width.clamp(min_w, workspace_area.width.max(1));
        let h = rect.height.clamp(min_h, workspace_area.height.max(1));
        let max_x = workspace_area.x + workspace_area.width.saturating_sub(w);
        let max_y = workspace_area.y + workspace_area.height.saturating_sub(h);
        let clamped = Rect::new(
            rect.x.clamp(workspace_area.x, max_x),
            rect.y.clamp(workspace_area.y, max_y),
            w,
            h,
        );
        self.z_stack.modal_rect = Some(clamped);
        clamped
    }

    /// Toggles `[□]` maximize on the active modal (`Help`, `Inspector`, `Palette`).
    pub fn toggle_maximize_modal(&mut self) {
        self.z_stack.modal_maximized = !self.z_stack.modal_maximized;
    }

    /// Toggles `[_]` minimize on `pane_id`, collapsing it to a 3-row title bar in `Stacked` mode
    /// or restoring its full size.
    pub fn toggle_minimize_pane(&mut self, pane_id: PaneId) {
        if self.zoomed_pane == Some(pane_id) {
            self.zoomed_pane = None;
        }
        if self.layout_mode == WorkspaceLayoutMode::Tiled {
            self.layout_mode = WorkspaceLayoutMode::Stacked;
        }
        let min_now = self.z_stack.toggle_minimize(pane_id);
        let (full_area, _) = self.current_workspace_area();
        let _ = self.resize_all(full_area);
        self.active_pane_mut().set_status_message(Some(format!(
            "Window #{pane_id} {}",
            if min_now { "MINIMIZED [_]" } else { "RESTORED" }
        )));
    }

    /// Resizes the active pane/window by `(dw, dh)` columns/rows in either `Stacked` or `Tiled` mode.
    pub fn resize_focused_pane_or_window(&mut self, dw: i16, dh: i16) {
        let (full_area, ws_area) = self.current_workspace_area();
        if let Some(modal_r) = self.active_modal_rect(ws_area) {
            let new_w = (i32::from(modal_r.width) + i32::from(dw)).max(20) as u16;
            let new_h = (i32::from(modal_r.height) + i32::from(dh)).max(5) as u16;
            self.set_modal_rect(Rect::new(modal_r.x, modal_r.y, new_w, new_h), ws_area);
            return;
        }
        let active_id = self.active_pane_id();
        if self.layout_mode == WorkspaceLayoutMode::Stacked || self.panes.len() == 1 {
            if self.panes.len() == 1 && self.layout_mode == WorkspaceLayoutMode::Tiled {
                self.layout_mode = WorkspaceLayoutMode::Stacked;
            }
            if let Some(r) = self.z_stack.resize_window(active_id, dw, dh, ws_area) {
                let _ = self.resize_all(full_area);
                self.active_pane_mut().set_status_message(Some(format!(
                    "Window #{active_id} size: {}x{} @ ({},{})",
                    r.width, r.height, r.x, r.y
                )));
            }
        } else {
            let mut changed = false;
            if dw != 0 {
                changed |= self.split_tree.adjust_ratio_directional(
                    active_id,
                    SplitDirection::Vertical,
                    dw.saturating_mul(2),
                );
            }
            if dh != 0 {
                changed |= self.split_tree.adjust_ratio_directional(
                    active_id,
                    SplitDirection::Horizontal,
                    dh.saturating_mul(3),
                );
            }
            if changed {
                let _ = self.resize_all(full_area);
                let rect_str = self
                    .compute_workspace_rects(ws_area)
                    .into_iter()
                    .find_map(|(id, r)| {
                        (id == active_id).then(|| format!("{}x{}", r.width, r.height))
                    })
                    .unwrap_or_else(|| "updated".to_string());
                self.active_pane_mut()
                    .set_status_message(Some(format!("Pane #{active_id} split size: {rect_str}")));
            }
        }
    }

    /// Moves the active window by `(dx, dy)` in `Stacked` mode, or swaps panes in `Tiled` mode.
    pub fn move_focused_pane_or_window(&mut self, dx: i16, dy: i16) {
        let (full_area, ws_area) = self.current_workspace_area();
        if let Some(modal_r) = self.active_modal_rect(ws_area) {
            let new_x = (i32::from(modal_r.x) + i32::from(dx)).max(i32::from(ws_area.x)) as u16;
            let new_y = (i32::from(modal_r.y) + i32::from(dy)).max(i32::from(ws_area.y)) as u16;
            self.set_modal_rect(Rect::new(new_x, new_y, modal_r.width, modal_r.height), ws_area);
            return;
        }
        let active_id = self.active_pane_id();
        if self.layout_mode == WorkspaceLayoutMode::Stacked || self.panes.len() == 1 {
            if self.panes.len() == 1 && self.layout_mode == WorkspaceLayoutMode::Tiled {
                self.layout_mode = WorkspaceLayoutMode::Stacked;
            }
            if let Some(r) = self.z_stack.move_window(active_id, dx, dy, ws_area) {
                let _ = self.resize_all(full_area);
                self.active_pane_mut().set_status_message(Some(format!(
                    "Window #{active_id} pos: ({},{}) [{}x{}]",
                    r.x, r.y, r.width, r.height
                )));
            }
        } else {
            let leaves = self.split_tree.leaves();
            if leaves.len() > 1
                && let Some(pos) = leaves.iter().position(|&id| id == active_id)
            {
                let step_forward = dx > 0 || dy > 0;
                let other_idx = if step_forward {
                    (pos + 1) % leaves.len()
                } else {
                    (pos + leaves.len() - 1) % leaves.len()
                };
                let other_id = leaves[other_idx];
                if self.split_tree.swap_leaves(active_id, other_id) {
                    let _ = self.resize_all(full_area);
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Swapped Pane #{active_id} <-> Pane #{other_id}"
                    )));
                }
            }
        }
    }

    /// Resets custom floating window geometries and equalizes all split ratios (`50%`).
    pub fn reset_window_geometries(&mut self) {
        let (full_area, _) = self.current_workspace_area();
        self.z_stack.reset_geometries();
        self.split_tree.reset_ratios();
        let _ = self.resize_all(full_area);
        self.active_pane_mut().set_status_message(Some(
            "Reset window cascade & equalized split ratios".to_string(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn z_stack_shuffles_top_to_bottom_bottom_to_top_and_selectively_raises() {
        let mut stack = ZStack::new(1);
        stack.push(2);
        stack.push(3);
        assert_eq!(stack.order(), &[1, 2, 3]);
        assert_eq!(stack.top(), Some(3));

        // Shuffle top-to-bottom: 3 goes to bottom (0), 2 becomes top
        assert_eq!(stack.shuffle_top_to_bottom(), Some(2));
        assert_eq!(stack.order(), &[3, 1, 2]);

        // Shuffle bottom-to-top: 3 comes back from bottom to top
        assert_eq!(stack.shuffle_bottom_to_top(), Some(3));
        assert_eq!(stack.order(), &[1, 2, 3]);

        // Selective raise pane #1 to top
        assert!(stack.raise(1));
        assert_eq!(stack.order(), &[2, 3, 1]);
        assert_eq!(stack.rank_of(1), Some((3, 3)));
    }
}
