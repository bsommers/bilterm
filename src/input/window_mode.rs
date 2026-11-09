//! Sticky Window mode key router (`KEY-01`, Phase 63-02).
//!
//! While [`KeyMode::Window`](crate::wm::KeyMode) is active every key lands here (it is wired in
//! `modal.rs::handle_wm_modal_key`, before raw PTY passthrough is even considered). The key is looked
//! up in [`WINDOW_MODE_BINDINGS`](super::window_mode_tables::WINDOW_MODE_BINDINGS) (plus the user's
//! `[keys.window_mode]` additions), lowered to an [`Action`] and run through `WindowManager::apply`
//! (`ARCH-R01`). Unknown keys are swallowed with a status hint: nothing reaches the child PTY.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::AppAction;
use super::chord::KeyChord;
use super::window_mode_tables::{WHICH_KEY_PIN, WINDOW_MODE_BINDINGS, WinScope, is_mode_control};
use crate::action::Action;
use crate::colon_command::parse_colon_command;
use crate::error::Result;
use crate::wm::{Modal, WindowManager};

/// Looks `key` up for `scope`. Modifiers are compared with SHIFT removed for characters and
/// `BackTab` (terminals report `Shift+j` as `J` and `Shift+Tab` as `BackTab`) and exactly otherwise,
/// so `Ctrl+h` or `Alt+x` never match a plain row.
pub fn window_mode_action(scope: WinScope, key: KeyEvent) -> Option<Action> {
    let mods = match key.code {
        KeyCode::Char(_) | KeyCode::BackTab => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    let find = |want: WinScope| {
        WINDOW_MODE_BINDINGS
            .iter()
            .find(|(s, m, code, a)| {
                *m == mods && *code == key.code && (*s == want || (*s == WinScope::Focus && is_mode_control(a)))
            })
            .map(|(_, _, _, a)| a.clone())
    };
    find(scope)
}

/// Action bound to `key` by the user's `[keys.window_mode]` table, if any.
fn user_window_action(wm: &WindowManager, key: KeyEvent) -> Option<Action> {
    let map = &wm.config().keys.window_mode;
    if map.is_empty() {
        return None;
    }
    let env = wm.key_env();
    let pressed = KeyChord::from_event(key, &env).folded();
    map.iter().find_map(|(chord, command)| {
        let bound = KeyChord::parse(chord).ok()?.folded();
        if bound == pressed { parse_colon_command(command)?.ok() } else { None }
    })
}

/// Actions that hand the keyboard to something else, so Window mode ends first.
fn hands_control_away(action: &Action) -> bool {
    matches!(
        action,
        Action::CopyModeToggle
            | Action::PaletteToggle
            | Action::Find(_)
            | Action::DrawerFocus
            | Action::AgentsList(_)
    )
}

/// `true` when holding `key` in Window mode should keep repeating (movement, resize, move steps).
pub(super) fn is_repeatable_window_key(wm: &WindowManager, key: KeyEvent) -> bool {
    let Some(sub) = wm.key_mode().window_sub() else { return false };
    matches!(
        window_mode_action(WinScope::of(sub), key),
        Some(Action::PaneFocusDirection { .. } | Action::PaneResize { .. } | Action::PaneMove { .. })
    )
}

/// Handles one key while Window mode is active. Always consumes the key.
pub(super) fn handle_window_mode_key(wm: &mut WindowManager, key: KeyEvent) -> Result<Option<AppAction>> {
    let Some(sub) = wm.key_mode().window_sub() else { return Ok(None) };
    wm.touch_which_key();
    let plain = key.modifiers.difference(KeyModifiers::SHIFT).is_empty();
    if plain && key.code == KeyCode::Char(WHICH_KEY_PIN) {
        wm.pin_which_key();
        return Ok(Some(AppAction::Handled));
    }
    let Some(action) = user_window_action(wm, key).or_else(|| window_mode_action(WinScope::of(sub), key)) else {
        wm.active_pane_mut().set_status_message(Some(format!("Window mode: unknown key, {WHICH_KEY_PIN} for help")));
        return Ok(Some(AppAction::Handled));
    };
    if hands_control_away(&action) {
        wm.exit_window_mode();
    }
    let result = wm.apply(action)?;
    if wm.key_mode().is_window() {
        if matches!(wm.modal(), Modal::None) {
            let line = crate::ui::which_key_ui::footer_line(&wm.key_mode(), &wm.keymap(), &wm.config().leader_key);
            wm.active_pane_mut().set_status_message(Some(line));
        } else {
            wm.exit_window_mode();
        }
    }
    Ok(Some(result))
}
