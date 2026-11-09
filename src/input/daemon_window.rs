//! Mouse handling for the DAEMON SUPERVISOR window.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use crate::wm::WindowManager;
use crate::wm::daemon_window::DaemonWindowHit;

pub(super) fn handle_daemon_window_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    if !wm.daemon_window().active || wm.daemon_window_rect(workspace_area).is_none() {
        return None;
    }
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        return None;
    }
    match wm.daemon_window_click(workspace_area, mouse.column, mouse.row) {
        DaemonWindowHit::Outside => None,
        _ => Some(AppAction::Handled),
    }
}
