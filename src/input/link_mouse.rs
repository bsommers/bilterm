//! Mouse handling for the tmux link banner (Phase 51.1, plan 10 fills the body).
//!
//! A left click on an interactive banner lowers to `Action::RemoteLink(Reconnect)` (uniformity
//! exception E4). Seam state (plan 01): never claims an event.

use crossterm::event::MouseEvent;
use ratatui::layout::Rect;

use super::AppAction;
use crate::wm::WindowManager;

/// Returns `Some` when `mouse` hit an interactive link banner and was handled.
pub fn handle_link_banner_click(
    _wm: &mut WindowManager,
    _mouse: MouseEvent,
    _workspace_area: Rect,
) -> Option<AppAction> {
    None
}
