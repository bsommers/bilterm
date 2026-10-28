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
use crate::vt::encode_osc52_clipboard;
use crate::wm::{Focus, WindowManager, encode_sgr_mouse_local, rect_contains};

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
        let title_w = if pane.show_version_badge() {
            concat!("3270-OIA v", env!("CARGO_PKG_VERSION")).chars().count()
        } else {
            "3270-OIA".chars().count()
        };
        let state_w = if pane.is_password_prompt() {
            15
        } else if pane.vt().is_alternate_screen()
            || pane.is_holding_inspected()
            || pane.is_page_holding()
            || pane.has_foreground_subcommand()
        {
            11
        } else if pane.vt().scrollback_offset() > 0 {
            10
        } else {
            9
        };
        let routing_w = if pane.copy_mode().active { 28 } else { 16 };
        let anchor_w = match pane.prompt().anchor {
            PromptAnchor::SplitTop | PromptAnchor::SplitBottom => 9,
            _ => 7,
        };
        let title_end = title_w;
        let state_end = title_end + 1 + state_w;
        let routing_end = state_end + 1 + routing_w;
        let pin_end = routing_end + 2 + anchor_w;
        let prot_end = if pane.is_read_only() { pin_end + 11 } else { pin_end };

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
        let clip = crate::clipboard::get_system_clipboard()
            .unwrap_or_else(|| pane.prompt().yank_buffer.clone());
        if !clip.is_empty() {
            return crate::input::handle_paste_event(pane, &clip);
        }
        return Ok(AppAction::Handled);
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
                    crate::clipboard::set_system_clipboard(&url);
                    pane.set_status_message(Some(format!("Yanked URL: {url}")));
                    return Ok(AppAction::Handled);
                }
            }

            // Alt+LeftClick on OutputCanvas: start rectangular block selection (VAL-04)
            if mouse.modifiers.contains(KeyModifiers::ALT) && in_canvas && !in_prompt {
                pane.prompt_mut().clear_selection();
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

            // Plain LeftClick on OutputCanvas: focus OutputCanvas cursor & start character stream selection
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
                    let _ = pane.yank_mouse_canvas_selection();
                } else {
                    let copy = pane.copy_mode_mut();
                    copy.active = true;
                    copy.anchor = None;
                    copy.cursor_row = rel_r;
                    copy.cursor_col = rel_c;
                    copy.selection_kind = CopySelectionKind::Char;
                    copy.visual_lock = false;
                    pane.set_status_message(Some(
                        "CANVAS SELECT: Arrows move | Shift+Arrows/Drag select | Enter/y copy | x to prompt | Tab/F8/Esc -> Prompt"
                            .to_string(),
                    ));
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
                        let _ = pane.yank_copy_selection();
                    }
                    PromptMouseDrag::CanvasCharSelect => {
                        if pane.copy_mode().has_selection() {
                            let _ = pane.yank_mouse_canvas_selection();
                        }
                    }
                    PromptMouseDrag::PromptTextSelect => {
                        if pane.prompt().has_selection()
                            && let Some(copied) = pane.prompt_mut().copy_selection()
                        {
                            pane.set_last_osc52_sequence(Some(encode_osc52_clipboard(&copied)));
                            pane.set_status_message(Some(format!(
                                "Selected & copied {} chars",
                                copied.chars().count()
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
    let rel_r = mouse
        .row
        .saturating_sub(canvas_rect.y)
        .min(canvas_rect.height.saturating_sub(1));
    let rel_c = mouse
        .column
        .saturating_sub(canvas_rect.x)
        .min(canvas_rect.width.saturating_sub(1));
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) if in_canvas => {
            if mouse.modifiers.contains(KeyModifiers::ALT) {
                let copy = pane.copy_mode_mut();
                copy.active = true;
                copy.anchor = Some((rel_r, rel_c));
                copy.cursor_row = rel_r;
                copy.cursor_col = rel_c;
                copy.selection_kind = CopySelectionKind::Block;
                pane.set_mouse_drag(Some(PromptMouseDrag::CanvasRectSelect));
            } else {
                let copy = pane.copy_mode_mut();
                copy.active = false;
                copy.anchor = None;
                copy.cursor_row = rel_r;
                copy.cursor_col = rel_c;
                copy.selection_kind = CopySelectionKind::Char;
                pane.set_mouse_drag(Some(PromptMouseDrag::CanvasCharSelect));
            }
            Ok(AppAction::Handled)
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(drag) = pane.mouse_drag() {
                match drag {
                    PromptMouseDrag::CanvasRectSelect => {
                        let copy = pane.copy_mode_mut();
                        copy.cursor_row = rel_r;
                        copy.cursor_col = rel_c;
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::CanvasCharSelect => {
                        let copy = pane.copy_mode_mut();
                        if copy.anchor.is_none() {
                            copy.anchor = Some((copy.cursor_row, copy.cursor_col));
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
                        let _ = pane.yank_copy_selection();
                    }
                    PromptMouseDrag::CanvasCharSelect if pane.copy_mode().has_selection() => {
                        let _ = pane.yank_mouse_canvas_selection();
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
    let top_off = u16::from(wm.config().show_top_menubar && screen_area.height > 3);
    let bot_off = u16::from(wm.config().show_dos_fkey_bar && screen_area.height > 2 + top_off);
    let workspace_area = Rect {
        x: screen_area.x,
        y: screen_area.y + top_off,
        width: screen_area.width,
        height: screen_area.height.saturating_sub(top_off + bot_off).max(1),
    };

    if top_off > 0 && mouse.row == screen_area.y {
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
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
        }
        return Ok(AppAction::Handled);
    }

    if bot_off > 0 && mouse.row == screen_area.y + screen_area.height.saturating_sub(1) {
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            let rel_col = mouse.column.saturating_sub(screen_area.x);
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
            wm.resize_all(workspace_area)?;
        }
        return Ok(AppAction::Handled);
    }

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

    let on_pane_status_bar = inner.height > 0
        && mouse.row == inner.y + inner.height.saturating_sub(1)
        && mouse.column >= inner.x
        && mouse.column < inner.x + inner.width;

    let (pane_inner, pane_mouse) = if let Focus::Pane(hit_id) = hit_focus {
        let scaled = wm.pane_scaled_inner_rect(hit_id, inner);
        if scaled != inner && inner.width > 0 && inner.height > 0 {
            let rel_c = mouse
                .column
                .saturating_sub(inner.x)
                .min(inner.width.saturating_sub(1));
            let rel_r = mouse
                .row
                .saturating_sub(inner.y)
                .min(inner.height.saturating_sub(1));
            let sc = scaled.x
                + ((u32::from(rel_c) * u32::from(scaled.width)) / u32::from(inner.width)) as u16;
            let sr = if on_pane_status_bar {
                scaled.y + scaled.height.saturating_sub(1)
            } else {
                scaled.y
                    + ((u32::from(rel_r) * u32::from(scaled.height)) / u32::from(inner.height))
                        as u16
            };
            (
                scaled,
                MouseEvent {
                    column: sc,
                    row: sr,
                    ..mouse
                },
            )
        } else {
            (inner, mouse)
        }
    } else {
        (inner, mouse)
    };

    let Some(pane) = wm.pane_by_focus_mut(hit_focus) else {
        return Ok(AppAction::Continue);
    };

    if !on_pane_status_bar && pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough {
        // If the child app has not enabled mouse tracking (?1000..1006), check DECSET ?1007 Alternate Scroll first
        if !pane.vt().is_mouse_sgr_or_tracking() {
            if let Some(scroll_bytes) = encode_alternate_scroll_wheel(pane, pane_mouse.kind) {
                pane.write_to_pty(&scroll_bytes)?;
                return Ok(AppAction::Handled);
            }
        } else if let Some(sgr_bytes) = encode_sgr_mouse_local(pane_mouse, pane_inner) {
            pane.write_to_pty(&sgr_bytes)?;
            return Ok(AppAction::Handled);
        }
    }

    handle_mouse_event(pane, pane_mouse, pane_inner)
}
