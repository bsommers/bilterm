//! Slide-out workspace drawer operations, selection, session tree helpers, and `Action::Drawer*` dispatch (`DRW-01`, `DRW-02`).

use ratatui::layout::Rect;

use super::{
    DrawerEdge, DrawerLayoutMode, DrawerRow, DrawerRowKind, DrawerVisibilityMode,
};
use crate::pane::PaneId;
use crate::remote::FleetNodeKind;
use crate::wm::WindowManager;

impl WindowManager {
    pub fn reflow_default_viewport(&mut self) {
        let area = Rect::new(0, 0, self.default_size.1, self.default_size.0);
        let _ = self.resize_all(area);
    }

    pub fn toggle_drawer(&mut self) {
        if self.is_drawer_effectively_visible()
            && (self.drawer.open
                || self.drawer.focused
                || self.config.drawer.visibility == DrawerVisibilityMode::Fixed
                || self.drawer.hover_active)
        {
            self.drawer.open = false;
            self.drawer.focused = false;
            self.drawer.nav_menu = None;
            self.drawer.hover_active = false;
            self.drawer.maximized = false;
            self.drawer.preview.close();
            if self.config.drawer.visibility == DrawerVisibilityMode::Fixed {
                self.config.drawer.visibility = DrawerVisibilityMode::OnDemand;
            }
        } else {
            self.drawer.open = true;
            self.drawer.focused = true;
            self.blur_utility_windows();
            self.sync_drawer_selection_to_active_pane();
        }
        self.reflow_default_viewport();
    }

    pub fn focus_drawer(&mut self) {
        self.drawer.open = true;
        self.drawer.focused = true;
        self.blur_utility_windows();
        self.sync_drawer_selection_to_active_pane();
        self.reflow_default_viewport();
    }

    pub(crate) fn blur_utility_windows(&mut self) {
        self.agents_window.blur();
        self.agents_list.blur();
        self.fonts_window.blur();
        self.command_window.blur();
        self.script_studio.blur();
        self.session_map.blur();
        if self.mcp_console.pinned {
            self.mcp_console.focused = false;
        }
    }

    pub fn close_drawer(&mut self) {
        self.drawer.focused = false;
        self.drawer.nav_menu = None;
        self.drawer.maximized = false;
        self.drawer.preview.close();
        if self.config.drawer.visibility != DrawerVisibilityMode::Fixed {
            self.drawer.open = false;
            self.drawer.hover_active = false;
        }
        self.reflow_default_viewport();
    }

    pub fn toggle_maximize_drawer(&mut self) -> bool {
        self.drawer.open = true;
        self.drawer.maximized = !self.drawer.maximized;
        self.reflow_default_viewport();
        self.drawer.maximized
    }

    pub fn sync_drawer_selection_to_active_pane(&mut self) {
        let rows = self.drawer_rows();
        if let Some(pos) = rows.iter().position(|r| r.is_focused_pane) {
            self.drawer.selected_index = pos;
        } else if self.drawer.selected_index >= rows.len() {
            self.drawer.selected_index = rows.len().saturating_sub(1);
        }
        self.on_drawer_selection_changed(false);
    }

    pub fn drawer_select_next(&mut self) {
        let len = self.drawer_rows().len();
        if len > 0 {
            self.drawer.selected_index = (self.drawer.selected_index + 1) % len;
        }
        self.on_drawer_selection_changed(true);
    }

    pub fn drawer_select_prev(&mut self) {
        let len = self.drawer_rows().len();
        if len > 0 {
            self.drawer.selected_index = (self.drawer.selected_index + len - 1) % len;
        }
        self.on_drawer_selection_changed(true);
    }

    /// Scrolls drawer row text horizontally; clamped so the longest row's tail stays reachable.
    pub fn drawer_hscroll_by(&mut self, delta: isize) -> bool {
        let max = self
            .drawer_rows()
            .iter()
            .map(|r| r.formatted_line().chars().count())
            .max()
            .unwrap_or(0)
            .saturating_sub(12);
        let cur = self.drawer.h_scroll as isize;
        self.drawer.h_scroll = cur.saturating_add(delta).clamp(0, max as isize) as usize;
        self.drawer.h_scroll as isize != cur
    }

    pub fn drawer_scroll_by(&mut self, delta: isize) {
        let len = self.drawer_rows().len();
        if len > 0 {
            let cur = self.drawer.selected_index as isize;
            let max_idx = (len - 1) as isize;
            self.drawer.selected_index = (cur.saturating_add(delta)).clamp(0, max_idx) as usize;
        }
        self.on_drawer_selection_changed(true);
    }

    pub fn selected_drawer_pane_id(&self) -> Option<PaneId> {
        let rows = self.drawer_rows();
        let row = rows.get(self.drawer.selected_index)?;
        match &row.kind {
            DrawerRowKind::Pane(id) => Some(*id),
            DrawerRowKind::FleetNode(name) => self.fleet.nodes.get(name).and_then(|n| match &n.kind {
                FleetNodeKind::SshPane { pane_id, .. } => Some(*pane_id),
                FleetNodeKind::SshTmux { pane_ids, .. } => pane_ids.first().copied(),
                FleetNodeKind::HostProfile { .. } => None,
            }),
            _ => None,
        }
    }

    pub fn drawer_activate_selected(&mut self, unfocus_after: bool) {
        let rows = self.drawer_rows();
        if let Some(row) = rows.get(self.drawer.selected_index) {
            match &row.kind {
                DrawerRowKind::Pane(id) => {
                    let target_id = *id;
                    if let Some(sess) = row
                        .navigator
                        .as_ref()
                        .and_then(|m| m.session_name.as_deref())
                        && sess != self.config.history_session
                    {
                        let sess_owned = sess.to_string();
                        let _ = self.switch_to_session_preserving_panes(&sess_owned);
                    }
                    self.focus_and_raise(target_id);
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Selected Pane #{target_id} (Tab/Esc to type in pane, ↑/↓ to keep browsing drawer)"
                    )));
                }
                DrawerRowKind::QuickTerminal => {
                    self.quick_terminal.visible = true;
                }
                DrawerRowKind::LayoutGroup(_) => {
                    self.toggle_layout_mode();
                }
                DrawerRowKind::FleetNode(name) => {
                    if let Some(pid) = self.selected_drawer_pane_id() {
                        self.focus_and_raise(pid);
                    } else {
                        let name_clone = name.clone();
                        self.active_pane_mut()
                            .set_status_message(Some(format!("Selected fleet node @{name_clone}")));
                    }
                }
                DrawerRowKind::SessionHeader(name) => {
                    if name != &self.config.history_session {
                        let name_clone = name.clone();
                        if let Ok((sess, count, _)) = self.switch_or_create_session(&name_clone) {
                            self.active_pane_mut().set_status_message(Some(format!(
                                "Switched to session '{sess}' ({count} pane(s))"
                            )));
                        }
                    } else if let Some(top_id) = self.z_stack.top() {
                        self.focus_and_raise(top_id);
                    }
                }
            }
        }
        if unfocus_after {
            self.drawer.focused = false;
            self.reflow_default_viewport();
        }
    }

    pub fn drawer_close_selected_pane(&mut self) -> bool {
        if let Some(id) = self.selected_drawer_pane_id() {
            let removed = self.remove_pane(id);
            let len = self.drawer_rows().len();
            if self.drawer.selected_index >= len && len > 0 {
                self.drawer.selected_index = len - 1;
            }
            return removed;
        }
        false
    }

    pub fn drawer_zoom_selected_pane(&mut self) {
        if let Some(id) = self.selected_drawer_pane_id() {
            self.focus_and_raise(id);
            self.toggle_zoom();
        }
    }

    pub fn selected_drawer_session_name(&self) -> String {
        let rows = self.drawer_rows();
        if let Some(DrawerRow {
            kind: DrawerRowKind::SessionHeader(s),
            ..
        }) = rows.get(self.drawer.selected_index)
        {
            return s.clone();
        }
        self.config.history_session.clone()
    }

    pub fn drawer_move_selected_session(&mut self, up: bool) {
        let rows = self.drawer_rows();
        if let Some(DrawerRow {
            kind: DrawerRowKind::Pane(id),
            ..
        }) = rows.get(self.drawer.selected_index)
        {
            let pane_id = *id;
            let ordered = self.ordered_pane_ids();
            if let Some(pos) = ordered.iter().position(|&p| p == pane_id) {
                let partner_opt = if up {
                    pos.checked_sub(1).and_then(|i| ordered.get(i).copied())
                } else {
                    ordered.get(pos + 1).copied()
                };
                if let Some(partner_id) = partner_opt
                    && self.split_tree.swap_leaves(pane_id, partner_id)
                {
                    self.reflow_default_viewport();
                    if let Some(new_row) = self
                        .drawer_rows()
                        .iter()
                        .position(|r| r.kind == DrawerRowKind::Pane(pane_id))
                    {
                        self.drawer.selected_index = new_row;
                    }
                    let dir = if up { "up" } else { "down" };
                    let order_lbl: Vec<String> = self
                        .ordered_pane_ids()
                        .into_iter()
                        .map(|p| format!("#{p}"))
                        .collect();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Moved Pane #{pane_id} {dir} (Window order: {})",
                        order_lbl.join(", ")
                    )));
                    return;
                }
            }
        }

        let target_sess = rows.get(self.drawer.selected_index).and_then(|r| match &r.kind {
            DrawerRowKind::SessionHeader(s) => Some(s.clone()),
            _ => None,
        });
        let name = target_sess
            .clone()
            .unwrap_or_else(|| self.config.history_session.clone());
        let _ = self.daemon_state.session_tree.move_node(&name, up);
        let dir = if up { "up" } else { "down" };
        if let Ok(order) = self.move_session_in_order(target_sess.as_deref(), dir) {
            if let Some(ref s) = target_sess
                && let Some(pos) = self
                    .drawer_rows()
                    .iter()
                    .position(|r| r.kind == DrawerRowKind::SessionHeader(s.clone()))
            {
                self.drawer.selected_index = pos;
            }
            self.active_pane_mut()
                .set_status_message(Some(format!("Session order: {}", order.join(", "))));
        }
    }

    /// Branches a new child session under the selected session in the drawer (`TREE-01`).
    pub fn drawer_branch_selected_session(&mut self) -> Option<String> {
        let parent = self.selected_drawer_session_name();
        self.sync_daemon_state_from_workspace();
        let child = format!(
            "{parent}/sub-{}",
            self.daemon_state.session_tree.nodes.len() + 1
        );
        if self.daemon_state.session_tree.branch(&parent, &child).is_ok() {
            if let Some(pos) = self
                .drawer_rows()
                .iter()
                .position(|r| r.kind == DrawerRowKind::SessionHeader(child.clone()))
            {
                self.drawer.selected_index = pos;
            }
            self.active_pane_mut().set_status_message(Some(format!(
                "Branched child session '{child}' under '{parent}'"
            )));
            Some(child)
        } else {
            None
        }
    }

    /// Reparents (`indent == true`, grafts under preceding session) or promotes to root (`indent == false`)
    /// the selected session in the drawer (`TREE-01`).
    pub fn drawer_reparent_selected_session(&mut self, indent: bool) -> bool {
        let rows = self.drawer_rows();
        let target = self.selected_drawer_session_name();
        self.sync_daemon_state_from_workspace();
        for r in &rows {
            if let DrawerRowKind::SessionHeader(s) = &r.kind
                && self.daemon_state.session_tree.node(s).is_none()
            {
                self.daemon_state.session_tree.upsert_session(
                    s,
                    crate::wm::session_tree::SessionNodeKind::Independent,
                    None,
                );
            }
        }
        let ok = if indent {
            let sess_names: Vec<String> = rows
                .iter()
                .filter_map(|r| match &r.kind {
                    DrawerRowKind::SessionHeader(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();
            let pos = sess_names.iter().position(|s| s == &target).unwrap_or(0);
            let parent_cand = if pos > 0 {
                sess_names.get(pos - 1).cloned()
            } else {
                sess_names.get(pos + 1).cloned()
            };
            if let Some(parent) = parent_cand
                && parent != target
            {
                match self.daemon_state.session_tree.graft(&target, &parent) {
                    Ok(()) => {
                        self.active_pane_mut().set_status_message(Some(format!(
                            "Reparented session '{target}' under '{parent}'"
                        )));
                        true
                    }
                    Err(e) => {
                        self.active_pane_mut().set_status_message(Some(e));
                        false
                    }
                }
            } else {
                false
            }
        } else {
            match self.daemon_state.session_tree.detach_subtree(&target) {
                Ok(()) => {
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Promoted session '{target}' to root [IND]"
                    )));
                    true
                }
                Err(e) => {
                    self.active_pane_mut().set_status_message(Some(e));
                    false
                }
            }
        };
        if let Some(pos) = self
            .drawer_rows()
            .iter()
            .position(|r| r.kind == DrawerRowKind::SessionHeader(target.clone()))
        {
            self.drawer.selected_index = pos;
        }
        ok
    }

    pub fn set_drawer_edge(&mut self, edge: Option<DrawerEdge>) -> DrawerEdge {
        self.config.drawer.edge = edge.unwrap_or_else(|| self.config.drawer.edge.cycle());
        self.reflow_default_viewport();
        self.config.drawer.edge
    }

    pub fn set_drawer_layout(&mut self, mode: Option<DrawerLayoutMode>) -> DrawerLayoutMode {
        self.config.drawer.layout = mode.unwrap_or_else(|| self.config.drawer.layout.toggle());
        self.reflow_default_viewport();
        self.config.drawer.layout
    }

    pub fn set_drawer_visibility(
        &mut self,
        mode: Option<DrawerVisibilityMode>,
    ) -> DrawerVisibilityMode {
        let next = mode.unwrap_or_else(|| self.config.drawer.visibility.toggle_pin());
        self.config.drawer.visibility = next;
        if next == DrawerVisibilityMode::Fixed {
            self.drawer.open = true;
        } else if next == DrawerVisibilityMode::OnDemand && !self.drawer.focused {
            self.drawer.open = false;
        }
        self.reflow_default_viewport();
        next
    }

    pub fn resize_drawer(&mut self, size: u16) {
        self.drawer.maximized = false;
        if self.config.drawer.edge.is_vertical() {
            self.config.drawer.set_width_cols_for_style(size.clamp(16, 100));
        } else {
            self.config.drawer.height_rows = size.clamp(3, 14);
        }
        self.reflow_default_viewport();
    }

    pub fn update_drawer_hover(&mut self, col: u16, row: u16, workspace_area: Rect) -> bool {
        let prev = self.drawer.hover_active;
        let at_edge = match self.config.drawer.edge {
            DrawerEdge::Left => col <= workspace_area.x + 1,
            DrawerEdge::Right => {
                col >= workspace_area
                    .x
                    .saturating_add(workspace_area.width.saturating_sub(2))
            }
            DrawerEdge::Top => row <= workspace_area.y + 1,
            DrawerEdge::Bottom => {
                row >= workspace_area
                    .y
                    .saturating_add(workspace_area.height.saturating_sub(2))
            }
        };
        let inside_open = self
            .drawer_rect(workspace_area)
            .is_some_and(|r| crate::wm::rect_contains(r, col, row));
        self.drawer.hover_active = at_edge || (prev && inside_open);
        if prev != self.drawer.hover_active
            && self.config.drawer.visibility == DrawerVisibilityMode::AutoHide
            && self.config.drawer.layout == DrawerLayoutMode::Push
        {
            let _ = self.resize_all(workspace_area);
        }
        prev != self.drawer.hover_active
    }

    /// Executes any `Action::Drawer*` variant (`ARCH-R01`, `DRW-01`).
    pub fn execute_drawer_action(&mut self, action: &crate::action::Action) -> bool {
        use crate::action::Action;
        let msg = match action {
            Action::DrawerToggle => {
                self.toggle_drawer();
                format!(
                    "Slide-Out Drawer: {}",
                    if self.is_drawer_effectively_visible() {
                        "OPEN"
                    } else {
                        "HIDDEN"
                    }
                )
            }
            Action::DrawerFocus => {
                self.focus_drawer();
                "Slide-Out Drawer: FOCUSED".to_string()
            }
            Action::DrawerDock(edge_opt) => format!(
                "Slide-Out Drawer Dock Edge: {}",
                self.set_drawer_edge(*edge_opt).label()
            ),
            Action::DrawerVisibility(vis_opt) => format!(
                "Slide-Out Drawer Visibility: {}",
                self.set_drawer_visibility(*vis_opt).label()
            ),
            Action::DrawerLayout(layout_opt) => format!(
                "Slide-Out Drawer Layout Mode: {}",
                self.set_drawer_layout(*layout_opt).label()
            ),
            Action::DrawerSize(0) => {
                if self.drawer.maximized {
                    self.drawer.maximized = false;
                    self.reflow_default_viewport();
                    "Slide-Out Drawer: RESTORED [_]".to_string()
                } else {
                    self.config.drawer.visibility = DrawerVisibilityMode::OnDemand;
                    self.close_drawer();
                    "Slide-Out Drawer: MINIMIZED [_]".to_string()
                }
            }
            Action::DrawerSize(u16::MAX) => {
                let max_now = self.toggle_maximize_drawer();
                format!(
                    "Slide-Out Drawer: {}",
                    if max_now { "MAXIMIZED [□]" } else { "RESTORED" }
                )
            }
            Action::DrawerSize(size) => {
                self.resize_drawer(*size);
                format!("Slide-Out Drawer Size: {size}")
            }
            _ => return false,
        };
        self.active_pane_mut().set_status_message(Some(msg));
        true
    }
}
