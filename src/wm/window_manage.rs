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
use crate::wm::{SavedWorkspaceSession, WindowManager};

#[path = "window_manage_bulk.rs"]
mod bulk;

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
    pub rename_dialog: Option<crate::wm::rename_dialog::RenameDialogState>,
    pub context_menu_anchor: Option<(u16, u16)>,
    pub background_sessions: BTreeMap<String, BackgroundSessionState>,
}

impl std::fmt::Debug for WindowManageState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowManageState")
            .field("selected_windows", &self.selected_windows)
            .field("undo_stack_len", &self.undo_stack.len())
            .field("inline_rename", &self.inline_rename)
            .field("rename_dialog", &self.rename_dialog)
            .field("context_menu_anchor", &self.context_menu_anchor)
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

    /// Opens the right-click context menu anchored at `(col, row)` across any UI surface.
    pub fn open_context_menu_at(&mut self, col: u16, row: u16) {
        self.window_manage.context_menu_anchor = Some((col, row));
        let _ = self.apply_drawer_nav(crate::wm::drawer::DrawerNavOp::MenuOpen(Some((col, row))));
    }

    /// Closes any open right-click context menu.
    pub fn close_context_menu(&mut self) {
        self.window_manage.context_menu_anchor = None;
        let _ = self.apply_drawer_nav(crate::wm::drawer::DrawerNavOp::MenuClose);
    }

    /// Returns the screen anchor `(col, row)` of an open context menu, if any.
    pub fn context_menu_anchor(&self) -> Option<(u16, u16)> {
        if !self.drawer.navigator_menu_open() {
            return None;
        }
        self.navigator_menu()
            .and_then(|m| m.anchor())
            .or(self.window_manage.context_menu_anchor)
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
            } => {
                if target_session.trim().is_empty() {
                    self.open_window_move_dialog(*window_id, *switch);
                    return Ok("Move window dialog opened".to_string());
                }
                self.window_manage.rename_dialog = None;
                self.window_manage.inline_rename = None;
                self.move_window_to_session(*window_id, target_session, *switch)
            }
            WindowManageOp::Rename {
                window_id,
                new_name,
            } => {
                if new_name.trim().is_empty() {
                    self.open_window_rename_dialog(*window_id);
                    return Ok("Rename window dialog opened".to_string());
                }
                self.window_manage.rename_dialog = None;
                self.window_manage.inline_rename = None;
                self.rename_window(*window_id, new_name)
            }
            WindowManageOp::Select(spec) => {
                let count = self.select_window(spec)?;
                Ok(format!("Selected {count} window(s)"))
            }
            WindowManageOp::Bulk(bulk_op) => {
                match bulk_op {
                    WindowBulkOp::Move { target_session } if target_session.trim().is_empty() => {
                        self.open_bulk_move_dialog();
                        return Ok("Bulk move dialog opened".to_string());
                    }
                    WindowBulkOp::RenamePrefix { prefix } if prefix.trim().is_empty() => {
                        self.open_bulk_rename_prefix_dialog();
                        return Ok("Bulk rename-prefix dialog opened".to_string());
                    }
                    _ => {}
                }
                self.window_manage.rename_dialog = None;
                self.window_manage.inline_rename = None;
                self.apply_bulk_op(bulk_op)
            }
            WindowManageOp::Undo => {
                self.window_manage.rename_dialog = None;
                self.window_manage.inline_rename = None;
                self.undo_topology()
            }
        }
    }

    /// Internal handler for session management operations (`SES-02`, `SES-04`).
    pub fn session_manage_apply(&mut self, op: &SessionManageOp) -> Result<String, String> {
        match op {
            SessionManageOp::Rename { old_name, new_name } => {
                if new_name.trim().is_empty() {
                    self.open_session_rename_dialog(old_name.clone());
                    return Ok("Rename session dialog opened".to_string());
                }
                self.window_manage.rename_dialog = None;
                self.window_manage.inline_rename = None;
                self.rename_session_unified(old_name.as_deref(), new_name)
                    .map(|(old, new)| format!("Renamed session '{old}' -> '{new}'"))
            }
            SessionManageOp::MoveOrder { name, target_pos } => self
                .move_session_in_order(name.as_deref(), target_pos)
                .map(|order| format!("Session order: {}", order.join(", "))),
            SessionManageOp::Undo => {
                self.window_manage.rename_dialog = None;
                self.window_manage.inline_rename = None;
                self.undo_topology()
            }
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

    /// Loads a `SavedWorkspaceSession` from in-memory cache or disk if available.
    pub(crate) fn load_session_snapshot_any(&self, session: &str) -> Option<SavedWorkspaceSession> {
        let clean = sanitize_profile_name(session);
        if let Some(snap) = self
            .layout_profiles_mem
            .get(session)
            .or_else(|| self.layout_profiles_mem.get(&clean))
        {
            return Some(snap.clone());
        }
        let dir = self.resolve_session_dir().ok()?;
        let path = dir.join(format!("{clean}.toml"));
        let raw = std::fs::read_to_string(path).ok()?;
        toml::from_str::<SavedWorkspaceSession>(&raw).ok()
    }

    fn persist_modified_session_snapshot(&mut self, session: &str, snap: SavedWorkspaceSession) {
        let clean = sanitize_profile_name(session);
        if let Ok(dir) = self.resolve_session_dir() {
            let path = dir.join(format!("{clean}.toml"));
            if path.is_file()
                && let Ok(toml_str) = toml::to_string_pretty(&snap)
            {
                let _ = std::fs::write(path, toml_str);
            }
        }
        self.layout_profiles_mem.insert(clean, snap);
    }

    /// Renames a window with duplicate detection and deterministic disambiguation (`SES-02`).
    pub fn rename_window(
        &mut self,
        window_id: Option<PaneId>,
        new_name: &str,
    ) -> Result<String, String> {
        let rows = self.drawer_rows();
        let drawer_pane = self.selected_drawer_pane_id();
        let hint = if window_id.is_none() || window_id == drawer_pane {
            self.selected_navigator_session(&rows)
        } else {
            None
        };
        self.rename_window_in_session(window_id, hint.as_deref(), new_name)
    }

    /// Renames a window in `session_hint` (or across active, background, and saved sessions) (`SES-02`).
    pub fn rename_window_in_session(
        &mut self,
        window_id: Option<PaneId>,
        session_hint: Option<&str>,
        new_name: &str,
    ) -> Result<String, String> {
        let clean = new_name.trim();
        validate_window_name(clean)?;
        let target_id = window_id
            .or_else(|| self.selected_drawer_pane_id())
            .unwrap_or_else(|| self.active_pane_id());
        let cur_session = self.config.history_session.clone();

        // 1. Active session when hint matches (or is None) and pane is in active panes
        if session_hint.is_none_or(|s| s == cur_session) && self.panes.contains_key(&target_id) {
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
            if let Some(pane) = self.panes.get_mut(&target_id) {
                pane.set_title(final_name.clone());
            }
            self.drawer.invalidate_saved_sessions();
            return Ok(format!("Renamed window {target_id} to '{final_name}'"));
        }

        // 2. Live background sessions
        let bg_key = session_hint
            .filter(|&s| {
                self.window_manage
                    .background_sessions
                    .get(s)
                    .is_some_and(|bg| bg.panes.contains_key(&target_id))
            })
            .map(ToOwned::to_owned)
            .or_else(|| {
                self.window_manage
                    .background_sessions
                    .iter()
                    .find_map(|(k, bg)| bg.panes.contains_key(&target_id).then(|| k.clone()))
            });

        if let Some(sess_name) = bg_key {
            let existing: HashSet<String> = self
                .window_manage
                .background_sessions
                .get(&sess_name)
                .map(|bg| {
                    bg.panes
                        .iter()
                        .filter(|&(&id, _)| id != target_id)
                        .map(|(_, p)| p.title().to_string())
                        .collect()
                })
                .unwrap_or_default();
            let final_name = disambiguate_name(clean, &existing);
            self.capture_topology_undo(
                &format!("rename window {target_id} to '{final_name}'"),
                vec![sess_name.clone()],
            );
            if let Some(bg) = self.window_manage.background_sessions.get_mut(&sess_name)
                && let Some(pane) = bg.panes.get_mut(&target_id)
            {
                pane.set_title(final_name.clone());
            }
            if let Some(mut snap) = self.load_session_snapshot_any(&sess_name) {
                if let Some(sp) = snap.panes.iter_mut().find(|p| p.id == target_id) {
                    sp.title = final_name.clone();
                    sp.custom_name = Some(final_name.clone());
                }
                self.persist_modified_session_snapshot(&sess_name, snap);
            }
            self.drawer.invalidate_saved_sessions();
            return Ok(format!("Renamed window {target_id} to '{final_name}'"));
        }

        // 3. Saved / cached sessions
        let mut candidate_sessions = Vec::new();
        if let Some(s) = session_hint {
            candidate_sessions.push(s.to_string());
        }
        for s in self.list_sessions() {
            if !candidate_sessions.contains(&s) {
                candidate_sessions.push(s);
            }
        }
        for sess in candidate_sessions {
            if let Some(mut snap) = self.load_session_snapshot_any(&sess)
                && snap.panes.iter().any(|p| p.id == target_id)
            {
                let existing: HashSet<String> = snap
                    .panes
                    .iter()
                    .filter(|p| p.id != target_id)
                    .map(|p| {
                        p.custom_name
                            .clone()
                            .unwrap_or_else(|| p.title.clone())
                    })
                    .collect();
                let final_name = disambiguate_name(clean, &existing);
                self.capture_topology_undo(
                    &format!("rename window {target_id} to '{final_name}'"),
                    vec![sess.clone()],
                );
                if let Some(sp) = snap.panes.iter_mut().find(|p| p.id == target_id) {
                    sp.title = final_name.clone();
                    sp.custom_name = Some(final_name.clone());
                }
                self.persist_modified_session_snapshot(&sess, snap);
                self.drawer.invalidate_saved_sessions();
                return Ok(format!("Renamed window {target_id} to '{final_name}'"));
            }
        }

        // 4. Fallback to active session if target_id exists in self.panes
        if self.panes.contains_key(&target_id) {
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
            if let Some(pane) = self.panes.get_mut(&target_id) {
                pane.set_title(final_name.clone());
            }
            self.drawer.invalidate_saved_sessions();
            return Ok(format!("Renamed window {target_id} to '{final_name}'"));
        }

        Err(format!("Window {target_id} not found"))
    }
}
