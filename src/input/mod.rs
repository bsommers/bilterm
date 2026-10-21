use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::error::Result;
use crate::pane::colon_command::{execute_colon_command, parse_colon_command};
use crate::pane::{CompositePane, PromptAnchor, PromptMouseDrag};
use crate::pty::line_discipline::InputRoutingMode;

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
pub fn handle_key_event(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    pane.refresh_line_discipline();

    // Global emergency quit: Ctrl+Q
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        return Ok(AppAction::Quit);
    }

    // If fuzzy history search is active, route keys to fuzzy search modal first
    if pane.is_prompt_visible() && pane.prompt.fuzzy_search.active {
        return handle_fuzzy_search_key(pane, key);
    }

    // Global manual toggle between CommandPrompt and RawPtyPassthrough: Ctrl+G or F12
    if (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g'))
        || key.code == KeyCode::F(12)
    {
        pane.toggle_manual_raw_override();
        return Ok(AppAction::Handled);
    }

    // Intra-pane prompt positioning, height resize, and OSC 133 command-block jump hotkeys
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
            KeyCode::Char('+') | KeyCode::Char('=') => {
                pane.resize_prompt_height(1);
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('-') => {
                pane.resize_prompt_height(-1);
                return Ok(AppAction::Handled);
            }
            KeyCode::PageUp => {
                pane.vt.jump_to_prev_command_block();
                return Ok(AppAction::Handled);
            }
            KeyCode::PageDown => {
                pane.vt.jump_to_next_command_block();
                return Ok(AppAction::Handled);
            }
            KeyCode::Left => {
                if pane.effective_routing_mode() == InputRoutingMode::CommandPrompt {
                    pane.prompt.move_word_left();
                    return Ok(AppAction::Handled);
                }
            }
            KeyCode::Right => {
                if pane.effective_routing_mode() == InputRoutingMode::CommandPrompt {
                    pane.prompt.move_word_right();
                    return Ok(AppAction::Handled);
                }
            }
            KeyCode::Enter => {
                if pane.effective_routing_mode() == InputRoutingMode::CommandPrompt {
                    pane.prompt.insert_newline();
                    return Ok(AppAction::Handled);
                }
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

fn handle_fuzzy_search_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('g') | KeyCode::Char('c') => {
                pane.prompt.cancel_fuzzy_search();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('r') | KeyCode::Char('n') => {
                pane.prompt.fuzzy_search.select_next();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('p') => {
                pane.prompt.fuzzy_search.select_prev();
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => {
            pane.prompt.cancel_fuzzy_search();
            Ok(AppAction::Handled)
        }
        KeyCode::Enter => {
            pane.prompt.accept_fuzzy_match();
            Ok(AppAction::Handled)
        }
        KeyCode::Up => {
            pane.prompt.fuzzy_search.select_prev();
            Ok(AppAction::Handled)
        }
        KeyCode::Down => {
            pane.prompt.fuzzy_search.select_next();
            Ok(AppAction::Handled)
        }
        KeyCode::Backspace => {
            pane.prompt.pop_fuzzy_query_char();
            Ok(AppAction::Handled)
        }
        KeyCode::Char(ch) => {
            pane.prompt.push_fuzzy_query_char(ch);
            Ok(AppAction::Handled)
        }
        _ => Ok(AppAction::Handled),
    }
}

fn handle_command_prompt_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    if key.modifiers.contains(KeyModifiers::SHIFT) && key.code == KeyCode::Enter {
        pane.prompt.insert_newline();
        return Ok(AppAction::Handled);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('r') => {
                pane.prompt.open_fuzzy_search();
                return Ok(AppAction::Handled);
            }
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
            KeyCode::Char('k') => {
                pane.prompt.kill_to_end_of_line();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('w') => {
                pane.prompt.kill_prev_word();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('y') => {
                pane.prompt.yank();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('z') => {
                pane.prompt.undo();
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
            // Check for in-prompt :command execution first (PANE-03, PANE-05)
            if let Some(cmd_res) = parse_colon_command(&submitted) {
                match cmd_res {
                    Ok(cmd) => {
                        if execute_colon_command(pane, cmd) {
                            return Ok(AppAction::Quit);
                        }
                    }
                    Err(err_msg) => {
                        pane.status_message = Some(err_msg);
                    }
                }
                return Ok(AppAction::Handled);
            }

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
            if !pane.prompt.move_cursor_up() {
                pane.prompt.history_prev();
            }
            Ok(AppAction::Handled)
        }
        KeyCode::Down => {
            if !pane.prompt.move_cursor_down() {
                pane.prompt.history_next();
            }
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

/// Routes a `crossterm::event::Event::Paste` payload (`PTY-04`, `DECSET 2004`).
///
/// - In `CommandPrompt` mode: inserts multi-line text directly into `FloatingPromptState` without
///   auto-executing newlines so the user can inspect/edit pasted scripts before pressing `Enter`.
/// - In `RawPtyPassthrough` mode: wraps pasted text in `\x1b[200~...\x1b[201~` when the child
///   program has enabled `DECSET 2004` bracketed paste, or writes raw bytes otherwise.
pub fn handle_paste_event(pane: &mut CompositePane, pasted_text: &str) -> Result<AppAction> {
    pane.refresh_line_discipline();
    match pane.effective_routing_mode() {
        InputRoutingMode::CommandPrompt => {
            pane.prompt.insert_str(pasted_text);
            Ok(AppAction::Handled)
        }
        InputRoutingMode::RawPtyPassthrough => {
            if let Some(pty) = &pane.pty {
                if pane.vt.is_bracketed_paste() {
                    let mut framed = Vec::with_capacity(pasted_text.len() + 12);
                    framed.extend_from_slice(b"\x1b[200~");
                    framed.extend_from_slice(pasted_text.as_bytes());
                    framed.extend_from_slice(b"\x1b[201~");
                    pty.write_all(&framed)?;
                } else {
                    pty.write_all(pasted_text.as_bytes())?;
                }
            }
            Ok(AppAction::Handled)
        }
    }
}

/// Routes SGR mouse events for intra-pane floating prompt dragging, height resizing,
/// `[▲][▼]` pin badge clicks, and canvas scrollback (`PANE-02`, `PANE-03`).
pub fn handle_mouse_event(
    pane: &mut CompositePane,
    mouse: MouseEvent,
    viewport: Rect,
) -> Result<AppAction> {
    if !pane.is_prompt_visible() {
        return Ok(AppAction::Continue);
    }

    let prompt_rect = pane.prompt_rect(viewport);
    let canvas_rect = pane.canvas_rect(viewport);
    let col = mouse.column;
    let row = mouse.row;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Top border of floating prompt: check [▲][▼] pin badges on right side or start vertical move drag
            if row == prompt_rect.y
                && col >= prompt_rect.x
                && col < prompt_rect.x + prompt_rect.width
            {
                let right_edge = prompt_rect.x + prompt_rect.width;
                // Layout on top-right border: ... [▲][▼]─┐ (right_edge - 8..right_edge - 5 is [▲], right_edge - 5..right_edge - 2 is [▼])
                if prompt_rect.width >= 16 {
                    let top_btn_start = right_edge.saturating_sub(8);
                    let bot_btn_start = right_edge.saturating_sub(5);
                    if col >= top_btn_start && col < top_btn_start + 3 {
                        pane.set_prompt_anchor(PromptAnchor::Top);
                        return Ok(AppAction::Handled);
                    }
                    if col >= bot_btn_start && col < bot_btn_start + 3 {
                        pane.set_prompt_anchor(PromptAnchor::Bottom);
                        return Ok(AppAction::Handled);
                    }
                }
                pane.mouse_drag = Some(PromptMouseDrag::MovePrompt {
                    grab_x_offset: col.saturating_sub(prompt_rect.x),
                });
                return Ok(AppAction::Handled);
            }

            // Bottom border of floating prompt: start height resize drag
            let bottom_y = prompt_rect.y + prompt_rect.height.saturating_sub(1);
            if row == bottom_y && col >= prompt_rect.x && col < prompt_rect.x + prompt_rect.width {
                pane.mouse_drag = Some(PromptMouseDrag::ResizePrompt);
                return Ok(AppAction::Handled);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(drag) = pane.mouse_drag {
                match drag {
                    PromptMouseDrag::MovePrompt { .. } => {
                        let rel_y = row.saturating_sub(canvas_rect.y);
                        if rel_y == 0 {
                            pane.set_prompt_anchor(PromptAnchor::Top);
                        } else if rel_y + prompt_rect.height >= canvas_rect.height {
                            pane.set_prompt_anchor(PromptAnchor::Bottom);
                        } else {
                            pane.set_prompt_anchor(PromptAnchor::Floating { y_offset: rel_y });
                        }
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePrompt => {
                        let new_height = row.saturating_sub(prompt_rect.y).saturating_add(1);
                        pane.set_prompt_height(new_height.clamp(3, 12));
                        return Ok(AppAction::Handled);
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if pane.mouse_drag.take().is_some() {
                return Ok(AppAction::Handled);
            }
        }
        MouseEventKind::ScrollUp => {
            pane.vt.scroll_up(3);
            return Ok(AppAction::Handled);
        }
        MouseEventKind::ScrollDown => {
            pane.vt.scroll_down(3);
            return Ok(AppAction::Handled);
        }
        _ => {}
    }

    Ok(AppAction::Continue)
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
        assert_eq!(pane.prompt.anchor, PromptAnchor::Floating { y_offset: 9 });

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
        assert_eq!(pane.prompt.height, 6);

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
        assert_eq!(pane.prompt.anchor, PromptAnchor::Top);

        // 4. Paste multiline text into CommandPrompt without auto-executing
        handle_paste_event(&mut pane, "line1\nline2").unwrap();
        assert_eq!(pane.prompt.lines, vec!["line1".to_string(), "line2".to_string()]);
    }
}
