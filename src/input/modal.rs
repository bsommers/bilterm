#[path = "modal_palette.rs"]
mod modal_palette;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::error::Result;
use crate::input::AppAction;
use crate::pane::{CompositePane, CopySelectionKind};
use crate::vt::sanitize_paste_bytes;
use crate::wm::WindowManager;

pub(super) use modal_palette::execute_menubar_selection;

pub(super) fn handle_hints_mode_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    let len = pane.hints_mode().hints.len();
    if len == 0 {
        pane.exit_hints_mode();
        return Ok(AppAction::Handled);
    }
    match key.code {
        KeyCode::Esc => pane.exit_hints_mode(),
        KeyCode::Enter => { let _ = pane.accept_hint_index(pane.hints_mode().selected.min(len - 1)); }
        KeyCode::Down | KeyCode::Tab => pane.hints_mode_mut().selected = (pane.hints_mode().selected + 1) % len,
        KeyCode::Up | KeyCode::BackTab => pane.hints_mode_mut().selected = (pane.hints_mode().selected + len - 1) % len,
        KeyCode::Backspace => { pane.hints_mode_mut().typed_prefix.pop(); }
        KeyCode::Char(ch) => { let _ = pane.push_hint_char(ch); }
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
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            pane.set_pending_unsafe_paste(None);
            pane.set_status_message(Some("Cancelled unsafe multi-line paste".to_string()));
        }
        _ => {}
    }
    Ok(AppAction::Handled)
}

pub(super) fn handle_copy_mode_key(pane: &mut CompositePane, key: KeyEvent) -> Result<AppAction> {
    let (rows, cols) = pane.vt().size();
    let max_r = rows.saturating_sub(1);
    let max_c = cols.saturating_sub(1);

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
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Down => {
                pane.exit_copy_mode();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('v') => {
                let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
                pane.copy_mode_mut().anchor = Some(cur);
                pane.copy_mode_mut().selection_kind = CopySelectionKind::Block;
                pane.copy_mode_mut().visual_lock = true;
                pane.set_status_message(Some("COPY MODE: VISUAL BLOCK (rectangular)".to_string()));
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                let _ = pane.yank_copy_selection();
                return Ok(AppAction::Handled);
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                let _ = pane.cut_copy_selection_to_prompt();
                return Ok(AppAction::Handled);
            }
            _ => {}
        }
    }

    let is_nav_key = matches!(
        key.code,
        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End | KeyCode::PageUp | KeyCode::PageDown
    );

    if is_nav_key {
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            let want_block = key.modifiers.contains(KeyModifiers::ALT)
                || pane.copy_mode().rect_mode
                || pane.copy_mode().selection_kind == CopySelectionKind::Block;
            if pane.copy_mode().anchor.is_none() {
                let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
                pane.copy_mode_mut().anchor = Some(cur);
                pane.copy_mode_mut().selection_kind = if want_block { CopySelectionKind::Block } else { CopySelectionKind::Char };
                pane.copy_mode_mut().visual_lock = want_block;
                let msg = if want_block {
                    "RECT SELECT: Shift+Arrows/hjkl selecting block (Enter/y=Copy, x=To Prompt)"
                } else {
                    "CANVAS SELECT: Shift+Arrows selecting (Enter/y/Ctrl+C=Copy, x=To Prompt)"
                };
                pane.set_status_message(Some(msg.to_string()));
            } else if want_block && pane.copy_mode().selection_kind != CopySelectionKind::Block {
                pane.copy_mode_mut().selection_kind = CopySelectionKind::Block;
                pane.copy_mode_mut().visual_lock = true;
            }
        } else if !pane.copy_mode().visual_lock && pane.copy_mode().anchor.is_some() {
            pane.copy_mode_mut().anchor = None;
            pane.copy_mode_mut().selection_kind = CopySelectionKind::None;
        }
    }

    let word_mod = (key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::SHIFT))
        || key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') | KeyCode::Tab | KeyCode::F(8) => {
            pane.exit_copy_mode();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            let step = if word_mod { 5 } else { 1 };
            pane.copy_mode_mut().cursor_col = pane.copy_mode().cursor_col.saturating_sub(step);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let step = if word_mod { 5 } else { 1 };
            pane.copy_mode_mut().cursor_col = (pane.copy_mode().cursor_col + step).min(max_c);
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
        KeyCode::Home | KeyCode::Char('0') => {
            pane.copy_mode_mut().cursor_col = 0;
        }
        KeyCode::End | KeyCode::Char('$') => {
            pane.copy_mode_mut().cursor_col = max_c;
        }
        KeyCode::Char('w') => {
            pane.copy_mode_mut().cursor_col = (pane.copy_mode().cursor_col + 5).min(max_c);
        }
        KeyCode::Char('B') => {
            pane.copy_mode_mut().cursor_col = pane.copy_mode().cursor_col.saturating_sub(5);
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if pane.copy_mode().visual_lock
                && pane.copy_mode().selection_kind == CopySelectionKind::Block
            {
                pane.copy_mode_mut().anchor = None;
                pane.copy_mode_mut().selection_kind = CopySelectionKind::None;
                pane.copy_mode_mut().visual_lock = false;
                pane.set_status_message(Some("COPY MODE: rectangular selection cleared".to_string()));
            } else {
                let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
                if pane.copy_mode().anchor.is_none() {
                    pane.copy_mode_mut().anchor = Some(cur);
                }
                pane.copy_mode_mut().selection_kind = CopySelectionKind::Block;
                pane.copy_mode_mut().visual_lock = true;
                pane.set_status_message(Some(
                    "COPY MODE: VISUAL BLOCK (rectangular) — Arrows/hjkl resize, Enter/y=Copy, x=To Prompt"
                        .to_string(),
                ));
            }
        }
        KeyCode::Char('b') => {
            if pane.copy_mode().selection_kind != CopySelectionKind::Block {
                let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
                pane.copy_mode_mut().anchor = Some(cur);
                pane.copy_mode_mut().selection_kind = CopySelectionKind::Block;
                pane.copy_mode_mut().visual_lock = true;
                pane.set_status_message(Some("COPY MODE: VISUAL BLOCK (rectangular)".to_string()));
            } else {
                pane.copy_mode_mut().cursor_col = pane.copy_mode().cursor_col.saturating_sub(5);
            }
        }
        KeyCode::Char('v') | KeyCode::Char(' ') => {
            if pane.copy_mode().visual_lock
                && pane.copy_mode().selection_kind == CopySelectionKind::Char
            {
                pane.copy_mode_mut().anchor = None;
                pane.copy_mode_mut().selection_kind = CopySelectionKind::None;
                pane.copy_mode_mut().visual_lock = false;
                pane.set_status_message(Some("COPY MODE: selection cleared".to_string()));
            } else {
                let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
                pane.copy_mode_mut().anchor = Some(cur);
                pane.copy_mode_mut().selection_kind = CopySelectionKind::Char;
                pane.copy_mode_mut().visual_lock = true;
                pane.set_status_message(Some(
                    "COPY MODE: VISUAL CHAR (move with Arrows/hjkl, Enter/y=Copy, x=To Prompt)"
                        .to_string(),
                ));
            }
        }
        KeyCode::Char('V') => {
            let cur = (pane.copy_mode().cursor_row, pane.copy_mode().cursor_col);
            pane.copy_mode_mut().anchor = Some(cur);
            pane.copy_mode_mut().selection_kind = CopySelectionKind::Line;
            pane.copy_mode_mut().visual_lock = true;
            pane.set_status_message(Some("COPY MODE: VISUAL LINE".to_string()));
        }
        KeyCode::Char('y') | KeyCode::Char('c') | KeyCode::Enter => {
            pane.yank_copy_selection();
        }
        KeyCode::Char('x') => {
            pane.cut_copy_selection_to_prompt();
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
            KeyCode::Char('g') | KeyCode::Char('c') => pane.prompt_mut().cancel_fuzzy_search(),
            KeyCode::Char('r') | KeyCode::Char('n') => pane.prompt_mut().fuzzy_search.select_next(),
            KeyCode::Char('p') => pane.prompt_mut().fuzzy_search.select_prev(),
            _ => {}
        }
        return Ok(AppAction::Handled);
    }
    match key.code {
        KeyCode::Esc => pane.prompt_mut().cancel_fuzzy_search(),
        KeyCode::Enter => { let _ = pane.prompt_mut().accept_fuzzy_match(); }
        KeyCode::Up => pane.prompt_mut().fuzzy_search.select_prev(),
        KeyCode::Down => pane.prompt_mut().fuzzy_search.select_next(),
        KeyCode::Backspace => pane.prompt_mut().pop_fuzzy_query_char(),
        KeyCode::Char(ch) => pane.prompt_mut().push_fuzzy_query_char(ch),
        _ => {}
    }
    Ok(AppAction::Handled)
}

pub(super) fn handle_history_popup_key(
    pane: &mut CompositePane,
    key: KeyEvent,
) -> Result<AppAction> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => pane.prompt_mut().history_popup_open = false,
        KeyCode::Enter => { let _ = pane.prompt_mut().accept_history_popup_selection(); }
        KeyCode::Up | KeyCode::Char('k') => pane.prompt_mut().history_popup_prev(),
        KeyCode::Down | KeyCode::Char('j') => pane.prompt_mut().history_popup_next(),
        KeyCode::Char('s') => {
            if let Ok(saved) = pane.export_history_to_temp_file(None) {
                pane.set_status_message(Some(format!("Saved history to {}", saved.display())));
            }
        }
        _ => {}
    }
    Ok(AppAction::Handled)
}

/// Handles active WindowManager modal overlays (`<F1>` Help, `<F9>` Inspector, Fuzzy Command Palette, Top Menu Bar, and Slide-Out Drawer).
pub(super) fn handle_wm_modal_key(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
    if wm.rename_dialog_open() {
        if wm.drawer().focused {
            wm.cancel_rename_dialog();
        } else if !wm.is_leader_pending() {
            wm.handle_rename_dialog_key(key);
            return Ok(Some(AppAction::Handled));
        }
    }

    // A workspace modal (Exit Guard, Help, Palette, Menu bar, Inspector) always has the keyboard
    // first: floating utility windows must not answer a modal they cannot even show on top of.
    // Plain F7 / Alt+F7 always reaches DrawerToggle so utility windows never lock out the drawer.
    let is_drawer_toggle_fkey = key.code == KeyCode::F(7)
        && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::ALT);
    if matches!(wm.modal(), crate::wm::Modal::None) && !is_drawer_toggle_fkey {
        if wm.command_window().captures_input()
            && !wm.is_leader_pending()
            && let Some(action) = super::command_window_keys::handle_command_window_key(wm, key)
        {
            return Ok(Some(action));
        }
        if wm.fonts_window().captures_input()
            && !wm.is_leader_pending()
            && let Some(action) = super::fonts_window_keys::handle_fonts_window_key(wm, key)
        {
            return Ok(Some(action));
        }
        if wm.script_studio().captures_input()
            && !wm.is_leader_pending()
            && let Some(action) = super::script_studio_keys::handle_script_studio_key(wm, key)
        {
            return Ok(Some(action));
        }
        if wm.agents_list().captures_input()
            && !wm.is_leader_pending()
            && let Some(action) = super::agents_list_keys::handle_agents_list_key(wm, key)
        {
            return Ok(Some(action));
        }
        if wm.session_map().captures_input()
            && !wm.is_leader_pending()
            && let Some(action) = super::session_map_keys::handle_session_map_key(wm, key)
        {
            return Ok(Some(action));
        }
        if wm.agents_window_captures_input()
            && !wm.is_leader_pending()
            && let Some(action) = super::agents_window::handle_agents_window_key(wm, key)
        {
            return Ok(Some(action));
        }
    }

    if wm.drawer().focused {
        if wm.is_leader_pending() {
            return Ok(None);
        }
        let inside_tmux = crate::tmux::TmuxEnv::detect().is_some();
        if super::keymap::is_leader_key_with_tmux(key, &wm.config().leader_key, inside_tmux) {
            return Ok(None);
        }
        if wm.drawer_config().style == crate::wm::drawer::DrawerStyle::Navigator
            && let Some(action) = super::drawer_navigator::handle_navigator_drawer_key(wm, key)
        {
            return Ok(Some(action));
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            && !matches!(
                key.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down | KeyCode::F(7)
            )
        {
            return Ok(None);
        }
        match key.code {
            KeyCode::F(7) | KeyCode::Char('q') => wm.toggle_drawer(),
            KeyCode::F(11) => {
                let _ = wm.apply(crate::action::Action::VisualStyleCommand(None));
            }
            KeyCode::Char('_') | KeyCode::Char('-') => {
                let _ = wm.apply(crate::action::Action::DrawerSize(0));
            }
            KeyCode::Char('*') => {
                let next = if wm.drawer_config().visibility == crate::wm::DrawerVisibilityMode::Fixed {
                    crate::wm::DrawerVisibilityMode::OnDemand
                } else {
                    crate::wm::DrawerVisibilityMode::Fixed
                };
                wm.set_drawer_visibility(Some(next));
            }
            KeyCode::Char(c @ ('[' | ']')) => {
                let n = wm.step_scale(crate::ui::typography::PanelTarget::Drawer, if c == ']' { 10 } else { -10 });
                let note = if wm.gui_surface { "" } else { " (applies in the GUI only)" };
                wm.active_pane_mut().set_status_message(Some(format!("Panel scale: {n}%{note}")));
            }
            KeyCode::Char('M') | KeyCode::F(5) => {
                let _ = wm.apply(crate::action::Action::DrawerSize(u16::MAX));
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
                wm.drawer_reparent_selected_session(false);
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
                wm.drawer_reparent_selected_session(true);
            }
            // Left/Right scroll long rows like the MCP console; Right at the end of the text still leaves the drawer.
            KeyCode::Left => {
                wm.drawer_hscroll_by(-6);
            }
            KeyCode::Right if wm.drawer_hscroll_by(6) => {}
            KeyCode::Esc | KeyCode::Tab | KeyCode::Right => {
                wm.drawer_mut().focused = false;
            }
            KeyCode::Down
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                wm.drawer_move_selected_session(false);
            }
            KeyCode::Up
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                wm.drawer_move_selected_session(true);
            }
            KeyCode::Char('J') => wm.drawer_move_selected_session(false),
            KeyCode::Char('K') => wm.drawer_move_selected_session(true),
            KeyCode::Char('N') => {
                if let Ok(name) = wm.create_new_session(None, false) {
                    wm.sync_drawer_selection_to_active_pane();
                    wm.active_pane_mut()
                        .set_status_message(Some(format!("Created new session '{name}'")));
                }
            }
            KeyCode::Char('b') | KeyCode::Char('B') => {
                wm.drawer_branch_selected_session();
            }
            KeyCode::Char('>') => {
                wm.drawer_reparent_selected_session(true);
            }
            KeyCode::Char('<') | KeyCode::Char('u') | KeyCode::Char('U') => {
                wm.drawer_reparent_selected_session(false);
            }
            KeyCode::Char('P') => {
                let target = wm.selected_drawer_session_name();
                wm.drawer_mut().focused = false;
                wm.active_pane_mut().prompt_mut().manual_raw_override = false;
                wm.active_pane_mut().prompt_mut().force_command_prompt = true;
                wm.active_pane_mut()
                    .prompt_mut()
                    .set_text(&format!(":session graft {target} "));
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                let target = wm.selected_drawer_session_name();
                if let Ok((saved, path)) = wm.save_and_activate_session(Some(&target)) {
                    wm.active_pane_mut().set_status_message(Some(format!(
                        "Saved session '{saved}' to {}",
                        path.display()
                    )));
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                let pane_target = wm
                    .drawer_rows()
                    .get(wm.drawer().selected_index)
                    .and_then(|row| match row.kind {
                        crate::wm::DrawerRowKind::Pane(pid) => Some(pid),
                        _ => None,
                    });
                if let Some(pid) = pane_target {
                    wm.drawer_mut().focused = false;
                    wm.active_pane_mut().prompt_mut().manual_raw_override = false;
                    wm.active_pane_mut().prompt_mut().force_command_prompt = true;
                    wm.active_pane_mut()
                        .prompt_mut()
                        .set_text(&format!(":window rename {pid} "));
                    wm.open_window_rename_dialog(Some(pid));
                } else {
                    let target = wm.selected_drawer_session_name();
                    wm.drawer_mut().focused = false;
                    wm.active_pane_mut().prompt_mut().manual_raw_override = false;
                    wm.active_pane_mut().prompt_mut().force_command_prompt = true;
                    wm.active_pane_mut()
                        .prompt_mut()
                        .set_text(&format!(":session rename {target} "));
                    wm.open_session_rename_dialog(Some(target));
                }
            }
            KeyCode::Char(':') => {
                wm.drawer_mut().focused = false;
                wm.active_pane_mut().prompt_mut().manual_raw_override = false;
                wm.active_pane_mut().prompt_mut().force_command_prompt = true;
                wm.active_pane_mut().prompt_mut().set_text(":");
            }
            KeyCode::Down | KeyCode::Char('j') => wm.drawer_select_next(),
            KeyCode::Up | KeyCode::Char('k') => wm.drawer_select_prev(),
            KeyCode::Char('h') => { wm.drawer_hscroll_by(-6); }
            KeyCode::Char('l') => { wm.drawer_hscroll_by(6); }
            KeyCode::PageDown => wm.drawer_scroll_by(5),
            KeyCode::PageUp => wm.drawer_scroll_by(-5),
            KeyCode::Home | KeyCode::Char('g') => wm.drawer_mut().selected_index = 0,
            KeyCode::End | KeyCode::Char('G') => wm.drawer_scroll_by(10_000),
            KeyCode::Enter | KeyCode::Char(' ') => wm.drawer_activate_selected(false),
            KeyCode::Char('n') | KeyCode::Char('+') | KeyCode::Char('c') | KeyCode::Char('w') => {
                if let Ok(id) = wm.split_focused(crate::wm::split_tree::SplitDirection::Horizontal) {
                    wm.sync_drawer_selection_to_active_pane();
                    wm.active_pane_mut()
                        .set_status_message(Some(format!("Created new window #{id}")));
                }
            }
            KeyCode::Char('v') | KeyCode::Char('|') | KeyCode::Char('%') => {
                if let Ok(id) = wm.split_focused(crate::wm::split_tree::SplitDirection::Vertical) {
                    wm.sync_drawer_selection_to_active_pane();
                    wm.active_pane_mut()
                        .set_status_message(Some(format!("Created new vertical window #{id}")));
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                wm.drawer_close_selected_pane();
            }
            KeyCode::Char('z') => wm.drawer_zoom_selected_pane(),
            KeyCode::Char('d') => { wm.set_drawer_edge(None); }
            KeyCode::Char('m') => {
                let mode = wm.set_drawer_layout(None);
                wm.active_pane_mut().set_status_message(Some(format!(
                    "Slide-Out Drawer Layout Mode: {} ({})",
                    mode.label(),
                    if mode == crate::wm::DrawerLayoutMode::Push { "Push-Out reflow" } else { "Fly-Out overlay" }
                )));
            }
            KeyCode::Char('p') => { wm.set_drawer_visibility(None); }
            KeyCode::Char('a') => { wm.execute_layout_profile_command("arrange grid"); }
            KeyCode::Char('A') => { wm.execute_layout_profile_command("arrange stack-grid"); }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                let pat = if wm.layout_mode() == crate::wm::WorkspaceLayoutMode::Tiled {
                    "arrange cascade"
                } else {
                    "arrange grid"
                };
                wm.execute_layout_profile_command(pat);
            }
            KeyCode::Char('L') => { wm.execute_layout_profile_command("save-pos"); }
            _ => {
                if matches!(key.code, KeyCode::F(_))
                    || key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                {
                    return Ok(None);
                }
            }
        }
        return Ok(Some(AppAction::Handled));
    }

    if wm.mcp_console().captures_input()
        && !wm.is_leader_pending()
        && !is_drawer_toggle_fkey
        && let Some(action) = modal_palette::handle_mcp_console_key(wm, key)?
    {
        return Ok(Some(action));
    }

    if let Some(action) = super::link_mouse::handle_link_banner_key(wm, key)? {
        return Ok(Some(action));
    }

    if wm.active_pane().held_exited_shell().is_some() && !wm.is_leader_pending() {
        match key.code {
            KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('R') if key.modifiers.is_empty() => {
                wm.respawn_active_pane();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('d') | KeyCode::Char('D') if key.modifiers.is_empty() => {
                wm.detach_client();
                return Ok(Some(AppAction::Quit));
            }
            _ => {}
        }
    }

    modal_palette::handle_active_modal_overlay_key(wm, key)
}
