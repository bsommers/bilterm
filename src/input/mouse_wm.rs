use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::error::Result;
use crate::input::AppAction;
use crate::wm::split_tree::SplitDirection;
use crate::wm::{
    DrawerEdge, WindowManager, WindowResizeEdge, WmMouseDrag, WorkspaceLayoutMode, rect_contains,
};

pub(super) fn detect_title_bar_badge_click(
    wm: &WindowManager,
    outer: Rect,
    col: u16,
) -> Option<u8> {
    if outer.width < 24 {
        return None;
    }
    let glyphs = wm.config().glyph_set();
    let min_len = glyphs.minimize_badge.chars().count() as u16;
    let zoom_len = glyphs.zoom_badge.chars().count() as u16;
    let close_len = glyphs.close_badge.chars().count() as u16;
    let right_edge = outer.x + outer.width;
    let start_x = right_edge.saturating_sub(min_len + zoom_len + close_len + 2);
    if col >= start_x && col < start_x + min_len {
        Some(0) // [_] Minimize
    } else if col >= start_x + min_len && col < start_x + min_len + zoom_len {
        Some(1) // [↕] Maximize / Zoom
    } else if col >= start_x + min_len + zoom_len && col < start_x + min_len + zoom_len + close_len
    {
        Some(2) // [▪] Close
    } else {
        None
    }
}

pub(super) fn detect_border_resize_edge(
    outer: Rect,
    col: u16,
    row: u16,
) -> Option<WindowResizeEdge> {
    let right_x = outer.x + outer.width.saturating_sub(1);
    let bottom_y = outer.y + outer.height.saturating_sub(1);
    if (row == bottom_y && col >= right_x.saturating_sub(1))
        || (col == right_x && row >= bottom_y.saturating_sub(1))
    {
        Some(WindowResizeEdge::BottomRight)
    } else if (row == bottom_y && col <= outer.x + 1)
        || (col == outer.x && row >= bottom_y.saturating_sub(1))
    {
        Some(WindowResizeEdge::BottomLeft)
    } else if (row == outer.y && col >= right_x.saturating_sub(1))
        || (col == right_x && row <= outer.y + 1)
    {
        Some(WindowResizeEdge::TopRight)
    } else if (row == outer.y && col <= outer.x + 1)
        || (col == outer.x && row <= outer.y + 1)
    {
        Some(WindowResizeEdge::TopLeft)
    } else if row == bottom_y {
        Some(WindowResizeEdge::Bottom)
    } else if row == outer.y {
        Some(WindowResizeEdge::Top)
    } else if col == outer.x {
        Some(WindowResizeEdge::Left)
    } else if col == right_x {
        Some(WindowResizeEdge::Right)
    } else {
        None
    }
}

pub(super) fn apply_rect_edge_resize(
    initial: Rect,
    edge: WindowResizeEdge,
    col: u16,
    row: u16,
    bounds: Rect,
) -> Rect {
    let (mut new_x, mut new_y, mut new_w, mut new_h) =
        (initial.x, initial.y, initial.width, initial.height);
    let (right_edge, bottom_edge) = (initial.x + initial.width, initial.y + initial.height);
    if matches!(
        edge,
        WindowResizeEdge::Right | WindowResizeEdge::TopRight | WindowResizeEdge::BottomRight
    ) {
        new_w = col.saturating_sub(initial.x).saturating_add(1);
    }
    if matches!(
        edge,
        WindowResizeEdge::Left | WindowResizeEdge::TopLeft | WindowResizeEdge::BottomLeft
    ) {
        new_x = col.clamp(bounds.x, right_edge.saturating_sub(16).max(bounds.x));
        new_w = right_edge.saturating_sub(new_x);
    }
    if matches!(
        edge,
        WindowResizeEdge::Bottom | WindowResizeEdge::BottomLeft | WindowResizeEdge::BottomRight
    ) {
        new_h = row.saturating_sub(initial.y).saturating_add(1);
    }
    if matches!(
        edge,
        WindowResizeEdge::Top | WindowResizeEdge::TopLeft | WindowResizeEdge::TopRight
    ) {
        new_y = row.clamp(bounds.y, bottom_edge.saturating_sub(5).max(bounds.y));
        new_h = bottom_edge.saturating_sub(new_y);
    }
    Rect::new(new_x, new_y, new_w, new_h)
}

pub(super) fn handle_modal_mouse_event(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    let modal_outer = wm.active_modal_rect(workspace_area)?;
    if !rect_contains(modal_outer, mouse.column, mouse.row) {
        return None;
    }
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.row == modal_outer.y {
                match detect_title_bar_badge_click(wm, modal_outer, mouse.column) {
                    Some(0 | 2) => wm.close_modal(),
                    Some(1) => wm.toggle_maximize_modal(),
                    _ => wm.set_wm_mouse_drag(Some(WmMouseDrag::MoveModal {
                        grab_dx: mouse.column.saturating_sub(modal_outer.x),
                        grab_dy: 0,
                    })),
                }
            } else if let Some(edge) =
                detect_border_resize_edge(modal_outer, mouse.column, mouse.row)
            {
                wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeModal {
                    edge,
                    initial_rect: modal_outer,
                }));
            }
            Some(AppAction::Handled)
        }
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            let up = matches!(mouse.kind, MouseEventKind::ScrollUp);
            match wm.modal_mut() {
                crate::wm::Modal::Help { scroll } | crate::wm::Modal::Inspector { scroll } => {
                    *scroll = if up {
                        scroll.saturating_sub(2)
                    } else {
                        scroll.saturating_add(2)
                    };
                }
                crate::wm::Modal::Palette { selected, .. } => {
                    *selected = if up {
                        selected.saturating_sub(1)
                    } else {
                        selected.saturating_add(1)
                    };
                }
                _ => {}
            }
            Some(AppAction::Handled)
        }
        _ => Some(AppAction::Handled),
    }
}

pub(super) fn handle_quick_terminal_border_mouse_down(
    wm: &mut WindowManager,
    outer: Rect,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Result<Option<AppAction>> {
    if mouse.row == outer.y {
        match detect_title_bar_badge_click(wm, outer, mouse.column) {
            Some(0 | 2) => wm.quick_terminal_mut().visible = false,
            Some(1) => {
                let qt = wm.quick_terminal_mut();
                qt.height_pct = if qt.height_pct >= 80 { 40 } else { 90 };
                wm.resize_all(workspace_area)?;
            }
            _ => {}
        }
        return Ok(Some(AppAction::Handled));
    }
    if detect_border_resize_edge(outer, mouse.column, mouse.row).is_some() {
        wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeQuickTerminal));
        return Ok(Some(AppAction::Handled));
    }
    Ok(None)
}

pub(super) fn handle_wm_frame_border_mouse_down(
    wm: &mut WindowManager,
    hit_id: crate::pane::PaneId,
    outer: Rect,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Result<Option<AppAction>> {
    let (col, row) = (mouse.column, mouse.row);
    if row == outer.y {
        match detect_title_bar_badge_click(wm, outer, col) {
            Some(0) => {
                wm.toggle_minimize_pane(hit_id);
                return Ok(Some(AppAction::Handled));
            }
            Some(1) => {
                if wm.z_stack().is_minimized(hit_id) {
                    wm.toggle_minimize_pane(hit_id);
                } else {
                    wm.toggle_zoom();
                    wm.resize_all(workspace_area)?;
                }
                return Ok(Some(AppAction::Handled));
            }
            Some(2) => {
                if wm.remove_pane(hit_id) {
                    wm.resize_all(workspace_area)?;
                }
                return Ok(Some(AppAction::Handled));
            }
            _ => {
                let on_title_grip = col > outer.x + 1 && col < outer.x + 18;
                if mouse.modifiers.contains(KeyModifiers::ALT)
                    || (wm.layout_mode() == WorkspaceLayoutMode::Stacked && on_title_grip)
                {
                    wm.set_wm_mouse_drag(Some(WmMouseDrag::MoveWindow {
                        pane_id: hit_id,
                        grab_dx: col.saturating_sub(outer.x),
                        grab_dy: 0,
                    }));
                    return Ok(Some(AppAction::Handled));
                }
            }
        }
    }
    if let Some(edge) = detect_border_resize_edge(outer, col, row) {
        wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeWindow {
            pane_id: hit_id,
            edge,
            initial_rect: outer,
        }));
        return Ok(Some(AppAction::Handled));
    }
    Ok(None)
}

pub(super) fn handle_wm_window_drag_event(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Result<Option<AppAction>> {
    let Some(drag) = wm.wm_mouse_drag() else {
        return Ok(None);
    };

    match mouse.kind {
        MouseEventKind::Drag(MouseButton::Left) => {
            let eff_area = wm.effective_workspace_area(workspace_area);
            match drag {
                WmMouseDrag::MoveWindow {
                    pane_id,
                    grab_dx,
                    grab_dy,
                } => {
                    if wm.layout_mode() == WorkspaceLayoutMode::Stacked
                        || wm.pane_count() == 1
                        || mouse.modifiers.contains(KeyModifiers::ALT)
                    {
                        if wm.layout_mode() == WorkspaceLayoutMode::Tiled {
                            wm.set_layout_mode(WorkspaceLayoutMode::Stacked);
                        }
                        let cur = wm
                            .z_stack()
                            .prepare_movable_rect(pane_id, eff_area)
                            .unwrap_or(eff_area);
                        let new_x = mouse.column.saturating_sub(grab_dx).max(eff_area.x);
                        let new_y = mouse.row.saturating_sub(grab_dy).max(eff_area.y);
                        let updated = wm.z_stack_mut().set_window_rect(
                            pane_id,
                            Rect::new(new_x, new_y, cur.width, cur.height),
                            eff_area,
                        );
                        wm.resize_all(workspace_area)?;
                        wm.active_pane_mut().set_status_message(Some(format!(
                            "Window #{pane_id} moved to ({},{}) [{}x{}]",
                            updated.x, updated.y, updated.width, updated.height
                        )));
                    } else {
                        for (other_id, rect) in wm.split_tree().compute_rects(eff_area) {
                            if other_id != pane_id && rect_contains(rect, mouse.column, mouse.row) {
                                if wm.split_tree_mut().swap_leaves(pane_id, other_id) {
                                    wm.resize_all(workspace_area)?;
                                    wm.active_pane_mut().set_status_message(Some(format!(
                                        "Swapped Pane #{pane_id} <-> Pane #{other_id}"
                                    )));
                                }
                                break;
                            }
                        }
                    }
                    Ok(Some(AppAction::Handled))
                }
                WmMouseDrag::ResizeWindow {
                    pane_id,
                    edge,
                    initial_rect,
                } => {
                    if wm.layout_mode() == WorkspaceLayoutMode::Stacked || wm.pane_count() == 1 {
                        if wm.layout_mode() == WorkspaceLayoutMode::Tiled {
                            wm.set_layout_mode(WorkspaceLayoutMode::Stacked);
                        }
                        let resized = apply_rect_edge_resize(
                            initial_rect,
                            edge,
                            mouse.column,
                            mouse.row,
                            eff_area,
                        );
                        let updated = wm.z_stack_mut().set_window_rect(pane_id, resized, eff_area);
                        wm.resize_all(workspace_area)?;
                        wm.active_pane_mut().set_status_message(Some(format!(
                            "Window #{pane_id} resized to {}x{} @ ({},{})",
                            updated.width, updated.height, updated.x, updated.y
                        )));
                    } else {
                        let mut changed = false;
                        let (init_r, init_b) = (
                            initial_rect.x + initial_rect.width.saturating_sub(1),
                            initial_rect.y + initial_rect.height.saturating_sub(1),
                        );
                        let clamp_c = |v: i32| {
                            v.clamp(
                                i32::from(eff_area.x),
                                i32::from(eff_area.x + eff_area.width.saturating_sub(1)),
                            ) as u16
                        };
                        let clamp_r = |v: i32| {
                            v.clamp(
                                i32::from(eff_area.y),
                                i32::from(eff_area.y + eff_area.height.saturating_sub(1)),
                            ) as u16
                        };
                        if matches!(
                            edge,
                            WindowResizeEdge::Left
                                | WindowResizeEdge::TopLeft
                                | WindowResizeEdge::BottomLeft
                        ) {
                            if wm.split_tree_mut().set_split_ratio_for_border(
                                pane_id,
                                SplitDirection::Vertical,
                                true,
                                mouse.column,
                                mouse.row,
                                eff_area,
                            ) {
                                changed = true;
                            } else if mouse.column != initial_rect.x {
                                let vc = clamp_c(
                                    i32::from(init_r)
                                        - (i32::from(mouse.column) - i32::from(initial_rect.x)),
                                );
                                changed |= wm.split_tree_mut().set_split_ratio_for_border(
                                    pane_id,
                                    SplitDirection::Vertical,
                                    false,
                                    vc,
                                    mouse.row,
                                    eff_area,
                                );
                            }
                        }
                        if matches!(
                            edge,
                            WindowResizeEdge::Right
                                | WindowResizeEdge::TopRight
                                | WindowResizeEdge::BottomRight
                        ) {
                            if wm.split_tree_mut().set_split_ratio_for_border(
                                pane_id,
                                SplitDirection::Vertical,
                                false,
                                mouse.column,
                                mouse.row,
                                eff_area,
                            ) {
                                changed = true;
                            } else if mouse.column != init_r {
                                let vc = clamp_c(
                                    i32::from(initial_rect.x)
                                        - (i32::from(mouse.column) - i32::from(init_r)),
                                );
                                changed |= wm.split_tree_mut().set_split_ratio_for_border(
                                    pane_id,
                                    SplitDirection::Vertical,
                                    true,
                                    vc,
                                    mouse.row,
                                    eff_area,
                                );
                            }
                        }
                        if matches!(
                            edge,
                            WindowResizeEdge::Top
                                | WindowResizeEdge::TopLeft
                                | WindowResizeEdge::TopRight
                        ) {
                            if wm.split_tree_mut().set_split_ratio_for_border(
                                pane_id,
                                SplitDirection::Horizontal,
                                true,
                                mouse.column,
                                mouse.row,
                                eff_area,
                            ) {
                                changed = true;
                            } else if mouse.row != initial_rect.y {
                                let vr = clamp_r(
                                    i32::from(init_b)
                                        - (i32::from(mouse.row) - i32::from(initial_rect.y)),
                                );
                                changed |= wm.split_tree_mut().set_split_ratio_for_border(
                                    pane_id,
                                    SplitDirection::Horizontal,
                                    false,
                                    mouse.column,
                                    vr,
                                    eff_area,
                                );
                            }
                        }
                        if matches!(
                            edge,
                            WindowResizeEdge::Bottom
                                | WindowResizeEdge::BottomLeft
                                | WindowResizeEdge::BottomRight
                        ) {
                            if wm.split_tree_mut().set_split_ratio_for_border(
                                pane_id,
                                SplitDirection::Horizontal,
                                false,
                                mouse.column,
                                mouse.row,
                                eff_area,
                            ) {
                                changed = true;
                            } else if mouse.row != init_b {
                                let vr = clamp_r(
                                    i32::from(initial_rect.y)
                                        - (i32::from(mouse.row) - i32::from(init_b)),
                                );
                                changed |= wm.split_tree_mut().set_split_ratio_for_border(
                                    pane_id,
                                    SplitDirection::Horizontal,
                                    true,
                                    mouse.column,
                                    vr,
                                    eff_area,
                                );
                            }
                        }
                        if changed {
                            wm.resize_all(workspace_area)?;
                        }
                    }
                    Ok(Some(AppAction::Handled))
                }
                WmMouseDrag::MoveModal { grab_dx, grab_dy } => {
                    if let Some(cur) = wm.active_modal_rect(workspace_area) {
                        let new_x = mouse.column.saturating_sub(grab_dx).max(workspace_area.x);
                        let new_y = mouse.row.saturating_sub(grab_dy).max(workspace_area.y);
                        wm.set_modal_rect(
                            Rect::new(new_x, new_y, cur.width, cur.height),
                            workspace_area,
                        );
                    }
                    Ok(Some(AppAction::Handled))
                }
                WmMouseDrag::ResizeModal { edge, initial_rect } => {
                    let resized = apply_rect_edge_resize(
                        initial_rect,
                        edge,
                        mouse.column,
                        mouse.row,
                        workspace_area,
                    );
                    wm.set_modal_rect(resized, workspace_area);
                    Ok(Some(AppAction::Handled))
                }
                WmMouseDrag::ResizeQuickTerminal => {
                    let rel_h = mouse.row.saturating_sub(workspace_area.y).saturating_add(1);
                    let pct = ((u32::from(rel_h) * 100) / u32::from(workspace_area.height.max(1)))
                        .clamp(15, 95) as u16;
                    wm.quick_terminal_mut().height_pct = pct;
                    wm.resize_all(workspace_area)?;
                    Ok(Some(AppAction::Handled))
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            wm.set_wm_mouse_drag(None);
            Ok(Some(AppAction::Handled))
        }
        _ => Ok(None),
    }
}

pub(super) fn handle_drawer_mouse_event(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    match mouse.kind {
        MouseEventKind::Moved => {
            if wm.update_drawer_hover(mouse.column, mouse.row, workspace_area) {
                return Some(AppAction::Handled);
            }
            return None;
        }
        MouseEventKind::Drag(MouseButton::Left) if wm.drawer().resizing_drag => {
            let new_size = match wm.drawer_config().edge {
                DrawerEdge::Left => mouse.column.saturating_sub(workspace_area.x) + 1,
                DrawerEdge::Right => workspace_area
                    .x
                    .saturating_add(workspace_area.width)
                    .saturating_sub(mouse.column),
                DrawerEdge::Top => mouse.row.saturating_sub(workspace_area.y) + 1,
                DrawerEdge::Bottom => workspace_area
                    .y
                    .saturating_add(workspace_area.height)
                    .saturating_sub(mouse.row),
            };
            wm.resize_drawer(new_size);
            let _ = wm.resize_all(workspace_area);
            return Some(AppAction::Handled);
        }
        MouseEventKind::Up(MouseButton::Left) if wm.drawer().resizing_drag => {
            wm.drawer_mut().resizing_drag = false;
            return Some(AppAction::Handled);
        }
        _ => {}
    }

    let dr = wm.drawer_rect(workspace_area)?;
    if !rect_contains(dr, mouse.column, mouse.row) {
        if matches!(mouse.kind, MouseEventKind::Down(_)) && wm.drawer().focused {
            wm.drawer_mut().focused = false;
        }
        return None;
    }

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let edge = wm.drawer_config().edge;
            let on_resize_border = match edge {
                DrawerEdge::Left => mouse.column == dr.x + dr.width.saturating_sub(1),
                DrawerEdge::Right => mouse.column == dr.x,
                DrawerEdge::Top => mouse.row == dr.y + dr.height.saturating_sub(1),
                DrawerEdge::Bottom => mouse.row == dr.y,
            };
            if on_resize_border {
                wm.drawer_mut().resizing_drag = true;
                return Some(AppAction::Handled);
            }

            if mouse.row == dr.y {
                let rel_x = mouse.column.saturating_sub(dr.x);
                if rel_x >= dr.width.saturating_sub(5) {
                    wm.close_drawer();
                } else if rel_x < 17 {
                    wm.set_drawer_edge(None);
                } else if rel_x < 26 {
                    wm.set_drawer_layout(None);
                } else {
                    wm.set_drawer_visibility(None);
                }
                let _ = wm.resize_all(workspace_area);
                return Some(AppAction::Handled);
            }

            wm.drawer_mut().focused = true;
            if edge.is_vertical() {
                let rel_row = usize::from(mouse.row.saturating_sub(dr.y + 1));
                let rows_len = wm.drawer_rows().len();
                if rel_row < rows_len {
                    wm.drawer_mut().selected_index = rel_row;
                    wm.drawer_activate_selected(false);
                }
            } else {
                wm.drawer_select_next();
                wm.drawer_activate_selected(false);
            }
            Some(AppAction::Handled)
        }
        MouseEventKind::ScrollUp => {
            wm.drawer_select_prev();
            Some(AppAction::Handled)
        }
        MouseEventKind::ScrollDown => {
            wm.drawer_select_next();
            Some(AppAction::Handled)
        }
        _ => Some(AppAction::Handled),
    }
}
