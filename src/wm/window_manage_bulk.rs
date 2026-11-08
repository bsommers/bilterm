//! Multi-window selection, atomic bulk operations, and single-step topology undo (`SES-03`, `SES-04`).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::Instant;

use crate::pane::{CompositePane, PaneId};
use crate::pty::PtyConfig;
use crate::wm::split_tree::{SplitDirection, SplitTree};
use crate::wm::{SavedWorkspaceSession, WindowManager, WorkspaceLayoutMode};

use super::{
    BackgroundSessionState, WindowBulkOp, WindowSelectTarget, WorkspaceTopologyUndo,
    disambiguate_name, validate_session_path, validate_window_name,
};

impl WindowManager {
    /// Updates window selection according to `target` (`SES-03`).
    pub fn select_window(&mut self, target: &WindowSelectTarget) -> Result<usize, String> {
        match target {
            WindowSelectTarget::Toggle(id) => {
                if !self.panes.contains_key(id) {
                    return Err(format!("Window {id} not found in active session"));
                }
                if self.window_manage.selected_windows.contains(id) {
                    self.window_manage.selected_windows.remove(id);
                } else {
                    self.window_manage.selected_windows.insert(*id);
                }
            }
            WindowSelectTarget::All => {
                for &id in self.panes.keys() {
                    self.window_manage.selected_windows.insert(id);
                }
            }
            WindowSelectTarget::None => {
                self.window_manage.selected_windows.clear();
            }
            WindowSelectTarget::Invert => {
                let mut inverted = BTreeSet::new();
                for &id in self.panes.keys() {
                    if !self.window_manage.selected_windows.contains(&id) {
                        inverted.insert(id);
                    }
                }
                self.window_manage.selected_windows = inverted;
            }
            WindowSelectTarget::Range { from, to } => {
                let order = self.z_stack.order();
                let pos_from = order.iter().position(|&x| x == *from);
                let pos_to = order.iter().position(|&x| x == *to);
                match (pos_from, pos_to) {
                    (Some(pf), Some(pt)) => {
                        let (start, end) = if pf <= pt { (pf, pt) } else { (pt, pf) };
                        for &id in &order[start..=end] {
                            self.window_manage.selected_windows.insert(id);
                        }
                    }
                    _ => {
                        let (min, max) = if from <= to { (*from, *to) } else { (*to, *from) };
                        for &id in self.panes.keys() {
                            if id >= min && id <= max {
                                self.window_manage.selected_windows.insert(id);
                            }
                        }
                    }
                }
            }
            WindowSelectTarget::Explicit(ids) => {
                self.window_manage.selected_windows.clear();
                for &id in ids {
                    if self.panes.contains_key(&id) {
                        self.window_manage.selected_windows.insert(id);
                    }
                }
            }
        }
        Ok(self.window_manage.selected_windows.len())
    }

    /// Executes an atomic all-or-nothing bulk operation on selected windows (`SES-03`).
    pub fn apply_bulk_op(&mut self, op: &WindowBulkOp) -> Result<String, String> {
        let selected: Vec<PaneId> = self
            .window_manage
            .selected_windows
            .iter()
            .copied()
            .collect();
        if selected.is_empty() {
            return Err("No windows selected for bulk operation".to_string());
        }

        // Pre-validate all selected windows exist in current session
        for &id in &selected {
            if !self.panes.contains_key(&id) {
                return Err(format!(
                    "Selected window {id} does not exist in active session"
                ));
            }
        }

        match op {
            WindowBulkOp::Move { target_session } => {
                let target_clean = target_session.trim();
                validate_session_path(target_clean)?;
                let cur_session = self.config.history_session.trim();
                if target_clean == cur_session {
                    return Err(format!(
                        "Cannot bulk move to current active session '{cur_session}'"
                    ));
                }
                if !self.session_exists(target_clean) {
                    return Err(format!("Target session '{target_clean}' not found"));
                }

                self.capture_topology_undo(
                    &format!(
                        "bulk move {} window(s) to '{target_clean}'",
                        selected.len()
                    ),
                    vec![cur_session.to_string(), target_clean.to_string()],
                );

                for &id in &selected {
                    let _ = self.move_window_to_session(Some(id), target_clean, false)?;
                }
                self.window_manage.selected_windows.clear();
                self.drawer.invalidate_saved_sessions();
                Ok(format!(
                    "Bulk moved {} window(s) to '{target_clean}'",
                    selected.len()
                ))
            }
            WindowBulkOp::Close => {
                // Strictly honor ExitProtectionPolicy (SAFE-01)
                if self.config.exit_protection == crate::pane::ExitProtectionPolicy::Guard {
                    for &id in &selected {
                        if let Some(pane) = self.panes.get(&id)
                            && let Some(cmd) = pane.resolve_running_command()
                        {
                            return Err(format!(
                                "Cannot bulk close window {id}: active process '{cmd}' is protected by exit guard (use --force to override)"
                            ));
                        }
                    }
                }

                self.capture_topology_undo(
                    &format!("bulk close {} window(s)", selected.len()),
                    vec![self.config.history_session.clone()],
                );

                for &id in &selected {
                    if self.panes.len() <= 1 {
                        let (rows, cols) = self.default_size;
                        let next_id = self.next_pane_id;
                        self.next_pane_id += 1;
                        let mut fresh = if self.headless_mode {
                            CompositePane::new_headless(next_id, rows, cols)
                        } else {
                            CompositePane::spawn(
                                next_id,
                                PtyConfig {
                                    rows,
                                    cols,
                                    ..Default::default()
                                },
                            )
                            .unwrap_or_else(|_| CompositePane::new_headless(next_id, rows, cols))
                        };
                        fresh.set_scroll_mode_from_config(self.config.scroll_mode);
                        self.panes.insert(next_id, fresh);
                        self.split_tree = SplitTree::new(next_id);
                        self.z_stack.push(next_id);
                    }
                    self.remove_pane(id);
                }
                self.window_manage.selected_windows.clear();
                self.drawer.invalidate_saved_sessions();
                Ok(format!("Bulk closed {} window(s)", selected.len()))
            }
            WindowBulkOp::ForceClose => {
                self.capture_topology_undo(
                    &format!("force bulk close {} window(s)", selected.len()),
                    vec![self.config.history_session.clone()],
                );

                for &id in &selected {
                    if self.panes.len() <= 1 {
                        let (rows, cols) = self.default_size;
                        let next_id = self.next_pane_id;
                        self.next_pane_id += 1;
                        let mut fresh = if self.headless_mode {
                            CompositePane::new_headless(next_id, rows, cols)
                        } else {
                            CompositePane::spawn(
                                next_id,
                                PtyConfig {
                                    rows,
                                    cols,
                                    ..Default::default()
                                },
                            )
                            .unwrap_or_else(|_| CompositePane::new_headless(next_id, rows, cols))
                        };
                        fresh.set_scroll_mode_from_config(self.config.scroll_mode);
                        self.panes.insert(next_id, fresh);
                        self.split_tree = SplitTree::new(next_id);
                        self.z_stack.push(next_id);
                    }
                    self.remove_pane(id);
                }
                self.window_manage.selected_windows.clear();
                self.drawer.invalidate_saved_sessions();
                Ok(format!("Force bulk closed {} window(s)", selected.len()))
            }
            WindowBulkOp::RenamePrefix { prefix } => {
                let clean_prefix = prefix.trim();
                validate_window_name(clean_prefix)
                    .map_err(|e| format!("Invalid prefix '{clean_prefix}': {e}"))?;

                let selected_set: HashSet<PaneId> = selected.iter().copied().collect();
                let mut assigned: HashSet<String> = self
                    .panes
                    .iter()
                    .filter(|(id, _)| !selected_set.contains(id))
                    .map(|(_, p)| p.title().to_string())
                    .collect();
                let mut planned: Vec<(PaneId, String)> = Vec::with_capacity(selected.len());
                for &id in &selected {
                    if let Some(pane) = self.panes.get(&id) {
                        let candidate = format!("{clean_prefix}{}", pane.title());
                        validate_window_name(&candidate)?;
                        let final_name = disambiguate_name(&candidate, &assigned);
                        validate_window_name(&final_name)?;
                        assigned.insert(final_name.clone());
                        planned.push((id, final_name));
                    }
                }

                self.capture_topology_undo(
                    &format!(
                        "bulk rename-prefix '{clean_prefix}' on {} window(s)",
                        selected.len()
                    ),
                    vec![self.config.history_session.clone()],
                );

                for (id, final_name) in planned {
                    if let Some(pane) = self.panes.get_mut(&id) {
                        pane.set_title(final_name);
                    }
                }
                self.drawer.invalidate_saved_sessions();
                Ok(format!(
                    "Prefixed {} window(s) with '{clean_prefix}'",
                    selected.len()
                ))
            }
            WindowBulkOp::Tile => {
                self.capture_topology_undo(
                    &format!("bulk tile {} window(s)", selected.len()),
                    vec![self.config.history_session.clone()],
                );

                if let Some(&first) = selected.first() {
                    let mut new_tree = SplitTree::new(first);
                    let mut prev = first;
                    for (idx, &id) in selected.iter().skip(1).enumerate() {
                        let dir = if idx % 2 == 0 {
                            SplitDirection::Vertical
                        } else {
                            SplitDirection::Horizontal
                        };
                        new_tree.split_leaf(prev, id, dir);
                        prev = id;
                    }
                    self.split_tree = new_tree;
                    self.layout_mode = WorkspaceLayoutMode::Tiled;
                    self.reflow_default_viewport();
                }
                Ok(format!("Bulk tiled {} window(s)", selected.len()))
            }
            WindowBulkOp::Pin(opt) => {
                self.capture_topology_undo(
                    &format!("bulk pin {} window(s)", selected.len()),
                    vec![self.config.history_session.clone()],
                );

                for &id in &selected {
                    if let Some(pane) = self.panes.get_mut(&id) {
                        let target = opt.unwrap_or(!pane.is_read_only());
                        pane.set_read_only(target);
                    }
                }
                Ok(format!(
                    "Updated pin status for {} window(s)",
                    selected.len()
                ))
            }
        }
    }

    /// Captures a snapshot of affected sessions and selection before a topology mutation (`SES-04`).
    pub fn capture_topology_undo(&mut self, desc: &str, affected_sessions: Vec<String>) {
        let mut snapshot_before = BTreeMap::new();
        let cur_session = self.config.history_session.clone();

        let active_snap = self.snapshot_session(&cur_session);
        snapshot_before.insert(cur_session.clone(), active_snap);

        for s in &affected_sessions {
            if s != &cur_session {
                if let Some(snap) = self.layout_profiles_mem.get(s) {
                    snapshot_before.insert(s.clone(), snap.clone());
                } else if let Some(bg) = self.window_manage.background_sessions.get(s) {
                    let panes_vec: Vec<crate::wm::SavedPaneState> = bg
                        .panes
                        .values()
                        .map(|p| {
                            let chrome = self.pane_chrome(p.id());
                            crate::wm::SavedPaneState {
                                id: p.id(),
                                title: p.title().to_string(),
                                cwd: p.resolve_cwd().to_string_lossy().into_owned(),
                                shell: Some(p.shell_program().to_string()),
                                shell_args: p.shell_args().to_vec(),
                                running_command: p.resolve_running_command(),
                                remote_target: p.remote_target().map(ToOwned::to_owned),
                                prompt_anchor: p.prompt().anchor,
                                prompt_height: p.prompt().height,
                                prompt_width_cols: p.prompt().width_cols,
                                prompt_x_offset: p.prompt().x_offset,
                                prompt_input: p.prompt().text(),
                                raw_passthrough: p.prompt().manual_raw_override,
                                font_scale_pct: p.font_options().scale_pct,
                                scroll_mode: Some(p.scroll_mode()),
                                scrollback_lines: Vec::new(),
                                read_only: p.is_read_only(),
                                history: p.prompt().history.clone(),
                                yank_history: Vec::new(),
                                custom_name: Some(p.title().to_string()),
                                theme: chrome.theme_name,
                                outline: chrome.outline,
                                bold: chrome.bold,
                                colors: chrome.colors,
                            }
                        })
                        .collect();
                    let snap = SavedWorkspaceSession {
                        name: s.clone(),
                        layout_mode: self.layout_mode,
                        split_tree: bg.split_tree.clone(),
                        z_order: bg.z_stack.order().to_vec(),
                        z_geometries: BTreeMap::new(),
                        zoomed_pane: bg.zoomed_pane,
                        detached: false,
                        panes: panes_vec,
                        selected_windows: bg.selected_windows.clone(),
                    };
                    snapshot_before.insert(s.clone(), snap);
                }
            }
        }

        let undo = WorkspaceTopologyUndo {
            description: desc.to_string(),
            timestamp: Instant::now(),
            active_session: cur_session,
            affected_sessions,
            snapshot_before,
            selection_before: self.window_manage.selected_windows.clone(),
        };

        self.window_manage.undo_stack.push(undo);
        if self.window_manage.undo_stack.len() > 20 {
            self.window_manage.undo_stack.remove(0);
        }
    }

    /// Reverts the last window or session management operation (`SES-04`).
    pub fn undo_topology(&mut self) -> Result<String, String> {
        let Some(entry) = self.window_manage.undo_stack.pop() else {
            return Err("No operations to undo".to_string());
        };

        for (sess_name, snap) in entry.snapshot_before {
            if sess_name == self.config.history_session {
                self.apply_saved_session_snapshot(&snap);
            } else {
                self.layout_profiles_mem.insert(sess_name.clone(), snap);
            }
        }

        self.window_manage.selected_windows = entry.selection_before;
        if entry.active_session != self.config.history_session {
            let _ = self.switch_to_session_preserving_panes(&entry.active_session);
        }

        self.drawer.invalidate_saved_sessions();
        Ok(format!("Undid: {}", entry.description))
    }

    /// Renames a session with path validation and topology undo (`SES-02`).
    pub fn rename_session_unified(
        &mut self,
        old_name: Option<&str>,
        new_name: &str,
    ) -> Result<(String, String), String> {
        let clean = new_name.trim();
        validate_session_path(clean)?;
        let cur_session = self.config.history_session.clone();
        let old = old_name.map(str::trim).unwrap_or(&cur_session);

        if old == clean {
            return Ok((old.to_string(), clean.to_string()));
        }

        self.capture_topology_undo(
            &format!("rename session '{old}' to '{clean}'"),
            vec![old.to_string(), clean.to_string()],
        );

        if let Some(bg) = self.window_manage.background_sessions.remove(old) {
            self.window_manage
                .background_sessions
                .insert(clean.to_string(), bg);
        }

        let res = self.rename_session(Some(old), clean);
        self.drawer.invalidate_saved_sessions();
        res
    }

    /// Switches sessions while preserving live PTY instances in [`BackgroundSessionState`] (`SES-01`).
    pub fn switch_to_session_preserving_panes(
        &mut self,
        target: &str,
    ) -> Result<usize, String> {
        let cur_session = self.config.history_session.clone();
        if cur_session == target {
            return Ok(self.panes.len());
        }

        let outgoing_panes = std::mem::take(&mut self.panes);
        let outgoing_bg = BackgroundSessionState {
            panes: outgoing_panes,
            split_tree: self.split_tree.clone(),
            z_stack: self.z_stack.clone(),
            zoomed_pane: self.zoomed_pane,
            selected_windows: self.window_manage.selected_windows.clone(),
        };
        self.window_manage
            .background_sessions
            .insert(cur_session.clone(), outgoing_bg);

        if let Some(bg) = self.window_manage.background_sessions.remove(target) {
            self.panes = bg.panes;
            self.split_tree = bg.split_tree;
            self.z_stack = bg.z_stack;
            self.zoomed_pane = bg.zoomed_pane;
            self.window_manage.selected_windows = bg.selected_windows;
            self.config.history_session = target.to_string();
            self.config.active_layout_profile = target.to_string();
            self.reflow_default_viewport();
            self.drawer.invalidate_saved_sessions();
            Ok(self.panes.len())
        } else {
            self.config.history_session = target.to_string();
            self.config.active_layout_profile = target.to_string();
            let count = if self
                .resolve_session_dir()
                .map(|d| d.join(format!("{target}.toml")).is_file())
                .unwrap_or(false)
            {
                self.restore_session(target).map_err(|e| e.to_string())?
            } else if self.layout_profiles_mem.contains_key(target) {
                self.switch_layout_profile(target)
                    .map_err(|e| e.to_string())?
            } else {
                let _ = self
                    .create_new_session(Some(target), false)
                    .map_err(|e| e.to_string())?;
                1
            };
            self.drawer.invalidate_saved_sessions();
            Ok(count)
        }
    }
}
