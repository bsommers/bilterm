//! Key handling for the focused navigator drawer (`FLT-01`, Phase 50-06).
//!
//! This module only resolves which key scopes are active, looks the key up in the declarative
//! [`NAVIGATOR_DRAWER_BINDINGS`](super::keymap::NAVIGATOR_DRAWER_BINDINGS) table and runs the resulting
//! [`Action`](crate::action::Action) through `WindowManager::apply`. It never changes drawer, pane or
//! prompt state itself (`ARCH-R01`).

use crossterm::event::KeyEvent;

use super::AppAction;
use super::keymap::{DrawerKeyScope, navigator_drawer_action};
use crate::wm::WindowManager;

/// Looks `key` up in the navigator key table; `None` lets the classic drawer handler run.
pub(super) fn handle_navigator_drawer_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let scopes = [DrawerKeyScope::Navigator];
    let action = navigator_drawer_action(&scopes, key)?;
    let _ = wm.apply(action);
    Some(AppAction::Handled)
}
