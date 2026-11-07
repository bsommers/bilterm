//! Navigator drawer style: compact workspace > session > pane tree (`FLT-01`, Phase 50 D-02).
//!
//! The classic drawer (`classic_drawer_rows` in the parent module) is untouched. This module owns the
//! style dispatcher [`WindowManager::drawer_rows`], the navigator row builder and the per-style width
//! helpers on [`DrawerConfig`].

use std::collections::BTreeSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{DrawerConfig, DrawerRow, DrawerRowKind, WorkspaceDrawer};
use crate::pane::PaneId;
use crate::vt::sanitize_display;
use crate::wm::agent_state::{AgentCounts, AgentState};
use crate::wm::agents_window::NodeKey;
use crate::wm::{SessionNodeKind, SessionTreeNode, WindowManager};

/// Visual style of the slide-out drawer (`drawer.style`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrawerStyle {
    /// Telemetry-rich classic rows (default, byte-identical to pre-Phase-50 output).
    #[default]
    Classic,
    /// Compact one-line-per-item workspace > session > pane tree. `herdr` is a deprecated hidden alias.
    #[serde(alias = "herdr")]
    Navigator,
}

impl DrawerStyle {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Navigator => "navigator",
        }
    }
    pub const fn toggle(self) -> Self {
        match self {
            Self::Classic => Self::Navigator,
            Self::Navigator => Self::Classic,
        }
    }
}

/// Tree level of a navigator drawer row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigatorLevel {
    Workspace,
    Session,
    Pane,
    /// Quick terminal and fleet nodes.
    Other,
}

/// Navigator-only metadata attached to a [`DrawerRow`] (`None` for every classic row).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavigatorRowMeta {
    pub depth: u8,
    pub level: NavigatorLevel,
    pub collapsed: bool,
    pub has_children: bool,
    /// `true` for the active session and its live panes; `false` for saved sessions.
    pub live: bool,
    /// Pane count rollup (live count for the active session, stored count for saved ones).
    pub pane_count: usize,
    /// Agent state of a live pane row (`None` for every other row).
    pub agent: Option<AgentState>,
    /// Per-state pane counts on the live session/workspace row (`None` for every other row).
    pub counts: Option<AgentCounts>,
    /// Who a live pane row is talking to within [`COMMS_WINDOW`] (`None` for quiet panes and other rows).
    pub comms: Option<CommsIndicator>,
    /// `true` while the newest event of that traffic is younger than [`COMMS_ACTIVE`] (the glyph animates).
    pub comms_active: bool,
    /// Second-line text of a pane row (`user@host  path`); `None` for every other row, when
    /// `drawer.row_lines = 1`, or when the pane exposes neither identity nor path.
    pub detail: Option<String>,
}

/// Partners considered for the comms indicator: events newer than this.
pub const COMMS_WINDOW: Duration = Duration::from_secs(60);
/// The comms glyph animates while the last event is younger than this.
pub const COMMS_ACTIVE: Duration = Duration::from_millis(1500);
/// Maximum chars of a partner short name on a row (T-50-28).
pub const PARTNER_LABEL_CHARS: usize = 6;

/// Pairing / fan-out communication shown at the right edge of a pane row (D-06).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommsIndicator {
    /// Exactly one partner: its sanitised short name and graph node.
    Pair { partner_label: String, partner_key: NodeKey },
    /// Two or more partners.
    FanOut { count: usize },
}

/// Keyboard sub-focus state of the navigator drawer (Phase 50-12).
#[derive(Clone, Debug, Default)]
pub struct NavigatorDrawerState {
    /// `Some(row)`: the comms glyph of drawer row `row` has keyboard focus. It counts only while
    /// `drawer().selected_index == row`, so moving the selection makes it inert.
    pub comms_focus: Option<usize>,
}

impl WindowManager {
    /// `true` while the comms glyph of the selected navigator row has keyboard focus.
    pub(crate) fn navigator_comms_focused(&self) -> bool {
        self.drawer.navigator.comms_focus == Some(self.drawer.selected_index)
    }

    /// Comms indicator of live pane `id` and whether it is flowing, measured against the tracker's last
    /// update instant (so rendering stays deterministic). `None` before the first tracker update.
    fn pane_comms(&self, id: PaneId) -> Option<(CommsIndicator, bool)> {
        let now = self.agents.last_update()?;
        let partners = self.partners_of(id, COMMS_WINDOW, now);
        let active = partners.iter().any(|p| now.saturating_duration_since(p.last) < COMMS_ACTIVE);
        let indicator = match partners.as_slice() {
            [] => return None,
            [one] => CommsIndicator::Pair {
                partner_label: sanitize_display(&one.endpoint.label(), PARTNER_LABEL_CHARS),
                partner_key: NodeKey::from_endpoint(&one.endpoint),
            },
            many => CommsIndicator::FanOut { count: many.len() },
        };
        Some((indicator, active))
    }

    /// `true` when any navigator pane row currently shows flowing traffic (drives the redraw cadence).
    pub(crate) fn navigator_comms_active(&self) -> bool {
        self.config.drawer.style == DrawerStyle::Navigator
            && self.ordered_pane_ids().into_iter().any(|id| self.pane_comms(id).is_some_and(|(_, a)| a))
    }
}

impl WorkspaceDrawer {
    /// Animation tick used for the working spinner (deterministic in tests).
    pub fn spinner_tick(&self) -> usize {
        self.tick_counter
    }
}

impl DrawerConfig {
    /// Lines per pane row in Navigator style: `1` or `2` (anything but `1` means `2`).
    pub const fn navigator_row_lines(&self) -> u8 {
        if self.row_lines == 1 { 1 } else { 2 }
    }

    /// Drawer width in columns for the active style (navigator keeps its own width).
    pub fn effective_width_cols(&self) -> u16 {
        match self.style {
            DrawerStyle::Navigator => self.navigator_width_cols,
            DrawerStyle::Classic => self.width_cols,
        }
    }

    /// Writes `cols` to the width field owned by the active style (never crosses styles).
    pub fn set_width_cols_for_style(&mut self, cols: u16) {
        match self.style {
            DrawerStyle::Navigator => self.navigator_width_cols = cols,
            DrawerStyle::Classic => self.width_cols = cols,
        }
    }
}

impl WindowManager {
    /// Builds the drawer rows for the configured [`DrawerStyle`]; the classic body lives in
    /// `classic_drawer_rows` (parent module) and is unchanged.
    pub fn drawer_rows(&self) -> Vec<DrawerRow> {
        match self.config.drawer.style {
            DrawerStyle::Classic => self.classic_drawer_rows(),
            DrawerStyle::Navigator => self.navigator_drawer_rows(),
        }
    }

    /// Navigator rows (`FLT-01`): workspaces (session-tree roots) > sessions (descendants) > live panes.
    ///
    /// Live panes appear only under the active session (`config.history_session`); saved sessions show
    /// a `pane_count` rollup. Quick terminal and fleet nodes follow at depth 0 as `Other`.
    pub(crate) fn navigator_drawer_rows(&self) -> Vec<DrawerRow> {
        let tree = &self.daemon_state.session_tree;
        let active = self.config.history_session.as_str();
        let saved = self.drawer.cached_saved_sessions(|| self.list_sessions());
        let ctx = WalkCtx { wm: self, active, pane_ids: self.ordered_pane_ids() };
        let mut rows = Vec::new();
        let mut visited = BTreeSet::new();

        if tree.node(active).is_none() {
            let node = SessionTreeNode::new(active, SessionNodeKind::Independent, None);
            ctx.walk(&node, 0, &mut visited, &mut rows);
        }
        for node in &tree.nodes {
            let is_root = node.parent.as_deref().is_none_or(|p| tree.node(p).is_none());
            let explicit = node.parent.is_some() || !node.children.is_empty() || node.name != "main";
            if is_root && (explicit || node.name == active || saved.contains(&node.name)) {
                ctx.walk(node, 0, &mut visited, &mut rows);
            }
        }
        for name in saved {
            if !visited.contains(&name) && tree.node(&name).is_none() && name != active {
                let mut node = SessionTreeNode::new(name, SessionNodeKind::Independent, None);
                node.pane_count = 0;
                ctx.walk(&node, 0, &mut visited, &mut rows);
            }
        }

        if self.quick_terminal.visible {
            rows.push(navigator_row(
                DrawerRowKind::QuickTerminal,
                "Quick Terminal".to_string(),
                true,
                other_meta(),
            ));
        }
        for name in self.fleet.nodes.keys() {
            rows.push(navigator_row(
                DrawerRowKind::FleetNode(name.clone()),
                sanitize_display(name, TITLE_MAX_CHARS),
                false,
                other_meta(),
            ));
        }
        rows
    }
}

/// Maximum chars of any title/name that reaches the layout (T-50-04).
const TITLE_MAX_CHARS: usize = 64;
/// Maximum tree depth emitted (T-50-04).
const MAX_DEPTH: usize = 8;

fn other_meta() -> NavigatorRowMeta {
    NavigatorRowMeta {
        depth: 0,
        level: NavigatorLevel::Other,
        collapsed: false,
        has_children: false,
        live: true,
        pane_count: 0,
        agent: None,
        counts: None,
        comms: None,
        comms_active: false,
        detail: None,
    }
}

/// Read-only context for the depth-first session-tree walk.
struct WalkCtx<'a> {
    wm: &'a WindowManager,
    active: &'a str,
    pane_ids: Vec<PaneId>,
}

impl WalkCtx<'_> {
    fn walk(&self, node: &SessionTreeNode, depth: usize, visited: &mut BTreeSet<String>, rows: &mut Vec<DrawerRow>) {
        if depth > MAX_DEPTH || !visited.insert(node.name.clone()) {
            return;
        }
        let tree = &self.wm.daemon_state.session_tree;
        let live = node.name == self.active;
        let children: Vec<&SessionTreeNode> = node.children.iter().filter_map(|c| tree.node(c)).collect();
        let has_children = !children.is_empty() || (live && !self.pane_ids.is_empty());
        let collapsed = node.collapsed && has_children;
        rows.push(navigator_row(
            DrawerRowKind::SessionHeader(node.name.clone()),
            sanitize_display(&node.name, TITLE_MAX_CHARS),
            false,
            NavigatorRowMeta {
                depth: depth as u8,
                level: if depth == 0 { NavigatorLevel::Workspace } else { NavigatorLevel::Session },
                collapsed,
                has_children,
                live,
                pane_count: if live { self.pane_ids.len() } else { node.pane_count },
                agent: None,
                counts: live.then(|| self.wm.agent_counts()),
                comms: None,
                comms_active: false,
                detail: None,
            },
        ));
        if collapsed {
            return;
        }
        if live {
            self.push_panes(depth + 1, rows);
        }
        for child in children {
            self.walk(child, depth + 1, visited, rows);
        }
    }

    fn push_panes(&self, depth: usize, rows: &mut Vec<DrawerRow>) {
        let top_id = self.wm.z_stack.top();
        let qt_visible = self.wm.quick_terminal.visible;
        let two_line = self.wm.config.drawer.navigator_row_lines() == 2;
        for &id in &self.pane_ids {
            let Some(pane) = self.wm.panes.get(&id) else { continue };
            let (comms, comms_active) = match self.wm.pane_comms(id) {
                Some((c, a)) => (Some(c), a),
                None => (None, false),
            };
            let is_selected = self.wm.is_window_selected(id);
            let raw_title = sanitize_display(pane.title(), TITLE_MAX_CHARS);
            let title = if is_selected {
                format!("[✓] {raw_title}")
            } else {
                raw_title
            };
            rows.push(navigator_row(
                DrawerRowKind::Pane(id),
                title,
                !qt_visible && top_id == Some(id),
                NavigatorRowMeta {
                    depth: depth.min(usize::from(u8::MAX)) as u8,
                    level: NavigatorLevel::Pane,
                    collapsed: false,
                    has_children: false,
                    live: true,
                    pane_count: 0,
                    agent: self.wm.agent_state(id),
                    counts: None,
                    comms,
                    comms_active,
                    detail: two_line.then(|| super::navigator_detail::pane_detail_line(pane)).flatten(),
                },
            ));
        }
    }
}

/// Builds a navigator row with every classic-only field neutral.
fn navigator_row(kind: DrawerRowKind, title: String, is_focused_pane: bool, meta: NavigatorRowMeta) -> DrawerRow {
    DrawerRow {
        kind,
        connector: "",
        focus_rank_badge: String::new(),
        spinner: ' ',
        sparkline: String::new(),
        cmd_badge: String::new(),
        mode_badges: Vec::new(),
        transport_badges: Vec::new(),
        title,
        is_focused_pane,
        navigator: Some(meta),
    }
}
