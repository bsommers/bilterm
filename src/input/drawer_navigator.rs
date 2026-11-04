//! Key handling for the focused navigator drawer (`FLT-01`, Phase 50-06).
//!
//! This module only resolves which key scopes are active, looks the key up in the declarative
//! [`NAVIGATOR_DRAWER_BINDINGS`](super::keymap::NAVIGATOR_DRAWER_BINDINGS) table and runs the resulting
//! [`Action`](crate::action::Action) through `WindowManager::apply`. It never changes drawer, pane or
//! prompt state itself (`ARCH-R01`).

use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::keymap::{DrawerKeyScope, navigator_drawer_action};
use crate::action::Action;
use crate::ui::drawer::navigator::{
    NavigatorButtonKind, navigator_fold_marker_cells, navigator_footer_buttons, navigator_row_at,
};
use crate::wm::agents_cmd::AgentsOp;
use crate::wm::drawer::DrawerNavOp;
use crate::wm::{DrawerRowKind, DrawerVisibilityMode, WindowManager};

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
    _dr: Rect,
    workspace_area: Rect,
) -> Option<AppAction> {
    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        let (col, row) = (mouse.column, mouse.row);
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
