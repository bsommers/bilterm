//! Drawer navigation operations (`FLT-01`, Phase 50 D-04/D-05).
//!
//! Every user-triggerable drawer navigation (leader chord, colon command, RPC) lowers to
//! [`DrawerNavOp`] inside [`crate::action::Action::DrawerNav`] and runs through
//! [`WindowManager::apply_drawer_nav`]. Later Phase 50 plans extend the enum.

use super::{CommsIndicator, DrawerRow, DrawerRowKind, DrawerStyle};
use crate::action::Action;
use crate::pane::PaneId;
use crate::wm::{SessionNodeKind, SplitDirection, WindowManager};
use crate::wm::agent_state::AgentState;
use crate::wm::agents_window::NodeKey;

/// One drawer navigation operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawerNavOp {
    /// Focus the next pane that needs attention (blocked, then errored, then done).
    NextAttention,
    /// Fold or unfold the selected session (a pane row folds its enclosing session).
    Fold,
    /// Focus the selected row (the keyboard twin of a click).
    Activate,
    /// Move the selection to the next row (wraps).
    Next,
    /// Move the selection to the previous row (wraps).
    Prev,
    /// Select a row by 0-based index (clamped); the operation a mouse click applies.
    Row(usize),
    /// Move the selection by `n` rows (clamped).
    Scroll(i16),
    /// Grow (`n > 0`) or shrink (`n < 0`) the drawer along its docking axis (clamped).
    Widen(i16),
    /// Create a new session and switch to it.
    NewSession,
    /// Split the focused pane into a new window.
    NewPane,
    /// Open the command prompt prefilled with `:drawer rename `.
    RenamePrompt,
    /// Rename the selected session.
    Rename(String),
    /// Close the selected pane.
    Close,
    /// Unfold every workspace and session; the selection stays on the same row.
    ExpandAll,
    /// Fold every workspace and session down to workspace rows (on an already fully folded tree it
    /// unfolds everything again, so the key toggles); the selection moves to its workspace row.
    CollapseAll,
    /// Add a window to the selected session (or the named one) and focus it. With a pane row selected
    /// that pane is the split target.
    AddWindow(Option<String>),
    /// Split the selected pane in the given direction and focus the new pane.
    AddPane(SplitDirection),
    /// Open the right-click context menu at a screen cell (`None`: at the selected row).
    MenuOpen(Option<(u16, u16)>),
    /// Dismiss the context menu.
    MenuClose,
    /// Run entry `n` (0-based, see [`super::menu::NavigatorMenuEntry::ALL`]) on the selected row and
    /// close the menu.
    MenuPick(usize),
    /// Open the agent traffic window on the selected pane row's conversation (its comms glyph).
    OpenComms,
    /// Give (`true`) or take away (`false`) keyboard focus of the selected row's comms glyph.
    CommsFocus(bool),
}

/// Attention tiers in priority order.
const ATTENTION_TIERS: [AgentState; 3] = [AgentState::Blocked, AgentState::Errored, AgentState::Done];

impl WindowManager {
    /// Applies one drawer navigation op; returns `true` when it changed focus or state.
    pub fn apply_drawer_nav(&mut self, op: DrawerNavOp) -> bool {
        let changed = self.apply_drawer_nav_op(op);
        // Comms focus follows the selection: once the selection leaves its row the focus is dropped.
        if !self.navigator_comms_focused() {
            self.drawer.navigator.comms_focus = None;
        }
        changed
    }

    fn apply_drawer_nav_op(&mut self, op: DrawerNavOp) -> bool {
        match op {
            DrawerNavOp::NextAttention => self.jump_next_attention().is_some(),
            DrawerNavOp::Fold => self.fold_selected_navigator_row(),
            DrawerNavOp::Activate => {
                self.drawer_activate_selected(false);
                true
            }
            DrawerNavOp::Next => {
                self.drawer_select_next();
                true
            }
            DrawerNavOp::Prev => {
                self.drawer_select_prev();
                true
            }
            DrawerNavOp::Row(i) => {
                let len = self.drawer_rows().len();
                self.drawer.selected_index = i.min(len.saturating_sub(1));
                len > 0
            }
            DrawerNavOp::Scroll(n) => {
                self.drawer_scroll_by(isize::from(n));
                true
            }
            DrawerNavOp::Widen(n) => {
                let cfg = &self.config.drawer;
                let cur = if cfg.edge.is_vertical() { cfg.effective_width_cols() } else { cfg.height_rows };
                self.resize_drawer(i32::from(cur).saturating_add(i32::from(n)).clamp(0, 1000) as u16);
                true
            }
            DrawerNavOp::NewSession => self.drawer_new_session(),
            DrawerNavOp::NewPane => self.drawer_new_pane(SplitDirection::Horizontal),
            DrawerNavOp::RenamePrompt => {
                self.drawer.focused = false;
                let prompt = self.active_pane_mut().prompt_mut();
                prompt.manual_raw_override = false;
                prompt.force_command_prompt = true;
                prompt.set_text(":drawer rename ");
                true
            }
            DrawerNavOp::Rename(name) => self.drawer_rename_selected(&name),
            DrawerNavOp::Close => self.drawer_close_selected_pane(),
            DrawerNavOp::ExpandAll => self.expand_all_navigator_rows(),
            DrawerNavOp::CollapseAll => self.collapse_all_navigator_rows(),
            DrawerNavOp::AddWindow(session) => self.drawer_add_in_selected(SplitDirection::Horizontal, session),
            DrawerNavOp::AddPane(dir) => self.drawer_add_in_selected(dir, None),
            op @ (DrawerNavOp::MenuOpen(_) | DrawerNavOp::MenuClose | DrawerNavOp::MenuPick(_)) => {
                self.apply_navigator_menu(op)
            }
            DrawerNavOp::OpenComms => self.drawer_open_comms(),
            DrawerNavOp::CommsFocus(on) => self.drawer_comms_focus(on),
        }
    }

    /// Opens the traffic window on the selected pane row: a pairing focuses the edge and filters the
    /// conversation to the pair, a fan-out selects the pane and filters to it, a quiet pane just selects it.
    fn drawer_open_comms(&mut self) -> bool {
        let rows = self.drawer_rows();
        let Some(row) = rows.get(self.drawer.selected_index) else { return false };
        let DrawerRowKind::Pane(id) = row.kind else { return false };
        let pane_key = NodeKey::pane(id);
        match row.navigator.as_ref().and_then(|m| m.comms.clone()) {
            Some(CommsIndicator::Pair { partner_key, .. }) => self.open_agents_window_on_edge(&partner_key, &pane_key),
            Some(CommsIndicator::FanOut { .. }) => {
                self.open_agents_window(Some(id));
                self.apply_agent_filter(&pane_key);
            }
            None => self.open_agents_window(Some(id)),
        }
        self.drawer.navigator.comms_focus = None;
        true
    }

    /// Sets comms focus on the selected row when it shows a comms glyph, or clears it.
    fn drawer_comms_focus(&mut self, on: bool) -> bool {
        if !on {
            return self.drawer.navigator.comms_focus.take().is_some();
        }
        let sel = self.drawer.selected_index;
        let has = self.drawer_rows().get(sel).and_then(|r| r.navigator.as_ref()).is_some_and(|m| m.comms.is_some());
        self.drawer.navigator.comms_focus = has.then_some(sel);
        has
    }

    /// Splits the selected pane (or, on a session row, the top pane of that session) in `dir`.
    ///
    /// A pane row makes that pane the split target; a session other than the active one is switched to
    /// first, so the new window never lands in the session that merely holds the keyboard focus.
    fn drawer_add_in_selected(&mut self, dir: SplitDirection, session: Option<String>) -> bool {
        let pane = if session.is_none() { self.selected_drawer_pane_id() } else { None };
        let target = session.unwrap_or_else(|| self.selected_drawer_session_name());
        if target != self.config.history_session {
            let known = self.daemon_state.session_tree.node(&target).is_some() || self.list_sessions().contains(&target);
            if !known || self.switch_or_create_session(&target).is_err() {
                self.active_pane_mut().set_status_message(Some(format!("No such session '{target}'")));
                return false;
            }
        }
        if let Some(id) = pane {
            self.focus_and_raise(id);
        }
        self.drawer_new_pane(dir)
    }

    /// Session name the selected row belongs to (its own name, or the enclosing session of a pane row).
    fn selected_navigator_session(&self, rows: &[DrawerRow]) -> Option<String> {
        let sel = self.drawer.selected_index;
        match rows.get(sel)?.kind {
            DrawerRowKind::SessionHeader(ref name) => Some(name.clone()),
            DrawerRowKind::Pane(_) => rows[..sel].iter().rev().find_map(|r| match &r.kind {
                DrawerRowKind::SessionHeader(name) => Some(name.clone()),
                _ => None,
            }),
            _ => None,
        }
    }

    /// Name of the workspace (session-tree root) containing `session`.
    fn navigator_workspace_of(&self, session: &str) -> String {
        let tree = &self.daemon_state.session_tree;
        let mut cur = session;
        for _ in 0..16 {
            match tree.node(cur).and_then(|n| n.parent.as_deref()).filter(|p| tree.node(p).is_some()) {
                Some(parent) => cur = parent,
                None => break,
            }
        }
        cur.to_string()
    }

    /// Unfolds every workspace and session and keeps the selection on the same logical row.
    fn expand_all_navigator_rows(&mut self) -> bool {
        let anchor = self.drawer_rows().get(self.drawer.selected_index).map(|r| r.kind.clone());
        for node in &mut self.daemon_state.session_tree.nodes {
            node.collapsed = false;
        }
        self.reselect_navigator_row(anchor.as_ref());
        true
    }

    /// Folds every workspace and session; a fully folded tree unfolds instead (toggle).
    fn collapse_all_navigator_rows(&mut self) -> bool {
        let rows = self.drawer_rows();
        let fully_collapsed =
            rows.iter().all(|r| r.navigator.as_ref().is_none_or(|m| !m.has_children || m.collapsed));
        if fully_collapsed {
            return self.expand_all_navigator_rows();
        }
        let root = self.selected_navigator_session(&rows).map(|s| self.navigator_workspace_of(&s));
        let active = self.config.history_session.clone();
        if self.daemon_state.session_tree.node(&active).is_none() {
            self.daemon_state.session_tree.upsert_session(&active, SessionNodeKind::Independent, None);
        }
        for node in &mut self.daemon_state.session_tree.nodes {
            node.collapsed = true;
        }
        self.reselect_navigator_row(root.map(DrawerRowKind::SessionHeader).as_ref());
        true
    }

    /// Selects the row of `kind` when it is still listed, else clamps the selection.
    fn reselect_navigator_row(&mut self, kind: Option<&DrawerRowKind>) {
        if let Some(pos) = kind.and_then(|k| self.drawer_rows().iter().position(|r| &r.kind == k)) {
            self.drawer.selected_index = pos;
        }
        self.clamp_drawer_selection();
    }

    /// Creates a new session, switches to it and selects its active pane (classic `N`).
    fn drawer_new_session(&mut self) -> bool {
        let Ok(name) = self.create_new_session(None, false) else { return false };
        self.sync_drawer_selection_to_active_pane();
        self.active_pane_mut().set_status_message(Some(format!("Created new session '{name}'")));
        true
    }

    /// Splits the focused pane in `dir` and selects the new pane (classic `n`).
    fn drawer_new_pane(&mut self, dir: SplitDirection) -> bool {
        let Ok(id) = self.split_focused(dir) else { return false };
        self.sync_drawer_selection_to_active_pane();
        let kind = if dir == SplitDirection::Vertical { "vertical window" } else { "window" };
        self.active_pane_mut().set_status_message(Some(format!("Created new {kind} #{id}")));
        true
    }

    /// Renames the selected session through the existing `rename` session-tree command (T-50-17).
    fn drawer_rename_selected(&mut self, name: &str) -> bool {
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            self.active_pane_mut().set_status_message(Some("Usage: :drawer rename <name>".to_string()));
            return false;
        }
        let target = self.selected_drawer_session_name();
        self.apply(Action::SessionTreeCommand(format!("rename {target} {name}"))).is_ok()
    }

    /// Folds the selected session row; a pane (or other nested) row folds its enclosing session row and
    /// moves the selection onto it. Returns `false` when the selection has nothing to fold.
    fn fold_selected_navigator_row(&mut self) -> bool {
        let rows = self.drawer_rows();
        let Some(sel) = Some(self.drawer.selected_index).filter(|&i| i < rows.len()) else {
            return false;
        };
        let target = match rows[sel].kind {
            DrawerRowKind::SessionHeader(_) => Some(sel),
            DrawerRowKind::Pane(_) => {
                let depth = rows[sel].navigator.as_ref().map_or(0, |m| m.depth);
                (0..sel).rev().find(|&i| {
                    matches!(rows[i].kind, DrawerRowKind::SessionHeader(_))
                        && rows[i].navigator.as_ref().is_some_and(|m| m.depth < depth)
                })
            }
            _ => None,
        };
        let Some(idx) = target else { return false };
        let DrawerRowKind::SessionHeader(name) = rows[idx].kind.clone() else { return false };
        if rows[idx].navigator.as_ref().is_some_and(|m| !m.has_children) {
            return false;
        }
        // The active session may not have a tree node yet (it is synthesised for the row walk).
        if self.daemon_state.session_tree.node(&name).is_none() {
            self.daemon_state.session_tree.upsert_session(&name, SessionNodeKind::Independent, None);
        }
        self.daemon_state.session_tree.toggle_fold(&name);
        self.drawer.selected_index = idx;
        self.clamp_drawer_selection();
        true
    }

    /// Keeps `selected_index` inside the current row list.
    pub(crate) fn clamp_drawer_selection(&mut self) {
        let len = self.drawer_rows().len();
        if self.drawer.selected_index >= len {
            self.drawer.selected_index = len.saturating_sub(1);
        }
    }

    /// Focuses the next pane needing attention and returns its id (`None` plus a status message
    /// when no pane needs attention).
    ///
    /// Picks the highest non-empty tier (Blocked > Errored > Done); inside the tier the search
    /// cycles in [`WindowManager::ordered_pane_ids`] order starting after the active pane and wraps.
    pub fn jump_next_attention(&mut self) -> Option<PaneId> {
        let ids = self.ordered_pane_ids();
        let active = self.active_pane_id();
        let start = ids.iter().position(|&id| id == active).map_or(0, |i| i + 1);
        let target = ATTENTION_TIERS.iter().find_map(|&tier| {
            (0..ids.len())
                .map(|off| ids[(start + off) % ids.len()])
                .find(|&id| self.agent_state(id) == Some(tier))
                .map(|id| (id, tier))
        });
        let Some((id, tier)) = target else {
            self.active_pane_mut().set_status_message(Some("No pane needs attention".to_string()));
            return None;
        };
        self.focus_and_raise(id);
        self.sync_drawer_selection_to_active_pane();
        self.active_pane_mut().set_status_message(Some(format!("Attention: #{id} {}", tier.as_str())));
        Some(id)
    }

    /// Sets (`Some`) or toggles (`None`) the drawer style, reflows the workspace and returns the new style.
    pub fn set_drawer_style(&mut self, style: Option<DrawerStyle>) -> DrawerStyle {
        let new = style.unwrap_or_else(|| self.config.drawer.style.toggle());
        self.config.drawer.style = new;
        let len = self.drawer_rows().len();
        if self.drawer.selected_index >= len {
            self.drawer.selected_index = len.saturating_sub(1);
        }
        self.reflow_default_viewport();
        self.active_pane_mut().set_status_message(Some(format!("Drawer style: {}", new.label())));
        new
    }
}
