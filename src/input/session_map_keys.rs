//! Keyboard navigation and pickup/drop handling for `SESSION MAP` (Phase 54, VIEW-04).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::AppAction;
use crate::action::Action;
use crate::wm::WindowManager;
use crate::wm::session_map::SessionMapOp;

const fn row(mods: KeyModifiers, code: KeyCode, op: SessionMapOp) -> (KeyModifiers, KeyCode, Action) {
    (mods, code, Action::SessionMap(op))
}

const N: KeyModifiers = KeyModifiers::NONE;
const SH: KeyModifiers = KeyModifiers::SHIFT;
const AL: KeyModifiers = KeyModifiers::ALT;

/// Static declarative key binding rows for the focused session map.
pub const SESSION_MAP_BINDINGS: &[(KeyModifiers, KeyCode, Action)] = &[
    row(N, KeyCode::Up, SessionMapOp::StepWindow(-1)),
    row(N, KeyCode::Char('k'), SessionMapOp::StepWindow(-1)),
    row(N, KeyCode::Down, SessionMapOp::StepWindow(1)),
    row(N, KeyCode::Char('j'), SessionMapOp::StepWindow(1)),
    row(N, KeyCode::Left, SessionMapOp::StepSession(-1)),
    row(N, KeyCode::Char('h'), SessionMapOp::StepSession(-1)),
    row(N, KeyCode::Right, SessionMapOp::StepSession(1)),
    row(N, KeyCode::Char('l'), SessionMapOp::StepSession(1)),
    row(N, KeyCode::Tab, SessionMapOp::StepSession(1)),
    row(N, KeyCode::BackTab, SessionMapOp::StepSession(-1)),
    row(N, KeyCode::Char(' '), SessionMapOp::Pickup(None)),
    row(N, KeyCode::Char('m'), SessionMapOp::Pickup(None)),
    row(N, KeyCode::Char('*'), SessionMapOp::Pin(None)),
    row(N, KeyCode::Char('d'), SessionMapOp::Dock(None)),
    row(N, KeyCode::Char('['), SessionMapOp::ScaleStep(-10)),
    row(N, KeyCode::Char(']'), SessionMapOp::ScaleStep(10)),
    row(SH, KeyCode::Left, SessionMapOp::Move { dx: -1, dy: 0 }),
    row(SH, KeyCode::Right, SessionMapOp::Move { dx: 1, dy: 0 }),
    row(SH, KeyCode::Up, SessionMapOp::Move { dx: 0, dy: -1 }),
    row(SH, KeyCode::Down, SessionMapOp::Move { dx: 0, dy: 1 }),
    row(AL, KeyCode::Left, SessionMapOp::Resize { dw: -1, dh: 0 }),
    row(AL, KeyCode::Right, SessionMapOp::Resize { dw: 1, dh: 0 }),
    row(AL, KeyCode::Up, SessionMapOp::Resize { dw: 0, dh: -1 }),
    row(AL, KeyCode::Down, SessionMapOp::Resize { dw: 0, dh: 1 }),
];

/// Resolves an action from key event and holding/pinned state.
pub fn session_map_action(key: KeyEvent, holding: bool, pinned: bool) -> Option<Action> {
    if key.code == KeyCode::Esc {
        return if holding {
            Some(Action::SessionMap(SessionMapOp::CancelDrag))
        } else if pinned {
            Some(Action::SessionMap(SessionMapOp::Blur))
        } else {
            Some(Action::SessionMap(SessionMapOp::Dismiss))
        };
    }
    if key.code == KeyCode::Enter {
        return if holding {
            Some(Action::SessionMap(SessionMapOp::Drop))
        } else {
            Some(Action::SessionMap(SessionMapOp::Pickup(None)))
        };
    }

    let code = match key.code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        other => other,
    };
    let mods = if matches!(key.code, KeyCode::Char(_) | KeyCode::BackTab) {
        key.modifiers & !KeyModifiers::SHIFT
    } else {
        key.modifiers
    };

    SESSION_MAP_BINDINGS
        .iter()
        .find(|(m, c, _)| *m == mods && *c == code)
        .map(|(_, _, a)| a.clone())
}

/// Dispatches key event to session map if focused.
pub fn handle_session_map_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let holding = wm.session_map().holding_pane_id.is_some() || wm.session_map().drag.is_some();
    let pinned = wm.session_map().pinned;

    let action = session_map_action(key, holding, pinned)?;
    let _ = wm.apply(action);
    Some(AppAction::Handled)
}
