//! MCP console, Fuzzy Command Palette, Top Menu Bar, Help, Inspector, and Exit Guard modal key handlers.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::colon_command::{execute_wm_colon_command, parse_colon_command};
use crate::error::Result;
use crate::input::AppAction;
use crate::wm::{Modal, WindowManager};

pub(in crate::input) fn execute_menubar_selection(
    wm: &mut WindowManager,
    category: usize,
    item: usize,
) -> Result<AppAction> {
    wm.close_modal();
    let cmd_str = crate::ui::modals::menubar_item_command(category, item).unwrap_or(":help");
    if let Some(Ok(cmd)) = parse_colon_command(cmd_str)
        && execute_wm_colon_command(wm, cmd)
    {
        return Ok(AppAction::Quit);
    }
    Ok(AppAction::Handled)
}

pub(super) fn handle_mcp_console_key(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
    // Pinned window: Esc hands the keyboard back to the panes without closing it.
    if wm.mcp_console().pinned && key.code == KeyCode::Esc {
        wm.mcp_console_mut().focused = false;
        return Ok(Some(AppAction::Handled));
    }
    if wm.mcp_console().file_browser.is_none() && !key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('*') => {
                let pin = !wm.mcp_console().pinned;
                wm.mcp_console_mut().set_pinned(pin);
                wm.active_pane_mut().set_status_message(Some(
                    if pin {
                        "MCP window pinned (Esc returns focus to panes; :mcp close unpins)"
                    } else {
                        "MCP window unpinned"
                    }
                    .to_string(),
                ));
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char(c @ ('[' | ']')) => {
                let n = wm.step_scale(
                    crate::ui::typography::PanelTarget::Mcp,
                    if c == ']' { 10 } else { -10 },
                );
                let note = if wm.gui_surface {
                    ""
                } else {
                    " (applies in the GUI only)"
                };
                wm.active_pane_mut()
                    .set_status_message(Some(format!("Panel scale: {n}%{note}")));
                return Ok(Some(AppAction::Handled));
            }
            _ => {}
        }
    }
    if wm.mcp_console().file_browser.is_some() {
        match key.code {
            KeyCode::Esc => {
                wm.mcp_console_mut().close_root_terminal();
                wm.reflow_default_viewport();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Up => {
                wm.mcp_console_mut().select_prev_row();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Down => {
                wm.mcp_console_mut().select_next_row();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::PageUp => {
                wm.mcp_console_mut().scroll_up(8);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::PageDown => {
                wm.mcp_console_mut().scroll_down(8);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Home => {
                wm.mcp_console_mut().select_first_row();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::End => {
                wm.mcp_console_mut().select_last_row();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Enter => {
                wm.activate_selected_mcp_row();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Backspace => {
                if let Some(ref mut fb) = wm.mcp_console_mut().file_browser {
                    if fb.preview_file.is_some() || fb.filter.is_empty() {
                        fb.navigate_up();
                    } else {
                        fb.filter.pop();
                    }
                }
                wm.mcp_console_mut().refresh_file_browser();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('u' | 'U') if key.modifiers == KeyModifiers::CONTROL => {
                if let Some(ref mut fb) = wm.mcp_console_mut().file_browser {
                    fb.filter.clear();
                }
                wm.mcp_console_mut().refresh_file_browser();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(ref mut fb) = wm.mcp_console_mut().file_browser {
                    fb.filter.push(c);
                }
                wm.mcp_console_mut().refresh_file_browser();
                return Ok(Some(AppAction::Handled));
            }
            _ => {}
        }
    } else if !key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                wm.mcp_console_mut().close_root_terminal();
                wm.reflow_default_viewport();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
                wm.reparent_selected_mcp_node(false);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
                wm.reparent_selected_mcp_node(true);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Down
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                wm.move_selected_mcp_item(false);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Up
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                wm.move_selected_mcp_item(true);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('J') => {
                wm.move_selected_mcp_item(false);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('K') => {
                wm.move_selected_mcp_item(true);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('>') => {
                wm.reparent_selected_mcp_node(true);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('<') | KeyCode::Char('u' | 'U') => {
                wm.reparent_selected_mcp_node(false);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('g' | 'G' | 'p' | 'P') => {
                let target = match wm.mcp_console().selected_action() {
                    Some(crate::mcp_console::McpRowAction::SessionNode(n)) => n,
                    _ => wm.config().history_session.clone(),
                };
                wm.mcp_console_mut().close_root_terminal();
                wm.reflow_default_viewport();
                wm.active_pane_mut().prompt_mut().manual_raw_override = false;
                wm.active_pane_mut().prompt_mut().force_command_prompt = true;
                wm.active_pane_mut()
                    .prompt_mut()
                    .set_text(&format!(":mcp graft {target} "));
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Left | KeyCode::Char('h') => {
                wm.mcp_console_mut().scroll_left(8);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Right | KeyCode::Char('l') => {
                wm.mcp_console_mut().scroll_right(8);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                let mode = wm.mcp_console_mut().toggle_layout_mode();
                wm.reflow_default_viewport();
                wm.active_pane_mut().set_status_message(Some(format!(
                    "MCP Window Mode: {} (Press 'd' to cycle dock edge)",
                    mode.label()
                )));
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                let edge = wm.mcp_console_mut().cycle_edge();
                wm.mcp_console_mut().layout_mode = crate::wm::DrawerLayoutMode::Push;
                wm.reflow_default_viewport();
                wm.active_pane_mut().set_status_message(Some(format!(
                    "MCP Window Docked (Push): {}",
                    edge.label()
                )));
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                wm.mcp_console_mut().select_prev_row();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                wm.mcp_console_mut().select_next_row();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Enter => {
                wm.activate_selected_mcp_row();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                wm.refresh_mcp_console_view();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                wm.toggle_freeze_selected_mcp_node();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('n' | 'N' | 'b' | 'B') => {
                wm.spawn_branch_under_selected_mcp_node();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('c' | '+') => {
                if let Ok(id) = wm.split_focused(crate::wm::split_tree::SplitDirection::Horizontal)
                {
                    wm.refresh_mcp_console_view();
                    wm.active_pane_mut()
                        .set_status_message(Some(format!("MCP: Created new window #{id}")));
                }
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('v' | '|') => {
                if let Ok(id) = wm.split_focused(crate::wm::split_tree::SplitDirection::Vertical) {
                    wm.refresh_mcp_console_view();
                    wm.active_pane_mut().set_status_message(Some(format!(
                        "MCP: Created new vertical window #{id}"
                    )));
                }
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::PageUp => {
                wm.mcp_console_mut().scroll_up(5);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::PageDown => {
                wm.mcp_console_mut().scroll_down(5);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Home => {
                wm.mcp_console_mut().select_first_row();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::End => {
                wm.mcp_console_mut().select_last_row();
                return Ok(Some(AppAction::Handled));
            }
            _ => {}
        }
    }
    Ok(None)
}

pub(super) fn handle_active_modal_overlay_key(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
    match wm.modal_mut() {
        Modal::None => {
            if !wm.is_leader_pending() {
                if key.modifiers == KeyModifiers::CONTROL
                    && matches!(key.code, KeyCode::Char('d') | KeyCode::Char('D'))
                    && wm.intercept_ctrl_d_if_needed()
                {
                    return Ok(Some(if wm.is_client_detached() {
                        AppAction::Quit
                    } else {
                        AppAction::Handled
                    }));
                }
                if key.code == KeyCode::Enter
                    && key.modifiers.is_empty()
                    && wm.active_pane().effective_routing_mode()
                        == crate::pty::line_discipline::InputRoutingMode::CommandPrompt
                    && !wm.active_pane().prompt().fuzzy_search.active
                    && !wm.active_pane().copy_mode().active
                    && !wm.active_pane().hints_mode().active
                    && wm.active_pane().pending_unsafe_paste().is_none()
                {
                    let text = wm.active_pane().prompt().text();
                    if wm.intercept_prompt_exit_if_needed(&text) {
                        let _ = wm.active_pane_mut().prompt_mut().take_submission();
                        return Ok(Some(if wm.is_client_detached() {
                            AppAction::Quit
                        } else {
                            AppAction::Handled
                        }));
                    }
                }
            }
            Ok(None)
        }
        Modal::ExitGuard(_) => match key.code {
            KeyCode::Left | KeyCode::BackTab => {
                if let Some(st) = wm.exit_guard_state_mut() {
                    st.select_prev();
                }
                Ok(Some(AppAction::Handled))
            }
            KeyCode::Right | KeyCode::Tab => {
                if let Some(st) = wm.exit_guard_state_mut() {
                    st.select_next();
                }
                Ok(Some(AppAction::Handled))
            }
            KeyCode::Enter => {
                let choice = wm
                    .exit_guard_state()
                    .map(|s| s.selected_choice())
                    .unwrap_or(crate::pane::ExitGuardChoice::RespawnShell);
                if wm.apply_exit_guard_choice(choice) {
                    Ok(Some(AppAction::Quit))
                } else {
                    Ok(Some(AppAction::Handled))
                }
            }
            _ => {
                if let Some(choice) =
                    crate::pane::ExitGuardChoice::from_key(key.code, key.modifiers)
                {
                    if wm.apply_exit_guard_choice(choice) {
                        Ok(Some(AppAction::Quit))
                    } else {
                        Ok(Some(AppAction::Handled))
                    }
                } else {
                    Ok(Some(AppAction::Handled))
                }
            }
        },
        Modal::Help { scroll } => {
            match key.code {
                KeyCode::F(1) | KeyCode::Esc | KeyCode::Char('q') => wm.close_modal(),
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(8),
                KeyCode::PageDown => *scroll = scroll.saturating_add(8),
                KeyCode::Home => *scroll = 0,
                KeyCode::End => *scroll = 500,
                _ => {}
            }
            Ok(Some(AppAction::Handled))
        }
        Modal::Inspector { scroll } => {
            match key.code {
                KeyCode::F(9) | KeyCode::Esc | KeyCode::Char('q') => wm.close_modal(),
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(6),
                KeyCode::PageDown => *scroll = scroll.saturating_add(6),
                _ => {}
            }
            Ok(Some(AppAction::Handled))
        }
        Modal::Palette { .. } => {
            match key.code {
                KeyCode::Char('p') | KeyCode::Char('P')
                    if key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    let q = wm
                        .palette_state()
                        .map(|(q, _)| q.to_string())
                        .unwrap_or_default();
                    wm.close_modal();
                    wm.command_window_mut().pinned = true;
                    if !q.is_empty() {
                        wm.command_window_mut().query = q;
                    }
                    wm.open_command_window();
                }
                KeyCode::Char('*') => {
                    let q = wm
                        .palette_state()
                        .map(|(q, _)| q.to_string())
                        .unwrap_or_default();
                    wm.close_modal();
                    wm.command_window_mut().pinned = true;
                    if !q.is_empty() {
                        wm.command_window_mut().query = q;
                    }
                    wm.open_command_window();
                }
                KeyCode::Esc => {
                    wm.close_modal();
                }
                KeyCode::Up => {
                    if let Modal::Palette { selected, .. } = wm.modal_mut() {
                        *selected = selected.saturating_sub(1);
                    }
                }
                KeyCode::Down => {
                    let count = wm.filtered_palette_items().len();
                    if count > 0
                        && let Modal::Palette { selected, .. } = wm.modal_mut()
                    {
                        *selected = (*selected + 1).min(count - 1);
                    }
                }
                KeyCode::Backspace => {
                    if let Modal::Palette { query, selected } = wm.modal_mut() {
                        query.pop();
                        *selected = 0;
                    }
                }
                KeyCode::Enter => {
                    let (raw_query, selected_idx) = wm
                        .palette_state()
                        .map(|(q, s)| (q.trim().to_string(), s))
                        .unwrap_or_default();
                    let items = wm.filtered_palette_items();
                    let sel = items
                        .get(selected_idx)
                        .or_else(|| items.first())
                        .map(|&(cmd, _, _)| cmd.to_string());
                    let candidate = if raw_query.starts_with(':')
                        && (sel.is_none() || selected_idx == 0)
                        && parse_colon_command(&raw_query).is_some_and(|r| r.is_ok())
                    {
                        Some(raw_query)
                    } else if sel.is_none() && !raw_query.is_empty() {
                        Some(format!(":{raw_query}"))
                    } else {
                        sel
                    };
                    wm.close_modal();
                    if let Some(cmd_str) = candidate {
                        if cmd_str.ends_with(' ') {
                            wm.drawer_mut().focused = false;
                            let p = wm.active_pane_mut().prompt_mut();
                            p.manual_raw_override = false;
                            p.force_command_prompt = true;
                            p.set_text(&cmd_str);
                            match cmd_str.trim() {
                                ":rename" | ":session rename" => {
                                    wm.open_session_rename_dialog(None);
                                }
                                ":window rename" | ":pane rename" | ":title" => {
                                    wm.open_window_rename_dialog(None);
                                }
                                _ => {}
                            }
                        } else {
                            match parse_colon_command(&cmd_str) {
                                Some(Ok(cmd)) => {
                                    if execute_wm_colon_command(wm, cmd) {
                                        return Ok(Some(AppAction::Quit));
                                    }
                                }
                                Some(Err(err_msg)) => {
                                    wm.drawer_mut().focused = false;
                                    let p = wm.active_pane_mut().prompt_mut();
                                    p.manual_raw_override = false;
                                    p.force_command_prompt = true;
                                    p.set_text(&cmd_str);
                                    wm.active_pane_mut().set_status_message(Some(err_msg));
                                }
                                None => {}
                            }
                        }
                    }
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Modal::Palette { query, selected } = wm.modal_mut() {
                        query.push(c);
                        *selected = 0;
                    }
                }
                _ => {}
            }
            Ok(Some(AppAction::Handled))
        }
        Modal::Find { .. } => crate::input::find_modal::handle_find_modal_key(wm, key),
        Modal::Menubar { category, item } => {
            let (cat, sel) = (*category, *item);
            match key.code {
                KeyCode::Esc => wm.close_modal(),
                KeyCode::Left => {
                    *category = (cat + 6) % 7;
                    *item = 0;
                }
                KeyCode::Right => {
                    *category = (cat + 1) % 7;
                    *item = 0;
                }
                KeyCode::Up => *item = sel.saturating_sub(1),
                KeyCode::Down => {
                    let max_idx =
                        crate::ui::modals::menubar_category_item_count(cat).saturating_sub(1);
                    *item = (sel + 1).min(max_idx);
                }
                KeyCode::Enter => return execute_menubar_selection(wm, cat, sel).map(Some),
                _ => {}
            }
            Ok(Some(AppAction::Handled))
        }
    }
}
