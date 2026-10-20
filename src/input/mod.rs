use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::error::Result;
use crate::pane::{CompositePane, PromptAnchor};
use crate::pty::line_discipline::InputRoutingMode;

/// High-level application actions returned by the input router.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppAction {
    /// Continue running the event loop.
    Continue,
    /// User requested clean application exit (`Ctrl+Q`).
    Quit,
    /// Bytes were dispatched to the active pane's PTY (or floating prompt state mutated).
    Handled,
}

/// Routes a `crossterm::event::KeyEvent` for the active `CompositePane` (`PTY-02`, `PTY-03`, `PANE-01`).
///
/// Always refreshes the three-signal line-discipline state before deciding whether to route
/// keystrokes to `FloatingPromptState` or encode them directly into the child PTY master stream.
pub fn handle_key_event(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    pane.refresh_line_discipline();

    // Global emergency quit: Ctrl+Q
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        return Ok(AppAction::Quit);
    }

    // Global manual toggle between CommandPrompt and RawPtyPassthrough: Ctrl+G or F12
    if (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g'))
        || key.code == KeyCode::F(12)
    {
        pane.toggle_manual_raw_override();
        return Ok(AppAction::Handled);
    }

    // Intra-pane prompt positioning hotkeys (Alt+T = Top, Alt+B = Bottom, Alt+Up/Down = Float move)
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Char('t') => {
                pane.set_prompt_anchor(PromptAnchor::Top);
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('b') => {
                pane.set_prompt_anchor(PromptAnchor::Bottom);
                return Ok(AppAction::Handled);
            }
            KeyCode::Up => {
                let h = pane.vt.size().0 + 1;
                pane.move_prompt_vertical(-1, h);
                return Ok(AppAction::Handled);
            }
            KeyCode::Down => {
                let h = pane.vt.size().0 + 1;
                pane.move_prompt_vertical(1, h);
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    match pane.effective_routing_mode() {
        InputRoutingMode::CommandPrompt => handle_command_prompt_key(pane, key),
        InputRoutingMode::RawPtyPassthrough => {
            let bytes = encode_key_to_pty_bytes(key);
            if !bytes.is_empty() {
                if let Some(pty) = &pane.pty {
                    pty.write_all(&bytes)?;
                }
            }
            Ok(AppAction::Handled)
        }
    }
}

fn handle_command_prompt_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') => {
                // If prompt has text, clear it; if empty, forward SIGINT (0x03) to PTY
                if !pane.prompt.input.is_empty() {
                    pane.prompt.clear_line();
                } else if let Some(pty) = &pane.pty {
                    pty.write_all(&[0x03])?;
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('u') => {
                pane.prompt.clear_line();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('a') => {
                pane.prompt.move_cursor_home();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('e') => {
                pane.prompt.move_cursor_end();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('d') => {
                if pane.prompt.input.is_empty() {
                    if let Some(pty) = &pane.pty {
                        pty.write_all(&[0x04])?;
                    }
                } else {
                    pane.prompt.delete_char();
                }
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Enter => {
            let submitted = pane.prompt.take_submission();
            pane.vt.reset_scrollback();
            pane.vt.record_command_submission(submitted.clone());
            if let Some(pty) = &pane.pty {
                let mut payload = submitted.into_bytes();
                payload.push(b'\n');
                pty.write_all(&payload)?;
            }
            Ok(AppAction::Handled)
        }
        KeyCode::Backspace => {
            pane.prompt.backspace();
            Ok(AppAction::Handled)
        }
        KeyCode::Delete => {
            pane.prompt.delete_char();
            Ok(AppAction::Handled)
        }
        KeyCode::Left => {
            pane.prompt.move_cursor_left();
            Ok(AppAction::Handled)
        }
        KeyCode::Right => {
            pane.prompt.move_cursor_right();
            Ok(AppAction::Handled)
        }
        KeyCode::Home => {
            pane.prompt.move_cursor_home();
            Ok(AppAction::Handled)
        }
        KeyCode::End => {
            pane.prompt.move_cursor_end();
            Ok(AppAction::Handled)
        }
        KeyCode::Up => {
            pane.prompt.history_prev();
            Ok(AppAction::Handled)
        }
        KeyCode::Down => {
            pane.prompt.history_next();
            Ok(AppAction::Handled)
        }
        KeyCode::PageUp => {
            pane.vt.scroll_up(5);
            Ok(AppAction::Handled)
        }
        KeyCode::PageDown => {
            pane.vt.scroll_down(5);
            Ok(AppAction::Handled)
        }
        KeyCode::Char(ch) => {
            pane.prompt.insert_char(ch);
            Ok(AppAction::Handled)
        }
        _ => Ok(AppAction::Continue),
    }
}

/// Encodes a `crossterm::event::KeyEvent` into standard VT100/xterm escape sequences for raw PTY passthrough.
pub fn encode_key_to_pty_bytes(key: KeyEvent) -> Vec<u8> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_lowercase() {
                return vec![(lower as u8) - b'a' + 1];
            }
        }
    }

    match key.code {
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            if key.modifiers.contains(KeyModifiers::ALT) {
                let mut out = vec![0x1b];
                out.extend_from_slice(s.as_bytes());
                out
            } else {
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::F(1) => b"\x1bOP".to_vec(),
        KeyCode::F(2) => b"\x1bOQ".to_vec(),
        KeyCode::F(3) => b"\x1bOR".to_vec(),
        KeyCode::F(4) => b"\x1bOS".to_vec(),
        KeyCode::F(5) => b"\x1b[15~".to_vec(),
        KeyCode::F(6) => b"\x1b[17~".to_vec(),
        KeyCode::F(7) => b"\x1b[18~".to_vec(),
        KeyCode::F(8) => b"\x1b[19~".to_vec(),
        KeyCode::F(9) => b"\x1b[20~".to_vec(),
        KeyCode::F(10) => b"\x1b[21~".to_vec(),
        KeyCode::F(11) => b"\x1b[23~".to_vec(),
        KeyCode::F(12) => b"\x1b[24~".to_vec(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_keystrokes_in_no_echo_mode_never_leak_into_floating_prompt() {
        let mut pane = CompositePane::new_headless(1, 25, 80);
        // Simulate sudo password prompt (!ECHO)
        pane.line_discipline.echo_enabled = false;
        assert_eq!(
            pane.effective_routing_mode(),
            InputRoutingMode::RawPtyPassthrough
        );

        for ch in "s3cr3tPass!".chars() {
            handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .expect("route key");
        }
        // Floating prompt buffer MUST remain completely empty
        assert_eq!(pane.prompt.input, "");
    }

    #[test]
    fn command_prompt_edits_and_records_command_block_on_enter() {
        let mut pane = CompositePane::new_headless(1, 25, 80);
        for ch in "echo 3270".chars() {
            handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .expect("type char");
        }
        assert_eq!(pane.prompt.input, "echo 3270");

        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("press enter");
        assert_eq!(pane.prompt.input, "");
        assert_eq!(pane.prompt.history, vec!["echo 3270".to_string()]);
        assert_eq!(pane.vt.command_blocks().len(), 1);
        assert_eq!(pane.vt.command_blocks()[0].command, "echo 3270");
    }
}
