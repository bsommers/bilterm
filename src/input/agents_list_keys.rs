//! Key handling for the AGENTS list window (Phase 51-07, UIT-03, `51-CONTRACT.md` section 4).
//!
//! While the list is focused ([`AgentsListState::captures_input`](crate::wm::agents_list::AgentsListState::captures_input))
//! keys are looked up in the declarative [`AGENTS_LIST_BINDINGS`] table and lowered to an
//! `Action::AgentsList(..)` run through `WindowManager::apply`. The window is non-modal: the
//! leader key is never consumed, the global toggle chord is handled first, and when the window is
//! not focused the caller never invokes this handler, so typing reaches the pane (UIT-03).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::AppAction;
use super::keymap::{default_global_action, is_leader_key_with_tmux};
use crate::action::Action;
use crate::wm::WindowManager;
use crate::wm::agents_list_cmd::AgentsListOp;

const fn row(mods: KeyModifiers, code: KeyCode, op: AgentsListOp) -> (KeyModifiers, KeyCode, Action) {
    (mods, code, Action::AgentsList(op))
}

const N: KeyModifiers = KeyModifiers::NONE;
const SH: KeyModifiers = KeyModifiers::SHIFT;
const AL: KeyModifiers = KeyModifiers::ALT;

/// Declarative in-window key table (`ARCH-R01.2`); every row has a colon twin (`op.colon_form()`).
///
/// The global toggle chord (`Ctrl+Alt+N`, `DEFAULT_GLOBAL_BINDINGS`) is handled before this table
/// and is therefore not a row here. Each row lowers to `Action::AgentsList`.
pub const AGENTS_LIST_BINDINGS: &[(KeyModifiers, KeyCode, Action)] = &[
    row(N, KeyCode::Up, AgentsListOp::Step(-1)),
    row(N, KeyCode::Char('k'), AgentsListOp::Step(-1)),
    row(N, KeyCode::Down, AgentsListOp::Step(1)),
    row(N, KeyCode::Char('j'), AgentsListOp::Step(1)),
    row(N, KeyCode::PageUp, AgentsListOp::Page(-1)),
    row(N, KeyCode::PageDown, AgentsListOp::Page(1)),
    row(N, KeyCode::Home, AgentsListOp::First),
    row(N, KeyCode::End, AgentsListOp::Last),
    row(N, KeyCode::Enter, AgentsListOp::Jump(None)),
    row(N, KeyCode::Char('z'), AgentsListOp::Zoom(None)),
    row(N, KeyCode::Char('f'), AgentsListOp::Filter(None)),
    row(N, KeyCode::Char('*'), AgentsListOp::Pin(None)),
    row(N, KeyCode::Char('['), AgentsListOp::ScaleStep(-10)),
    row(N, KeyCode::Char(']'), AgentsListOp::ScaleStep(10)),
    row(SH, KeyCode::Left, AgentsListOp::Move { dx: -1, dy: 0 }),
    row(SH, KeyCode::Right, AgentsListOp::Move { dx: 1, dy: 0 }),
    row(SH, KeyCode::Up, AgentsListOp::Move { dx: 0, dy: -1 }),
    row(SH, KeyCode::Down, AgentsListOp::Move { dx: 0, dy: 1 }),
    row(AL, KeyCode::Left, AgentsListOp::Resize { dw: -1, dh: 0 }),
    row(AL, KeyCode::Right, AgentsListOp::Resize { dw: 1, dh: 0 }),
    row(AL, KeyCode::Up, AgentsListOp::Resize { dw: 0, dh: -1 }),
    row(AL, KeyCode::Down, AgentsListOp::Resize { dw: 0, dh: 1 }),
    row(N, KeyCode::Char('d'), AgentsListOp::Dock(None)),
    // Esc hands the keyboard back to the pane and leaves the list open; `q` closes it (FOCUS-01).
    row(N, KeyCode::Esc, AgentsListOp::Blur),
    row(N, KeyCode::Char('q'), AgentsListOp::Close),
];

/// Looks up `key` in [`AGENTS_LIST_BINDINGS`].
///
/// SHIFT is removed from the modifiers for `KeyCode::Char` and `KeyCode::BackTab` (as for
/// `agents_window_action`); every other code is compared with exact modifiers.
pub fn agents_list_action(key: KeyEvent) -> Option<Action> {
    let lookup_mods = match key.code {
        KeyCode::Char(_) | KeyCode::BackTab => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    AGENTS_LIST_BINDINGS
        .iter()
        .find(|(mods, code, _)| *mods == lookup_mods && *code == key.code)
        .map(|(_, _, action)| action.clone())
}

/// Handles `key` while the list captures input.
///
/// Returns `None` for the leader key so the leader table stays reachable; the global toggle chord
/// closes the window again; every other key is consumed (applied when it has a row) so keys never
/// leak into the pane below.
pub(super) fn handle_agents_list_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let inside_tmux = crate::tmux::TmuxEnv::detect().is_some();
    if is_leader_key_with_tmux(key, &wm.config().leader_key, inside_tmux) {
        return None;
    }
    if let Some(toggle @ Action::AgentsList(AgentsListOp::Toggle)) = default_global_action(key, false) {
        let _ = wm.apply(toggle);
        return Some(AppAction::Handled);
    }
    if let Some(action) = agents_list_action(key) {
        let _ = wm.apply(action);
    }
    Some(AppAction::Handled)
}
