//! Keyboard input handling for the SCRIPT STUDIO window (Plan 51.2-03, WIN-03).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::AppAction;
use super::keymap::is_leader_key_with_tmux;
use crate::action::Action;
use crate::wm::WindowManager;
use crate::wm::script_studio::ScriptStudioOp;

fn act(op: ScriptStudioOp) -> Action {
    Action::ScriptStudio(op)
}

/// Handles keyboard input while Script Studio owns focus.
pub fn handle_script_studio_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let inside_tmux = crate::tmux::TmuxEnv::detect().is_some();
    if is_leader_key_with_tmux(key, &wm.config().leader_key, inside_tmux) {
        return None;
    }

    // Ctrl+Alt+S toggles Script Studio globally
    if key.modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::ALT)
        && (key.code == KeyCode::Char('s') || key.code == KeyCode::Char('S'))
    {
        let _ = wm.apply(act(ScriptStudioOp::Toggle));
        return Some(AppAction::Handled);
    }

    let edit_mode = wm.script_studio().edit_mode;
    if edit_mode {
        handle_edit_mode_key(wm, key);
    } else {
        handle_nav_mode_key(wm, key);
    }

    Some(AppAction::Handled)
}

fn handle_edit_mode_key(wm: &mut WindowManager, key: KeyEvent) {
    let mods = key.modifiers;
    match key.code {
        KeyCode::Esc => {
            let _ = wm.apply(act(ScriptStudioOp::EditMode(Some(false))));
        }
        KeyCode::Enter => {
            let _ = wm.apply(act(ScriptStudioOp::NewLine));
        }
        KeyCode::Backspace => {
            let _ = wm.apply(act(ScriptStudioOp::Backspace));
        }
        KeyCode::Left => {
            let _ = wm.apply(act(ScriptStudioOp::CursorLeft));
        }
        KeyCode::Right => {
            let _ = wm.apply(act(ScriptStudioOp::CursorRight));
        }
        KeyCode::Up => {
            let _ = wm.apply(act(ScriptStudioOp::CursorUp));
        }
        KeyCode::Down => {
            let _ = wm.apply(act(ScriptStudioOp::CursorDown));
        }
        KeyCode::F(5) => {
            let _ = wm.apply(act(ScriptStudioOp::RunAll));
        }
        KeyCode::F(10) => {
            let _ = wm.apply(act(ScriptStudioOp::StepNext));
        }
        KeyCode::Char(c) if !mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::ALT) => {
            let _ = wm.apply(act(ScriptStudioOp::InsertChar(c)));
        }
        _ => {}
    }
}

fn handle_nav_mode_key(wm: &mut WindowManager, key: KeyEvent) {
    let mods = key.modifiers;
    match (mods, key.code) {
        // Run & Step
        (KeyModifiers::NONE, KeyCode::F(5)) | (KeyModifiers::NONE, KeyCode::Enter) => {
            let _ = wm.apply(act(ScriptStudioOp::RunAll));
        }
        (KeyModifiers::NONE, KeyCode::F(10))
        | (KeyModifiers::NONE, KeyCode::Char('n'))
        | (KeyModifiers::NONE, KeyCode::Char(' ')) => {
            let _ = wm.apply(act(ScriptStudioOp::StepNext));
        }
        (KeyModifiers::NONE, KeyCode::Char('r')) => {
            let _ = wm.apply(act(ScriptStudioOp::ResetStep));
        }
        (KeyModifiers::SHIFT, KeyCode::Char('R')) | (KeyModifiers::NONE, KeyCode::Char('R')) => {
            let _ = wm.apply(act(ScriptStudioOp::Record(None)));
        }
        (KeyModifiers::NONE, KeyCode::Char('s')) => {
            let _ = wm.apply(act(ScriptStudioOp::Save(None)));
        }
        // Mode switching & editing
        (KeyModifiers::NONE, KeyCode::Char('i')) | (KeyModifiers::NONE, KeyCode::Char('e')) => {
            let _ = wm.apply(act(ScriptStudioOp::EditMode(Some(true))));
        }
        (KeyModifiers::NONE, KeyCode::Char('o')) => {
            let _ = wm.apply(act(ScriptStudioOp::NewLine));
            let _ = wm.apply(act(ScriptStudioOp::EditMode(Some(true))));
        }
        (KeyModifiers::NONE, KeyCode::Char('x')) | (KeyModifiers::NONE, KeyCode::Delete) => {
            let _ = wm.apply(act(ScriptStudioOp::DeleteLine));
        }
        // Navigation
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            let _ = wm.apply(act(ScriptStudioOp::CursorUp));
        }
        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            let _ = wm.apply(act(ScriptStudioOp::CursorDown));
        }
        (KeyModifiers::NONE, KeyCode::Left) | (KeyModifiers::NONE, KeyCode::Char('h')) => {
            let _ = wm.apply(act(ScriptStudioOp::CursorLeft));
        }
        (KeyModifiers::NONE, KeyCode::Right) | (KeyModifiers::NONE, KeyCode::Char('l')) => {
            let _ = wm.apply(act(ScriptStudioOp::CursorRight));
        }
        // Window options
        (KeyModifiers::NONE, KeyCode::Char('*')) | (KeyModifiers::NONE, KeyCode::Char('p')) => {
            let _ = wm.apply(act(ScriptStudioOp::Pin(None)));
        }
        (KeyModifiers::NONE, KeyCode::Char('d')) => {
            let _ = wm.apply(act(ScriptStudioOp::Dock(None)));
        }
        // Moving (Shift + Arrows)
        (KeyModifiers::SHIFT, KeyCode::Left) => {
            let _ = wm.apply(act(ScriptStudioOp::Move { dx: -1, dy: 0 }));
        }
        (KeyModifiers::SHIFT, KeyCode::Right) => {
            let _ = wm.apply(act(ScriptStudioOp::Move { dx: 1, dy: 0 }));
        }
        (KeyModifiers::SHIFT, KeyCode::Up) => {
            let _ = wm.apply(act(ScriptStudioOp::Move { dx: 0, dy: -1 }));
        }
        (KeyModifiers::SHIFT, KeyCode::Down) => {
            let _ = wm.apply(act(ScriptStudioOp::Move { dx: 0, dy: 1 }));
        }
        // Resizing (Alt + Arrows)
        (KeyModifiers::ALT, KeyCode::Left) => {
            let _ = wm.apply(act(ScriptStudioOp::Resize { dw: -1, dh: 0 }));
        }
        (KeyModifiers::ALT, KeyCode::Right) => {
            let _ = wm.apply(act(ScriptStudioOp::Resize { dw: 1, dh: 0 }));
        }
        (KeyModifiers::ALT, KeyCode::Up) => {
            let _ = wm.apply(act(ScriptStudioOp::Resize { dw: 0, dh: -1 }));
        }
        (KeyModifiers::ALT, KeyCode::Down) => {
            let _ = wm.apply(act(ScriptStudioOp::Resize { dw: 0, dh: 1 }));
        }
        // Dismiss
        (KeyModifiers::NONE, KeyCode::Esc) | (KeyModifiers::NONE, KeyCode::Char('q')) => {
            let _ = wm.apply(act(ScriptStudioOp::Dismiss));
        }
        _ => {}
    }
}
