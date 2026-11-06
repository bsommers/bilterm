//! AGENTS list floating window (Phase 51-01 tracer, UIT-03): window state, rows derived on demand
//! from the agent tracker, and the `:agents list` verb applier.
//!
//! Like the AGENT TRAFFIC window this is NOT a `Modal`: it floats above the panes, owns the keyboard
//! only while focused ([`AgentsListState::captures_input`]) and pointer events outside it reach the
//! panes. Only one of the two windows owns the keyboard at a time. Frozen by `51-CONTRACT.md`
//! section 2.2; plan 51-05 owns the selection, scroll, filter, geometry and dock semantics.

use std::time::Instant;

use ratatui::layout::Rect;

use crate::pane::PaneId;
use crate::vt::sanitize_display;
use crate::wm::WindowManager;
use crate::wm::WindowResizeEdge;
use crate::wm::agent_state::AgentState;
use crate::wm::agents_list_cmd::{AgentsListOp, ListFilter};

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

    /// Opens the window (focused) and releases the traffic window's keyboard.
    pub fn open_agents_list(&mut self) {
        if !self.agents.has_updated() {
            self.update_agent_states(Instant::now());
        }
        self.agents_list.active = true;
        self.agents_list.focused = true;
        self.agents_window.blur();
    }

    /// Closes the window and drops focus, pin and any drag.
    pub fn close_agents_list(&mut self) {
        self.agents_list.active = false;
        self.agents_list.focused = false;
        self.agents_list.pinned = false;
        self.agents_list.drag = None;
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
            scale_pct: self.scale_setting(crate::ui::typography::PanelTarget::AgentsList),
        }
    }

    /// Applies one `:agents list` verb; returns an optional status message.
    pub fn agents_list_apply(&mut self, op: &AgentsListOp) -> Option<String> {
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
            // Selection, jump, zoom, pin, scale, filter, dock, geometry and scroll: plan 51-05 owns
            // these semantics; the tracer slice proves the Action -> apply path with a no-op.
            AgentsListOp::Step(_)
            | AgentsListOp::First
            | AgentsListOp::Last
            | AgentsListOp::Page(_)
            | AgentsListOp::SelectPane(_)
            | AgentsListOp::Jump(_)
            | AgentsListOp::Zoom(_)
            | AgentsListOp::Pin(_)
            | AgentsListOp::ScaleStep(_)
            | AgentsListOp::Filter(_)
            | AgentsListOp::Dock(_)
            | AgentsListOp::Move { .. }
            | AgentsListOp::Resize { .. }
            | AgentsListOp::SetRect { .. }
            | AgentsListOp::Size(_)
            | AgentsListOp::Scroll(_) => return None,
        }
        None
    }
}
