//! Key handling for the focused navigator drawer (`FLT-01`, Phase 50-06).
//!
//! This module only resolves which key scopes are active, looks the key up in the declarative
//! [`NAVIGATOR_DRAWER_BINDINGS`](super::keymap::NAVIGATOR_DRAWER_BINDINGS) table and runs the resulting
//! [`Action`](crate::action::Action) through `WindowManager::apply`. It never changes drawer, pane or
//! prompt state itself (`ARCH-R01`).

use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::mouse_wm::detect_title_bar_badge_click;
use super::keymap::{DrawerKeyScope, navigator_drawer_action};
use crate::action::Action;
use crate::ui::drawer::navigator::{
    NavigatorButtonKind, navigator_fold_marker_cells, navigator_footer_buttons, navigator_row_at,
};
use crate::wm::agents_cmd::AgentsOp;
use crate::wm::drawer::DrawerNavOp;
use crate::wm::{DrawerEdge, DrawerRowKind, DrawerVisibilityMode, WindowManager};

/// Looks `key` up in the navigator key table; `None` lets the classic drawer handler run.
pub(super) fn handle_navigator_drawer_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let scopes = [DrawerKeyScope::Navigator];
    let action = navigator_drawer_action(&scopes, key)?;
    let _ = wm.apply(action);
    Some(AppAction::Handled)
}

/// Action a footer button applies, the same one its key twin lowers to. Reads state only to choose the
/// pin direction and the selected pane id for zoom.
pub(super) fn navigator_button_action(wm: &WindowManager, kind: NavigatorButtonKind) -> Option<Action> {
    Some(match kind {
        NavigatorButtonKind::Pin => Action::DrawerVisibility(Some(
            if wm.drawer_config().visibility == DrawerVisibilityMode::Fixed {
                DrawerVisibilityMode::OnDemand
            } else {
                DrawerVisibilityMode::Fixed
            },
        )),
        NavigatorButtonKind::Attention => Action::DrawerNav(DrawerNavOp::NextAttention),
        NavigatorButtonKind::NewSession => Action::DrawerNav(DrawerNavOp::NewSession),
        NavigatorButtonKind::NewPane => Action::DrawerNav(DrawerNavOp::NewPane),
        NavigatorButtonKind::Rename => Action::DrawerNav(DrawerNavOp::RenamePrompt),
        NavigatorButtonKind::Close => Action::DrawerNav(DrawerNavOp::Close),
        NavigatorButtonKind::Zoom => {
            let pane = wm.drawer_rows().get(wm.drawer().selected_index).and_then(|r| match r.kind {
                DrawerRowKind::Pane(id) => Some(id),
                _ => None,
            });
            Action::Agents(AgentsOp::Zoom(pane))
        }
    })
}

/// Mouse handling for the navigator-style drawer; the caller guarantees the event is inside `dr`.
///
/// Every visible effect lowers to an [`Action`] applied with `WindowManager::apply` (`ARCH-R01`), so the
/// keyboard, the colon prompt and the mouse share one code path. Events no gesture claims are swallowed so
/// the classic absolute-column zones are never reached in navigator style.
pub(super) fn handle_navigator_drawer_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    dr: Rect,
    workspace_area: Rect,
) -> Option<AppAction> {
    if matches!(mouse.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
        // Navigator rows truncate instead of scrolling sideways, so Shift scrolls like the plain wheel.
        let step = if matches!(mouse.kind, MouseEventKind::ScrollUp) { -3 } else { 3 };
        let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Scroll(step)));
        return Some(AppAction::Handled);
    }
    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        let (col, row) = (mouse.column, mouse.row);
        if row == dr.y
            && let Some(badge) = detect_title_bar_badge_click(wm, dr, col)
        {
            match badge {
                0 => {
                    let _ = wm.apply(Action::DrawerSize(0));
                }
                1 => {
                    let _ = wm.apply(Action::DrawerSize(u16::MAX));
                }
                _ => {
                    let _ = wm.apply(Action::DrawerVisibility(Some(DrawerVisibilityMode::OnDemand)));
                    let _ = wm.apply(Action::DrawerToggle);
                }
            }
            return Some(AppAction::Handled);
        }
        let on_resize_border = match wm.drawer_config().edge {
            DrawerEdge::Left => col == dr.x + dr.width.saturating_sub(1),
            DrawerEdge::Right => col == dr.x,
            DrawerEdge::Top => row == dr.y + dr.height.saturating_sub(1),
            DrawerEdge::Bottom => row == dr.y,
        };
        if on_resize_border {
            // Pointer-gesture bookkeeping, not an operation: the resize itself lowers to `DrawerSize`.
            wm.drawer_mut().resizing_drag = true;
            return Some(AppAction::Handled);
        }
        let button = navigator_footer_buttons(wm, workspace_area)
            .into_iter()
            .find(|b| b.y == row && col >= b.x && col < b.x + b.width);
        if let Some(b) = button {
            if let Some(action) = navigator_button_action(wm, b.kind) {
                let _ = wm.apply(action);
            }
            return Some(AppAction::Handled);
        }
        let marker = navigator_fold_marker_cells(wm, workspace_area).into_iter().find(|c| c.x == col && c.y == row);
        if let Some(cell) = marker {
            let _ = wm.apply(Action::DrawerFocus);
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Row(cell.row_index)));
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Fold));
        } else if let Some(i) = navigator_row_at(wm, workspace_area, col, row) {
            let _ = wm.apply(Action::DrawerFocus);
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Row(i)));
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Activate));
        }
    }
    Some(AppAction::Handled)
}

/// Edge-drag motion in navigator style: lowers to `Action::DrawerSize`. The clamp keeps a drag away from
/// `0` and `u16::MAX`, the minimize and maximize values; `resize_drawer` applies the real per-edge range.
pub(super) fn navigator_drag_resize(wm: &mut WindowManager, new_size: u16) -> Option<AppAction> {
    let _ = wm.apply(Action::DrawerSize(new_size.clamp(1, u16::MAX - 1)));
    Some(AppAction::Handled)
}
