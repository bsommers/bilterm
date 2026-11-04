//! Key handling for the AGENT TRAFFIC window (`FLT-01`, Phase 50-08).
//!
//! This module only resolves which key scopes are active, looks the key up in the declarative
//! [`AGENTS_WINDOW_BINDINGS`](super::keymap::AGENTS_WINDOW_BINDINGS) table and runs the resulting
//! [`Action`](crate::action::Action) through `WindowManager::apply`. It never changes window, pane or
//! selection state itself (`ARCH-R01`).

use crossterm::event::KeyEvent;

use super::AppAction;
use super::keymap::{AgentsKeyScope, agents_window_action, is_leader_key_with_tmux};
use crate::wm::WindowManager;

/// Handles `key` while the agents window captures input.
///
/// Returns `None` for the leader key so the leader table stays reachable; every other key is
/// consumed (looked up, applied when it has a row, otherwise ignored) so keys never leak into the
/// pane below.
pub(super) fn handle_agents_window_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let inside_tmux = crate::tmux::TmuxEnv::detect().is_some();
    if is_leader_key_with_tmux(key, &wm.config().leader_key, inside_tmux) {
        return None;
    }
    let scopes = [AgentsKeyScope::Graph, AgentsKeyScope::Window];
    if let Some(action) = agents_window_action(&scopes, key) {
        let _ = wm.apply(action);
    }
    Some(AppAction::Handled)
}
