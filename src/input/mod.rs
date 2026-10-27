pub mod encode;
pub mod keymap;
pub mod modal;
pub mod mouse;
mod mouse_wm;

pub use encode::{
    encode_alternate_scroll_wheel, encode_key_to_pty_bytes, encode_key_to_pty_bytes_full,
    encode_key_to_pty_bytes_with_modes,
};
pub use keymap::{
    DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS,
    default_global_action, default_leader_action, default_pane_action, is_leader_key,
    is_leader_key_with_tmux, normalize_macos_option_key,
};
pub use mouse::{handle_mouse_event, handle_wm_mouse_event};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::colon_command::{execute_colon_command, parse_colon_command};
use crate::error::Result;
use crate::input::keymap::{
    handle_pane_alt_or_fkey_shortcut, handle_wm_enter_submission, handle_wm_leader_key,
    handle_wm_shortcut_key, handle_wm_user_binding_or_leader_entry,
};
use crate::input::modal::{
    handle_copy_mode_key, handle_fuzzy_search_key, handle_hints_mode_key,
    handle_history_popup_key, handle_unsafe_paste_modal_key, handle_wm_modal_key,
};
use crate::pane::CompositePane;
use crate::pty::line_discipline::InputRoutingMode;
use crate::vt::{encode_osc52_clipboard, is_unsafe_paste, sanitize_paste_bytes};
use crate::wm::WindowManager;

/// High-level application actions returned by the input router.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppAction {
    /// Continue running the event loop.
    Continue,
    /// User requested clean application exit (`Ctrl+Q` or `:quit`).
    Quit,
    /// Bytes were dispatched to the active pane's PTY (or floating prompt state mutated).
    Handled,
}

/// Routes a `crossterm::event::KeyEvent` for the active `CompositePane` (`PTY-02..04`, `PANE-01..05`).
pub fn handle_key_event(pane: &mut CompositePane, raw_key: KeyEvent) -> Result<AppAction> {
    pane.refresh_line_discipline();
    let key = normalize_macos_option_key(raw_key, true);

    // Global emergency quit: Ctrl+Q
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        return Ok(AppAction::Quit);
    }

    // If an unsafe raw PTY paste is awaiting user confirmation (FND-01), route keys to the modal first
    if pane.pending_unsafe_paste().is_some() {
        return handle_unsafe_paste_modal_key(pane, key);
    }

    // If keyboard-driven Vi Copy Mode is active on the OutputCanvas (VAL-04), route keys to it first
    if pane.copy_mode().active {
        return handle_copy_mode_key(pane, key);
    }

    // If QuickSelect 3270 Field Hints mode is active on the OutputCanvas (ECO-01), route keys to it first
    if pane.hints_mode().active {
        return handle_hints_mode_key(pane, key);
    }

    // If fuzzy history search is active, route keys to fuzzy search modal first
    if pane.is_prompt_visible() && pane.prompt().fuzzy_search.active {
        return handle_fuzzy_search_key(pane, key);
    }

    // If the Borland Command History Pop-up Window is open, route keys to it first
    if pane.is_prompt_visible() && pane.prompt().history_popup_open {
        return handle_history_popup_key(pane, key);
    }

    // Global manual toggle between CommandPrompt and RawPtyPassthrough: Ctrl+G or F12
    if (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g'))
        || key.code == KeyCode::F(12)
    {
        pane.toggle_manual_raw_override();
        return Ok(AppAction::Handled);
    }

    // Global 3-way mode cycle (Floating Prompt -> Fixed Prompt -> Regular Terminal -> Floating Prompt):
    // Active in CommandPrompt mode, manual RegularTerminal mode, and false-positive !ECHO PTY wrappers (asciinema)
    // when not inside a DECSET 1049 ALT_SCREEN app like htop/vim or an active canvas password prompt.
    if !pane.vt().is_alternate_screen()
        && !pane.is_active_canvas_password_prompt()
        && (key.code == KeyCode::F(4)
            || (key.modifiers.contains(KeyModifiers::ALT)
                && matches!(key.code, KeyCode::Char('e') | KeyCode::Char('E'))))
    {
        pane.cycle_prompt_mode();
        return Ok(AppAction::Handled);
    }

    // When in RawPtyPassthrough (including DECSET 1049 alternate screen apps like vim/htop/emacs),
    // allow copying an active mouse canvas selection with Ctrl+C before passing keys to the child PTY.
    if pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
            && pane.copy_mode().has_selection()
        {
            let _ = pane.yank_copy_selection();
            return Ok(AppAction::Handled);
        }
        if pane.is_read_only() {
            pane.set_status_message(Some(
                "🔒 PROTECTED: Pane is locked read-only (:lock off or Ctrl+X l to unlock)"
                    .to_string(),
            ));
            return Ok(AppAction::Handled);
        }
        let bytes = encode_key_to_pty_bytes_full(
            raw_key,
            pane.vt().is_application_cursor(),
            pane.vt().is_backarrow_key_67(),
            pane.vt().kitty_keyboard_flags(),
        );
        if !bytes.is_empty() {
            pane.write_to_pty(&bytes)?;
        }
        return Ok(AppAction::Handled);
    }

    if let Some(action) = handle_pane_alt_or_fkey_shortcut(pane, key)? {
        return Ok(action);
    }

    handle_command_prompt_key(pane, key)
}

fn handle_command_prompt_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        let word_mod = key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Enter => {
                pane.prompt_mut().insert_newline();
                return Ok(AppAction::Handled);
            }
            KeyCode::PageUp => {
                pane.prompt_mut().scroll_up(2);
                return Ok(AppAction::Handled);
            }
            KeyCode::PageDown => {
                pane.prompt_mut().scroll_down(2);
                return Ok(AppAction::Handled);
            }
            KeyCode::Left => {
                if word_mod {
                    pane.prompt_mut().select_word_left();
                } else {
                    pane.prompt_mut().select_left();
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Right => {
                if word_mod {
                    pane.prompt_mut().select_word_right();
                } else {
                    pane.prompt_mut().select_right();
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Up => {
                pane.prompt_mut().select_up();
                return Ok(AppAction::Handled);
            }
            KeyCode::Down => {
                pane.prompt_mut().select_down();
                return Ok(AppAction::Handled);
            }
            KeyCode::Home => {
                pane.prompt_mut().select_home();
                return Ok(AppAction::Handled);
            }
            KeyCode::End => {
                pane.prompt_mut().select_end();
                return Ok(AppAction::Handled);
            }
            KeyCode::Delete => {
                if let Some(cut) = pane.prompt_mut().cut_selection_or_line() {
                    pane.set_last_osc52_sequence(Some(encode_osc52_clipboard(&cut)));
                    pane.set_status_message(Some(format!(
                        "Cut {} chars to clipboard",
                        cut.chars().count()
                    )));
                }
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Up => {
                pane.enter_copy_mode();
                return Ok(AppAction::Handled);
            }
            KeyCode::Left => {
                pane.prompt_mut().move_word_left();
                return Ok(AppAction::Handled);
            }
            KeyCode::Right => {
                pane.prompt_mut().move_word_right();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('r') => {
                pane.prompt_mut().open_fuzzy_search();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if pane.prompt().has_selection() {
                    if let Some(copied) = pane.prompt_mut().copy_selection() {
                        pane.set_last_osc52_sequence(Some(encode_osc52_clipboard(&copied)));
                        pane.prompt_mut().clear_selection();
                        pane.set_status_message(Some(format!(
                            "Copied {} chars to clipboard",
                            copied.chars().count()
                        )));
                    }
                    return Ok(AppAction::Handled);
                }
                if pane.copy_mode().has_selection() {
                    let _ = pane.yank_copy_selection();
                    return Ok(AppAction::Handled);
                }
                // If a foreground subcommand (e.g. `tail -f /dev/random`) is running or the prompt is empty,
                // forward SIGINT (0x03) to the PTY; also clear any partial prompt text and running banner.
                let has_fg_sub = pane.has_foreground_subcommand();
                let prompt_was_empty = pane.prompt().input.is_empty();
                pane.prompt_mut().running_command = None;
                if !prompt_was_empty {
                    pane.prompt_mut().clear_line();
                }
                if prompt_was_empty || has_fg_sub {
                    pane.write_to_pty(&[0x03])?;
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                let clip = crate::clipboard::get_system_clipboard()
                    .unwrap_or_else(|| pane.prompt().yank_buffer.clone());
                if !clip.is_empty() {
                    return handle_paste_event(pane, &clip);
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('u') => {
                pane.prompt_mut().clear_line();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('k') => {
                let had_sel = pane.prompt().has_selection();
                pane.prompt_mut().kill_to_end_of_line();
                if had_sel {
                    let y = pane.prompt().yank_buffer.clone();
                    pane.set_last_osc52_sequence(Some(encode_osc52_clipboard(&y)));
                    pane.set_status_message(Some(format!(
                        "Cut {} chars to clipboard",
                        y.chars().count()
                    )));
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('w') => {
                let had_sel = pane.prompt().has_selection();
                pane.prompt_mut().kill_prev_word();
                if had_sel {
                    let y = pane.prompt().yank_buffer.clone();
                    pane.set_last_osc52_sequence(Some(encode_osc52_clipboard(&y)));
                    pane.set_status_message(Some(format!(
                        "Cut {} chars to clipboard",
                        y.chars().count()
                    )));
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('y') => {
                pane.prompt_mut().yank();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('z') => {
                pane.prompt_mut().undo();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || (pane.prompt().cursor_row == 0
                        && pane.prompt().cursor_col == 0
                        && !pane.prompt().input.is_empty())
                {
                    pane.prompt_mut().select_all();
                    pane.set_status_message(Some("Selected all prompt text".to_string()));
                } else {
                    pane.prompt_mut().move_cursor_home();
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('e') => {
                pane.prompt_mut().move_cursor_end();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('d') => {
                if pane.prompt().input.is_empty() {
                    pane.write_to_pty(&[0x04])?;
                } else {
                    pane.prompt_mut().delete_char();
                }
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Enter => {
            let current_text = pane.prompt().text();
            // Check for in-prompt :command execution first (PANE-03, PANE-05)
            if let Some(cmd_res) = parse_colon_command(&current_text) {
                let _ = pane.prompt_mut().take_submission();
                match cmd_res {
                    Ok(cmd) => {
                        if execute_colon_command(pane, cmd) {
                            return Ok(AppAction::Quit);
                        }
                    }
                    Err(err_msg) => {
                        pane.set_status_message(Some(err_msg));
                    }
                }
                return Ok(AppAction::Handled);
            }

            if pane.is_read_only() {
                let _ = pane.submit_prompt();
                return Ok(AppAction::Handled);
            }

            pane.submit_prompt()?;
            Ok(AppAction::Handled)
        }
        KeyCode::Tab => {
            pane.complete_prompt_tab(key.modifiers.contains(KeyModifiers::SHIFT));
            Ok(AppAction::Handled)
        }
        KeyCode::BackTab => {
            pane.complete_prompt_tab(true);
            Ok(AppAction::Handled)
        }
        KeyCode::Esc => {
            pane.prompt_mut().completion.clear();
            pane.prompt_mut().clear_selection();
            pane.copy_mode_mut().clear_selection();
            Ok(AppAction::Handled)
        }
        KeyCode::Backspace => {
            pane.prompt_mut().backspace();
            Ok(AppAction::Handled)
        }
        KeyCode::Delete => {
            pane.prompt_mut().delete_char();
            Ok(AppAction::Handled)
        }
        KeyCode::Left => {
            pane.prompt_mut().move_cursor_left();
            Ok(AppAction::Handled)
        }
        KeyCode::Right => {
            pane.prompt_mut().move_cursor_right();
            Ok(AppAction::Handled)
        }
        KeyCode::Home => {
            pane.prompt_mut().move_cursor_home();
            Ok(AppAction::Handled)
        }
        KeyCode::End => {
            pane.prompt_mut().move_cursor_end();
            Ok(AppAction::Handled)
        }
        KeyCode::Up => {
            if !pane.prompt_mut().move_cursor_up() {
                pane.prompt_mut().history_prev();
            }
            Ok(AppAction::Handled)
        }
        KeyCode::Down => {
            if !pane.prompt_mut().move_cursor_down() {
                pane.prompt_mut().history_next();
            }
            Ok(AppAction::Handled)
        }
        KeyCode::PageUp => {
            pane.vt_mut().scroll_up(5);
            Ok(AppAction::Handled)
        }
        KeyCode::PageDown => {
            pane.vt_mut().scroll_down(5);
            Ok(AppAction::Handled)
        }
        KeyCode::Char(ch) => {
            pane.prompt_mut().insert_char(ch);
            Ok(AppAction::Handled)
        }
        _ => Ok(AppAction::Continue),
    }
}

/// Routes a `crossterm::event::Event::Paste` payload (`PTY-04`, `DECSET 2004`, `FND-01`).
///
/// - Always sanitizes dangerous C0/DEL control bytes via `sanitize_paste_bytes` (`FND-01`).
/// - In `CommandPrompt` mode: inserts sanitized text directly into `FloatingPromptState` without
///   auto-executing newlines so the user can inspect/edit pasted scripts before pressing `Enter`.
/// - In `RawPtyPassthrough` mode: checks `is_unsafe_paste` (unbracketed multi-line paste or embedded
///   `\x1b[201~` breakout) and stages `pending_unsafe_paste` for user confirmation when unsafe;
///   otherwise wraps in `\x1b[200~...\x1b[201~` if `DECSET 2004` is active or writes raw bytes.
pub fn handle_paste_event(pane: &mut CompositePane, pasted_text: &str) -> Result<AppAction> {
    pane.refresh_line_discipline();
    let sanitized = sanitize_paste_bytes(pasted_text);
    match pane.effective_routing_mode() {
        InputRoutingMode::CommandPrompt => {
            pane.prompt_mut().insert_str(&sanitized);
            Ok(AppAction::Handled)
        }
        InputRoutingMode::RawPtyPassthrough => {
            let bracketed = pane.vt().is_bracketed_paste();
            if is_unsafe_paste(pasted_text, bracketed) {
                pane.set_pending_unsafe_paste(Some(pasted_text.to_string()));
                pane.set_status_message(Some(
                    "UNSAFE MULTI-LINE PASTE DETECTED: Press [y/Enter] to confirm or [n/Esc] to cancel"
                        .to_string(),
                ));
                return Ok(AppAction::Handled);
            }
            if bracketed {
                let mut framed = Vec::with_capacity(sanitized.len() + 12);
                framed.extend_from_slice(b"\x1b[200~");
                framed.extend_from_slice(sanitized.as_bytes());
                framed.extend_from_slice(b"\x1b[201~");
                pane.write_to_pty(&framed)?;
            } else {
                pane.write_to_pty(sanitized.as_bytes())?;
            }
            Ok(AppAction::Handled)
        }
    }
}

/// Routes a `KeyEvent` across the multi-pane `WindowManager` (`WM-01..05`, `ARCH-R01.2`).
pub fn handle_wm_key_event(wm: &mut WindowManager, raw_key: KeyEvent) -> Result<AppAction> {
    let key = normalize_macos_option_key(raw_key, wm.config().macos_option_as_alt);

    // Global emergency quit: Ctrl+Q
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        return wm.apply(Action::Quit);
    }

    // Active WM modal overlays (Help, Inspector, Palette, Menu Bar)
    if let Some(action) = handle_wm_modal_key(wm, key)? {
        return Ok(action);
    }

    // If Prefix/Leader key (`Ctrl+B` / `Ctrl+X` by default) is awaiting the next chord, dispatch it
    if wm.is_leader_pending() {
        wm.set_leader_pending(false);
        wm.active_pane_mut().set_status_message(None);
        return handle_wm_leader_key(wm, key);
    }

    if let Some(action) = handle_wm_user_binding_or_leader_entry(wm, key)? {
        return Ok(action);
    }

    // Refresh active pane line discipline before deciding whether F1..F10 / Alt+<key> should pass through to Raw PTY
    wm.active_pane_mut().refresh_line_discipline();
    let in_raw_passthrough = wm.active_pane().effective_routing_mode()
        == InputRoutingMode::RawPtyPassthrough
        && wm.active_pane().pending_unsafe_paste().is_none()
        && !wm.active_pane().copy_mode().active
        && !wm.active_pane().hints_mode().active;

    if in_raw_passthrough {
        return handle_key_event(wm.active_pane_mut(), raw_key);
    }

    // Global WM shortcuts (Palette, Quake, F1..F10, Alt-chords)
    if let Some(action) = handle_wm_shortcut_key(wm, key)? {
        return Ok(action);
    }

    if let Some(action) = handle_wm_enter_submission(wm, key)? {
        return Ok(action);
    }

    handle_key_event(wm.active_pane_mut(), key)
}

#[cfg(test)]
mod tests;
