//! `herdr` drawer style: compact workspace > session > pane tree (`FLT-01`, Phase 50 D-02).
//!
//! The classic drawer (`classic_drawer_rows` in the parent module) is untouched. This module owns the
//! style dispatcher [`WindowManager::drawer_rows`], the herdr row builder and the per-style width
//! helpers on [`DrawerConfig`].

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{DrawerConfig, DrawerRow, DrawerRowKind, WorkspaceDrawer};
use crate::pane::PaneId;
use crate::vt::sanitize_display;
use crate::wm::agent_state::{AgentCounts, AgentState};
use crate::wm::{SessionNodeKind, SessionTreeNode, WindowManager};

/// Visual style of the slide-out drawer (`drawer.style`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrawerStyle {
    /// Telemetry-rich classic rows (default, byte-identical to pre-Phase-50 output).
    #[default]
    Classic,
    /// Compact one-line-per-item workspace > session > pane tree.
    Herdr,
}

impl DrawerStyle {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Herdr => "herdr",
        }
    }
    pub const fn toggle(self) -> Self {
        match self {
            Self::Classic => Self::Herdr,
            Self::Herdr => Self::Classic,
        }
    }
}

/// Tree level of a herdr drawer row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HerdrLevel {
    Workspace,
    Session,
    Pane,
    /// Quick terminal and fleet nodes.
    Other,
}

/// Herdr-only metadata attached to a [`DrawerRow`] (`None` for every classic row).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HerdrRowMeta {
    pub depth: u8,
    pub level: HerdrLevel,
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
}

impl WorkspaceDrawer {
    /// Animation tick used for the working spinner (deterministic in tests).
    pub fn spinner_tick(&self) -> usize {
        self.tick_counter
    }
}

impl DrawerConfig {
    /// Drawer width in columns for the active style (herdr keeps its own width).
    pub fn effective_width_cols(&self) -> u16 {
        match self.style {
            DrawerStyle::Herdr => self.herdr_width_cols,
            DrawerStyle::Classic => self.width_cols,
        }
    }

    /// Writes `cols` to the width field owned by the active style (never crosses styles).
    pub fn set_width_cols_for_style(&mut self, cols: u16) {
        match self.style {
            DrawerStyle::Herdr => self.herdr_width_cols = cols,
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
            DrawerStyle::Herdr => self.herdr_drawer_rows(),
        }
    }

    /// Herdr rows (`FLT-01`): workspaces (session-tree roots) > sessions (descendants) > live panes.
    ///
    /// Live panes appear only under the active session (`config.history_session`); saved sessions show
    /// a `pane_count` rollup. Quick terminal and fleet nodes follow at depth 0 as `Other`.
    pub(crate) fn herdr_drawer_rows(&self) -> Vec<DrawerRow> {
        let tree = &self.daemon_state.session_tree;
        let active = self.config.history_session.as_str();
        let saved = self.list_sessions();
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
            rows.push(herdr_row(
                DrawerRowKind::QuickTerminal,
                "Quick Terminal".to_string(),
                true,
                other_meta(),
            ));
        }
        for name in self.fleet.nodes.keys() {
            rows.push(herdr_row(
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

fn other_meta() -> HerdrRowMeta {
    HerdrRowMeta {
        depth: 0,
        level: HerdrLevel::Other,
        collapsed: false,
        has_children: false,
        live: true,
        pane_count: 0,
        agent: None,
        counts: None,
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
        rows.push(herdr_row(
            DrawerRowKind::SessionHeader(node.name.clone()),
            sanitize_display(&node.name, TITLE_MAX_CHARS),
            false,
            HerdrRowMeta {
                depth: depth as u8,
                level: if depth == 0 { HerdrLevel::Workspace } else { HerdrLevel::Session },
                collapsed,
                has_children,
                live,
                pane_count: if live { self.pane_ids.len() } else { node.pane_count },
                agent: None,
                counts: live.then(|| self.wm.agent_counts()),
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
        for &id in &self.pane_ids {
            let Some(pane) = self.wm.panes.get(&id) else { continue };
            rows.push(herdr_row(
                DrawerRowKind::Pane(id),
                sanitize_display(pane.title(), TITLE_MAX_CHARS),
                !qt_visible && top_id == Some(id),
                HerdrRowMeta {
                    depth: depth.min(usize::from(u8::MAX)) as u8,
                    level: HerdrLevel::Pane,
                    collapsed: false,
                    has_children: false,
                    live: true,
                    pane_count: 0,
                    agent: self.wm.agent_state(id),
                    counts: None,
                },
            ));
        }
    }
}

/// Builds a herdr row with every classic-only field neutral.
fn herdr_row(kind: DrawerRowKind, title: String, is_focused_pane: bool, meta: HerdrRowMeta) -> DrawerRow {
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
        herdr: Some(meta),
    }
}
