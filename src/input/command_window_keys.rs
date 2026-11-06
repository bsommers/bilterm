//! Key handling for the Command Palette window (WIN-02, Phase 51.2-02).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::AppAction;
use crate::action::Action;
use crate::wm::WindowManager;
use crate::wm::command_window::CommandWindowOp;

/// Pure key-to-Action translation for the command window.
pub fn command_window_action(key: KeyEvent) -> Option<Action> {
    let act = |op| Some(Action::CommandWindow(op));
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('u') | KeyCode::Char('U') => return act(CommandWindowOp::QueryClear),
            KeyCode::Char('l') | KeyCode::Char('L') => return act(CommandWindowOp::ClearLog),
            _ => {}
        }
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Char('p') | KeyCode::Char('P') => return act(CommandWindowOp::Pin(None)),
            KeyCode::Char('d') | KeyCode::Char('D') => return act(CommandWindowOp::Dock(None)),
            KeyCode::Left => return act(CommandWindowOp::Resize { dw: -1, dh: 0 }),
            KeyCode::Right => return act(CommandWindowOp::Resize { dw: 1, dh: 0 }),
            KeyCode::Up => return act(CommandWindowOp::Resize { dw: 0, dh: -1 }),
            KeyCode::Down => return act(CommandWindowOp::Resize { dw: 0, dh: 1 }),
            _ => {}
        }
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Left => return act(CommandWindowOp::Move { dx: -1, dy: 0 }),
            KeyCode::Right => return act(CommandWindowOp::Move { dx: 1, dy: 0 }),
            KeyCode::Up => return act(CommandWindowOp::Move { dx: 0, dy: -1 }),
            KeyCode::Down => return act(CommandWindowOp::Move { dx: 0, dy: 1 }),
            _ => {}
        }
    }
    match key.code {
        KeyCode::Up => act(CommandWindowOp::Step(-1)),
        KeyCode::Down => act(CommandWindowOp::Step(1)),
        KeyCode::PageUp => act(CommandWindowOp::Page(-1)),
        KeyCode::PageDown => act(CommandWindowOp::Page(1)),
        KeyCode::Home => act(CommandWindowOp::First),
        KeyCode::End => act(CommandWindowOp::Last),
        KeyCode::Enter => act(CommandWindowOp::ExecuteSelected),
        KeyCode::Backspace => act(CommandWindowOp::QueryBackspace),
        KeyCode::Esc => act(CommandWindowOp::Dismiss),
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            act(CommandWindowOp::QueryChar(c))
        }
        _ => None,
    }
}

/// Applies `key` to `wm` when the command window captures input.
pub(super) fn handle_command_window_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    if let Some(action) = command_window_action(key) {
        match wm.apply(action) {
            Ok(app_action) => Some(app_action),
            Err(_) => Some(AppAction::Handled),
        }
    } else {
        None
    }
}
