//! Keyboard-ownership routing (`FOCUS-01`): the focus-cycle chord, the dispatch of a key to the
//! one side window that owns the keyboard, and the pointer-event wrapper that keeps the
//! single-owner invariant. The owner comes from [`WindowManager::keyboard_owner`].

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;

use super::AppAction;
use crate::action::Action;
use crate::error::Result;
use crate::wm::WindowManager;
use crate::wm::keyboard_focus::KeyboardOwner;

/// The focus-cycle chord (`Ctrl+Alt+O` by default) is answered before any side window sees the
/// key and also over raw PTY passthrough, so it always moves the keyboard on, whichever window
/// holds it. With no side window open and the pane as owner there is nothing to cycle: the chord
/// is left to the normal routing (a full-screen program in the pane receives it).
pub(super) fn handle_focus_chord(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    if wm.is_leader_pending() {
        return None;
    }
    let action = match wm.keymap().global_action(key, false) {
        Some(action @ Action::KeyboardFocus(_)) => action,
        _ => return None,
    };
    if wm.keyboard_owner().is_pane() && wm.keyboard_focus_ring().is_empty() {
        return None;
    }
    Some(wm.apply(action).unwrap_or(AppAction::Handled))
}

/// Offers `key` to the side window that owns the keyboard. `None` when the pane, the drawer or the
/// MCP console owns it (their handlers follow in `handle_wm_modal_key`) or the window declines the
/// key (the leader key, for instance).
pub(super) fn dispatch_side_window_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    if wm.is_leader_pending() {
        return None;
    }
    match wm.keyboard_owner() {
        KeyboardOwner::AgentsHud => super::agents_hud::handle_agents_hud_key(wm, key),
        KeyboardOwner::CommandWindow => super::command_window_keys::handle_command_window_key(wm, key),
        KeyboardOwner::Clipboard => super::clipboard_window_keys::handle_clipboard_window_key(wm, key),
        KeyboardOwner::TrafficMonitor => super::traffic_monitor_keys::handle_traffic_monitor_key(wm, key),
        KeyboardOwner::Fonts => super::fonts_window_keys::handle_fonts_window_key(wm, key),
        KeyboardOwner::Daemon => wm.daemon_window_key(key).then_some(AppAction::Handled),
        KeyboardOwner::ScriptStudio => super::script_studio_keys::handle_script_studio_key(wm, key),
        KeyboardOwner::AgentsList => super::agents_list_keys::handle_agents_list_key(wm, key),
        KeyboardOwner::SessionMap => super::session_map_keys::handle_session_map_key(wm, key),
        KeyboardOwner::AgentsWindow => super::agents_window::handle_agents_window_key(wm, key),
        KeyboardOwner::Pane | KeyboardOwner::Drawer | KeyboardOwner::McpConsole => None,
    }
}

/// Routes one pointer event through the window manager, then settles keyboard ownership: a window
/// focused by the press becomes the only owner.
pub fn handle_wm_mouse_event(wm: &mut WindowManager, mouse: MouseEvent, screen_area: Rect) -> Result<AppAction> {
    let result = super::mouse::route_wm_mouse_event(wm, mouse, screen_area);
    wm.settle_keyboard_owner();
    result
}
