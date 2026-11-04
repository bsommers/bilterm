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
use crate::ui::drawer::navigator::navigator_row_at;
use crate::wm::WindowManager;
use crate::wm::drawer::DrawerNavOp;

/// Looks `key` up in the navigator key table; `None` lets the classic drawer handler run.
pub(super) fn handle_navigator_drawer_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let scopes = [DrawerKeyScope::Navigator];
    let action = navigator_drawer_action(&scopes, key)?;
    let _ = wm.apply(action);
    Some(AppAction::Handled)
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
    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind
        && let Some(i) = navigator_row_at(wm, workspace_area, mouse.column, mouse.row)
    {
        let _ = wm.apply(Action::DrawerFocus);
        let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Row(i)));
        let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Activate));
    }
    Some(AppAction::Handled)
}
