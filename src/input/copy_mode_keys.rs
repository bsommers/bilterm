//! Declarative key dispatch tables and handlers for Copy Mode (`COPY-02`, `ARCH-R04`).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::error::Result;
use crate::input::AppAction;
use crate::pane::copy_motion::{next_word_start, prev_word_start, word_end};
use crate::pane::copy_search::{find_next_match, CopySearchState, SearchDir};
use crate::pane::{CompositePane, CopyOrigin, CopySelectionKind};

/// Handles key events when Copy Mode is active on `pane`.
pub fn handle_copy_mode_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    // 1. Mouse transient guard (COPY-01)
    if pane.copy_mode().origin == CopyOrigin::Mouse {
        let is_copy = matches!(
            (key.modifiers, key.code),
            (KeyModifiers::NONE, KeyCode::Char('y' | 'Y') | KeyCode::Enter | KeyCode::Char('x') | KeyCode::F(8))
                | (KeyModifiers::CONTROL, KeyCode::Char('c' | 'C' | 'g'))
                | (KeyModifiers::SHIFT, KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End)
        ) || (key.modifiers.contains(KeyModifiers::CONTROL)
            && key.modifiers.contains(KeyModifiers::SHIFT)
            && matches!(key.code, KeyCode::Char('c' | 'C')));

        if key.code == KeyCode::Esc {
            pane.copy_mode_mut().clear_selection();
            return Ok(AppAction::Handled);
        }
        if !is_copy {
            pane.copy_mode_mut().clear_selection();
            return crate::input::handle_key_event(pane, key);
        }
    }

    // 2. Interactive search input mode
    if let Some(ref mut search) = pane.copy_mode_mut().search_state
        && search.editing
    {
        match key.code {
            KeyCode::Esc => {
                let origin = search.origin_cursor;
                pane.copy_mode_mut().cursor_row = origin.0;
                pane.copy_mode_mut().cursor_col = origin.1;
                pane.copy_mode_mut().search_state = None;
                return Ok(AppAction::Handled);
            }
            KeyCode::Enter => {
                search.editing = false;
                let query = search.query.clone();
                let dir = search.dir;
                return execute_search_jump(pane, &query, dir);
            }
            KeyCode::Backspace => {
                search.query.pop();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                search.query.push(c);
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    let (rows, cols) = pane.vt().size();
    let max_r = rows.saturating_sub(1);
    let max_c = cols.saturating_sub(1);
    let top_abs = pane.vt_mut().viewport_top_abs();
    pane.copy_mode_mut().viewport_top_abs = top_abs;

    // 3. Global overrides within copy mode
    if (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g'))
        || (key.code == KeyCode::F(12) && !key.modifiers.contains(KeyModifiers::SHIFT))
    {
        pane.toggle_manual_raw_override();
        return Ok(AppAction::Handled);
    }
    if (key.code == KeyCode::F(4) && key.modifiers.is_empty())
        || (key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('e') | KeyCode::Char('E')))
    {
        pane.cycle_prompt_mode();
        return Ok(AppAction::Handled);
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        if matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C')) {
            pane.exit_copy_mode();
            return Ok(AppAction::Handled);
        }
        if matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')) {
            pane.toggle_rect_select_mode();
            return Ok(AppAction::Handled);
        }
        if matches!(key.code, KeyCode::Char('w') | KeyCode::Char('W')) {
            let _ = pane.yank_copy_selection();
            return Ok(AppAction::Handled);
        }
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') | KeyCode::Char('C') => {
                let _ = pane.yank_copy_selection();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                let _ = pane.cut_copy_selection_to_prompt();
                return Ok(AppAction::Handled);
            }
            KeyCode::Down => {
                pane.exit_copy_mode();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char(' ') => {
                // Emacs set-mark (toggle Char selection)
                return toggle_selection(pane, CopySelectionKind::Char, top_abs);
            }
            KeyCode::Char('v') => {
                return toggle_selection(pane, CopySelectionKind::Block, top_abs);
            }
            KeyCode::Char('u') => {
                // Half page up
                let half = rows / 2;
                if pane.copy_mode().cursor_row >= half {
                    pane.copy_mode_mut().cursor_row -= half;
                } else {
                    pane.copy_mode_mut().cursor_row = 0;
                    pane.vt_mut().scroll_up(usize::from(half));
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('d') => {
                // Half page down
                let half = rows / 2;
                if pane.copy_mode().cursor_row + half <= max_r {
                    pane.copy_mode_mut().cursor_row += half;
                } else {
                    pane.copy_mode_mut().cursor_row = max_r;
                    pane.vt_mut().scroll_down(usize::from(half));
                }
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('b') => {
                // Emacs backward char
                pane.copy_mode_mut().cursor_col = pane.copy_mode().cursor_col.saturating_sub(1);
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('f') => {
                // Emacs forward char
                pane.copy_mode_mut().cursor_col = (pane.copy_mode().cursor_col + 1).min(max_c);
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('n') => {
                // Emacs next line
                return move_down(pane, max_r);
            }
            KeyCode::Char('p') => {
                // Emacs prev line
                return move_up(pane);
            }
            KeyCode::Char('a') => {
                pane.copy_mode_mut().cursor_col = 0;
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('e') => {
                pane.copy_mode_mut().cursor_col = max_c;
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('s') => {
                start_search(pane, SearchDir::Forward);
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('r') => {
                start_search(pane, SearchDir::Backward);
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    // 4. Shift+Arrow selection extension
    let is_nav_key = matches!(
        key.code,
        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End | KeyCode::PageUp | KeyCode::PageDown
    );
    if is_nav_key
        && key.modifiers.contains(KeyModifiers::SHIFT)
        && pane.copy_mode().anchor.is_none()
    {
        let want_block = key.modifiers.contains(KeyModifiers::ALT) || pane.copy_mode().rect_mode;
        let kind = if want_block {
            CopySelectionKind::Block
        } else {
            CopySelectionKind::Char
        };
        let cr = pane.copy_mode().cursor_row;
        let cc = pane.copy_mode().cursor_col;
        pane.copy_mode_mut().begin_selection(cr, cc, kind, top_abs);
        // Shift-started Char selections are transient (dropped by plain navigation);
        // block selections stay locked.
        pane.copy_mode_mut().visual_lock = want_block;
    } else if is_nav_key && key.modifiers.contains(KeyModifiers::SHIFT) {
        let want_block = key.modifiers.contains(KeyModifiers::ALT) || pane.copy_mode().rect_mode;
        if want_block && pane.copy_mode().selection_kind != CopySelectionKind::Block {
            pane.copy_mode_mut().selection_kind = CopySelectionKind::Block;
            pane.copy_mode_mut().visual_lock = true;
        }
    } else if is_nav_key
        && !pane.copy_mode().visual_lock
        && pane.copy_mode().anchor.is_some()
    {
        let cm = pane.copy_mode_mut();
        cm.anchor = None;
        cm.anchor_abs = None;
        cm.selection_kind = CopySelectionKind::None;
    }

    // Word motion modifiers: Ctrl/Alt+Left/Right and Alt+f/b step by real words (62-02 plan).
    let word_mod = (key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::SHIFT))
        || key.modifiers.contains(KeyModifiers::CONTROL);
    let word_code = match key.code {
        KeyCode::Right => Some(KeyCode::Char('w')),
        KeyCode::Left => Some(KeyCode::Char('b')),
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => Some(KeyCode::Char('w')),
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => Some(KeyCode::Char('b')),
        _ => None,
    };
    let key_code = match word_code {
        Some(c) if word_mod => c,
        _ => key.code,
    };

    // Two-key sequence state (`gg`)
    let was_pending_g = pane.copy_mode().pending_g;
    pane.copy_mode_mut().pending_g = false;

    match key_code {
        KeyCode::Esc | KeyCode::Char('q') => {
            if pane.copy_mode().has_selection() {
                pane.copy_mode_mut().clear_selection();
            } else {
                pane.exit_copy_mode();
            }
        }
        KeyCode::Char('i') | KeyCode::Tab | KeyCode::F(8) => {
            pane.exit_copy_mode();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            pane.copy_mode_mut().cursor_col = pane.copy_mode().cursor_col.saturating_sub(1);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            pane.copy_mode_mut().cursor_col = (pane.copy_mode().cursor_col + 1).min(max_c);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            return move_up(pane);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            return move_down(pane, max_r);
        }
        KeyCode::Char('w') => {
            let cur_row = pane.copy_mode().cursor_row;
            let cur_col = pane.copy_mode().cursor_col;
            let row_text = pane.vt_mut().abs_row_text(top_abs + usize::from(cur_row)).unwrap_or_default();
            if let Some(next_c) = next_word_start(&row_text, cur_col) {
                pane.copy_mode_mut().cursor_col = next_c.min(max_c);
            } else if cur_row < max_r {
                pane.copy_mode_mut().cursor_row += 1;
                pane.copy_mode_mut().cursor_col = 0;
            } else {
                pane.vt_mut().scroll_down(1);
                pane.copy_mode_mut().cursor_col = 0;
            }
        }
        KeyCode::Char('b') => {
            let cur_row = pane.copy_mode().cursor_row;
            let cur_col = pane.copy_mode().cursor_col;
            let row_text = pane.vt_mut().abs_row_text(top_abs + usize::from(cur_row)).unwrap_or_default();
            if let Some(prev_c) = prev_word_start(&row_text, cur_col) {
                pane.copy_mode_mut().cursor_col = prev_c;
            } else if cur_row > 0 {
                pane.copy_mode_mut().cursor_row -= 1;
                let prev_row_text = pane.vt_mut().abs_row_text(top_abs + usize::from(cur_row - 1)).unwrap_or_default();
                pane.copy_mode_mut().cursor_col = prev_row_text.chars().count().saturating_sub(1) as u16;
            } else {
                pane.vt_mut().scroll_up(1);
            }
        }
        KeyCode::Char('e') => {
            let cur_row = pane.copy_mode().cursor_row;
            let cur_col = pane.copy_mode().cursor_col;
            let row_text = pane.vt_mut().abs_row_text(top_abs + usize::from(cur_row)).unwrap_or_default();
            if let Some(end_c) = word_end(&row_text, cur_col) {
                pane.copy_mode_mut().cursor_col = end_c.min(max_c);
            }
        }
        KeyCode::Home | KeyCode::Char('0') => {
            pane.copy_mode_mut().cursor_col = 0;
        }
        KeyCode::Char('^') => {
            let cur_row = pane.copy_mode().cursor_row;
            let row_text = pane.vt_mut().abs_row_text(top_abs + usize::from(cur_row)).unwrap_or_default();
            let first_non_ws = row_text.chars().position(|c| !c.is_whitespace()).unwrap_or(0) as u16;
            pane.copy_mode_mut().cursor_col = first_non_ws.min(max_c);
        }
        KeyCode::End | KeyCode::Char('$') => {
            pane.copy_mode_mut().cursor_col = max_c;
        }
        KeyCode::Char('g') => {
            if was_pending_g {
                // `gg`: top of scrollback
                pane.vt_mut().set_scrollback_offset(usize::MAX);
                pane.copy_mode_mut().cursor_row = 0;
                pane.copy_mode_mut().cursor_col = 0;
            } else {
                pane.copy_mode_mut().pending_g = true;
            }
        }
        KeyCode::Char('G') => {
            // `G`: bottom of screen
            pane.vt_mut().reset_scrollback();
            pane.copy_mode_mut().cursor_row = max_r;
        }
        KeyCode::Char('H') => {
            pane.copy_mode_mut().cursor_row = 0;
        }
        KeyCode::Char('M') => {
            pane.copy_mode_mut().cursor_row = rows / 2;
        }
        KeyCode::Char('L') => {
            pane.copy_mode_mut().cursor_row = max_r;
        }
        KeyCode::PageUp => {
            if pane.copy_mode().cursor_row >= 8 {
                pane.copy_mode_mut().cursor_row -= 8;
            } else {
                pane.copy_mode_mut().cursor_row = 0;
                pane.vt_mut().scroll_up(8);
            }
        }
        KeyCode::PageDown => {
            if pane.copy_mode().cursor_row + 8 <= max_r {
                pane.copy_mode_mut().cursor_row += 8;
            } else {
                pane.copy_mode_mut().cursor_row = max_r;
                pane.vt_mut().scroll_down(8);
            }
        }
        KeyCode::Char('v') | KeyCode::Char(' ') => {
            return toggle_selection(pane, CopySelectionKind::Char, top_abs);
        }
        KeyCode::Char('V') => {
            return toggle_selection(pane, CopySelectionKind::Line, top_abs);
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            return toggle_selection(pane, CopySelectionKind::Block, top_abs);
        }
        KeyCode::Char('y') | KeyCode::Char('c') | KeyCode::Enter => {
            pane.yank_copy_selection();
        }
        KeyCode::Char('Y') => {
            // Yank line
            let r = pane.copy_mode().cursor_row;
            pane.copy_mode_mut().begin_selection(r, 0, CopySelectionKind::Line, top_abs);
            pane.copy_mode_mut().cursor_col = max_c;
            pane.yank_copy_selection();
        }
        KeyCode::Char('x') => {
            pane.cut_copy_selection_to_prompt();
        }
        KeyCode::Char('o') | KeyCode::Char('O') => {
            // Swap anchor and cursor
            if let Some((ar, ac)) = pane.copy_mode().anchor {
                let (cr, cc) = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
                pane.copy_mode_mut().anchor = Some((cr, cc));
                pane.copy_mode_mut().cursor_row = ar;
                pane.copy_mode_mut().cursor_col = ac;
            }
        }
        KeyCode::Char('/') => {
            start_search(pane, SearchDir::Forward);
        }
        KeyCode::Char('?') => {
            start_search(pane, SearchDir::Backward);
        }
        KeyCode::Char('n') => {
            repeat_search(pane, false);
        }
        KeyCode::Char('N') => {
            repeat_search(pane, true);
        }
        _ => {}
    }

    Ok(AppAction::Handled)
}

fn move_up(pane: &mut CompositePane) -> Result<AppAction> {
    if pane.copy_mode().cursor_row > 0 {
        pane.copy_mode_mut().cursor_row -= 1;
    } else {
        pane.vt_mut().scroll_up(1);
    }
    Ok(AppAction::Handled)
}

fn move_down(pane: &mut CompositePane, max_r: u16) -> Result<AppAction> {
    if pane.copy_mode().cursor_row < max_r {
        pane.copy_mode_mut().cursor_row += 1;
    } else {
        pane.vt_mut().scroll_down(1);
    }
    Ok(AppAction::Handled)
}

fn toggle_selection(
    pane: &mut CompositePane,
    kind: CopySelectionKind,
    top_abs: usize,
) -> Result<AppAction> {
    if pane.copy_mode().visual_lock && pane.copy_mode().selection_kind == kind {
        pane.copy_mode_mut().anchor = None;
        pane.copy_mode_mut().anchor_abs = None;
        pane.copy_mode_mut().selection_kind = CopySelectionKind::None;
        pane.copy_mode_mut().visual_lock = false;
    } else if pane.copy_mode().anchor.is_some() {
        // Switching kind keeps the existing anchor (e.g. Shift+Arrows then `r`).
        pane.copy_mode_mut().selection_kind = kind;
        pane.copy_mode_mut().visual_lock = true;
    } else {
        let (r, c) = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
        pane.copy_mode_mut().begin_selection(r, c, kind, top_abs);
    }
    Ok(AppAction::Handled)
}

fn start_search(pane: &mut CompositePane, dir: SearchDir) {
    let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
    pane.copy_mode_mut().search_state = Some(CopySearchState::new(dir, cur));
}

pub fn execute_search_jump(pane: &mut CompositePane, query: &str, dir: SearchDir) -> Result<AppAction> {
    let matches = pane.vt_mut().search_history(query);
    if matches.is_empty() {
        pane.set_status_toast(format!("Pattern not found: {query}"), 2000);
        return Ok(AppAction::Handled);
    }
    let top_abs = pane.vt_mut().viewport_top_abs();
    let cur_abs = top_abs + usize::from(pane.copy_mode().cursor_row);
    let cur_col = pane.copy_mode().cursor_col;

    if let Some((target_abs, target_col, _)) = find_next_match(&matches, cur_abs, cur_col, dir) {
        jump_to_abs_match(pane, target_abs, target_col);
    }
    Ok(AppAction::Handled)
}

fn repeat_search(pane: &mut CompositePane, reverse: bool) {
    let Some(search) = pane.copy_mode().search_state.clone() else {
        return;
    };
    if search.query.is_empty() {
        return;
    }
    let mut dir = search.dir;
    if reverse {
        dir = match dir {
            SearchDir::Forward => SearchDir::Backward,
            SearchDir::Backward => SearchDir::Forward,
        };
    }
    let _ = execute_search_jump(pane, &search.query, dir);
}

fn jump_to_abs_match(pane: &mut CompositePane, target_abs: usize, target_col: u16) {
    let (rows, _) = pane.vt().size();
    let max_r = rows.saturating_sub(1);
    let used = pane.vt_mut().scrollback_used_rows();

    if target_abs < used {
        pane.vt_mut().set_scrollback_offset(used - target_abs);
        pane.copy_mode_mut().cursor_row = 0;
    } else {
        pane.vt_mut().reset_scrollback();
        pane.copy_mode_mut().cursor_row = ((target_abs - used) as u16).min(max_r);
    }
    pane.copy_mode_mut().cursor_col = target_col;
    pane.copy_mode_mut().viewport_top_abs = pane.vt_mut().viewport_top_abs();
    pane.copy_mode_mut().active = true;
}
