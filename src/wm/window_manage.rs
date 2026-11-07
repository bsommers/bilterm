//! Window and session management operations, types, and state (`SES-01..05`).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::pane::{CompositePane, PaneId};
use crate::pty::PtyConfig;
use crate::wm::journal::validate_safe_name;
use crate::wm::layout_profile::sanitize_profile_name;
use crate::wm::split_tree::{SplitDirection, SplitTree};
use crate::wm::z_stack::ZStack;
use crate::wm::{SavedWorkspaceSession, WindowManager, WorkspaceLayoutMode};

/// Core operations for window management (`SES-01..04`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowManageOp {
    /// Moves a window (default: active window) to target_session.
    Move {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window_id: Option<PaneId>,
        target_session: String,
        #[serde(default)]
        switch: bool,
    },
    /// Renames a window (default: active window).
    Rename {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window_id: Option<PaneId>,
        new_name: String,
    },
    /// Updates the multi-selection state for windows.
    Select(WindowSelectTarget),
    /// Executes an atomic bulk operation on currently selected windows.
    Bulk(WindowBulkOp),
    /// Reverts the last window/session topology mutation.
    Undo,
}

/// Core operations for session management (`SES-02`, `SES-04`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionManageOp {
    /// Renames a session (default: active session if old_name is None).
    Rename {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_name: Option<String>,
        new_name: String,
    },
    /// Moves session position in user-defined ordering.
    MoveOrder {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        target_pos: String,
    },
    /// Reverts the last session topology mutation.
    Undo,
}

/// Targets for window multi-selection (`SES-03`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowSelectTarget {
    /// Toggles selection of a specific window.
    Toggle(PaneId),
    /// Selects a contiguous range of windows between from and to in visual/z-order.
    Range { from: PaneId, to: PaneId },
    /// Selects all windows in the current session.
    All,
    /// Clears window selection.
    None,
    /// Inverts window selection in the current session.
    Invert,
    /// Sets selection explicitly to the given set of window IDs.
    Explicit(Vec<PaneId>),
}

/// Bulk operations executable on multi-selected windows (`SES-03`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowBulkOp {
    /// Moves all selected windows to target_session atomically.
    Move { target_session: String },
    /// Closes all selected windows, honoring ExitProtectionPolicy.
    Close,
    /// Force-closes all selected windows, bypassing ExitProtectionPolicy.
    ForceClose,
    /// Prepends prefix to all selected window names.
    RenamePrefix { prefix: String },
    /// Tiles all selected windows into the active workspace split layout.
    Tile,
    /// Sets or toggles pinned status for all selected windows.
    Pin(Option<bool>),
}

/// Alias for [`WindowSelectTarget`].
pub type SelectSpec = WindowSelectTarget;
/// Alias for [`WindowBulkOp`].
pub type BulkWindowOp = WindowBulkOp;
/// Alias for [`WorkspaceTopologyUndo`].
pub type TopologyUndoEntry = WorkspaceTopologyUndo;

/// Inline rename input state inside Navigator drawer (`SES-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineRenameState {
    pub pane_id: PaneId,
    pub original_name: String,
    pub current_input: String,
}

/// Live background session state holding intact [`CompositePane`]s (`SES-01`).
pub struct BackgroundSessionState {
    pub panes: BTreeMap<PaneId, CompositePane>,
    pub split_tree: SplitTree,
    pub z_stack: ZStack,
    pub zoomed_pane: Option<PaneId>,
    pub selected_windows: BTreeSet<PaneId>,
}

impl std::fmt::Debug for BackgroundSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundSessionState")
            .field("panes_count", &self.panes.len())
            .field("split_tree", &self.split_tree)
            .field("z_stack", &self.z_stack)
            .field("zoomed_pane", &self.zoomed_pane)
            .field("selected_windows", &self.selected_windows)
            .finish()
    }
}

/// Snapshot for single-step topology undo (`SES-04`).
#[derive(Clone, Debug)]
pub struct WorkspaceTopologyUndo {
    pub description: String,
    pub timestamp: Instant,
    pub active_session: String,
    pub affected_sessions: Vec<String>,
    pub snapshot_before: BTreeMap<String, SavedWorkspaceSession>,
    pub selection_before: BTreeSet<PaneId>,
}

/// Persistent window management state stored on `WindowManager` (`SES-01..05`).
#[derive(Default)]
pub struct WindowManageState {
    pub selected_windows: BTreeSet<PaneId>,
    pub undo_stack: Vec<WorkspaceTopologyUndo>,
    pub inline_rename: Option<InlineRenameState>,
    pub background_sessions: BTreeMap<String, BackgroundSessionState>,
}

impl std::fmt::Debug for WindowManageState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowManageState")
            .field("selected_windows", &self.selected_windows)
            .field("undo_stack_len", &self.undo_stack.len())
            .field("inline_rename", &self.inline_rename)
            .field("background_sessions_count", &self.background_sessions.len())
            .finish()
    }
}

/// Validates that a window name is non-empty, <= 64 chars, and passes `validate_safe_name`.
pub fn validate_window_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Window name cannot be empty".to_string());
    }
    if trimmed.len() > 64 {
        return Err(format!(
            "Window name too long ({} chars, max 64)",
            trimmed.len()
        ));
    }
    validate_safe_name(trimmed).map_err(|e| format!("Invalid window name: {e}"))
}

/// Validates a session path (`a` or `a/b/c`), ensuring each segment passes `validate_safe_name`.
pub fn validate_session_path(path: &str) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Session path cannot be empty".to_string());
    }
    for segment in trimmed.split('/') {
        if segment.is_empty() {
            return Err("Session path contains empty segment".to_string());
        }
        if segment.len() > 64 {
            return Err(format!(
                "Session segment '{segment}' exceeds 64 characters"
            ));
        }
        validate_safe_name(segment)
            .map_err(|e| format!("Invalid session segment '{segment}': {e}"))?;
    }
    Ok(())
}

/// Generates a deterministic disambiguated name if `base` collides with any name in `existing`.
pub fn disambiguate_name(base: &str, existing: &HashSet<String>) -> String {
    if !existing.contains(base) {
        return base.to_string();
    }
    let mut counter = 2;
    loop {
        let candidate = format!("{base}-{counter}");
        if !existing.contains(&candidate) {
            return candidate;
        }
        counter += 1;
    }
}

impl WindowManager {
    /// Returns a reference to the window management state.
    pub fn window_manage(&self) -> &WindowManageState {
        &self.window_manage
    }

    /// Returns a mutable reference to the window management state.
    pub fn window_manage_mut(&mut self) -> &mut WindowManageState {
        &mut self.window_manage
    }

    /// Returns the active set of selected window IDs.
    pub fn selected_windows(&self) -> &BTreeSet<PaneId> {
        &self.window_manage.selected_windows
    }

    /// Checks if a window is selected.
    pub fn is_window_selected(&self, pane_id: PaneId) -> bool {
        self.window_manage.selected_windows.contains(&pane_id)
    }

    /// Clears the window selection.
    pub fn clear_window_selection(&mut self) {
        self.window_manage.selected_windows.clear();
    }

    /// Checks whether a session exists in memory, in background, in tree, or on disk (`SES-01`).
    pub fn session_exists(&self, name: &str) -> bool {
        let clean = sanitize_profile_name(name);
        let cur = sanitize_profile_name(&self.config.history_session);
        clean == cur
            || name.trim() == self.config.history_session.trim()
            || self.layout_profiles_mem.contains_key(name)
            || self.layout_profiles_mem.contains_key(&clean)
            || self.window_manage.background_sessions.contains_key(name)
            || self.window_manage.background_sessions.contains_key(&clean)
            || self.daemon_state.session_tree.node(name).is_some()
            || self.daemon_state.session_tree.node(&clean).is_some()
            || self
                .resolve_session_dir()
                .map(|d| d.join(format!("{clean}.toml")).is_file())
                .unwrap_or(false)
    }

    /// Dispatches a window management operation through `WindowManager`.
    pub fn apply_window_manage(&mut self, op: &WindowManageOp) -> Result<String, String> {
        self.window_manage_apply(op)
    }

    /// Dispatches a session management operation through `WindowManager`.
    pub fn apply_session_manage(&mut self, op: &SessionManageOp) -> Result<String, String> {
        self.session_manage_apply(op)
    }

    /// Internal handler for window management operations (`SES-01..04`).
    pub fn window_manage_apply(&mut self, op: &WindowManageOp) -> Result<String, String> {
        match op {
            WindowManageOp::Move {
                window_id,
                target_session,
                switch,
            } => self.move_window_to_session(*window_id, target_session, *switch),
            WindowManageOp::Rename {
                window_id,
                new_name,
            } => self.rename_window(*window_id, new_name),
            WindowManageOp::Select(spec) => {
                let count = self.select_window(spec)?;
                Ok(format!("Selected {count} window(s)"))
            }
            WindowManageOp::Bulk(bulk_op) => self.apply_bulk_op(bulk_op),
            WindowManageOp::Undo => self.undo_topology(),
        }
    }

    /// Internal handler for session management operations (`SES-02`, `SES-04`).
    pub fn session_manage_apply(&mut self, op: &SessionManageOp) -> Result<String, String> {
        match op {
            SessionManageOp::Rename { old_name, new_name } => {
                if new_name.trim().is_empty() {
                    return Err("Usage: :session rename [old_name] <new_name>".to_string());
                }
                self.rename_session_unified(old_name.as_deref(), new_name)
                    .map(|(old, new)| format!("Renamed session '{old}' -> '{new}'"))
            }
            SessionManageOp::MoveOrder { name, target_pos } => self
                .move_session_in_order(name.as_deref(), target_pos)
                .map(|order| format!("Session order: {}", order.join(", "))),
            SessionManageOp::Undo => self.undo_topology(),
        }
    }

    /// Moves a window (default: active window) to `target_session` intact (`SES-01`).
    pub fn move_window_to_session(
        &mut self,
        window_id: Option<PaneId>,
        target_session: &str,
        switch: bool,
    ) -> Result<String, String> {
        let target_clean = target_session.trim();
        validate_session_path(target_clean)?;
        let cur_session = self.config.history_session.trim();
        let target_id = window_id.unwrap_or_else(|| self.active_pane_id());

        if target_clean == cur_session {
            return Err(format!(
                "Window {target_id} is already in session '{cur_session}'"
            ));
        }

        if !self.session_exists(target_clean) {
            return Err(format!("Target session '{target_clean}' not found"));
        }

        if !self.panes.contains_key(&target_id) {
            return Err(format!("Window {target_id} not found in active session"));
        }

        // Capture single-step undo snapshot before mutating
        self.capture_topology_undo(
            &format!("move window {target_id} to session '{target_clean}'"),
            vec![cur_session.to_string(), target_clean.to_string()],
        );

        // Ensure source session is never left empty
        if self.panes.len() <= 1 {
            let next_id = self.next_pane_id;
            self.next_pane_id += 1;
            let (rows, cols) = self.default_size;
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

        // Detach live CompositePane from active session
        let pane = self.panes.remove(&target_id).expect("pane exists");
        let _ = self.split_tree.remove_leaf(target_id);
        self.z_stack.remove(target_id);
        if self.zoomed_pane == Some(target_id) {
            self.zoomed_pane = None;
        }
        self.window_manage.selected_windows.remove(&target_id);
        self.reflow_default_viewport();

        // Attach live CompositePane into target background session
        let bg = self
            .window_manage
            .background_sessions
            .entry(target_clean.to_string())
            .or_insert_with(|| {
                let snap = self.layout_profiles_mem.get(target_clean);
                let split_tree = snap
                    .map(|s| s.split_tree.clone())
                    .unwrap_or_else(|| SplitTree::new(target_id));
                let z_stack = snap
                    .map(|s| {
                        ZStack::from_order_and_geometries(s.z_order.clone(), BTreeMap::new())
                    })
                    .unwrap_or_else(|| ZStack::new(target_id));
                BackgroundSessionState {
                    panes: BTreeMap::new(),
                    split_tree,
                    z_stack,
                    zoomed_pane: None,
                    selected_windows: BTreeSet::new(),
                }
            });

        if !bg.panes.is_empty() {
            let top_id = bg.z_stack.top().unwrap_or(target_id);
            bg.split_tree.split_leaf(
                top_id,
                target_id,
                SplitDirection::Horizontal,
            );
            bg.z_stack.push(target_id);
        } else {
            bg.split_tree = SplitTree::new(target_id);
            bg.z_stack = ZStack::new(target_id);
        }
        bg.panes.insert(target_id, pane);

        if switch {
            self.switch_to_session_preserving_panes(target_clean)?;
        }

        self.drawer.invalidate_saved_sessions();
        Ok(format!(
            "Moved window {target_id} to session '{target_clean}'"
        ))
    }

    /// Renames a window with duplicate detection and deterministic disambiguation (`SES-02`).
    pub fn rename_window(
        &mut self,
        window_id: Option<PaneId>,
        new_name: &str,
    ) -> Result<String, String> {
        let clean = new_name.trim();
        validate_window_name(clean)?;
        let target_id = window_id.unwrap_or_else(|| self.active_pane_id());
        let cur_session = self.config.history_session.clone();

        // Check duplicate collision in active session and deterministically disambiguate
        let mut existing = HashSet::new();
        for (&id, p) in &self.panes {
            if id != target_id {
                existing.insert(p.title().to_string());
            }
        }
        let final_name = disambiguate_name(clean, &existing);

        self.capture_topology_undo(
            &format!("rename window {target_id} to '{final_name}'"),
            vec![cur_session],
        );

        let Some(pane) = self.panes.get_mut(&target_id) else {
            return Err(format!("Window {target_id} not found"));
        };
        pane.set_title(final_name.clone());
        self.drawer.invalidate_saved_sessions();
        Ok(format!("Renamed window {target_id} to '{final_name}'"))
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

    /// Updates window selection according to `spec` (`SES-03`).
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
                        let (min, max) =
                            if from <= to { (*from, *to) } else { (*to, *from) };
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
        let selected: Vec<PaneId> =
            self.window_manage.selected_windows.iter().copied().collect();
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
                            .unwrap_or_else(|_| {
                                CompositePane::new_headless(next_id, rows, cols)
                            })
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
                            .unwrap_or_else(|_| {
                                CompositePane::new_headless(next_id, rows, cols)
                            })
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
                validate_safe_name(clean_prefix).map_err(|e| {
                    format!("Invalid prefix '{clean_prefix}': {e}")
                })?;

                self.capture_topology_undo(
                    &format!(
                        "bulk rename-prefix '{clean_prefix}' on {} window(s)",
                        selected.len()
                    ),
                    vec![self.config.history_session.clone()],
                );

                for &id in &selected {
                    if let Some(pane) = self.panes.get_mut(&id) {
                        let original = pane.title().to_string();
                        pane.set_title(format!("{clean_prefix}{original}"));
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

    /// Switches sessions while preserving live PTY instances in [`BackgroundSessionState`] (`SES-01`).
    pub fn switch_to_session_preserving_panes(
        &mut self,
        target: &str,
    ) -> Result<usize, String> {
        let cur_session = self.config.history_session.clone();
        if cur_session == target {
            return Ok(self.panes.len());
        }

        // Stash active session into background_sessions
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

        // Retrieve target from background_sessions if present
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
