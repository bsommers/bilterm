//! Key handling for the `CLIPBOARD` viewer window (`CLIP-03`, `CLIP-04`, Phase 57).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::AppAction;
use crate::action::Action;
use crate::wm::WindowManager;
use crate::wm::clipboard_registry::ClipboardFilterScope;
use crate::wm::clipboard_window::ClipboardWindowOp;

/// Pure key-to-Action translation for the `CLIPBOARD` window when it captures input.
pub fn clipboard_window_action(key: KeyEvent, search_focused: bool) -> Option<Action> {
    let act = |op| Some(Action::ClipboardWindow(op));

    if key.modifiers.contains(KeyModifiers::CONTROL)
        && key.modifiers.contains(KeyModifiers::ALT)
        && matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V'))
    {
        return act(ClipboardWindowOp::Toggle);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('u') | KeyCode::Char('U') => return act(ClipboardWindowOp::SearchClear),
            KeyCode::Char('l') | KeyCode::Char('L') => return act(ClipboardWindowOp::ClearUnpinned),
            _ => {}
        }
    }

    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Char('p') | KeyCode::Char('P') => return act(ClipboardWindowOp::Pin(None)),
            KeyCode::Char('d') | KeyCode::Char('D') => return act(ClipboardWindowOp::Dock(None)),
            KeyCode::Left => return act(ClipboardWindowOp::Resize { dw: -2, dh: 0 }),
            KeyCode::Right => return act(ClipboardWindowOp::Resize { dw: 2, dh: 0 }),
            KeyCode::Up => return act(ClipboardWindowOp::Resize { dw: 0, dh: -1 }),
            KeyCode::Down => return act(ClipboardWindowOp::Resize { dw: 0, dh: 1 }),
            _ => {}
        }
    }

    if key.modifiers.contains(KeyModifiers::SHIFT)
        && matches!(
            key.code,
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
        )
    {
        return match key.code {
            KeyCode::Left => act(ClipboardWindowOp::Move { dx: -2, dy: 0 }),
            KeyCode::Right => act(ClipboardWindowOp::Move { dx: 2, dy: 0 }),
            KeyCode::Up => act(ClipboardWindowOp::Move { dx: 0, dy: -1 }),
            KeyCode::Down => act(ClipboardWindowOp::Move { dx: 0, dy: 1 }),
            _ => None,
        };
    }

    if search_focused {
        return match key.code {
            KeyCode::Esc => act(ClipboardWindowOp::SearchFocus(false)),
            KeyCode::Enter => act(ClipboardWindowOp::SearchFocus(false)),
            KeyCode::Backspace => act(ClipboardWindowOp::SearchBackspace),
            KeyCode::Up => act(ClipboardWindowOp::Step(-1)),
            KeyCode::Down => act(ClipboardWindowOp::Step(1)),
            KeyCode::Tab => act(ClipboardWindowOp::CycleScope(1)),
            KeyCode::BackTab => act(ClipboardWindowOp::CycleScope(-1)),
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                act(ClipboardWindowOp::SearchChar(c))
            }
            _ => None,
        };
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => act(ClipboardWindowOp::Step(-1)),
        KeyCode::Down | KeyCode::Char('j') => act(ClipboardWindowOp::Step(1)),
        KeyCode::PageUp => act(ClipboardWindowOp::Page(-1)),
        KeyCode::PageDown => act(ClipboardWindowOp::Page(1)),
        KeyCode::Home | KeyCode::Char('g') => act(ClipboardWindowOp::First),
        KeyCode::End | KeyCode::Char('G') => act(ClipboardWindowOp::Last),
        KeyCode::Tab => act(ClipboardWindowOp::CycleScope(1)),
        KeyCode::BackTab => act(ClipboardWindowOp::CycleScope(-1)),
        KeyCode::Char('1') => act(ClipboardWindowOp::SetScope(ClipboardFilterScope::All)),
        KeyCode::Char('2') => act(ClipboardWindowOp::SetScope(ClipboardFilterScope::Session)),
        KeyCode::Char('3') => act(ClipboardWindowOp::SetScope(ClipboardFilterScope::Window)),
        KeyCode::Char('4') => act(ClipboardWindowOp::SetScope(ClipboardFilterScope::Pinned)),
        KeyCode::Char('5') => act(ClipboardWindowOp::SetScope(ClipboardFilterScope::Media)),
        KeyCode::Char('s') => act(ClipboardWindowOp::CycleSessionFilter),
        KeyCode::Char('w') => act(ClipboardWindowOp::CycleWindowFilter),
        KeyCode::Char('/') => act(ClipboardWindowOp::SearchFocus(true)),
        KeyCode::Enter | KeyCode::Char('p') => act(ClipboardWindowOp::PasteSelected),
        KeyCode::Char('y') => act(ClipboardWindowOp::YankSelected),
        KeyCode::Char('*') | KeyCode::Char('P') => act(ClipboardWindowOp::TogglePinSelected),
        KeyCode::Char('x') | KeyCode::Delete => act(ClipboardWindowOp::DeleteSelected),
        KeyCode::Char('c') | KeyCode::Char('C') => act(ClipboardWindowOp::ClearUnpinned),
        KeyCode::Char('d') => act(ClipboardWindowOp::Dock(None)),
        KeyCode::Char('[') => act(ClipboardWindowOp::Scale(-10)),
        KeyCode::Char(']') => act(ClipboardWindowOp::Scale(10)),
        KeyCode::Backspace => act(ClipboardWindowOp::SearchBackspace),
        KeyCode::Esc => act(ClipboardWindowOp::Blur),
        KeyCode::Char('q') => act(ClipboardWindowOp::Dismiss),
        _ => None,
    }
}

/// Applies `key` to `wm` when the `CLIPBOARD` window captures input.
pub(super) fn handle_clipboard_window_key(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Option<AppAction> {
    let search_focused = wm.clipboard_window().search_focused;
    if let Some(action) = clipboard_window_action(key, search_focused) {
        match wm.apply(action) {
            Ok(app_action) => Some(app_action),
            Err(_) => Some(AppAction::Handled),
        }
    } else {
        None
    }
}
