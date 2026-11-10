use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::error::Result;
use crate::input::AppAction;
use crate::input::encode::encode_alternate_scroll_wheel;
use crate::input::mouse_wm::{
    handle_drawer_mouse_event, handle_modal_mouse_event, handle_quick_terminal_border_mouse_down,
    handle_wm_frame_border_mouse_down, handle_wm_window_drag_event,
};
use crate::pane::{CompositePane, CopySelectionKind, PromptAnchor, PromptMouseDrag};
use crate::pty::line_discipline::InputRoutingMode;
use crate::wm::{Focus, WindowManager, rect_contains};

fn prompt_editor_coords_from_mouse(
    pane: &CompositePane,
    prompt_rect: Rect,
    col: u16,
    row: u16,
) -> (usize, usize) {
    let inner_x = prompt_rect.x.saturating_add(1);
    let inner_y = prompt_rect.y.saturating_add(1);
    let inner_h = usize::from(prompt_rect.height.saturating_sub(2));
    let has_running_banner = pane.prompt().running_command.is_some() && inner_h >= 2;
    let has_completion_bar =
        pane.prompt().completion.is_active() && inner_h >= (2 + usize::from(has_running_banner));
    let reserved_rows = usize::from(has_running_banner) + usize::from(has_completion_bar);
    let editor_rows = inner_h.saturating_sub(reserved_rows).max(1);
    let auto_start_row = pane
        .prompt()
        .cursor_row
        .saturating_sub(editor_rows.saturating_sub(1));
    let max_start_row = pane.prompt().lines.len().saturating_sub(editor_rows);
    let start_row = if pane.prompt().scroll_offset > 0 {
        pane.prompt().scroll_offset.min(max_start_row)
    } else {
        auto_start_row
    };
    let vis_r = usize::from(row.saturating_sub(inner_y)).saturating_sub(usize::from(has_running_banner));
    let editor_row = start_row.saturating_add(vis_r);

    let prefix_len = 4usize;
    let avail_text_cols = usize::from(prompt_rect.width.saturating_sub(2))
        .saturating_sub(prefix_len)
        .max(1);
    let h_scroll = if editor_row == pane.prompt().cursor_row
        && pane.prompt().cursor_col >= avail_text_cols
    {
        pane.prompt().cursor_col + 1 - avail_text_cols
    } else {
        pane.prompt().h_scroll_offset
    };
    let vis_c = usize::from(col.saturating_sub(inner_x)).saturating_sub(prefix_len);
    let editor_col = h_scroll.saturating_add(vis_c);
    (editor_row, editor_col)
}

fn handle_oia_status_bar_mouse(
    pane: &mut CompositePane,
    mouse: MouseEvent,
    status_rect: Rect,
    viewport: Rect,
) -> Result<AppAction> {
    if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        let rel_col = usize::from(mouse.column.saturating_sub(status_rect.x));
        let layout = crate::ui::oia::oia_layout(pane);
        let title_end = layout.title_end;
        let state_end = layout.state_end;
        let routing_end = layout.routing_end;
        let pin_end = layout.pin_end;
        let prot_end = layout.prot_end;

        if pane.is_page_holding() && rel_col <= state_end.max(21) {
            pane.scroll_page_advance();
        } else if rel_col < title_end {
            let next = !pane.show_version_badge();
            pane.set_show_version_badge(next);
            pane.set_status_message(Some(format!(
                "OIA Version Badge: {}",
                if next { "ON" } else { "OFF" }
            )));
        } else if rel_col < state_end && pane.vt().scrollback_offset() > 0 {
            pane.clear_page_hold();
        } else if rel_col < routing_end {
            if pane.copy_mode().active {
                pane.exit_copy_mode();
            } else {
                pane.cycle_prompt_mode();
                let _ = pane.resize_viewport(viewport);
            }
        } else if rel_col < pin_end {
            let next_anchor = match pane.prompt().anchor {
                PromptAnchor::Top => PromptAnchor::Bottom,
                PromptAnchor::Bottom => PromptAnchor::SplitTop,
                PromptAnchor::SplitTop => PromptAnchor::SplitBottom,
                PromptAnchor::SplitBottom | PromptAnchor::Floating { .. } => PromptAnchor::Top,
            };
            pane.set_prompt_anchor(next_anchor);
            let _ = pane.resize_viewport(viewport);
            pane.set_status_message(Some(format!("Prompt anchor: {next_anchor:?}")));
        } else if pane.is_read_only() && rel_col < prot_end {
            pane.toggle_read_only();
        } else {
            pane.toggle_block_page_scroll_mode();
        }
    }
    Ok(AppAction::Handled)
}

/// Routes SGR mouse events for intra-pane floating prompt dragging, height resizing,
/// `[▲][▼]` pin badge clicks, prompt text selection, `Ctrl+Click` URL yanking (`VAL-03`),
/// `LeftDrag` character selection and `Alt+LeftDrag` rectangular canvas selection (`VAL-04`),
/// `RightClick`/`MiddleClick` clipboard paste, `DECSET ?1007` alternate scroll (`FND-02`),
/// and canvas scrollback (`PANE-02`, `PANE-03`).
pub fn handle_mouse_event(
    pane: &mut CompositePane,
    mut mouse: MouseEvent,
    viewport: Rect,
) -> Result<AppAction> {
    let status_rect = pane.status_bar_rect(viewport);
    if status_rect.height > 0
        && mouse.row == status_rect.y
        && mouse.column >= status_rect.x
        && mouse.column < status_rect.x + status_rect.width
    {
        if pane.mouse_drag().is_some() {
            match mouse.kind {
                MouseEventKind::Drag(MouseButton::Left) => {
                    mouse.row = status_rect.y.saturating_sub(1);
                }
                MouseEventKind::Up(MouseButton::Left) => {}
                _ => return Ok(AppAction::Handled),
            }
        } else {
            return handle_oia_status_bar_mouse(pane, mouse, status_rect, viewport);
        }
    }

    // Right-click or Middle-click pastes from OS clipboard / yank_buffer
    if matches!(
        mouse.kind,
        MouseEventKind::Down(MouseButton::Right | MouseButton::Middle)
    ) {
        return crate::input::paste_clipboard_into_pane(pane);
    }

    let canvas_rect = pane.canvas_rect(viewport);
    let col = mouse.column;
    let row = mouse.row;
    let in_canvas = col >= canvas_rect.x
        && col < canvas_rect.x + canvas_rect.width
        && row >= canvas_rect.y
        && row < canvas_rect.y + canvas_rect.height;

    if !pane.is_prompt_visible() {
        if let Some(scroll_bytes) = encode_alternate_scroll_wheel(pane, mouse.kind) {
            pane.write_to_pty(&scroll_bytes)?;
            return Ok(AppAction::Handled);
        }
        return handle_canvas_only_mouse_event(pane, mouse, canvas_rect, in_canvas);
    }

    let prompt_rect = pane.prompt_rect(viewport);
    let in_prompt = col >= prompt_rect.x
        && col < prompt_rect.x + prompt_rect.width
        && row >= prompt_rect.y
        && row < prompt_rect.y + prompt_rect.height;

    if in_canvas
        && let Some(act) = super::mouse_media::handle_media_canvas_mouse(pane, mouse, canvas_rect, in_prompt)
    {
        return Ok(act);
    }

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Ctrl+LeftClick on OutputCanvas: check for URL at clicked cell and yank via OSC 52 (VAL-03)
            if mouse.modifiers.contains(KeyModifiers::CONTROL) && in_canvas && !in_prompt {
                let rel_r = row.saturating_sub(canvas_rect.y);
                let rel_c = col.saturating_sub(canvas_rect.x);
                if let Some(url) = pane.vt().url_at(rel_r, rel_c) {
                    pane.record_yank(&url);
                    pane.set_status_message(Some(format!("Yanked URL: {url}")));
                    return Ok(AppAction::Handled);
                }
            }

            // Alt+LeftClick or Rectangular Mode LeftClick on OutputCanvas: start rectangular block selection (VAL-04)
            if (mouse.modifiers.contains(KeyModifiers::ALT) || pane.copy_mode().rect_mode)
                && in_canvas
                && !in_prompt
            {
                pane.prompt_mut().clear_selection();
                let rel_r = row
                    .saturating_sub(canvas_rect.y)
                    .min(canvas_rect.height.saturating_sub(1));
                let rel_c = col
                    .saturating_sub(canvas_rect.x)
                    .min(canvas_rect.width.saturating_sub(1));
                let copy = pane.copy_mode_mut();
                copy.active = true;
                copy.anchor = Some((rel_r, rel_c));
                copy.cursor_row = rel_r;
                copy.cursor_col = rel_c;
                copy.selection_kind = CopySelectionKind::Block;
                copy.visual_lock = true;
                pane.set_mouse_drag(Some(PromptMouseDrag::CanvasRectSelect));
                pane.set_status_message(Some(
                    "RECT SELECT [1×1]: Drag or Shift+Arrows | Enter/y copy | x to prompt | Esc clear"
                        .to_string(),
                ));
                return Ok(AppAction::Handled);
            }

            // Top border of floating prompt: check [▲][▼] pin badges on right side or start vertical move drag
            if row == prompt_rect.y
                && col >= prompt_rect.x
                && col < prompt_rect.x + prompt_rect.width
            {
                let right_edge = prompt_rect.x + prompt_rect.width;
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

            // Interior of floating prompt: position cursor & start prompt text selection
            if in_prompt && row > prompt_rect.y && row < bottom_y && col > prompt_rect.x && col < right_x {
                pane.copy_mode_mut().clear_selection();
                let (ed_r, ed_c) = prompt_editor_coords_from_mouse(pane, prompt_rect, col, row);
                let extend = mouse.modifiers.contains(KeyModifiers::SHIFT);
                pane.prompt_mut().set_cursor_at(ed_r, ed_c, extend);
                pane.set_mouse_drag(Some(PromptMouseDrag::PromptTextSelect));
                return Ok(AppAction::Handled);
            }

            // Plain LeftClick on OutputCanvas: position canvas cursor without activating copy mode until drag/Shift
            if in_canvas && !in_prompt {
                pane.prompt_mut().clear_selection();
                let rel_r = row
                    .saturating_sub(canvas_rect.y)
                    .min(canvas_rect.height.saturating_sub(1));
                let rel_c = col
                    .saturating_sub(canvas_rect.x)
                    .min(canvas_rect.width.saturating_sub(1));
                if mouse.modifiers.contains(KeyModifiers::SHIFT)
                    && pane.copy_mode().anchor.is_some()
                {
                    let copy = pane.copy_mode_mut();
                    copy.active = true;
                    copy.cursor_row = rel_r;
                    copy.cursor_col = rel_c;
                    let _ = pane.stage_mouse_canvas_selection();
                } else {
                    let copy = pane.copy_mode_mut();
                    copy.active = false;
                    copy.anchor = None;
                    copy.cursor_row = rel_r;
                    copy.cursor_col = rel_c;
                    copy.selection_kind = CopySelectionKind::Char;
                    copy.visual_lock = false;
                }
                pane.set_mouse_drag(Some(PromptMouseDrag::CanvasCharSelect));
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
                        pane.prompt_mut().x_offset =
                            Some(prompt_rect.x.saturating_sub(canvas_rect.x));
                        let max_w = canvas_rect.x + canvas_rect.width - prompt_rect.x;
                        let new_width = col.saturating_sub(prompt_rect.x).saturating_add(1);
                        pane.set_prompt_width(Some(
                            new_width.clamp(16.min(max_w.max(1)), max_w.max(16)),
                        ));
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePromptLeft => {
                        let right_edge = prompt_rect.x + prompt_rect.width;
                        let max_col = right_edge.saturating_sub(16).max(canvas_rect.x);
                        let clamped_col = col.clamp(canvas_rect.x, max_col);
                        let new_width = right_edge.saturating_sub(clamped_col).max(16);
                        pane.prompt_mut().x_offset =
                            Some(clamped_col.saturating_sub(canvas_rect.x));
                        pane.set_prompt_width(Some(new_width));
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePromptCorner => {
                        pane.prompt_mut().x_offset =
                            Some(prompt_rect.x.saturating_sub(canvas_rect.x));
                        let max_w = canvas_rect.x + canvas_rect.width - prompt_rect.x;
                        let new_height = row.saturating_sub(prompt_rect.y).saturating_add(1);
                        let new_width = col.saturating_sub(prompt_rect.x).saturating_add(1);
                        pane.set_prompt_height(new_height.clamp(3, 12));
                        pane.set_prompt_width(Some(
                            new_width.clamp(16.min(max_w.max(1)), max_w.max(16)),
                        ));
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
                    PromptMouseDrag::CanvasCharSelect => {
                        let rel_r = row
                            .saturating_sub(canvas_rect.y)
                            .min(canvas_rect.height.saturating_sub(1));
                        let rel_c = col
                            .saturating_sub(canvas_rect.x)
                            .min(canvas_rect.width.saturating_sub(1));
                        let copy = pane.copy_mode_mut();
                        copy.active = true;
                        if copy.anchor.is_none() {
                            copy.anchor = Some((copy.cursor_row, copy.cursor_col));
                        }
                        copy.cursor_row = rel_r;
                        copy.cursor_col = rel_c;
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::PromptTextSelect => {
                        let (ed_r, ed_c) =
                            prompt_editor_coords_from_mouse(pane, prompt_rect, col, row);
                        pane.prompt_mut().set_cursor_at(ed_r, ed_c, true);
                        return Ok(AppAction::Handled);
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if let Some(drag) = pane.take_mouse_drag() {
                match drag {
                    PromptMouseDrag::CanvasRectSelect => {
                        let _ = pane.stage_mouse_canvas_selection();
                    }
                    PromptMouseDrag::CanvasCharSelect => {
                        if pane.copy_mode().has_selection() {
                            let _ = pane.stage_mouse_canvas_selection();
                        }
                    }
                    PromptMouseDrag::PromptTextSelect => {
                        if let Some(sel) = pane.prompt().selected_text() {
                            pane.set_status_message(Some(format!(
                                "Selected {} chars (Ctrl+C copy | Ctrl+X cut | Esc clear)",
                                sel.chars().count()
                            )));
                        }
                    }
                    _ => {}
                }
                return Ok(AppAction::Handled);
            }
        }
        MouseEventKind::ScrollUp => {
            if in_prompt && pane.prompt().lines.len() > 1 {
                pane.prompt_mut().scroll_up(1);
            } else {
                pane.scroll_up_in_mode(3);
            }
            return Ok(AppAction::Handled);
        }
        MouseEventKind::ScrollDown => {
            if in_prompt && pane.prompt().lines.len() > 1 {
                pane.prompt_mut().scroll_down(1);
            } else {
                pane.scroll_down_in_mode(3);
            }
            return Ok(AppAction::Handled);
        }
        _ => {}
    }

    Ok(AppAction::Continue)
}

fn handle_canvas_only_mouse_event(
    pane: &mut CompositePane,
    mouse: MouseEvent,
    canvas_rect: Rect,
    in_canvas: bool,
) -> Result<AppAction> {
    let rel_r = mouse.row.saturating_sub(canvas_rect.y).min(canvas_rect.height.saturating_sub(1));
    let rel_c = mouse.column.saturating_sub(canvas_rect.x).min(canvas_rect.width.saturating_sub(1));
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) if in_canvas => {
            if mouse.modifiers.contains(KeyModifiers::CONTROL)
                && let Some(url) = pane.vt().url_at(rel_r, rel_c)
            {
                pane.record_yank(&url);
                pane.set_status_message(Some(format!("Yanked URL: {url}")));
                return Ok(AppAction::Handled);
            }
            let want_rect = mouse.modifiers.contains(KeyModifiers::ALT)
                || pane.copy_mode().rect_mode
                || (pane.copy_mode().active && pane.copy_mode().selection_kind == CopySelectionKind::Block);
            let extend = mouse.modifiers.contains(KeyModifiers::SHIFT) && pane.copy_mode().anchor.is_some();
            if want_rect {
                let copy = pane.copy_mode_mut();
                copy.active = true;
                if !extend {
                    copy.anchor = Some((rel_r, rel_c));
                }
                copy.cursor_row = rel_r;
                copy.cursor_col = rel_c;
                copy.selection_kind = CopySelectionKind::Block;
                copy.visual_lock = true;
                if extend {
                    let _ = pane.stage_mouse_canvas_selection();
                } else {
                    pane.set_status_message(Some(
                        "RECT SELECT [1×1]: Drag or Shift+Arrows | Enter/y copy | x to paste | Esc clear".to_string(),
                    ));
                }
                pane.set_mouse_drag(Some(PromptMouseDrag::CanvasRectSelect));
            } else if extend {
                let copy = pane.copy_mode_mut();
                copy.active = true;
                copy.cursor_row = rel_r;
                copy.cursor_col = rel_c;
                let _ = pane.stage_mouse_canvas_selection();
                pane.set_mouse_drag(Some(PromptMouseDrag::CanvasCharSelect));
            } else {
                let copy = pane.copy_mode_mut();
                copy.active = false;
                copy.anchor = None;
                copy.cursor_row = rel_r;
                copy.cursor_col = rel_c;
                copy.selection_kind = CopySelectionKind::Char;
                copy.visual_lock = false;
                pane.set_mouse_drag(Some(PromptMouseDrag::CanvasCharSelect));
            }
            Ok(AppAction::Handled)
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(drag) = pane.mouse_drag() {
                match drag {
                    PromptMouseDrag::CanvasRectSelect => {
                        let copy = pane.copy_mode_mut();
                        copy.active = true;
                        copy.selection_kind = CopySelectionKind::Block;
                        copy.visual_lock = true;
                        copy.cursor_row = rel_r;
                        copy.cursor_col = rel_c;
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::CanvasCharSelect => {
                        let copy = pane.copy_mode_mut();
                        copy.active = true;
                        if copy.anchor.is_none() {
                            copy.anchor = Some((copy.cursor_row, copy.cursor_col));
                        }
                        if mouse.modifiers.contains(KeyModifiers::ALT) || copy.rect_mode {
                            copy.selection_kind = CopySelectionKind::Block;
                            copy.visual_lock = true;
                        }
                        copy.cursor_row = rel_r;
                        copy.cursor_col = rel_c;
                        return Ok(AppAction::Handled);
                    }
                    _ => {}
                }
            }
            Ok(AppAction::Continue)
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if let Some(drag) = pane.take_mouse_drag() {
                match drag {
                    PromptMouseDrag::CanvasRectSelect => {
                        let _ = pane.stage_mouse_canvas_selection();
                    }
                    PromptMouseDrag::CanvasCharSelect if pane.copy_mode().has_selection() => {
                        let _ = pane.stage_mouse_canvas_selection();
                    }
                    _ => {}
                }
                return Ok(AppAction::Handled);
            }
            Ok(AppAction::Continue)
        }
        MouseEventKind::ScrollUp => {
            pane.scroll_up_in_mode(3);
            Ok(AppAction::Handled)
        }
        MouseEventKind::ScrollDown => {
            pane.scroll_down_in_mode(3);
            Ok(AppAction::Handled)
        }
        _ => Ok(AppAction::Continue),
    }
}

/// Routes a `MouseEvent` across the multi-pane `WindowManager`, hit-testing in reverse z-order,
/// selectively raising clicked panes (`WM-02`), and translating global coordinates to local SGR 1006 (`WM-03`)
/// or `DECSET ?1007` alternate scroll (`FND-02`).
pub fn handle_wm_mouse_event(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    screen_area: Rect,
) -> Result<AppAction> {
    if wm.rename_dialog_open() {
        wm.handle_rename_dialog_mouse(mouse, screen_area);
        return Ok(AppAction::Handled);
    }

    if let Some(menu) = crate::ui::drawer::navigator::standalone_context_menu_layout(wm, screen_area)
        && matches!(
            mouse.kind,
            MouseEventKind::Down(MouseButton::Left | MouseButton::Right)
        )
    {
        if let Some(entry_idx) = crate::ui::drawer::navigator::standalone_context_menu_entry_at(
            wm,
            screen_area,
            mouse.column,
            mouse.row,
        ) {
            wm.close_context_menu();
            wm.apply_drawer_nav(crate::wm::drawer::DrawerNavOp::MenuPick(entry_idx));
            let shell = crate::ui::shell::ShellLayout::compute(screen_area, wm.config(), wm.pane_count());
            wm.resize_all(shell.workspace)?;
            return Ok(AppAction::Handled);
        }
        wm.close_context_menu();
        if rect_contains(menu.rect, mouse.column, mouse.row) {
            return Ok(AppAction::Handled);
        }
    }

    let shell = crate::ui::shell::ShellLayout::compute(screen_area, wm.config(), wm.pane_count());
    let top_off = shell.top_offset();
    let bot_off = u16::from(shell.bottom_bar.is_some());
    let workspace_area = shell.workspace;

    if shell.top_bar_kind == Some(crate::ui::shell::TopBarKind::Pills) && mouse.row == screen_area.y {
        if let Some(bar) = shell.top_bar
            && let Some(pid) = crate::ui::navigator::pill_at(wm, bar, mouse.column)
        {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    wm.drawer_mut().focused = false;
                    wm.focus_and_raise(pid);
                }
                MouseEventKind::Down(MouseButton::Right) => {
                    wm.drawer_mut().focused = false;
                    wm.focus_and_raise(pid);
                    if let Some(idx) = wm
                        .drawer_rows()
                        .iter()
                        .position(|r| r.kind == crate::wm::DrawerRowKind::Pane(pid))
                    {
                        wm.drawer_mut().selected_index = idx;
                    }
                    wm.open_context_menu_at(mouse.column, mouse.row);
                }
                _ => {}
            }
        }
        return Ok(AppAction::Handled);
    }

    // An open menubar dropdown outranks both floating windows (51-08): a click on an item the
    // window happens to cover must reach the menu, not the window. Same for a modal (palette, help,
    // find): the part of it drawn over a floating window belongs to the modal (51-12).
    let over_modal = wm
        .active_modal_rect(workspace_area)
        .is_some_and(|r| rect_contains(r, mouse.column, mouse.row));
    if wm.menubar_state().is_none() && !over_modal {
        if let Some(action) = super::daemon_window::handle_daemon_window_mouse(wm, mouse, workspace_area) {
            return Ok(action);
        }
        if let Some(action) = super::fonts_window::handle_fonts_window_mouse(wm, mouse, workspace_area) {
            return Ok(action);
        }
        if let Some(action) = super::command_window::handle_command_window_mouse(wm, mouse, workspace_area) {
            return Ok(action);
        }
        if let Some(action) = super::clipboard_window::handle_clipboard_window_mouse(wm, mouse, workspace_area) {
            return Ok(action);
        }
        if let Some(action) = super::traffic_monitor::handle_traffic_monitor_mouse(wm, mouse, workspace_area) {
            return Ok(action);
        }
        if let Some(action) = super::script_studio::handle_script_studio_mouse(wm, mouse, workspace_area) {
            return Ok(action);
        }

        if let Some(action) = super::session_map::handle_session_map_mouse(wm, mouse, workspace_area) {
            return Ok(action);
        }

        // The AGENTS list is the smaller, top-most panel: it sees the event before the traffic window.
        if let Some(action) = super::agents_list::handle_agents_list_mouse(wm, mouse, workspace_area) {
            return Ok(action);
        }

        // The agents window floats above panes and drawer: it sees the event first and swallows
        // everything inside it (including the active pane's frame badges beneath it).
        if let Some(action) = super::agents_window::handle_agents_window_mouse(wm, mouse, workspace_area) {
            return Ok(action);
        }
    }

    if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) && wm.is_framed() {
        let active = wm.active_pane_id();
        let hit = wm
            .compute_workspace_rects(workspace_area)
            .into_iter()
            .find(|&(id, _)| id == active)
            .and_then(|(_, outer)| crate::ui::pane_actions::action_at(wm.config(), outer, mouse.column, mouse.row));
        if let Some(action) = hit {
            let _ = wm.apply(action);
            wm.resize_all(workspace_area)?;
            return Ok(AppAction::Handled);
        }
    }

    if top_off > 0 && mouse.row == screen_area.y {
        let rel_col = mouse.column.saturating_sub(screen_area.x);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(cat) = crate::ui::modals::menubar_category_at_col(rel_col) {
                    if cat == 6 && wm. is_help_visible() {
                        wm.toggle_help();
                    } else if wm.menubar_open_category() == Some(cat) {
                        if cat == 6 {
                            wm.toggle_help();
                        } else {
                            wm.set_menubar_open_category(None);
                        }
                    } else {
                        wm.set_menubar_open_category(Some(cat));
                    }
                } else {
                    wm.set_menubar_open_category(None);
                }
            }
            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Left) => {
                if wm.menubar_open_category().is_some()
                    && let Some(cat) = crate::ui::modals::menubar_category_at_col(rel_col)
                    && wm.menubar_open_category() != Some(cat)
                {
                    wm.set_menubar_open_category(Some(cat));
                }
            }
            _ => {}
        }
        return Ok(AppAction::Handled);
    }

    if bot_off > 0 && mouse.row == screen_area.y + screen_area.height.saturating_sub(1) {
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            let rel_col = mouse.column.saturating_sub(screen_area.x);
            let is_modern = matches!(
                wm.config().palette().name.as_str(),
                "MODERN-SLATE" | "LIQUID-GLASS" | "MIDNIGHT" | "ARCTIC"
            );
            if is_modern {
                match rel_col {
                    0..=10 => wm.toggle_help(),
                    11..=22 => {
                        let _ = wm.split_focused(crate::wm::split_tree::SplitDirection::Horizontal);
                    }
                    23..=33 => wm.toggle_zoom(),
                    34..=46 => wm.toggle_drawer(),
                    47..=59 => {
                        let _ = wm.apply(crate::action::Action::VisualStyleCommand(None));
                    }
                    60..=68 => wm.active_pane_mut().toggle_manual_raw_override(),
                    _ => {
                        let all = crate::ui::theme::ThemePreset::all();
                        let cur = wm.config().theme;
                        let idx = all.iter().position(|&t| t == cur).unwrap_or(0);
                        wm.config_mut().theme = all[(idx + 1) % all.len()];
                    }
                }
            } else {
                match rel_col {
                    0..=8 => wm.toggle_help(),
                    9..=18 => {
                        let _ = wm.split_focused(crate::wm::split_tree::SplitDirection::Horizontal);
                    }
                    19..=27 => wm.toggle_zoom(),
                    28..=36 => {
                        let _ = wm.shuffle_deck_next();
                    }
                    37..=47 => wm.toggle_quick_terminal(),
                    48..=56 => wm.active_pane_mut().toggle_manual_raw_override(),
                    _ => {
                        let all = crate::ui::theme::ThemePreset::all();
                        let cur = wm.config().theme;
                        let idx = all.iter().position(|&t| t == cur).unwrap_or(0);
                        wm.config_mut().theme = all[(idx + 1) % all.len()];
                    }
                }
            }
            wm.resize_all(workspace_area)?;
        }
        return Ok(AppAction::Handled);
    }

    // An in-flight move/resize drag (set by a prior Down) owns Drag/Up events even while the
    // pointer is over a modal, whose handler would otherwise swallow them.
    if let Some(action) = handle_wm_window_drag_event(wm, mouse, workspace_area)? {
        return Ok(action);
    }

    if let Some(action) = handle_modal_mouse_event(wm, mouse, workspace_area) {
        return Ok(action);
    }

    if let Some(action) = handle_drawer_mouse_event(wm, mouse, workspace_area) {
        return Ok(action);
    }

    if let Some(action) = super::link_mouse::handle_link_banner_click(wm, mouse, workspace_area) {
        return Ok(action);
    }

    if super::mouse_focus::swallow_focus_click_tail(wm, mouse.kind) {
        return Ok(AppAction::Handled);
    }

    let Some((hit_focus, outer, inner)) = super::mouse_focus::resolve_hit(wm, mouse, workspace_area)
    else {
        return Ok(AppAction::Continue);
    };

    let was_active = super::mouse_focus::is_active_hit(wm, hit_focus);
    super::mouse_focus::focus_on_down(wm, mouse.kind, hit_focus);

    if mouse.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(mouse.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown)
    {
        if let Focus::Pane(hit_id) = hit_focus {
            wm.focus_and_raise(hit_id);
        }
        if matches!(mouse.kind, MouseEventKind::ScrollUp) {
            wm.font_zoom_in();
        } else {
            wm.font_zoom_out();
        }
        wm.resize_all(workspace_area)?;
        return Ok(AppAction::Handled);
    }

    if !rect_contains(inner, mouse.column, mouse.row) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => match hit_focus {
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
            },
            MouseEventKind::Down(MouseButton::Right) => {
                if let Focus::Pane(hit_id) = hit_focus
                    && wm.is_framed()
                {
                    wm.focus_and_raise(hit_id);
                    if let Some(idx) = wm
                        .drawer_rows()
                        .iter()
                        .position(|r| r.kind == crate::wm::DrawerRowKind::Pane(hit_id))
                    {
                        wm.drawer_mut().selected_index = idx;
                    }
                    wm.open_context_menu_at(mouse.column, mouse.row);
                    return Ok(AppAction::Handled);
                }
            }
            _ => {}
        }
    }

    // GUARD-03: a click on an inactive pane is consumed purely as focus+raise.
    if super::mouse_focus::consumes_inactive_click(was_active, mouse.kind) {
        super::mouse_focus::mark_focus_click(wm);
        return Ok(AppAction::Handled);
    }

    let (pane_inner, pane_mouse) =
        wm.translate_pane_mouse_event(hit_focus, inner, screen_area, mouse);

    let on_pane_status_bar = pane_inner.height > 0
        && pane_mouse.row == pane_inner.y + pane_inner.height.saturating_sub(1)
        && pane_mouse.column >= pane_inner.x
        && pane_mouse.column < pane_inner.x + pane_inner.width;
    if on_pane_status_bar && matches!(pane_mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        let status_rect = Rect::new(
            pane_inner.x,
            pane_inner.y + pane_inner.height.saturating_sub(1),
            pane_inner.width,
            1,
        );
        if let Some(p) = wm.pane_by_focus(hit_focus) {
            let col = pane_mouse.column;
            if let Some(r) = crate::ui::oia::oia_mic_badge_rect(p, status_rect) && col >= r.x && col < r.x + r.width {
                let _ = wm.apply(crate::action::Action::Voice(crate::action::VoiceOp::Toggle));
                return Ok(AppAction::Handled);
            }
            if let Some(r) = crate::ui::oia::oia_mouse_badge_rect(p, status_rect) && col >= r.x && col < r.x + r.width {
                let _ = wm.apply(crate::action::Action::SelectCommand(Some("mouse toggle".to_string())));
                return Ok(AppAction::Handled);
            }
            if let Some(r) = crate::ui::oia::oia_copy_badge_rect(p, status_rect) && col >= r.x && col < r.x + r.width {
                let _ = wm.apply(crate::action::Action::CopyModeToggle);
                return Ok(AppAction::Handled);
            }
        }
    }

    let auto_copy = wm.config().copy_on_select;
    let override_policy =
        crate::input::mouse_select::OverridePolicy::parse(&wm.config().mouse_select_override);
    let Some(pane) = wm.pane_by_focus_mut(hit_focus) else {
        return Ok(AppAction::Continue);
    };
    let route = crate::input::mouse_select::route(
        pane_mouse.modifiers,
        pane_mouse.kind,
        pane.vt().is_mouse_sgr_or_tracking(),
        pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough,
        pane.copy_mode().active,
        pane.copy_mode().rect_mode,
        pane.mouse_drag().is_some(),
        pane.mouse_select_sticky(),
        override_policy,
    );

    if !on_pane_status_bar
        && route == crate::input::mouse_select::MouseRoute::ToChild
        && pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough
    {
        // If the child app has not enabled mouse tracking (?1000..1006), check DECSET ?1007 Alternate Scroll first
        if !pane.vt().is_mouse_sgr_or_tracking() {
            if let Some(scroll_bytes) = encode_alternate_scroll_wheel(pane, pane_mouse.kind) {
                pane.write_to_pty(&scroll_bytes)?;
                return Ok(AppAction::Handled);
            }
        } else if let Some(sgr_bytes) = crate::wm::quick_terminal::encode_sgr_mouse_with_mode(
            pane_mouse,
            pane_inner,
            pane.vt().mouse_report_level(),
        ) {
            pane.write_to_pty(&sgr_bytes)?;
            return Ok(AppAction::Handled);
        }
    }

    if let crate::input::mouse_select::MouseRoute::Local(_) = route
        && matches!(pane_mouse.kind, MouseEventKind::Down(MouseButton::Left))
    {
        let canvas_rect = pane.canvas_rect(pane_inner);
        let on_prompt_pin_badge = pane.is_prompt_visible() && {
            let pr = pane.prompt_rect(pane_inner);
            let right_edge = pr.x + pr.width;
            pr.width >= 16
                && pane_mouse.row == pr.y
                && pane_mouse.column >= right_edge.saturating_sub(8)
                && pane_mouse.column < right_edge.saturating_sub(2)
        };
        // A floating prompt overlays the canvas: clicks on it (text, border resize, corner) belong
        // to the prompt handler, never to canvas char-select.
        let on_prompt = pane.is_prompt_visible() && rect_contains(pane.prompt_rect(pane_inner), pane_mouse.column, pane_mouse.row);
        let has_media_canvas = pane.browser_canvas().is_some() || pane.video_canvas().is_some();
        if !on_prompt_pin_badge
            && !on_prompt
            && !has_media_canvas
            && !pane.copy_mode().rect_mode
            && !pane_mouse.modifiers.contains(KeyModifiers::ALT)
            && !pane_mouse.modifiers.contains(KeyModifiers::CONTROL)
            && pane_mouse.column >= canvas_rect.x
            && pane_mouse.column < canvas_rect.x + canvas_rect.width
            && pane_mouse.row >= canvas_rect.y
            && pane_mouse.row < canvas_rect.y + canvas_rect.height
        {
            let rel_r = pane_mouse.row.saturating_sub(canvas_rect.y).min(canvas_rect.height.saturating_sub(1));
            let rel_c = pane_mouse.column.saturating_sub(canvas_rect.x).min(canvas_rect.width.saturating_sub(1));
            let top_abs = pane.vt_mut().viewport_top_abs();
            let count = pane.record_click(pane.last_tick_ms(), rel_c, rel_r);
            if count == 2 {
                let text = pane.vt_mut().abs_row_text(top_abs + usize::from(rel_r)).unwrap_or_default();
                let (sc, ec) = crate::pane::copy_motion::word_bounds_at(&text, rel_c);
                let copy = pane.copy_mode_mut();
                copy.begin_selection(rel_r, sc, crate::pane::CopySelectionKind::Char, top_abs);
                copy.cursor_col = ec;
                copy.active = true;
                copy.origin = crate::pane::CopyOrigin::Mouse;
                return Ok(AppAction::Handled);
            } else if count == 3 {
                let copy = pane.copy_mode_mut();
                copy.begin_selection(rel_r, 0, crate::pane::CopySelectionKind::Line, top_abs);
                copy.active = true;
                copy.origin = crate::pane::CopyOrigin::Mouse;
                return Ok(AppAction::Handled);
            }
            let copy = pane.copy_mode_mut();
            copy.begin_selection(rel_r, rel_c, crate::pane::CopySelectionKind::Char, top_abs);
            copy.active = true;
            copy.visual_lock = false;
            copy.origin = crate::pane::CopyOrigin::Mouse;
            pane.set_mouse_drag(Some(PromptMouseDrag::CanvasCharSelect));
            return Ok(AppAction::Handled);
        }
    }

    let res = handle_mouse_event(pane, pane_mouse, pane_inner);
    if matches!(pane_mouse.kind, MouseEventKind::Up(MouseButton::Left))
        && let Some(p) = wm.pane_by_focus_mut(hit_focus)
    {
        let is_mouse_origin = p.copy_mode().origin == crate::pane::CopyOrigin::Mouse;
        let no_extent = p.copy_mode().anchor == Some((p.copy_mode().cursor_row, p.copy_mode().cursor_col));
        if is_mouse_origin && no_extent {
            p.copy_mode_mut().clear_selection();
        } else if auto_copy
            && p.copy_mode().has_selection()
            && p.copy_mode().selection_kind != crate::pane::CopySelectionKind::Block
        {
            let _ = p.yank_mouse_canvas_selection();
            if is_mouse_origin
                && (p.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough
                    || p.vt().is_alternate_screen())
            {
                p.copy_mode_mut().active = false;
            }
        } else if auto_copy && let Some(copied) = p.prompt_mut().copy_selection() {
            p.record_yank_with_source(
                crate::wm::clipboard_registry::ClipboardSource::PromptCopy,
                &copied,
            );
        }
    }
    wm.sync_clipboard_records();
    wm.set_mouse_pixel_hint(None);
    res
}
