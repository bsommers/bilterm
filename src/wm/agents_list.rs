//! AGENTS list floating window (Phase 51-01 tracer, UIT-03): window state, rows derived on demand
//! from the agent tracker, and the `:agents list` verb applier.
//!
//! Like the AGENT TRAFFIC window this is NOT a `Modal`: it floats above the panes, owns the keyboard
//! only while focused ([`AgentsListState::captures_input`]) and pointer events outside it reach the
//! panes. Only one of the two windows owns the keyboard at a time. Frozen by `51-CONTRACT.md`
//! section 2.2; plan 51-05 owns the selection, scroll, filter, geometry and dock semantics.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use ratatui::layout::Rect;

use crate::pane::PaneId;
use crate::vt::sanitize_display;
use crate::wm::WindowManager;
use crate::wm::WindowResizeEdge;
use crate::wm::agent_state::AgentState;
use crate::ui::typography::PanelTarget;
use crate::wm::agents_list_cmd::{AgentsListOp, ListFilter};
use crate::wm::agents_list_geometry::LIST_MIN_WINDOW;
use crate::wm::panel_dock::DockPanel;

/// Maximum characters of an agent name shown in a row.
pub const LIST_NAME_CAP: usize = 24;

/// Pointer-gesture bookkeeping of the list window (visible effects always lower to an `Action`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListDrag {
    /// Dragging the title: the grab offset inside the window.
    Move { grab_dx: u16, grab_dy: u16 },
    /// Dragging a border or corner: the edge and the rect at press time.
    Resize { edge: WindowResizeEdge, initial: Rect },
}

/// State of the AGENTS list window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentsListState {
    /// Window is open.
    pub active: bool,
    /// Pinned open (Esc only releases the keyboard).
    pub pinned: bool,
    /// Window owns the keyboard.
    pub focused: bool,
    /// Manual rect (virtual panel space), `None` = default placement.
    pub rect: Option<Rect>,
    /// Docked into the tiling strip (Phase 51-09).
    pub docked: bool,
    /// Selected row.
    pub selected: Option<PaneId>,
    /// First visible row.
    pub scroll: usize,
    /// Which rows are shown.
    pub filter: ListFilter,
    /// Active pointer gesture.
    pub drag: Option<ListDrag>,
    /// Row index the selection last had (nearest-row fallback when its pane closes).
    sel_idx: usize,
    /// Signature of the rows at the last tick (live-refresh change detection); `None` = unknown.
    rows_sig: Option<u64>,
}

impl AgentsListState {
    /// `true` while the window is open and owns the keyboard.
    pub fn captures_input(&self) -> bool {
        self.active && self.focused
    }

    /// Releases the keyboard (and any drag) without closing the window.
    pub fn blur(&mut self) {
        self.focused = false;
        self.drag = None;
    }
}

/// One row of the list: one tracked pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentsListRow {
    pub pane_id: PaneId,
    /// Sanitised identity token (never screen text or arguments).
    pub name: String,
    pub state: AgentState,
    pub since_ms: u64,
    /// A profile matched or the pane reported its state.
    pub is_agent: bool,
    /// The row is the active pane.
    pub is_active_pane: bool,
}

/// Machine-readable summary of the window (RPC `agents_list` object, 51-10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentsListSummary {
    pub active: bool,
    pub pinned: bool,
    pub focused: bool,
    pub docked: bool,
    pub filter: ListFilter,
    pub selected: Option<PaneId>,
    pub row_count: usize,
    pub scale_pct: u16,
}

impl WindowManager {
    /// The AGENTS list window state.
    pub fn agents_list(&self) -> &AgentsListState {
        &self.agents_list
    }

    /// Mutable AGENTS list window state.
    pub fn agents_list_mut(&mut self) -> &mut AgentsListState {
        &mut self.agents_list
    }

    /// Opens the window (focused) and releases the traffic window's keyboard. A fresh open selects
    /// the active pane's row (the first row when the active pane is filtered out).
    pub fn open_agents_list(&mut self) {
        if !self.agents.has_updated() {
            self.update_agent_states(Instant::now());
        }
        let was_active = self.agents_list.active;
        self.agents_list.active = true;
        self.agents_list.focused = true;
        self.agents_window.blur();
        self.fonts_window.blur();
        self.command_window.blur();
        self.script_studio.blur();
        if was_active {
            self.normalize_selection();
        } else {
            self.select_initial_row();
        }
        self.agents_list.rows_sig = Some(self.rows_signature());
        self.ensure_selected_visible();
    }

    /// Closes the window and drops focus, pin, dock and any drag.
    pub fn close_agents_list(&mut self) {
        if self.agents_list.docked {
            self.set_panel_docked(DockPanel::AgentsList, false);
        }
        self.agents_list.active = false;
        self.agents_list.focused = false;
        self.agents_list.pinned = false;
        self.agents_list.drag = None;
        self.agents_list.rows_sig = None;
    }

    /// Rows in pane-id order with the filter applied, derived from the agent tracker on demand.
    pub fn agents_list_rows(&self) -> Vec<AgentsListRow> {
        let active = self.active_pane_id();
        self.agent_statuses()
            .into_iter()
            .map(|s| AgentsListRow {
                pane_id: s.pane_id,
                name: sanitize_display(&s.name, LIST_NAME_CAP),
                state: s.state,
                since_ms: s.since_ms,
                is_agent: s.profile.is_some() || s.reported,
                is_active_pane: s.pane_id == active,
            })
            .filter(|r| self.agents_list.filter == ListFilter::All || r.is_agent)
            .collect()
    }

    /// Summary for RPC and tests.
    pub fn agents_list_summary(&self) -> AgentsListSummary {
        let s = &self.agents_list;
        AgentsListSummary {
            active: s.active,
            pinned: s.pinned,
            focused: s.focused,
            docked: s.docked,
            filter: s.filter,
            selected: s.selected,
            row_count: self.agents_list_rows().len(),
            scale_pct: self.scale_setting(PanelTarget::AgentsList),
        }
    }

    /// Applies one `:agents list` verb; returns an optional status message.
    pub fn agents_list_apply(&mut self, op: &AgentsListOp) -> Option<String> {
        let needs_open = !matches!(
            op,
            AgentsListOp::Open
                | AgentsListOp::Close
                | AgentsListOp::Toggle
                | AgentsListOp::Dismiss
                | AgentsListOp::Blur
                | AgentsListOp::SelectPane(_)
                | AgentsListOp::ScaleStep(_)
                | AgentsListOp::Filter(_)
        );
        if needs_open && !self.agents_list.active {
            return Some(NOT_OPEN.to_string());
        }
        if self.agents_list.active {
            self.normalize_selection();
        }
        match op {
            AgentsListOp::Open => self.open_agents_list(),
            AgentsListOp::Close => self.close_agents_list(),
            AgentsListOp::Toggle => {
                if self.agents_list.active {
                    self.close_agents_list();
                } else {
                    self.open_agents_list();
                }
            }
            AgentsListOp::Dismiss => {
                if self.agents_list.pinned {
                    self.agents_list.blur();
                } else {
                    self.close_agents_list();
                }
            }
            AgentsListOp::Blur => self.agents_list.blur(),
            AgentsListOp::Step(n) => self.step_selection(i64::from(*n), false),
            AgentsListOp::Page(n) => self.step_selection(i64::from(*n), true),
            AgentsListOp::First => self.select_row_index(0),
            AgentsListOp::Last => self.select_row_index(usize::MAX),
            AgentsListOp::Scroll(n) => self.scroll_list(i64::from(*n)),
            AgentsListOp::SelectPane(id) => return self.select_pane(*id),
            AgentsListOp::Filter(f) => return Some(self.set_list_filter(*f)),
            AgentsListOp::Jump(opt) => return self.jump(*opt),
            AgentsListOp::Zoom(opt) => return self.zoom(*opt),
            AgentsListOp::Pin(opt) => {
                let pin = opt.unwrap_or(!self.agents_list.pinned);
                self.agents_list.pinned = pin;
                return Some(
                    if pin {
                        "Agents list pinned (Esc returns focus to panes; :agents list unpin unpins)"
                    } else {
                        "Agents list unpinned"
                    }
                    .to_string(),
                );
            }
            AgentsListOp::ScaleStep(delta) => {
                let n = self.step_scale(PanelTarget::AgentsList, *delta);
                let note = if self.gui_surface { "" } else { " (applies in the GUI only)" };
                return Some(format!("Panel scale: {n}%{note}"));
            }
            AgentsListOp::Dock(opt) => {
                let on = opt.unwrap_or(!self.agents_list.docked);
                self.set_panel_docked(DockPanel::AgentsList, on);
                if on && !self.agents_list.docked {
                    return Some("No room to dock the agents list".to_string());
                }
            }
            AgentsListOp::Move { dx, dy } => self.move_agents_list(*dx, *dy),
            AgentsListOp::Resize { dw, dh } => self.resize_agents_list(*dw, *dh),
            AgentsListOp::SetRect { x, y, w, h } => {
                self.set_agents_list_rect(Rect::new(*x, *y, *w, *h));
            }
            AgentsListOp::Size(size) => self.set_agents_list_size(*size),
        }
        None
    }

    /// Workspace as the list window sees it: the scaled (virtual) cell space shared with the
    /// renderer and the mouse handler (identity at 100%). Built on the real shell workspace, not the
    /// effective one: the effective area has the dock strip removed, which would clip a docked
    /// list's slot away (51-09).
    pub(crate) fn agents_list_workspace(&self) -> Rect {
        self.panel_virtual_area(self.dock_base_area(), PanelTarget::AgentsList)
    }

    /// Rows that fit in the window's row area (at least 1).
    fn list_visible_rows(&self) -> usize {
        self.agents_list_layout(self.agents_list_workspace()).map_or(1, |l| l.visible_rows.max(1))
    }

    /// Hash of what a row shows (not its age), for live-refresh change detection.
    fn rows_signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        for r in self.agents_list_rows() {
            (r.pane_id, &r.name, std::mem::discriminant(&r.state), r.is_agent, r.is_active_pane)
                .hash(&mut h);
        }
        h.finish()
    }

    /// `true` when the list is open and its rows changed since the last call (poll-loop hook, runs
    /// even while the Navigator drawer is hidden). Nothing is computed while the list is closed.
    pub(crate) fn agents_list_tick_changed(&mut self) -> bool {
        if !self.agents_list.active {
            return false;
        }
        let sig = self.rows_signature();
        if self.agents_list.rows_sig == Some(sig) {
            return false;
        }
        self.agents_list.rows_sig = Some(sig);
        self.normalize_selection();
        self.clamp_scroll();
        true
    }

    /// Keeps the selection when its pane still has a row; otherwise moves it to the row now at the
    /// previous index (the nearest one), or clears it when no rows remain. Never dangles.
    fn normalize_selection(&mut self) {
        let rows = self.agents_list_rows();
        let st = &mut self.agents_list;
        let Some(sel) = st.selected else { return };
        if let Some(i) = rows.iter().position(|r| r.pane_id == sel) {
            st.sel_idx = i;
        } else if rows.is_empty() {
            st.selected = None;
            st.sel_idx = 0;
        } else {
            st.sel_idx = st.sel_idx.min(rows.len() - 1);
            st.selected = Some(rows[st.sel_idx].pane_id);
        }
    }

    /// Fresh open: the active pane's row, else the first row, else nothing.
    fn select_initial_row(&mut self) {
        let rows = self.agents_list_rows();
        let active = self.active_pane_id();
        let idx = rows
            .iter()
            .position(|r| r.pane_id == active)
            .or(if rows.is_empty() { None } else { Some(0) });
        self.agents_list.scroll = 0;
        self.agents_list.selected = idx.map(|i| rows[i].pane_id);
        self.agents_list.sel_idx = idx.unwrap_or(0);
    }

    /// Clamps the scroll offset to `[0, rows - visible]`.
    fn clamp_scroll(&mut self) {
        let max_scroll = self.agents_list_rows().len().saturating_sub(self.list_visible_rows());
        self.agents_list.scroll = self.agents_list.scroll.min(max_scroll);
    }

    /// Scrolls the minimum needed to bring the selected row into view.
    fn ensure_selected_visible(&mut self) {
        if !self.agents_list.active {
            return;
        }
        let vis = self.list_visible_rows();
        let rows = self.agents_list_rows();
        let mut scroll = self.agents_list.scroll;
        let pos = self.agents_list.selected.and_then(|id| rows.iter().position(|r| r.pane_id == id));
        if let Some(i) = pos {
            if i < scroll {
                scroll = i;
            } else if i >= scroll.saturating_add(vis) {
                scroll = i + 1 - vis;
            }
            self.agents_list.sel_idx = i;
        }
        self.agents_list.scroll = scroll.min(rows.len().saturating_sub(vis));
    }

    /// Selects the row at `index` (clamped to the last row) and scrolls it into view.
    fn select_row_index(&mut self, index: usize) {
        let rows = self.agents_list_rows();
        if rows.is_empty() {
            return;
        }
        let i = index.min(rows.len() - 1);
        self.agents_list.selected = Some(rows[i].pane_id);
        self.agents_list.sel_idx = i;
        self.ensure_selected_visible();
    }

    /// Moves the selection by `n` rows, or `n` pages of `visible - 1` rows; clamps at both ends.
    fn step_selection(&mut self, n: i64, pages: bool) {
        let rows = self.agents_list_rows();
        let len = rows.len();
        if len == 0 {
            return;
        }
        let page = i64::try_from(self.list_visible_rows().saturating_sub(1).max(1)).unwrap_or(1);
        let delta = if pages { n.saturating_mul(page) } else { n };
        let cur = self.agents_list.selected.and_then(|id| rows.iter().position(|r| r.pane_id == id));
        let last = i64::try_from(len - 1).unwrap_or(i64::MAX);
        let target = match cur {
            Some(c) => i64::try_from(c).unwrap_or(0).saturating_add(delta),
            None if delta >= 0 => 0,
            None => last,
        };
        self.select_row_index(usize::try_from(target.clamp(0, last)).unwrap_or(0));
    }

    /// Scrolls the rows by `n`; the selection is not touched.
    fn scroll_list(&mut self, n: i64) {
        let max_scroll = self.agents_list_rows().len().saturating_sub(self.list_visible_rows());
        let cur = i64::try_from(self.agents_list.scroll).unwrap_or(0);
        let max = i64::try_from(max_scroll).unwrap_or(0);
        self.agents_list.scroll = usize::try_from(cur.saturating_add(n).clamp(0, max)).unwrap_or(0);
    }

    /// `SelectPane`: opens a closed list; unknown (or filtered out) ids report and change nothing.
    fn select_pane(&mut self, id: PaneId) -> Option<String> {
        if !self.agents_list.active {
            self.open_agents_list();
        }
        match self.agents_list_rows().iter().position(|r| r.pane_id == id) {
            Some(i) => {
                self.select_row_index(i);
                None
            }
            None => Some(format!("No agent in pane {id}")),
        }
    }

    /// `Filter`: `None` toggles. Re-normalises the selection and resets the scroll.
    fn set_list_filter(&mut self, filter: Option<ListFilter>) -> String {
        self.agents_list.filter = filter.unwrap_or(match self.agents_list.filter {
            ListFilter::All => ListFilter::Agents,
            ListFilter::Agents => ListFilter::All,
        });
        self.agents_list.scroll = 0;
        self.normalize_selection();
        self.ensure_selected_visible();
        match self.agents_list.filter {
            ListFilter::All => "Agents list: all panes",
            ListFilter::Agents => "Agents list: detected agents only",
        }
        .to_string()
    }

    /// `Jump`: focus and raise the pane (this releases both windows' keyboard), then close the
    /// list unless it is pinned. Target: the explicit id, else the selected row, else the active pane.
    fn jump(&mut self, explicit: Option<PaneId>) -> Option<String> {
        let id = explicit.or(self.agents_list.selected).unwrap_or_else(|| self.active_pane_id());
        if !self.panes.contains_key(&id) {
            return Some(format!("No agent in pane {id}"));
        }
        self.focus_and_raise(id);
        if !self.agents_list.pinned {
            self.close_agents_list();
        }
        Some(format!("Focused pane {id}"))
    }

    /// `Zoom`: open the traffic window on the pane's node (no target: without a node).
    fn zoom(&mut self, explicit: Option<PaneId>) -> Option<String> {
        match explicit.or(self.agents_list.selected) {
            Some(id) if !self.panes.contains_key(&id) => Some(format!("No agent in pane {id}")),
            target => {
                self.open_agents_window(target);
                None
            }
        }
    }

    /// Releases the dock through the seam API (the flag is never written directly).
    fn release_list_dock(&mut self) {
        if self.agents_list.docked {
            self.set_panel_docked(DockPanel::AgentsList, false);
        }
    }

    /// Sets the window rect clamped to the workspace (min 20x6, or the workspace when smaller) and
    /// releases the dock. Returns the stored rect.
    fn set_agents_list_rect(&mut self, rect: Rect) -> Rect {
        let ws = self.agents_list_workspace();
        let (ws_w, ws_h) = (ws.width.max(1), ws.height.max(1));
        let w = rect.width.clamp(LIST_MIN_WINDOW.0.min(ws_w), ws_w);
        let h = rect.height.clamp(LIST_MIN_WINDOW.1.min(ws_h), ws_h);
        let max_x = ws.x + ws.width.saturating_sub(w);
        let max_y = ws.y + ws.height.saturating_sub(h);
        let clamped = Rect::new(rect.x.clamp(ws.x, max_x), rect.y.clamp(ws.y, max_y), w, h);
        self.agents_list.rect = Some(clamped);
        self.release_list_dock();
        self.clamp_scroll();
        clamped
    }

    fn move_agents_list(&mut self, dx: i16, dy: i16) {
        let Some(cur) = self.agents_list_rect(self.agents_list_workspace()) else { return };
        let x = (i32::from(cur.x) + i32::from(dx)).clamp(0, i32::from(u16::MAX)) as u16;
        let y = (i32::from(cur.y) + i32::from(dy)).clamp(0, i32::from(u16::MAX)) as u16;
        self.set_agents_list_rect(Rect::new(x, y, cur.width, cur.height));
    }

    fn resize_agents_list(&mut self, dw: i16, dh: i16) {
        let Some(cur) = self.agents_list_rect(self.agents_list_workspace()) else { return };
        let w = (i32::from(cur.width) + i32::from(dw)).clamp(0, i32::from(u16::MAX)) as u16;
        let h = (i32::from(cur.height) + i32::from(dh)).clamp(0, i32::from(u16::MAX)) as u16;
        self.set_agents_list_rect(Rect::new(cur.x, cur.y, w, h));
    }

    /// `Size(Some)` keeps the position; `Size(None)` returns to the default placement.
    fn set_agents_list_size(&mut self, size: Option<(u16, u16)>) {
        match size {
            None => {
                self.agents_list.rect = None;
                self.release_list_dock();
                self.clamp_scroll();
            }
            Some((cols, rows)) => {
                let Some(cur) = self.agents_list_rect(self.agents_list_workspace()) else { return };
                self.set_agents_list_rect(Rect::new(cur.x, cur.y, cols, rows));
            }
        }
    }
}

const NOT_OPEN: &str = "Agents list is not open";

