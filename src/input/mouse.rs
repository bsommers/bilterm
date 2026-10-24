use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::error::Result;
use crate::input::AppAction;
use crate::input::encode::encode_alternate_scroll_wheel;
use crate::pane::{CompositePane, CopySelectionKind, PromptAnchor, PromptMouseDrag};
use crate::pty::line_discipline::InputRoutingMode;
use crate::vt::encode_osc52_clipboard;
use crate::wm::{WindowManager, encode_sgr_mouse_local};

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
            if let Some(pty) = &pane.pty {
                pty.write_all(&scroll_bytes)?;
            }
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
                if let Some(url) = pane.vt.url_at(rel_r, rel_c) {
                    let seq = encode_osc52_clipboard(&url);
                    pane.last_osc52_sequence = Some(seq);
                    pane.prompt.yank_buffer = url.clone();
                    pane.status_message = Some(format!("Yanked URL: {url}"));
                    return Ok(AppAction::Handled);
                }
            }

            // Alt+LeftClick on OutputCanvas: start rectangular block selection (VAL-04)
            if mouse.modifiers.contains(KeyModifiers::ALT) && in_canvas && !in_prompt {
                let rel_r = row.saturating_sub(canvas_rect.y);
                let rel_c = col.saturating_sub(canvas_rect.x);
                pane.copy_mode.active = true;
                pane.copy_mode.anchor = Some((rel_r, rel_c));
                pane.copy_mode.cursor_row = rel_r;
                pane.copy_mode.cursor_col = rel_c;
                pane.copy_mode.selection_kind = CopySelectionKind::Block;
                pane.mouse_drag = Some(PromptMouseDrag::CanvasRectSelect);
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
                pane.mouse_drag = Some(PromptMouseDrag::MovePrompt {
                    grab_x_offset: col.saturating_sub(prompt_rect.x),
                });
                return Ok(AppAction::Handled);
            }

            // Bottom border of floating prompt: check bottom-right corner (`◢`) or start height resize drag
            let bottom_y = prompt_rect.y + prompt_rect.height.saturating_sub(1);
            let right_x = prompt_rect.x + prompt_rect.width.saturating_sub(1);
            if row == bottom_y && col == right_x {
                pane.mouse_drag = Some(PromptMouseDrag::ResizePromptCorner);
                return Ok(AppAction::Handled);
            }
            if row == bottom_y && col >= prompt_rect.x && col < prompt_rect.x + prompt_rect.width {
                pane.mouse_drag = Some(PromptMouseDrag::ResizePrompt);
                return Ok(AppAction::Handled);
            }

            // Left border of floating prompt: start left width resize drag
            if col == prompt_rect.x && row > prompt_rect.y && row < bottom_y {
                pane.mouse_drag = Some(PromptMouseDrag::ResizePromptLeft);
                return Ok(AppAction::Handled);
            }

            // Right border of floating prompt: start right width resize drag
            if col == right_x && row > prompt_rect.y && row < bottom_y {
                pane.mouse_drag = Some(PromptMouseDrag::ResizePromptRight);
                return Ok(AppAction::Handled);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(drag) = pane.mouse_drag {
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
                        if pane.prompt.width_cols.is_some() {
                            let target_x = col
                                .saturating_sub(grab_x_offset)
                                .saturating_sub(canvas_rect.x);
                            let max_x = canvas_rect.width.saturating_sub(prompt_rect.width);
                            pane.prompt.x_offset = Some(target_x.min(max_x));
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
                        pane.prompt.x_offset = Some(clamped_col.saturating_sub(canvas_rect.x));
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
                        pane.copy_mode.cursor_row = rel_r;
                        pane.copy_mode.cursor_col = rel_c;
                        return Ok(AppAction::Handled);
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if let Some(drag) = pane.mouse_drag.take() {
                if drag == PromptMouseDrag::CanvasRectSelect {
                    let _ = pane.yank_copy_selection();
                }
                return Ok(AppAction::Handled);
            }
        }
        MouseEventKind::ScrollUp => {
            if in_prompt && pane.prompt.lines.len() > 1 {
                pane.prompt.scroll_up(1);
            } else {
                pane.vt.scroll_up(3);
            }
            return Ok(AppAction::Handled);
        }
        MouseEventKind::ScrollDown => {
            if in_prompt && pane.prompt.lines.len() > 1 {
                pane.prompt.scroll_down(1);
            } else {
                pane.vt.scroll_down(3);
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
    if wm.config.show_top_menubar
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
        if wm.menubar_open_category == Some(cat) {
            wm.menubar_open_category = None;
        } else {
            wm.menubar_open_category = Some(cat);
            wm.menubar_selected_item = 0;
        }
        return Ok(AppAction::Handled);
    }
    let Some((hit_id, _outer, inner)) = wm.hit_test_pane(mouse.column, mouse.row, screen_area)
    else {
        return Ok(AppAction::Continue);
    };

    if matches!(mouse.kind, MouseEventKind::Down(_)) && hit_id != 0 {
        wm.focus_and_raise(hit_id);
    }

    let pane = if hit_id == 0 {
        &mut wm.quick_terminal.pane
    } else if let Some(p) = wm.panes.get_mut(&hit_id) {
        p
    } else {
        return Ok(AppAction::Continue);
    };

    if pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough {
        // If the child app has not enabled mouse tracking (?1000..1006), check DECSET ?1007 Alternate Scroll first
        if !pane.vt.is_mouse_sgr_or_tracking() {
            if let Some(scroll_bytes) = encode_alternate_scroll_wheel(pane, mouse.kind) {
                if let Some(pty) = &pane.pty {
                    pty.write_all(&scroll_bytes)?;
                }
                return Ok(AppAction::Handled);
            }
        } else if let Some(sgr_bytes) = encode_sgr_mouse_local(mouse, inner) {
            if let Some(pty) = &pane.pty {
                pty.write_all(&sgr_bytes)?;
            }
            return Ok(AppAction::Handled);
        }
    }

    handle_mouse_event(pane, mouse, inner)
}
