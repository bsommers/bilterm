//! Drawer Window Peek & Live Preview Pane state, geometry, and target resolution (`DRW-PEEK-01..03`, Phase 56).
//!
//! Renders a non-destructive floating preview pane beside the slide-out workspace drawer so users can
//! inspect occluded stacked windows, minimized windows, live background-session windows, or saved
//! session snapshots before committing focus.

use ratatui::layout::Rect;

use super::{DrawerEdge, DrawerRowKind, DrawerVisibilityMode};
use crate::action::DrawerPreviewOp;
use crate::pane::PaneId;
use crate::wm::{SavedWorkspaceSession, WindowManager};

/// Resolved target currently displayed inside the Drawer Peek Preview Pane (`DRW-PEEK-01..03`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawerPreviewTarget {
    /// A live `CompositePane` in the currently active session (`wm.panes`).
    ActivePane(PaneId),
    /// The Quake-style Quick Terminal (`wm.quick_terminal.pane`).
    QuickTerminal,
    /// A live `CompositePane` residing in a background session (`wm.window_manage.background_sessions`).
    BackgroundPane {
        session: String,
        pane_id: PaneId,
    },
    /// A saved/inactive session snapshot (`SavedWorkspaceSession`) or session header row.
    SavedSession {
        name: String,
    },
}

impl DrawerPreviewTarget {
    /// Short type label used by JSON-RPC / MCP responses and diagnostics.
    pub const fn kind_label(&self) -> &'static str {
        match self {
            Self::ActivePane(_) => "active_pane",
            Self::QuickTerminal => "quick_terminal",
            Self::BackgroundPane { .. } => "background_pane",
            Self::SavedSession { .. } => "saved_session",
        }
    }
}

/// Runtime state of the Drawer Peek Preview Pane owned by [`super::WorkspaceDrawer`] (`DRW-PEEK-01`).
#[derive(Clone, Debug, Default)]
pub struct DrawerPreviewState {
    /// Whether the floating Peek Preview Pane is currently open/visible.
    pub active: bool,
    /// Whether scrolling (`Up`/`Down`/`j`/`k`/mouse wheel) in the drawer automatically opens the preview.
    pub auto_preview_on_scroll: bool,
    /// Current resolved preview target (synced with the highlighted drawer row or an explicit target).
    pub target: Option<DrawerPreviewTarget>,
    /// Explicitly pinned target (`:drawer preview pane <id>` or `:drawer preview session <name>`),
    /// cleared automatically when the user moves the drawer selection.
    pub explicit_target: Option<DrawerPreviewTarget>,
    /// Cached snapshot for `DrawerPreviewTarget::SavedSession { name }` so rendering does zero disk I/O.
    pub cached_saved_session: Option<(String, Option<SavedWorkspaceSession>)>,
}

impl DrawerPreviewState {
    /// Closes the preview pane and drops any transient target/cache state.
    pub fn close(&mut self) {
        self.active = false;
        self.target = None;
        self.explicit_target = None;
        self.cached_saved_session = None;
    }
}

impl WindowManager {
    /// Resolves the [`DrawerPreviewTarget`] for the currently highlighted drawer row (or explicit pin).
    pub fn resolve_drawer_preview_target(&self) -> Option<DrawerPreviewTarget> {
        if let Some(ref explicit) = self.drawer.preview.explicit_target {
            match explicit {
                DrawerPreviewTarget::ActivePane(id) if self.panes.contains_key(id) => {
                    return Some(explicit.clone());
                }
                DrawerPreviewTarget::BackgroundPane { session, pane_id }
                    if self
                        .window_manage
                        .background_sessions
                        .get(session)
                        .is_some_and(|bg| bg.panes.contains_key(pane_id)) =>
                {
                    return Some(explicit.clone());
                }
                DrawerPreviewTarget::QuickTerminal | DrawerPreviewTarget::SavedSession { .. } => {
                    return Some(explicit.clone());
                }
                _ => {}
            }
        }

        let rows = self.drawer_rows();
        let row = rows.get(self.drawer.selected_index)?;
        match &row.kind {
            DrawerRowKind::Pane(id) => {
                if let Some(sess) = row
                    .navigator
                    .as_ref()
                    .and_then(|m| m.session_name.as_deref())
                    && sess != self.config.history_session
                    && self
                        .window_manage
                        .background_sessions
                        .get(sess)
                        .is_some_and(|bg| bg.panes.contains_key(id))
                {
                    return Some(DrawerPreviewTarget::BackgroundPane {
                        session: sess.to_string(),
                        pane_id: *id,
                    });
                }
                if self.panes.contains_key(id) {
                    return Some(DrawerPreviewTarget::ActivePane(*id));
                }
                for (sess, bg) in &self.window_manage.background_sessions {
                    if bg.panes.contains_key(id) {
                        return Some(DrawerPreviewTarget::BackgroundPane {
                            session: sess.clone(),
                            pane_id: *id,
                        });
                    }
                }
                None
            }
            DrawerRowKind::QuickTerminal => Some(DrawerPreviewTarget::QuickTerminal),
            DrawerRowKind::LayoutGroup(_) => self
                .z_stack
                .top()
                .or_else(|| self.ordered_pane_ids().first().copied())
                .map(DrawerPreviewTarget::ActivePane),
            DrawerRowKind::FleetNode(_) => self
                .selected_drawer_pane_id()
                .map(DrawerPreviewTarget::ActivePane),
            DrawerRowKind::SessionHeader(name) => {
                if name == &self.config.history_session {
                    self.z_stack
                        .top()
                        .or_else(|| self.ordered_pane_ids().first().copied())
                        .map(DrawerPreviewTarget::ActivePane)
                } else if let Some(bg) = self.window_manage.background_sessions.get(name) {
                    if let Some(pid) = bg
                        .z_stack
                        .top()
                        .or_else(|| bg.panes.keys().min().copied())
                    {
                        Some(DrawerPreviewTarget::BackgroundPane {
                            session: name.clone(),
                            pane_id: pid,
                        })
                    } else {
                        Some(DrawerPreviewTarget::SavedSession { name: name.clone() })
                    }
                } else {
                    Some(DrawerPreviewTarget::SavedSession { name: name.clone() })
                }
            }
        }
    }

    /// Synchronizes `drawer.preview.target` and `cached_saved_session` after row selection changes.
    pub fn sync_drawer_preview_cache(&mut self) {
        let resolved = self.resolve_drawer_preview_target();
        if let Some(DrawerPreviewTarget::SavedSession { ref name }) = resolved {
            let needs_load = self
                .drawer
                .preview
                .cached_saved_session
                .as_ref()
                .is_none_or(|(cached_name, _)| cached_name != name);
            if needs_load {
                let snap = self.load_session_snapshot_any(name);
                self.drawer.preview.cached_saved_session = Some((name.clone(), snap));
            }
        } else {
            self.drawer.preview.cached_saved_session = None;
        }
        self.drawer.preview.target = resolved;
    }

    /// Called when the user scrolls/moves selection in the drawer: clears any explicit pin,
    /// activates preview if `auto_preview_on_scroll` is enabled, and refreshes the target cache.
    pub fn on_drawer_selection_changed(&mut self, from_scroll: bool) {
        self.drawer.preview.explicit_target = None;
        if from_scroll && self.drawer.preview.auto_preview_on_scroll {
            self.drawer.preview.active = true;
        }
        self.sync_drawer_preview_cache();
    }

    /// Computes the floating screen rectangle for the Peek Preview Pane adjacent to `drawer_rect(area)`.
    /// Never mutates `SplitTree` or triggers `SIGWINCH` on any PTY.
    pub fn drawer_preview_rect(&self, area: Rect) -> Option<Rect> {
        if !self.drawer.preview.active || !self.is_drawer_effectively_visible() {
            return None;
        }
        let dr = self.drawer_rect(area)?;
        self.resolve_drawer_preview_target()?;

        let (x, y, w, h) = match self.config.drawer.edge {
            DrawerEdge::Left => {
                let x = dr.x.saturating_add(dr.width);
                let avail_w = area.x.saturating_add(area.width).saturating_sub(x);
                let w = ((avail_w * 55) / 100)
                    .max(36)
                    .min(avail_w.saturating_sub(1));
                let y = dr.y.saturating_add(1).min(area.y.saturating_add(area.height.saturating_sub(1)));
                let h = dr
                    .height
                    .saturating_sub(2)
                    .max(8)
                    .min(area.height.saturating_sub(2));
                (x, y, w, h)
            }
            DrawerEdge::Right => {
                let avail_w = dr.x.saturating_sub(area.x);
                let w = ((avail_w * 55) / 100)
                    .max(36)
                    .min(avail_w.saturating_sub(1));
                let x = dr.x.saturating_sub(w);
                let y = dr.y.saturating_add(1).min(area.y.saturating_add(area.height.saturating_sub(1)));
                let h = dr
                    .height
                    .saturating_sub(2)
                    .max(8)
                    .min(area.height.saturating_sub(2));
                (x, y, w, h)
            }
            DrawerEdge::Top => {
                let y = dr.y.saturating_add(dr.height);
                let avail_h = area.y.saturating_add(area.height).saturating_sub(y);
                let h = ((avail_h * 60) / 100)
                    .max(10)
                    .min(avail_h.saturating_sub(1));
                let w = ((area.width * 65) / 100)
                    .max(40)
                    .min(area.width.saturating_sub(4));
                let x = area.x + area.width.saturating_sub(w) / 2;
                (x, y, w, h)
            }
            DrawerEdge::Bottom => {
                let avail_h = dr.y.saturating_sub(area.y);
                let h = ((avail_h * 60) / 100)
                    .max(10)
                    .min(avail_h.saturating_sub(1));
                let y = dr.y.saturating_sub(h);
                let w = ((area.width * 65) / 100)
                    .max(40)
                    .min(area.width.saturating_sub(4));
                let x = area.x + area.width.saturating_sub(w) / 2;
                (x, y, w, h)
            }
        };

        if w < 20 || h < 6 {
            return None;
        }
        Some(Rect::new(x, y, w, h))
    }

    /// Returns `true` if screen coordinate `(col, row)` hits the `[×]` dismiss button on the preview pane.
    pub fn drawer_preview_close_hit(&self, area: Rect, col: u16, row: u16) -> bool {
        let Some(pr) = self.drawer_preview_rect(area) else {
            return false;
        };
        let on_header_row = row == pr.y || row == pr.y.saturating_add(1);
        let right_edge = pr.x.saturating_add(pr.width);
        let close_start = right_edge.saturating_sub(6);
        on_header_row && col >= close_start && col < right_edge
    }

    /// Opens the Peek Preview Pane (opening the drawer as well if it was hidden).
    pub fn open_drawer_preview(&mut self) {
        if !self.is_drawer_effectively_visible() {
            self.drawer.open = true;
            self.drawer.focused = true;
            self.blur_utility_windows();
            self.sync_drawer_selection_to_active_pane();
            self.reflow_default_viewport();
        }
        self.drawer.preview.active = true;
        self.sync_drawer_preview_cache();
        self.active_pane_mut().set_status_message(Some(
            "Drawer Peek Preview: ON (Enter/Click to focus, Esc/v to close)".to_string(),
        ));
    }

    /// Closes the Peek Preview Pane without changing active workspace focus.
    pub fn close_drawer_preview(&mut self) {
        self.drawer.preview.close();
        self.active_pane_mut()
            .set_status_message(Some("Drawer Peek Preview: OFF".to_string()));
    }

    /// Toggles the Peek Preview Pane on or off.
    pub fn toggle_drawer_preview(&mut self) -> bool {
        if self.drawer.preview.active && self.is_drawer_effectively_visible() {
            self.close_drawer_preview();
            false
        } else {
            self.open_drawer_preview();
            true
        }
    }

    /// Commits focus/restore/switch to the currently previewed window or session and closes the preview.
    pub fn commit_drawer_preview(&mut self) -> bool {
        let Some(target) = self.resolve_drawer_preview_target() else {
            return false;
        };
        self.drawer.preview.close();
        self.drawer.focused = false;
        self.drawer.nav_menu = None;
        if self.config.drawer.visibility != DrawerVisibilityMode::Fixed {
            self.drawer.open = false;
            self.drawer.hover_active = false;
        }

        match target {
            DrawerPreviewTarget::ActivePane(id) => {
                if self.z_stack.is_minimized(id) {
                    self.z_stack.toggle_minimize(id);
                }
                self.focus_and_raise(id);
                self.reflow_default_viewport();
                self.active_pane_mut().set_status_message(Some(format!(
                    "Focused Window #{id} from preview"
                )));
            }
            DrawerPreviewTarget::QuickTerminal => {
                self.quick_terminal.visible = true;
                self.reflow_default_viewport();
                self.active_pane_mut().set_status_message(Some(
                    "Opened Quick Terminal from preview".to_string(),
                ));
            }
            DrawerPreviewTarget::BackgroundPane { session, pane_id } => {
                let _ = self.switch_to_session_preserving_panes(&session);
                if self.z_stack.is_minimized(pane_id) {
                    self.z_stack.toggle_minimize(pane_id);
                }
                self.focus_and_raise(pane_id);
                self.reflow_default_viewport();
                self.active_pane_mut().set_status_message(Some(format!(
                    "Switched to session '{session}' and focused Window #{pane_id}"
                )));
            }
            DrawerPreviewTarget::SavedSession { name } => {
                if name != self.config.history_session {
                    if self.window_manage.background_sessions.contains_key(&name) {
                        let _ = self.switch_to_session_preserving_panes(&name);
                    } else {
                        let _ = self.switch_or_create_session(&name);
                    }
                }
                self.reflow_default_viewport();
                self.active_pane_mut().set_status_message(Some(format!(
                    "Switched to session '{name}' from preview"
                )));
            }
        }
        true
    }

    /// Executes a [`DrawerPreviewOp`] via unified `Action::DrawerPreview` dispatch (`ARCH-R01`).
    pub fn execute_drawer_preview_op(&mut self, op: DrawerPreviewOp) {
        match op {
            DrawerPreviewOp::On => {
                self.drawer.preview.explicit_target = None;
                self.open_drawer_preview();
            }
            DrawerPreviewOp::Off => {
                self.close_drawer_preview();
            }
            DrawerPreviewOp::Toggle => {
                self.drawer.preview.explicit_target = None;
                self.toggle_drawer_preview();
            }
            DrawerPreviewOp::Commit => {
                if !self.commit_drawer_preview() {
                    self.drawer_activate_selected(true);
                }
            }
            DrawerPreviewOp::AutoToggle => {
                self.drawer.preview.auto_preview_on_scroll =
                    !self.drawer.preview.auto_preview_on_scroll;
                let state_lbl = if self.drawer.preview.auto_preview_on_scroll {
                    "ON"
                } else {
                    "OFF"
                };
                self.active_pane_mut().set_status_message(Some(format!(
                    "Drawer auto-preview on scroll: {state_lbl}"
                )));
            }
            DrawerPreviewOp::Row(idx) => {
                if !self.is_drawer_effectively_visible() {
                    self.drawer.open = true;
                    self.drawer.focused = true;
                    self.blur_utility_windows();
                    self.reflow_default_viewport();
                }
                let len = self.drawer_rows().len();
                if len > 0 {
                    self.drawer.selected_index = idx.min(len - 1);
                }
                self.drawer.preview.explicit_target = None;
                self.drawer.preview.active = true;
                self.sync_drawer_preview_cache();
                let sel = self.drawer.selected_index;
                self.active_pane_mut().set_status_message(Some(format!(
                    "Drawer Peek Preview: row {sel}"
                )));
            }
            DrawerPreviewOp::Pane(id) => {
                if !self.is_drawer_effectively_visible() {
                    self.drawer.open = true;
                    self.drawer.focused = true;
                    self.blur_utility_windows();
                    self.reflow_default_viewport();
                }
                let rows = self.drawer_rows();
                if let Some(pos) = rows
                    .iter()
                    .position(|r| r.kind == DrawerRowKind::Pane(id))
                {
                    self.drawer.selected_index = pos;
                    self.drawer.preview.explicit_target = None;
                } else if self.panes.contains_key(&id) {
                    self.drawer.preview.explicit_target =
                        Some(DrawerPreviewTarget::ActivePane(id));
                } else if let Some((sess, _)) = self
                    .window_manage
                    .background_sessions
                    .iter()
                    .find(|(_, bg)| bg.panes.contains_key(&id))
                {
                    self.drawer.preview.explicit_target =
                        Some(DrawerPreviewTarget::BackgroundPane {
                            session: sess.clone(),
                            pane_id: id,
                        });
                } else {
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Preview failed: Window #{id} not found"
                    )));
                    return;
                }
                self.drawer.preview.active = true;
                self.sync_drawer_preview_cache();
                self.active_pane_mut().set_status_message(Some(format!(
                    "Drawer Peek Preview: Window #{id}"
                )));
            }
            DrawerPreviewOp::Session(name) => {
                let clean = name.trim().to_string();
                if clean.is_empty() {
                    self.active_pane_mut().set_status_message(Some(
                        "Usage: :drawer preview session <name>".to_string(),
                    ));
                    return;
                }
                if !self.is_drawer_effectively_visible() {
                    self.drawer.open = true;
                    self.drawer.focused = true;
                    self.blur_utility_windows();
                    self.reflow_default_viewport();
                }
                let rows = self.drawer_rows();
                if let Some(pos) = rows
                    .iter()
                    .position(|r| r.kind == DrawerRowKind::SessionHeader(clean.clone()))
                {
                    self.drawer.selected_index = pos;
                    self.drawer.preview.explicit_target = None;
                } else {
                    self.drawer.preview.explicit_target =
                        Some(DrawerPreviewTarget::SavedSession {
                            name: clean.clone(),
                        });
                }
                self.drawer.preview.active = true;
                self.sync_drawer_preview_cache();
                self.active_pane_mut().set_status_message(Some(format!(
                    "Drawer Peek Preview: session '{clean}'"
                )));
            }
        }
    }
}
