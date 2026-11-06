//! Mouse handling for the FONTS utility window (`WIN-01`, Phase 51.2).

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::mouse_wm::apply_rect_edge_resize;
use crate::action::Action;
use crate::wm::WindowManager;
use crate::wm::agents_list::ListDrag;
use crate::wm::fonts_window::{FontsWindowBadge, FontsWindowHit, FontsWindowOp};

/// Handles pointer interactions for the `FONTS` utility window.
pub(super) fn handle_fonts_window_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    if !wm.fonts_window().active || wm.fonts_window_rect(workspace_area).is_none() {
        return None;
    }

    // Active pointer drag takes priority over all other hits
    if let Some(drag) = wm.fonts_window().drag.clone() {
        match mouse.kind {
            MouseEventKind::Drag(MouseButton::Left) => {
                let cur = wm.fonts_window_rect(workspace_area)?;
                let rect = match drag {
                    ListDrag::Move { grab_dx, grab_dy } => Rect::new(
                        mouse.column.saturating_sub(grab_dx),
                        mouse.row.saturating_sub(grab_dy),
                        cur.width,
                        cur.height,
                    ),
                    ListDrag::Resize { edge, initial } => {
                        apply_rect_edge_resize(initial, edge, mouse.column, mouse.row, workspace_area)
                    }
                };
                let op = FontsWindowOp::SetRect {
                    x: rect.x,
                    y: rect.y,
                    w: rect.width,
                    h: rect.height,
                };
                let _ = wm.apply(Action::FontsWindow(op));
                return Some(AppAction::Handled);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                wm.fonts_window_mut().drag = None;
                return Some(AppAction::Handled);
            }
            _ => {}
        }
    }

    let hit = wm.fonts_window_hit(workspace_area, mouse.column, mouse.row);

    if hit == FontsWindowHit::Outside {
        if wm.fonts_window().focused && matches!(mouse.kind, MouseEventKind::Down(_)) {
            let _ = wm.apply(Action::FontsWindow(FontsWindowOp::Blur));
            // Let the click fall through to activate underlying pane or split
            return None;
        }
        return None;
    }

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            let _ = wm.apply(Action::FontsWindow(FontsWindowOp::Scroll(-3)));
            Some(AppAction::Handled)
        }
        MouseEventKind::ScrollDown => {
            let _ = wm.apply(Action::FontsWindow(FontsWindowOp::Scroll(3)));
            Some(AppAction::Handled)
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(outer) = wm.fonts_window_rect(workspace_area) else {
                return Some(AppAction::Handled);
            };
            match hit {
                FontsWindowHit::Badge(FontsWindowBadge::Close) => {
                    let _ = wm.apply(Action::FontsWindow(FontsWindowOp::Close));
                }
                FontsWindowHit::Badge(FontsWindowBadge::Dock) => {
                    let _ = wm.apply(Action::FontsWindow(FontsWindowOp::Dock(None)));
                }
                FontsWindowHit::Badge(FontsWindowBadge::Pin) => {
                    let _ = wm.apply(Action::FontsWindow(FontsWindowOp::Pin(None)));
                }
                FontsWindowHit::Badge(FontsWindowBadge::Scan) => {
                    let _ = wm.apply(Action::FontsWindow(FontsWindowOp::Scan));
                }
                FontsWindowHit::Title => {
                    wm.fonts_window_mut().drag = Some(ListDrag::Move {
                        grab_dx: mouse.column.saturating_sub(outer.x),
                        grab_dy: mouse.row.saturating_sub(outer.y),
                    });
                }
                FontsWindowHit::Border(edge) => {
                    wm.fonts_window_mut().drag = Some(ListDrag::Resize {
                        edge,
                        initial: outer,
                    });
                }
                FontsWindowHit::Row(idx) => {
                    if wm.fonts_window().selected == idx {
                        let _ = wm.apply(Action::FontsWindow(FontsWindowOp::ApplySelected));
                    } else {
                        let _ = wm.apply(Action::FontsWindow(FontsWindowOp::SelectRow(idx)));
                    }
                }
                _ => {}
            }
            Some(AppAction::Handled)
        }
        _ => Some(AppAction::Handled),
    }
}
