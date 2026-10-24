use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::colon_command::{
    execute_colon_command, execute_wm_colon_command, parse_colon_command,
};
use crate::error::Result;
use crate::input::AppAction;
use crate::pane::{CompositePane, CopySelectionKind};
use crate::vt::sanitize_paste_bytes;
use crate::wm::WindowManager;

pub(super) fn handle_hints_mode_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    let len = pane.hints_mode().hints.len();
    if len == 0 {
        pane.exit_hints_mode();
        return Ok(AppAction::Handled);
    }
    match key.code {
        KeyCode::Esc => {
            pane.exit_hints_mode();
        }
        KeyCode::Enter => {
            let sel = pane.hints_mode().selected.min(len - 1);
            let _ = pane.accept_hint_index(sel);
        }
        KeyCode::Down | KeyCode::Tab => {
            pane.hints_mode_mut().selected = (pane.hints_mode().selected + 1) % len;
        }
        KeyCode::Up | KeyCode::BackTab => {
            pane.hints_mode_mut().selected = (pane.hints_mode().selected + len - 1) % len;
        }
        KeyCode::Backspace => {
            pane.hints_mode_mut().typed_prefix.pop();
        }
        KeyCode::Char(ch) => {
            let _ = pane.push_hint_char(ch);
        }
        _ => {}
    }
    Ok(AppAction::Handled)
}

pub(super) fn handle_unsafe_paste_modal_key(
    pane: &mut CompositePane,
    key: KeyEvent,
) -> Result<AppAction> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            if let Some(payload) = pane.take_pending_unsafe_paste() {
                let clean = sanitize_paste_bytes(&payload).replace("\x1b[201~", "");
                if pane.vt().is_bracketed_paste() {
                    let mut framed = Vec::with_capacity(clean.len() + 12);
                    framed.extend_from_slice(b"\x1b[200~");
                    framed.extend_from_slice(clean.as_bytes());
                    framed.extend_from_slice(b"\x1b[201~");
                    pane.write_to_pty(&framed)?;
                } else {
                    pane.write_to_pty(clean.as_bytes())?;
                }
                pane.set_status_message(Some(format!("Confirmed paste ({} chars)", clean.len())));
            }
            Ok(AppAction::Handled)
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            pane.set_pending_unsafe_paste(None);
            pane.set_status_message(Some("Cancelled unsafe multi-line paste".to_string()));
            Ok(AppAction::Handled)
        }
        _ => Ok(AppAction::Handled),
    }
}

pub(super) fn handle_copy_mode_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    let (rows, cols) = pane.vt().size();
    let max_r = rows.saturating_sub(1);
    let max_c = cols.saturating_sub(1);

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('v') {
        let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
        pane.copy_mode_mut().anchor = Some(cur);
        pane.copy_mode_mut().selection_kind = CopySelectionKind::Block;
        pane.set_status_message(Some("COPY MODE: VISUAL BLOCK (rectangular)".to_string()));
        return Ok(AppAction::Handled);
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            pane.exit_copy_mode();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            pane.copy_mode_mut().cursor_col = pane.copy_mode().cursor_col.saturating_sub(1);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            pane.copy_mode_mut().cursor_col = (pane.copy_mode().cursor_col + 1).min(max_c);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if pane.copy_mode().cursor_row > 0 {
                pane.copy_mode_mut().cursor_row -= 1;
            } else {
                pane.vt_mut().scroll_up(1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if pane.copy_mode().cursor_row < max_r {
                pane.copy_mode_mut().cursor_row += 1;
            } else {
                pane.vt_mut().scroll_down(1);
            }
        }
        KeyCode::Home | KeyCode::Char('0') => {
            pane.copy_mode_mut().cursor_col = 0;
        }
        KeyCode::End | KeyCode::Char('$') => {
            pane.copy_mode_mut().cursor_col = max_c;
        }
        KeyCode::Char('w') => {
            pane.copy_mode_mut().cursor_col = (pane.copy_mode().cursor_col + 5).min(max_c);
        }
        KeyCode::Char('b') => {
            // 'b' toggles Rectangular Block selection when no modifier or moves word back if already in char mode
            if pane.copy_mode().selection_kind != CopySelectionKind::Block {
                let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
                pane.copy_mode_mut().anchor = Some(cur);
                pane.copy_mode_mut().selection_kind = CopySelectionKind::Block;
                pane.set_status_message(Some("COPY MODE: VISUAL BLOCK (rectangular)".to_string()));
            } else {
                pane.copy_mode_mut().cursor_col = pane.copy_mode().cursor_col.saturating_sub(5);
            }
        }
        KeyCode::Char('v') => {
            let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
            pane.copy_mode_mut().anchor = Some(cur);
            pane.copy_mode_mut().selection_kind = CopySelectionKind::Char;
            pane.set_status_message(Some("COPY MODE: VISUAL CHAR".to_string()));
        }
        KeyCode::Char('V') => {
            let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
            pane.copy_mode_mut().anchor = Some(cur);
            pane.copy_mode_mut().selection_kind = CopySelectionKind::Line;
            pane.set_status_message(Some("COPY MODE: VISUAL LINE".to_string()));
        }
        KeyCode::Char('y') | KeyCode::Enter => {
            pane.yank_copy_selection();
        }
        _ => {}
    }
    Ok(AppAction::Handled)
}

pub(super) fn handle_fuzzy_search_key(
    pane: &mut CompositePane,
    key: KeyEvent,
) -> Result<AppAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('g') | KeyCode::Char('c') => {
                pane.prompt_mut().cancel_fuzzy_search();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('r') | KeyCode::Char('n') => {
                pane.prompt_mut().fuzzy_search.select_next();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('p') => {
                pane.prompt_mut().fuzzy_search.select_prev();
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => {
            pane.prompt_mut().cancel_fuzzy_search();
            Ok(AppAction::Handled)
        }
        KeyCode::Enter => {
            pane.prompt_mut().accept_fuzzy_match();
            Ok(AppAction::Handled)
        }
        KeyCode::Up => {
            pane.prompt_mut().fuzzy_search.select_prev();
            Ok(AppAction::Handled)
        }
        KeyCode::Down => {
            pane.prompt_mut().fuzzy_search.select_next();
            Ok(AppAction::Handled)
        }
        KeyCode::Backspace => {
            pane.prompt_mut().pop_fuzzy_query_char();
            Ok(AppAction::Handled)
        }
        KeyCode::Char(ch) => {
            pane.prompt_mut().push_fuzzy_query_char(ch);
            Ok(AppAction::Handled)
        }
        _ => Ok(AppAction::Handled),
    }
}

pub(super) fn handle_history_popup_key(
    pane: &mut CompositePane,
    key: KeyEvent,
) -> Result<AppAction> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            pane.prompt_mut().history_popup_open = false;
            Ok(AppAction::Handled)
        }
        KeyCode::Enter => {
            pane.prompt_mut().accept_history_popup_selection();
            Ok(AppAction::Handled)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            pane.prompt_mut().history_popup_prev();
            Ok(AppAction::Handled)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            pane.prompt_mut().history_popup_next();
            Ok(AppAction::Handled)
        }
        KeyCode::Char('s') => {
            if let Ok(saved) = pane.export_history_to_temp_file(None) {
                pane.set_status_message(Some(format!("Saved history to {}", saved.display())));
            }
            Ok(AppAction::Handled)
        }
        _ => Ok(AppAction::Handled),
    }
}

pub(super) fn execute_menubar_selection(
    wm: &mut WindowManager,
    category: usize,
    item: usize,
) -> Result<AppAction> {
    wm.menubar_open_category = None;
    let cmd_str = match (category, item) {
        (0, 0) => ":inspector",
        (0, 1) => ":session save default",
        (0, 2) => ":session restore default",
        (0, 3) => ":quit",
        (1, 0) => ":split h",
        (1, 1) => ":split v",
        (1, 2) => ":zoom",
        (1, 3) => ":layout stack",
        (2, 0) => ":prompt top",
        (2, 1) => ":prompt bottom",
        (2, 2) => ":prompt split",
        (2, 3) => ":mode cycle",
        (3, 0) => ":hints",
        (3, 1) => ":copy",
        (3, 2) => ":crosshair",
        (3, 3) => ":fkeybar",
        (4, 0) => ":theme turbo-blue",
        (4, 1) => ":theme turbo-cyan",
        (4, 2) => ":theme 3278",
        (4, 3) => ":theme 3279",
        (5, 0) => ":broadcast",
        (5, 1) => ":lock",
        (5, 2) => ":quake",
        (5, 3) => ":raw",
        _ => ":help",
    };
    if let Some(Ok(cmd)) = parse_colon_command(cmd_str)
        && execute_wm_colon_command(wm, cmd)
    {
        return Ok(AppAction::Quit);
    }
    Ok(AppAction::Handled)
}

/// Handles active WindowManager modal overlays (`<F1>` Help, `<F9>` Inspector, Fuzzy Command Palette, and Top Menu Bar).
pub(super) fn handle_wm_modal_key(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
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
        return Ok(Some(AppAction::Handled));
    }

    // If the <F9> Live 3270 Terminal Inspector modal is open, route keys to it first (BEL-01)
    if wm.inspector_visible {
        match key.code {
            KeyCode::F(9) | KeyCode::Esc | KeyCode::Char('q') => {
                wm.inspector_visible = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                wm.inspector_scroll = wm.inspector_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                wm.inspector_scroll = wm.inspector_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                wm.inspector_scroll = wm.inspector_scroll.saturating_sub(6);
            }
            KeyCode::PageDown => {
                wm.inspector_scroll = wm.inspector_scroll.saturating_add(6);
            }
            _ => {}
        }
        return Ok(Some(AppAction::Handled));
    }

    // If the Fuzzy Command Palette modal is open, route keys to it first (v0.6.0)
    if wm.palette_visible {
        match key.code {
            KeyCode::Esc => {
                wm.palette_visible = false;
            }
            KeyCode::Up => {
                wm.palette_selected = wm.palette_selected.saturating_sub(1);
            }
            KeyCode::Down => {
                let count = wm.filtered_palette_items().len();
                if count > 0 {
                    wm.palette_selected = (wm.palette_selected + 1).min(count - 1);
                }
            }
            KeyCode::Backspace => {
                wm.palette_query.pop();
                wm.palette_selected = 0;
            }
            KeyCode::Enter => {
                let items = wm.filtered_palette_items();
                let sel = items
                    .get(wm.palette_selected)
                    .or_else(|| items.first())
                    .map(|&(cmd, _, _)| cmd);
                wm.palette_visible = false;
                if let Some(cmd_str) = sel
                    && let Some(Ok(cmd)) = parse_colon_command(cmd_str)
                {
                    if execute_wm_colon_command(wm, cmd.clone()) {
                        return Ok(Some(AppAction::Quit));
                    }
                    if execute_colon_command(wm.active_pane_mut(), cmd) {
                        return Ok(Some(AppAction::Quit));
                    }
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                wm.palette_query.push(c);
                wm.palette_selected = 0;
            }
            _ => {}
        }
        return Ok(Some(AppAction::Handled));
    }

    // If a top Borland Turbo Vision Menu Bar pull-down is open, route keys to it first (ECO-03)
    if let Some(cat) = wm.menubar_open_category {
        match key.code {
            KeyCode::Esc => {
                wm.menubar_open_category = None;
            }
            KeyCode::Left => {
                wm.menubar_open_category = Some((cat + 6) % 7);
                wm.menubar_selected_item = 0;
            }
            KeyCode::Right => {
                wm.menubar_open_category = Some((cat + 1) % 7);
                wm.menubar_selected_item = 0;
            }
            KeyCode::Up => {
                wm.menubar_selected_item = wm.menubar_selected_item.saturating_sub(1);
            }
            KeyCode::Down => {
                wm.menubar_selected_item = (wm.menubar_selected_item + 1).min(3);
            }
            KeyCode::Enter => {
                return execute_menubar_selection(wm, cat, wm.menubar_selected_item).map(Some);
            }
            _ => {}
        }
        return Ok(Some(AppAction::Handled));
    }

    Ok(None)
}
