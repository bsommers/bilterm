use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::error::Result;
use crate::input::AppAction;
use crate::input::encode::encode_alternate_scroll_wheel;
use crate::pane::{CompositePane, CopySelectionKind, PromptAnchor, PromptMouseDrag};
use crate::pty::line_discipline::InputRoutingMode;
use crate::vt::encode_osc52_clipboard;
use crate::wm::split_tree::SplitDirection;
use crate::wm::{
    DrawerEdge, Focus, WindowManager, WindowResizeEdge, WmMouseDrag, WorkspaceLayoutMode,
    encode_sgr_mouse_local, rect_contains,
};

/// Routes SGR mouse events for intra-pane floating prompt dragging, height resizing,
/// `[▲][▼]` pin badge clicks, `Ctrl+Click` URL yanking (`VAL-03`), `Alt+LeftDrag` rectangular canvas selection (`VAL-04`),
/// `DECSET ?1007` alternate scroll (`FND-02`), and canvas scrollback (`PANE-02`, `PANE-03`).
pub fn handle_mouse_event(
    pane: &mut CompositePane,
    mouse: MouseEvent,
    viewport: Rect,
) -> Result<AppAction> {
    let status_rect = pane.status_bar_rect(viewport);
    if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        && mouse.row == status_rect.y
        && mouse.column >= status_rect.x
        && mouse.column < status_rect.x + status_rect.width
    {
        pane.cycle_prompt_mode();
        return Ok(AppAction::Handled);
    }

    if !pane.is_prompt_visible() {
        if let Some(scroll_bytes) = encode_alternate_scroll_wheel(pane, mouse.kind) {
            pane.write_to_pty(&scroll_bytes)?;
            return Ok(AppAction::Handled);
        }
        return Ok(AppAction::Continue);
    }

    let prompt_rect = pane.prompt_rect(viewport);
    let canvas_rect = pane.canvas_rect(viewport);
    let col = mouse.column;
    let row = mouse.row;
    let in_prompt = col >= prompt_rect.x
        && col < prompt_rect.x + prompt_rect.width
        && row >= prompt_rect.y
        && row < prompt_rect.y + prompt_rect.height;
    let in_canvas = col >= canvas_rect.x
        && col < canvas_rect.x + canvas_rect.width
        && row >= canvas_rect.y
        && row < canvas_rect.y + canvas_rect.height;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Ctrl+LeftClick on OutputCanvas: check for URL at clicked cell and yank via OSC 52 (VAL-03)
            if mouse.modifiers.contains(KeyModifiers::CONTROL) && in_canvas && !in_prompt {
                let rel_r = row.saturating_sub(canvas_rect.y);
                let rel_c = col.saturating_sub(canvas_rect.x);
                if let Some(url) = pane.vt().url_at(rel_r, rel_c) {
                    let seq = encode_osc52_clipboard(&url);
                    pane.set_last_osc52_sequence(Some(seq));
                    pane.prompt_mut().yank_buffer = url.clone();
                    pane.set_status_message(Some(format!("Yanked URL: {url}")));
                    return Ok(AppAction::Handled);
                }
            }

            // Alt+LeftClick on OutputCanvas: start rectangular block selection (VAL-04)
            if mouse.modifiers.contains(KeyModifiers::ALT) && in_canvas && !in_prompt {
                let rel_r = row.saturating_sub(canvas_rect.y);
                let rel_c = col.saturating_sub(canvas_rect.x);
                let copy = pane.copy_mode_mut();
                copy.active = true;
                copy.anchor = Some((rel_r, rel_c));
                copy.cursor_row = rel_r;
                copy.cursor_col = rel_c;
                copy.selection_kind = CopySelectionKind::Block;
                pane.set_mouse_drag(Some(PromptMouseDrag::CanvasRectSelect));
                return Ok(AppAction::Handled);
            }

            // Top border of floating prompt: check [▲][▼] pin badges on right side or start vertical move drag
            if row == prompt_rect.y
                && col >= prompt_rect.x
                && col < prompt_rect.x + prompt_rect.width
            {
                let right_edge = prompt_rect.x + prompt_rect.width;
                // Layout on top-right border: ... [▲][▼]─┐ (right_edge - 8..right_edge - 5 is [▲], right_edge - 5..right_edge - 2 is [▼])
                if prompt_rect.width >= 16 {
                    let top_btn_start = right_edge.saturating_sub(8);
                    let bot_btn_start = right_edge.saturating_sub(5);
                    if col >= top_btn_start && col < top_btn_start + 3 {
                        pane.set_prompt_anchor(PromptAnchor::Top);
                        return Ok(AppAction::Handled);
                    }
                    if col >= bot_btn_start && col < bot_btn_start + 3 {
                        pane.set_prompt_anchor(PromptAnchor::Bottom);
                        return Ok(AppAction::Handled);
                    }
                }
                pane.set_mouse_drag(Some(PromptMouseDrag::MovePrompt {
                    grab_x_offset: col.saturating_sub(prompt_rect.x),
                }));
                return Ok(AppAction::Handled);
            }

            // Bottom border of floating prompt: check bottom-right corner (`◢`) or start height resize drag
            let bottom_y = prompt_rect.y + prompt_rect.height.saturating_sub(1);
            let right_x = prompt_rect.x + prompt_rect.width.saturating_sub(1);
            if row == bottom_y && col == right_x {
                pane.set_mouse_drag(Some(PromptMouseDrag::ResizePromptCorner));
                return Ok(AppAction::Handled);
            }
            if row == bottom_y && col >= prompt_rect.x && col < prompt_rect.x + prompt_rect.width {
                pane.set_mouse_drag(Some(PromptMouseDrag::ResizePrompt));
                return Ok(AppAction::Handled);
            }

            // Left border of floating prompt: start left width resize drag
            if col == prompt_rect.x && row > prompt_rect.y && row < bottom_y {
                pane.set_mouse_drag(Some(PromptMouseDrag::ResizePromptLeft));
                return Ok(AppAction::Handled);
            }

            // Right border of floating prompt: start right width resize drag
            if col == right_x && row > prompt_rect.y && row < bottom_y {
                pane.set_mouse_drag(Some(PromptMouseDrag::ResizePromptRight));
                return Ok(AppAction::Handled);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(drag) = pane.mouse_drag() {
                match drag {
                    PromptMouseDrag::MovePrompt { grab_x_offset } => {
                        let rel_y = row.saturating_sub(canvas_rect.y);
                        if rel_y == 0 {
                            pane.set_prompt_anchor(PromptAnchor::Top);
                        } else if rel_y + prompt_rect.height >= canvas_rect.height {
                            pane.set_prompt_anchor(PromptAnchor::Bottom);
                        } else {
                            pane.set_prompt_anchor(PromptAnchor::Floating { y_offset: rel_y });
                        }
                        if pane.prompt().width_cols.is_some() {
                            let target_x = col
                                .saturating_sub(grab_x_offset)
                                .saturating_sub(canvas_rect.x);
                            let max_x = canvas_rect.width.saturating_sub(prompt_rect.width);
                            pane.prompt_mut().x_offset = Some(target_x.min(max_x));
                        }
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePrompt => {
                        let new_height = row.saturating_sub(prompt_rect.y).saturating_add(1);
                        pane.set_prompt_height(new_height.clamp(3, 12));
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePromptRight => {
                        let new_width = col.saturating_sub(prompt_rect.x).saturating_add(1);
                        pane.set_prompt_width(Some(new_width.clamp(16, canvas_rect.width.max(16))));
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePromptLeft => {
                        let right_edge = prompt_rect.x + prompt_rect.width;
                        let clamped_col = col.clamp(canvas_rect.x, right_edge.saturating_sub(16));
                        let new_width = right_edge.saturating_sub(clamped_col).max(16);
                        pane.prompt_mut().x_offset = Some(clamped_col.saturating_sub(canvas_rect.x));
                        pane.set_prompt_width(Some(new_width));
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePromptCorner => {
                        let new_height = row.saturating_sub(prompt_rect.y).saturating_add(1);
                        let new_width = col.saturating_sub(prompt_rect.x).saturating_add(1);
                        pane.set_prompt_height(new_height.clamp(3, 12));
                        pane.set_prompt_width(Some(new_width.clamp(16, canvas_rect.width.max(16))));
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::CanvasRectSelect => {
                        let rel_r = row
                            .saturating_sub(canvas_rect.y)
                            .min(canvas_rect.height.saturating_sub(1));
                        let rel_c = col
                            .saturating_sub(canvas_rect.x)
                            .min(canvas_rect.width.saturating_sub(1));
                        let copy = pane.copy_mode_mut();
                        copy.cursor_row = rel_r;
                        copy.cursor_col = rel_c;
                        return Ok(AppAction::Handled);
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if let Some(drag) = pane.take_mouse_drag() {
                if drag == PromptMouseDrag::CanvasRectSelect {
                    let _ = pane.yank_copy_selection();
                }
                return Ok(AppAction::Handled);
            }
        }
        MouseEventKind::ScrollUp => {
            if in_prompt && pane.prompt().lines.len() > 1 {
                pane.prompt_mut().scroll_up(1);
            } else {
                pane.vt_mut().scroll_up(3);
            }
            return Ok(AppAction::Handled);
        }
        MouseEventKind::ScrollDown => {
            if in_prompt && pane.prompt().lines.len() > 1 {
                pane.prompt_mut().scroll_down(1);
            } else {
                pane.vt_mut().scroll_down(3);
            }
            return Ok(AppAction::Handled);
        }
        _ => {}
    }

    Ok(AppAction::Continue)
}

/// Routes a `MouseEvent` across the multi-pane `WindowManager`, hit-testing in reverse z-order,
/// selectively raising clicked panes (`WM-02`), and translating global coordinates to local SGR 1006 (`WM-03`)
/// or `DECSET ?1007` alternate scroll (`FND-02`).
pub fn handle_wm_mouse_event(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    screen_area: Rect,
) -> Result<AppAction> {
    if wm.config().show_top_menubar
        && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        && mouse.row == screen_area.y
    {
        let rel_col = mouse.column.saturating_sub(screen_area.x);
        let cat = match rel_col {
            0..=10 => 0,
            11..=19 => 1,
            20..=28 => 2,
            29..=35 => 3,
            36..=43 => 4,
            44..=50 => 5,
            _ => 6,
        };
        if wm.menubar_open_category() == Some(cat) {
            wm.set_menubar_open_category(None);
        } else {
            wm.set_menubar_open_category(Some(cat));
        }
        return Ok(AppAction::Handled);
    }

    let top_off = u16::from(wm.config().show_top_menubar && screen_area.height > 3);
    let bot_off = u16::from(wm.config().show_dos_fkey_bar && screen_area.height > 2 + top_off);
    let workspace_area = Rect {
        x: screen_area.x,
        y: screen_area.y + top_off,
        width: screen_area.width,
        height: screen_area.height.saturating_sub(top_off + bot_off).max(1),
    };

    if let Some(action) = handle_drawer_mouse_event(wm, mouse, workspace_area) {
        return Ok(action);
    }

    if let Some(action) = handle_wm_window_drag_event(wm, mouse, workspace_area)? {
        return Ok(action);
    }

    if let Some(action) = handle_modal_mouse_event(wm, mouse, workspace_area) {
        return Ok(action);
    }

    let Some((hit_focus, outer, inner)) = wm.hit_test_pane(mouse.column, mouse.row, workspace_area)
    else {
        return Ok(AppAction::Continue);
    };

    if matches!(mouse.kind, MouseEventKind::Down(_))
        && let Focus::Pane(hit_id) = hit_focus
    {
        wm.focus_and_raise(hit_id);
    }

    if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        && !rect_contains(inner, mouse.column, mouse.row)
    {
        match hit_focus {
            Focus::Quick => {
                if let Some(action) =
                    handle_quick_terminal_border_mouse_down(wm, outer, mouse, workspace_area)?
                {
                    return Ok(action);
                }
            }
            Focus::Pane(hit_id) if wm.is_framed() => {
                if let Some(action) =
                    handle_wm_frame_border_mouse_down(wm, hit_id, outer, mouse, workspace_area)?
                {
                    return Ok(action);
                }
            }
            _ => {}
        }
    }

    let Some(pane) = wm.pane_by_focus_mut(hit_focus) else {
        return Ok(AppAction::Continue);
    };

    if pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough {
        // If the child app has not enabled mouse tracking (?1000..1006), check DECSET ?1007 Alternate Scroll first
        if !pane.vt().is_mouse_sgr_or_tracking() {
            if let Some(scroll_bytes) = encode_alternate_scroll_wheel(pane, mouse.kind) {
                pane.write_to_pty(&scroll_bytes)?;
                return Ok(AppAction::Handled);
            }
        } else if let Some(sgr_bytes) = encode_sgr_mouse_local(mouse, inner) {
            pane.write_to_pty(&sgr_bytes)?;
            return Ok(AppAction::Handled);
        }
    }

    handle_mouse_event(pane, mouse, inner)
}

fn detect_title_bar_badge_click(wm: &WindowManager, outer: Rect, col: u16) -> Option<u8> {
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
    } else if col >= start_x + min_len + zoom_len && col < start_x + min_len + zoom_len + close_len {
        Some(2) // [▪] Close
    } else {
        None
    }
}

fn detect_border_resize_edge(outer: Rect, col: u16, row: u16) -> Option<WindowResizeEdge> {
    let right_x = outer.x + outer.width.saturating_sub(1);
    let bottom_y = outer.y + outer.height.saturating_sub(1);
    if (row == bottom_y && col >= right_x.saturating_sub(1))
        || (col == right_x && row >= bottom_y.saturating_sub(1))
    {
        Some(WindowResizeEdge::BottomRight)
    } else if row == bottom_y && col <= outer.x + 1 {
        Some(WindowResizeEdge::BottomLeft)
    } else if row == bottom_y {
        Some(WindowResizeEdge::Bottom)
    } else if col == outer.x {
        Some(WindowResizeEdge::Left)
    } else if col == right_x {
        Some(WindowResizeEdge::Right)
    } else {
        None
    }
}

fn apply_rect_edge_resize(
    initial_rect: Rect,
    edge: WindowResizeEdge,
    col: u16,
    row: u16,
    bounds: Rect,
) -> Rect {
    let mut new_x = initial_rect.x;
    let mut new_y = initial_rect.y;
    let mut new_w = initial_rect.width;
    let mut new_h = initial_rect.height;
    let right_edge = initial_rect.x + initial_rect.width;
    let bottom_edge = initial_rect.y + initial_rect.height;

    if matches!(edge, WindowResizeEdge::Right | WindowResizeEdge::BottomRight) {
        new_w = col.saturating_sub(initial_rect.x).saturating_add(1);
    }
    if matches!(edge, WindowResizeEdge::Left | WindowResizeEdge::BottomLeft) {
        new_x = col.clamp(bounds.x, right_edge.saturating_sub(16));
        new_w = right_edge.saturating_sub(new_x);
    }
    if matches!(
        edge,
        WindowResizeEdge::Bottom | WindowResizeEdge::BottomLeft | WindowResizeEdge::BottomRight
    ) {
        new_h = row.saturating_sub(initial_rect.y).saturating_add(1);
    }
    if matches!(edge, WindowResizeEdge::Top) {
        new_y = row.clamp(bounds.y, bottom_edge.saturating_sub(5));
        new_h = bottom_edge.saturating_sub(new_y);
    }
    Rect::new(new_x, new_y, new_w, new_h)
}

fn handle_modal_mouse_event(
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
            } else if let Some(edge) = detect_border_resize_edge(modal_outer, mouse.column, mouse.row) {
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
                    *scroll = if up { scroll.saturating_sub(2) } else { scroll.saturating_add(2) };
                }
                crate::wm::Modal::Palette { selected, .. } => {
                    *selected = if up { selected.saturating_sub(1) } else { selected.saturating_add(1) };
                }
                _ => {}
            }
            Some(AppAction::Handled)
        }
        _ => Some(AppAction::Handled),
    }
}

fn handle_quick_terminal_border_mouse_down(
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

fn handle_wm_frame_border_mouse_down(
    wm: &mut WindowManager,
    hit_id: crate::pane::PaneId,
    outer: Rect,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Result<Option<AppAction>> {
    let (col, row) = (mouse.column, mouse.row);
    if row == outer.y {
        match detect_title_bar_badge_click(wm, outer, col) {
            Some(0) => wm.toggle_minimize_pane(hit_id),
            Some(1) => {
                if wm.z_stack().is_minimized(hit_id) {
                    wm.toggle_minimize_pane(hit_id);
                } else {
                    wm.toggle_zoom();
                    wm.resize_all(workspace_area)?;
                }
            }
            Some(2) => {
                if wm.remove_pane(hit_id) {
                    wm.resize_all(workspace_area)?;
                }
            }
            _ => wm.set_wm_mouse_drag(Some(WmMouseDrag::MoveWindow {
                pane_id: hit_id,
                grab_dx: col.saturating_sub(outer.x),
                grab_dy: 0,
            })),
        }
        return Ok(Some(AppAction::Handled));
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

fn handle_wm_window_drag_event(
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
                        if matches!(
                            edge,
                            WindowResizeEdge::Left
                                | WindowResizeEdge::Right
                                | WindowResizeEdge::BottomLeft
                                | WindowResizeEdge::BottomRight
                        ) {
                            changed |= wm.split_tree_mut().set_split_ratio_at_point(
                                pane_id,
                                SplitDirection::Vertical,
                                mouse.column,
                                mouse.row,
                                eff_area,
                            );
                        }
                        if matches!(
                            edge,
                            WindowResizeEdge::Top
                                | WindowResizeEdge::Bottom
                                | WindowResizeEdge::BottomLeft
                                | WindowResizeEdge::BottomRight
                        ) {
                            changed |= wm.split_tree_mut().set_split_ratio_at_point(
                                pane_id,
                                SplitDirection::Horizontal,
                                mouse.column,
                                mouse.row,
                                eff_area,
                            );
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
                    let rel_h = mouse
                        .row
                        .saturating_sub(workspace_area.y)
                        .saturating_add(1);
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

fn handle_drawer_mouse_event(
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

            // Header bar click: close button `[▪]` on right or cycle edge/mode/pin badges
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

            // Row click inside vertical or horizontal drawer
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
