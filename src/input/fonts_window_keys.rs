//! Key handling for the FONTS utility window (`WIN-01`, Phase 51.2).
//!
//! While the FONTS window captures input ([`FontsWindowState::captures_input`](crate::wm::fonts_window::FontsWindowState::captures_input))
//! keystrokes are looked up in declarative key tables and dispatched to `Action::FontsWindow(..)`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::AppAction;
use super::keymap::is_leader_key_with_tmux;
use crate::action::Action;
use crate::wm::WindowManager;
use crate::wm::fonts_window::FontsWindowOp;

const fn row(
    mods: KeyModifiers,
    code: KeyCode,
    op: FontsWindowOp,
) -> (KeyModifiers, KeyCode, Action) {
    (mods, code, Action::FontsWindow(op))
}

const N: KeyModifiers = KeyModifiers::NONE;
const SH: KeyModifiers = KeyModifiers::SHIFT;
const AL: KeyModifiers = KeyModifiers::ALT;

/// Declarative key bindings for the focused FONTS utility window.
pub const FONTS_WINDOW_BINDINGS: &[(KeyModifiers, KeyCode, Action)] = &[
    row(N, KeyCode::Up, FontsWindowOp::Step(-1)),
    row(N, KeyCode::Char('k'), FontsWindowOp::Step(-1)),
    row(N, KeyCode::Down, FontsWindowOp::Step(1)),
    row(N, KeyCode::Char('j'), FontsWindowOp::Step(1)),
    row(N, KeyCode::PageUp, FontsWindowOp::Page(-1)),
    row(N, KeyCode::PageDown, FontsWindowOp::Page(1)),
    row(N, KeyCode::Home, FontsWindowOp::First),
    row(N, KeyCode::End, FontsWindowOp::Last),
    row(N, KeyCode::Enter, FontsWindowOp::ApplySelected),
    row(N, KeyCode::Char('a'), FontsWindowOp::ApplyAll),
    row(N, KeyCode::Char('+'), FontsWindowOp::SizeStep(1)),
    row(N, KeyCode::Char('='), FontsWindowOp::SizeStep(1)),
    row(N, KeyCode::Char('-'), FontsWindowOp::SizeStep(-1)),
    row(N, KeyCode::Char(']'), FontsWindowOp::ScaleStep(10)),
    row(N, KeyCode::Char('['), FontsWindowOp::ScaleStep(-10)),
    row(N, KeyCode::Char('s'), FontsWindowOp::Scan),
    row(N, KeyCode::Char('*'), FontsWindowOp::Pin(None)),
    row(N, KeyCode::Char('p'), FontsWindowOp::Pin(None)),
    row(N, KeyCode::Char('d'), FontsWindowOp::Dock(None)),
    row(SH, KeyCode::Left, FontsWindowOp::Move { dx: -1, dy: 0 }),
    row(SH, KeyCode::Right, FontsWindowOp::Move { dx: 1, dy: 0 }),
    row(SH, KeyCode::Up, FontsWindowOp::Move { dx: 0, dy: -1 }),
    row(SH, KeyCode::Down, FontsWindowOp::Move { dx: 0, dy: 1 }),
    row(AL, KeyCode::Left, FontsWindowOp::Resize { dw: -1, dh: 0 }),
    row(AL, KeyCode::Right, FontsWindowOp::Resize { dw: 1, dh: 0 }),
    row(AL, KeyCode::Up, FontsWindowOp::Resize { dw: 0, dh: -1 }),
    row(AL, KeyCode::Down, FontsWindowOp::Resize { dw: 0, dh: 1 }),
    row(N, KeyCode::Esc, FontsWindowOp::Dismiss),
    row(N, KeyCode::Char('q'), FontsWindowOp::Dismiss),
];

/// Looks up `key` in [`FONTS_WINDOW_BINDINGS`].
pub fn fonts_window_action(key: KeyEvent) -> Option<Action> {
    let lookup_mods = match key.code {
        KeyCode::Char(_) | KeyCode::BackTab => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    FONTS_WINDOW_BINDINGS
        .iter()
        .find(|(mods, code, _)| *mods == lookup_mods && *code == key.code)
        .map(|(_, _, action)| action.clone())
}

/// Handles `key` while the FONTS window captures input.
pub(super) fn handle_fonts_window_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let inside_tmux = crate::tmux::TmuxEnv::detect().is_some();
    if is_leader_key_with_tmux(key, &wm.config().leader_key, inside_tmux) {
        return None;
    }
    if let Some(action) = fonts_window_action(key) {
        let _ = wm.apply(action);
    }
    Some(AppAction::Handled)
}
