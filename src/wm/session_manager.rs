//! Session creation, naming/renaming, switching, deletion, and list reordering (`src/wm/session_manager.rs`).

use std::collections::BTreeMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::{SavedWorkspaceSession, WindowManager, WorkspaceLayoutMode};
use crate::error::Result;
use crate::pane::CompositePane;
use crate::pty::PtyConfig;
use crate::wm::layout_profile::sanitize_profile_name;
use crate::wm::session_tree::SessionNodeKind;
use crate::wm::split_tree::SplitTree;
use crate::wm::z_stack::ZStack;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PersistedSessionOrder {
    pub order: Vec<String>,
}

impl WindowManager {
    fn should_persist_sessions_to_disk(&self) -> bool {
        !self.headless_mode || self.config.session_dir.is_some()
    }

    /// Loads the user-defined session order from `<session_dir>/session_order.toml` merged with `config.session_order`.
    pub fn load_session_order(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if let Ok(dir) = self.resolve_session_dir() {
            let p = dir.join("session_order.toml");
            if let Ok(raw) = std::fs::read_to_string(p)
                && let Ok(parsed) = toml::from_str::<PersistedSessionOrder>(&raw)
            {
                for item in parsed.order {
                    let clean = sanitize_profile_name(&item);
                    if !out.contains(&clean) {
                        out.push(clean);
                    }
                }
            }
        }
        for item in &self.config.session_order {
            let clean = sanitize_profile_name(item);
            if !out.contains(&clean) {
                out.push(clean);
            }
        }
        out
    }

    /// Saves the user-defined session order to `config.session_order` and `<session_dir>/session_order.toml`.
    pub fn save_session_order(&mut self, order: Vec<String>) {
        let mut dedup = Vec::with_capacity(order.len());
        for item in order {
            let clean = sanitize_profile_name(&item);
            if !dedup.contains(&clean) {
                dedup.push(clean);
            }
        }
        self.config.session_order = dedup.clone();
        if self.should_persist_sessions_to_disk()
            && let Ok(dir) = self.resolve_session_dir()
        {
            let _ = std::fs::create_dir_all(&dir);
            let payload = PersistedSessionOrder { order: dedup };
            if let Ok(toml_str) = toml::to_string_pretty(&payload) {
                let _ = std::fs::write(dir.join("session_order.toml"), toml_str);
            }
        }
    }

    /// Lists all saved workspace session names found in `<session_dir>` (and in-memory session snapshots),
    /// ordered by the user's custom `session_order` first and then alphabetically (`:session list`, `ECO-02`).
    pub fn list_sessions(&self) -> Vec<String> {
        let dir = match self.resolve_session_dir() {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = %e, "session dir unavailable");
                return Vec::new();
            }
        };
        if self.config.session_dir.is_some() && std::fs::read_dir(&dir).is_err() {
            return Vec::new();
        }
        let mut names: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("toml")
                    && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
                    && !matches!(stem, "layout_profile_meta" | "session_order")
                {
                    let owned = stem.to_owned();
                    if !names.contains(&owned) {
                        names.push(owned);
                    }
                }
            }
        }
        for mem_name in self.layout_profiles_mem.keys() {
            if !matches!(mem_name.as_str(), "layout_profile_meta" | "session_order")
                && !names.contains(mem_name)
            {
                names.push(mem_name.clone());
            }
        }
        let order = self.load_session_order();
        names.sort_by(|a, b| {
            let pos_a = order.iter().position(|x| x == a);
            let pos_b = order.iter().position(|x| x == b);
            match (pos_a, pos_b) {
                (Some(ia), Some(ib)) => ia.cmp(&ib),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.cmp(b),
            }
        });
        names
    }

    /// Returns all sessions including the currently active `history_session`, ordered by `session_order`.
    pub fn ordered_sessions_with_active(&self) -> Vec<String> {
        let mut list = self.list_sessions();
        let active = sanitize_profile_name(&self.config.history_session);
        if !list.contains(&active) {
            let order = self.load_session_order();
            if order.contains(&active) {
                list.push(active);
                list.sort_by(|a, b| {
                    let pos_a = order.iter().position(|x| x == a);
                    let pos_b = order.iter().position(|x| x == b);
                    match (pos_a, pos_b) {
                        (Some(ia), Some(ib)) => ia.cmp(&ib),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => a.cmp(b),
                    }
                });
            } else {
                list.insert(0, active);
            }
        }
        list
    }

    /// Generates the next available default session name (`session-2`, `session-3`, ...).
    pub fn generate_next_session_name(&self) -> String {
        let existing = self.ordered_sessions_with_active();
        let mut idx = 2usize;
        loop {
            let candidate = format!("session-{idx}");
            if !existing.contains(&candidate) {
                return candidate;
            }
            idx += 1;
        }
    }

    /// Resets the workspace to a clean 1-pane session named `name` (or auto-generated if `None`).
    /// When `blank` is `true`, clears initial command history and scrollback without saving the outgoing session.
    pub fn create_new_session(&mut self, name_opt: Option<&str>, blank: bool) -> Result<String> {
        let outgoing = sanitize_profile_name(&self.config.history_session);
        if !blank && self.config.layout_autosave {
            let snap = self.snapshot_session(&outgoing);
            self.layout_profiles_mem.insert(outgoing.clone(), snap);
            if self.should_persist_sessions_to_disk() {
                let _ = self.save_session(&outgoing);
            }
        }

        let new_name = match name_opt.filter(|s| !s.trim().is_empty()) {
            Some(raw) => sanitize_profile_name(raw),
            None if blank => "blank".to_string(),
            None => self.generate_next_session_name(),
        };

        let (rows, cols) = self.default_size;
        let mut initial_pane = if self.headless_mode {
            CompositePane::new_headless(1, rows, cols)
        } else {
            CompositePane::spawn(
                1,
                PtyConfig {
                    rows,
                    cols,
                    ..Default::default()
                },
            )
            .unwrap_or_else(|_| CompositePane::new_headless(1, rows, cols))
        };
        initial_pane.set_scroll_mode_from_config(self.config.scroll_mode);

        let mut new_panes = BTreeMap::new();
        new_panes.insert(1, initial_pane);
        self.panes = new_panes;
        self.split_tree = SplitTree::new(1);
        self.z_stack = ZStack::new(1);
        self.layout_mode = WorkspaceLayoutMode::Tiled;
        self.zoomed_pane = None;
        self.next_pane_id = 2;
        self.client_detached = false;

        self.config.history_session = new_name.clone();
        self.config.active_layout_profile = new_name.clone();

        if blank {
            self.active_pane_mut().prompt_mut().history.clear();
            let _ = self.active_pane_mut().vt_mut().clear_scrollback();
        } else {
            self.sync_history_config();
            let snap = self.snapshot_session(&new_name);
            self.layout_profiles_mem.insert(new_name.clone(), snap);
            if self.should_persist_sessions_to_disk() {
                let _ = self.save_session(&new_name);
            }
            let mut order = self.ordered_sessions_with_active();
            if !order.contains(&new_name) {
                order.push(new_name.clone());
            }
            self.save_session_order(order);
        }

        self.daemon_state
            .session_tree
            .upsert_session(&new_name, SessionNodeKind::Rooted, None);
        self.daemon_state.session_panes.insert(new_name.clone(), 1);
        self.reflow_default_viewport();
        self.rebalance_scrollback(Instant::now());
        Ok(new_name)
    }

    /// Renames `old_name_opt` (or the active session when `None`) to `new_raw`, preserving its slot in `session_order`.
    pub fn rename_session(
        &mut self,
        old_name_opt: Option<&str>,
        new_raw: &str,
    ) -> std::result::Result<(String, String), String> {
        if new_raw.trim().is_empty() {
            return Err("Usage: :session rename [old_name] <new_name>".to_string());
        }
        let old_name = old_name_opt
            .filter(|s| !s.trim().is_empty())
            .map(sanitize_profile_name)
            .unwrap_or_else(|| sanitize_profile_name(&self.config.history_session));
        let new_name = sanitize_profile_name(new_raw);
        if old_name == new_name {
            return Ok((old_name, new_name));
        }

        let mut order = self.ordered_sessions_with_active();
        if let Some(pos) = order.iter().position(|x| x == &old_name) {
            order[pos] = new_name.clone();
        } else {
            order.push(new_name.clone());
        }

        if let Some(mut snap) = self.layout_profiles_mem.remove(&old_name) {
            snap.name = new_name.clone();
            self.layout_profiles_mem.insert(new_name.clone(), snap);
        }

        if let Ok(dir) = self.resolve_session_dir() {
            let old_path = dir.join(format!("{old_name}.toml"));
            let new_path = dir.join(format!("{new_name}.toml"));
            if old_path.is_file() {
                if let Ok(raw) = std::fs::read_to_string(&old_path)
                    && let Ok(mut saved) = toml::from_str::<SavedWorkspaceSession>(&raw)
                {
                    saved.name = new_name.clone();
                    if let Ok(toml_str) = toml::to_string_pretty(&saved) {
                        let _ = std::fs::write(&new_path, toml_str);
                    }
                } else {
                    let _ = std::fs::rename(&old_path, &new_path);
                }
                let _ = std::fs::remove_file(&old_path);
            }
        }

        if sanitize_profile_name(&self.config.history_session) == old_name {
            self.config.history_session = new_name.clone();
            if sanitize_profile_name(&self.config.active_layout_profile) == old_name {
                self.config.active_layout_profile = new_name.clone();
            }
            if sanitize_profile_name(&self.config.default_layout_profile) == old_name {
                self.config.default_layout_profile = new_name.clone();
            }
            self.sync_history_config();
            let snap = self.snapshot_session(&new_name);
            self.layout_profiles_mem.insert(new_name.clone(), snap);
            if self.should_persist_sessions_to_disk() {
                let _ = self.save_session(&new_name);
            }
        }

        if let Some(cnt) = self.daemon_state.session_panes.remove(&old_name) {
            self.daemon_state.session_panes.insert(new_name.clone(), cnt);
        }
        self.daemon_state
            .session_tree
            .upsert_session(&new_name, SessionNodeKind::Rooted, None);
        self.save_session_order(order);
        Ok((old_name, new_name))
    }

    /// Switches to an existing session `name`, or creates a new session named `name` if it does not exist yet.
    pub fn switch_or_create_session(&mut self, name: &str) -> Result<(String, usize, bool)> {
        let target = sanitize_profile_name(name);
        let exists_on_disk = self
            .resolve_session_dir()
            .map(|d| d.join(format!("{target}.toml")).is_file())
            .unwrap_or(false);
        let exists_in_mem = self.layout_profiles_mem.contains_key(&target);
        if exists_on_disk || exists_in_mem {
            let outgoing = sanitize_profile_name(&self.config.history_session);
            if self.config.layout_autosave && outgoing != target {
                let snap = self.snapshot_session(&outgoing);
                self.layout_profiles_mem.insert(outgoing.clone(), snap);
                if self.should_persist_sessions_to_disk() {
                    let _ = self.save_session(&outgoing);
                }
            }
            let count = if exists_on_disk {
                self.restore_session(&target)?
            } else {
                self.switch_layout_profile(&target)?
            };
            self.config.history_session = target.clone();
            self.config.active_layout_profile = target.clone();
            self.sync_history_config();
            self.reflow_default_viewport();
            Ok((target, count, false))
        } else {
            let created = self.create_new_session(Some(&target), false)?;
            Ok((created, 1, true))
        }
    }

    /// Moves `session_opt` (or the active session) to `target_pos` (`up`, `down`, `top`, `bottom`, or 1-based index).
    pub fn move_session_in_order(
        &mut self,
        session_opt: Option<&str>,
        target_pos: &str,
    ) -> std::result::Result<Vec<String>, String> {
        let mut order = self.ordered_sessions_with_active();
        if order.is_empty() {
            return Err("No sessions available to reorder".to_string());
        }
        let target = session_opt
            .filter(|s| !s.trim().is_empty())
            .map(sanitize_profile_name)
            .unwrap_or_else(|| sanitize_profile_name(&self.config.history_session));
        if !order.contains(&target) {
            order.push(target.clone());
        }
        let cur_idx = order.iter().position(|x| x == &target).unwrap_or(0);
        let len = order.len();
        let new_idx = match target_pos.trim().to_ascii_lowercase().as_str() {
            "up" | "-1" | "prev" | "previous" | "k" => cur_idx.saturating_sub(1),
            "down" | "+1" | "next" | "j" => (cur_idx + 1).min(len.saturating_sub(1)),
            "top" | "first" | "start" => 0,
            "bottom" | "last" | "end" => len.saturating_sub(1),
            num_str => match num_str.parse::<usize>() {
                Ok(n) if n >= 1 => (n - 1).min(len.saturating_sub(1)),
                _ => {
                    return Err(
                        "Usage: :session move [name] <up|down|top|bottom|1..N>".to_string(),
                    );
                }
            },
        };
        let item = order.remove(cur_idx);
        order.insert(new_idx, item);
        self.save_session_order(order.clone());
        Ok(order)
    }

    /// Reorders the session list so `names` appear first in the given order, followed by any remaining sessions.
    pub fn reorder_sessions(&mut self, names: &[&str]) -> Vec<String> {
        let existing = self.ordered_sessions_with_active();
        let mut new_order: Vec<String> = Vec::with_capacity(existing.len().max(names.len()));
        for &raw in names {
            let clean = sanitize_profile_name(raw);
            if !new_order.contains(&clean) {
                new_order.push(clean);
            }
        }
        for s in existing {
            if !new_order.contains(&s) {
                new_order.push(s);
            }
        }
        self.save_session_order(new_order.clone());
        new_order
    }

    /// Deletes a saved session from disk and removes it from `session_order`.
    pub fn delete_session(&mut self, name: &str) -> std::result::Result<String, String> {
        let target = sanitize_profile_name(name);
        if target == sanitize_profile_name(&self.config.history_session) {
            return Err(format!(
                "Cannot delete currently active session '{target}' (switch or create another session first)"
            ));
        }
        let mut removed = self.layout_profiles_mem.remove(&target).is_some();
        if let Ok(dir) = self.resolve_session_dir() {
            let p = dir.join(format!("{target}.toml"));
            if p.is_file() && std::fs::remove_file(p).is_ok() {
                removed = true;
            }
        }
        self.daemon_state.session_panes.remove(&target);
        let mut order = self.load_session_order();
        if let Some(pos) = order.iter().position(|x| x == &target) {
            order.remove(pos);
            self.save_session_order(order);
            removed = true;
        }
        if removed {
            Ok(target)
        } else {
            Err(format!("Session '{target}' not found"))
        }
    }

    /// Handles `:session new|blank|rename|switch|move|reorder|delete` subcommands.
    pub(super) fn execute_session_management_command(&mut self, subcmd: &str) -> bool {
        let parts: Vec<&str> = subcmd.split_whitespace().collect();
        let Some(&verb) = parts.first() else {
            return false;
        };
        match verb {
            "new" | "create" => {
                let blank = matches!(parts.get(1).copied(), Some("--blank" | "-B" | "blank"));
                let name_arg = if blank {
                    parts.get(2).copied()
                } else {
                    parts.get(1).copied()
                };
                let msg = match self.create_new_session(name_arg, blank) {
                    Ok(name) => format!(
                        "Created and switched to {}session '{name}' (1 pane)",
                        if blank { "blank " } else { "new " }
                    ),
                    Err(e) => format!("Session create error: {e}"),
                };
                self.active_pane_mut().set_status_message(Some(msg));
                true
            }
            "blank" | "clean" => {
                let name_arg = parts.get(1).copied();
                let msg = match self.create_new_session(name_arg, true) {
                    Ok(name) => format!("Started blank session '{name}' (clean 1-pane workspace)"),
                    Err(e) => format!("Blank session error: {e}"),
                };
                self.active_pane_mut().set_status_message(Some(msg));
                true
            }
            "rename" | "name" => {
                let res = if parts.len() >= 3 {
                    self.rename_session(parts.get(1).copied(), parts[2])
                } else if let Some(&new_n) = parts.get(1) {
                    self.rename_session(None, new_n)
                } else {
                    Err("Usage: :session rename [old_name] <new_name>".to_string())
                };
                let msg = match res {
                    Ok((old_n, new_n)) => format!("Renamed session '{old_n}' -> '{new_n}'"),
                    Err(e) => {
                        self.record_automation_error_msg(e.clone());
                        e
                    }
                };
                self.active_pane_mut().set_status_message(Some(msg));
                true
            }
            "switch" | "open" => {
                let Some(&name) = parts.get(1) else {
                    let msg = "Usage: :session switch <name>".to_string();
                    self.record_automation_error_msg(msg.clone());
                    self.active_pane_mut().set_status_message(Some(msg));
                    return true;
                };
                let msg = match self.switch_or_create_session(name) {
                    Ok((sess, count, created)) => {
                        if created {
                            format!("Created and switched to new session '{sess}' ({count} pane)")
                        } else {
                            format!("Switched to session '{sess}' ({count} pane(s))")
                        }
                    }
                    Err(e) => format!("Session switch error: {e}"),
                };
                self.active_pane_mut().set_status_message(Some(msg));
                true
            }
            "move" => {
                let res = if parts.len() >= 3 {
                    self.move_session_in_order(parts.get(1).copied(), parts[2])
                } else if let Some(&dir) = parts.get(1) {
                    self.move_session_in_order(None, dir)
                } else {
                    Err("Usage: :session move [name] <up|down|top|bottom|1..N>".to_string())
                };
                let msg = match res {
                    Ok(order) => format!("Session order: {}", order.join(", ")),
                    Err(e) => {
                        self.record_automation_error_msg(e.clone());
                        e
                    }
                };
                self.active_pane_mut().set_status_message(Some(msg));
                true
            }
            "up" | "down" | "top" | "bottom" => {
                let sess_opt = parts.get(1).copied();
                let msg = match self.move_session_in_order(sess_opt, verb) {
                    Ok(order) => format!("Session order: {}", order.join(", ")),
                    Err(e) => e,
                };
                self.active_pane_mut().set_status_message(Some(msg));
                true
            }
            "reorder" | "order" => {
                if parts.len() < 2 {
                    let msg = "Usage: :session reorder <session1> <session2> ...".to_string();
                    self.record_automation_error_msg(msg.clone());
                    self.active_pane_mut().set_status_message(Some(msg));
                    return true;
                }
                let order = self.reorder_sessions(&parts[1..]);
                self.active_pane_mut()
                    .set_status_message(Some(format!("Session order: {}", order.join(", "))));
                true
            }
            "swap" => {
                let (Some(&a), Some(&b)) = (parts.get(1), parts.get(2)) else {
                    let msg = "Usage: :session swap <session1> <session2>".to_string();
                    self.record_automation_error_msg(msg.clone());
                    self.active_pane_mut().set_status_message(Some(msg));
                    return true;
                };
                let mut order = self.ordered_sessions_with_active();
                let clean_a = sanitize_profile_name(a);
                let clean_b = sanitize_profile_name(b);
                if !order.contains(&clean_a) {
                    order.push(clean_a.clone());
                }
                if !order.contains(&clean_b) {
                    order.push(clean_b.clone());
                }
                if let (Some(ia), Some(ib)) = (
                    order.iter().position(|x| x == &clean_a),
                    order.iter().position(|x| x == &clean_b),
                ) {
                    order.swap(ia, ib);
                    self.save_session_order(order.clone());
                }
                self.active_pane_mut()
                    .set_status_message(Some(format!("Session order: {}", order.join(", "))));
                true
            }
            "delete" | "rm" | "remove" => {
                let Some(&name) = parts.get(1) else {
                    let msg = "Usage: :session delete <name>".to_string();
                    self.record_automation_error_msg(msg.clone());
                    self.active_pane_mut().set_status_message(Some(msg));
                    return true;
                };
                let msg = match self.delete_session(name) {
                    Ok(del) => format!("Deleted session '{del}'"),
                    Err(e) => {
                        self.record_automation_error_msg(e.clone());
                        e
                    }
                };
                self.active_pane_mut().set_status_message(Some(msg));
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colon_command::{execute_wm_colon_command, parse_colon_command};
    use crate::input::handle_wm_mouse_event;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    #[test]
    fn session_new_rename_and_reorder_preserves_custom_order() {
        let mut wm = WindowManager::new_headless(24, 80);
        let dir = std::env::temp_dir().join(format!(
            "bilterm_session_mgr_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        wm.config_mut().session_dir = Some(dir.to_string_lossy().into_owned());

        execute_wm_colon_command(&mut wm, parse_colon_command(":session new alpha").unwrap().unwrap());
        assert_eq!(wm.config().history_session, "alpha");

        execute_wm_colon_command(&mut wm, parse_colon_command(":session new beta").unwrap().unwrap());
        assert_eq!(wm.config().history_session, "beta");

        execute_wm_colon_command(&mut wm, parse_colon_command(":session blank gamma").unwrap().unwrap());
        assert_eq!(wm.config().history_session, "gamma");
        assert_eq!(wm.pane_count(), 1);

        execute_wm_colon_command(&mut wm, parse_colon_command(":session rename gamma delta").unwrap().unwrap());
        assert_eq!(wm.config().history_session, "delta");

        execute_wm_colon_command(&mut wm, parse_colon_command(":session reorder delta alpha beta default").unwrap().unwrap());
        let ordered = wm.list_sessions();
        assert_eq!(ordered, vec!["delta", "alpha", "beta", "default"]);

        execute_wm_colon_command(&mut wm, parse_colon_command(":session move delta bottom").unwrap().unwrap());
        let ordered2 = wm.list_sessions();
        assert_eq!(ordered2, vec!["alpha", "beta", "default", "delta"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn borland_menubar_help_and_modal_rows_are_clickable() {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.config_mut().show_top_menubar = true;
        let screen = Rect::new(0, 0, 80, 24);

        // " Help " sits at category 6 (columns 46..52)
        assert_eq!(crate::ui::modals::menubar_category_at_col(47), Some(6));

        // 1. Click " Help " on the top menu bar -> opens category 6 dropdown
        let click_help_bar = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 47,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        handle_wm_mouse_event(&mut wm, click_help_bar, screen).unwrap();
        assert_eq!(wm.menubar_open_category(), Some(6));

        // 2. Click item 0 ("Help Reference (F1)") inside the Help dropdown (row = workspace_area.y + 1 = 2)
        let dropdown = crate::ui::modals::menubar_dropdown_rect(6, Rect::new(0, 1, 80, 22));
        let click_dropdown_item0 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: dropdown.x + 2,
            row: dropdown.y + 1,
            modifiers: KeyModifiers::NONE,
        };
        handle_wm_mouse_event(&mut wm, click_dropdown_item0, screen).unwrap();
        assert!(wm.is_help_visible());

        // 3. Click row 2 inside the open Help modal ("Ctrl+X ? / F1 / :help") -> closes Help modal
        let modal_rect = wm.active_modal_rect(Rect::new(0, 1, 80, 22)).unwrap();
        let inner = crate::wm::inset_window_frame(modal_rect);
        let click_help_row = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: inner.x + 2,
            row: inner.y + 2,
            modifiers: KeyModifiers::NONE,
        };
        handle_wm_mouse_event(&mut wm, click_help_row, screen).unwrap();
        assert!(!wm.is_help_visible());
    }
}
