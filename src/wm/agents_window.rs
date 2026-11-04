//! Agent traffic zoom window (Phase 50-07, D-07/D-08): window state, graph scene aggregated from
//! the traffic bus, node layout and the selection model.
//!
//! The window is NOT a `Modal` variant: like the pinned `:mcp` console it can stay open without
//! owning the keyboard ([`AgentsWindowState::captures_input`]). Entry points (`:agents zoom`,
//! keys) are bound in later plans; everything here is reachable through [`WindowManager`].

use std::collections::{BTreeMap, BTreeSet};

use ratatui::layout::Rect;

use crate::pane::PaneId;
use crate::vt::sanitize_display;
use crate::wm::WindowManager;
use crate::wm::agent_state::AgentState;
use crate::wm::traffic::TrafficEndpoint;

/// Maximum number of graph nodes (live panes are always kept; oldest other endpoints drop first).
pub const NODE_CAP: usize = 64;
/// Maximum characters of a node label.
pub const LABEL_CAP: usize = 12;
/// Smallest workspace in which the window renders at all.
pub const MIN_WORKSPACE: (u16, u16) = (30, 10);
/// Smallest default window size.
const MIN_WINDOW: (u16, u16) = (40, 12);
/// Default share of the inner width given to the graph pane, in percent.
pub const DEFAULT_SPLITTER_PCT: u16 = 55;
/// An edge is "active" while it carried one of the newest [`ACTIVE_EVENTS`] bus events.
const ACTIVE_EVENTS: u64 = 3;
/// Maximum characters of a session / agent name inside a [`NodeKey`].
const KEY_NAME_CAP: usize = 64;

/// Stable string id of a graph node: `user`, `api`, `mcp`, `pane:<id>`, `session:<name>`, `agent:<name>`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeKey(pub String);

impl NodeKey {
    /// Maps a bus endpoint to its node key.
    pub fn from_endpoint(ep: &TrafficEndpoint) -> Self {
        Self(match ep {
            TrafficEndpoint::User => "user".to_string(),
            TrafficEndpoint::Api => "api".to_string(),
            TrafficEndpoint::Mcp => "mcp".to_string(),
            TrafficEndpoint::Pane(id) => format!("pane:{id}"),
            TrafficEndpoint::Session(n) => format!("session:{}", sanitize_display(n, KEY_NAME_CAP)),
            TrafficEndpoint::Agent(n) => format!("agent:{}", sanitize_display(n, KEY_NAME_CAP)),
        })
    }

    /// Key of the node for pane `id`.
    pub fn pane(id: PaneId) -> Self {
        Self(format!("pane:{id}"))
    }

    /// The key as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Pane id when this key names a pane.
    pub fn pane_id(&self) -> Option<PaneId> {
        self.0.strip_prefix("pane:").and_then(|s| s.parse().ok())
    }
}

/// Direction of a selection move on the graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphDir {
    Left,
    Right,
    Up,
    Down,
}

/// One node of the scene.
#[derive(Clone, Debug)]
pub struct SceneNode {
    pub key: NodeKey,
    /// Sanitised label (at most [`LABEL_CAP`] chars).
    pub label: String,
    /// Normalised position, `0.0..=1.0` on both axes.
    pub pos: (f32, f32),
    /// Agent state for pane nodes.
    pub state: Option<AgentState>,
    pub selected: bool,
}

/// One undirected edge of the scene (`from < to` by key).
#[derive(Clone, Debug)]
pub struct SceneEdge {
    pub from: NodeKey,
    pub to: NodeKey,
    /// Number of bus events between the two endpoints (either direction).
    pub count: usize,
    /// Sequence number of the newest such event.
    pub last_seq: u64,
    /// `true` while the edge carried one of the newest bus events.
    pub active: bool,
    pub selected: bool,
}

/// Nodes and edges to draw, positions normalised.
#[derive(Clone, Debug, Default)]
pub struct GraphScene {
    pub nodes: Vec<SceneNode>,
    pub edges: Vec<SceneEdge>,
}

/// Selected node and (optionally) one of its incident edges.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphSelection {
    pub node: Option<NodeKey>,
    /// Edge identified by its `(from, to)` pair (`from < to`).
    pub edge: Option<(NodeKey, NodeKey)>,
}

/// Node placement state: positions pinned by the user (node drag, 50-09).
#[derive(Clone, Debug, Default)]
pub struct GraphLayout {
    pub pinned: BTreeMap<NodeKey, (f32, f32)>,
}

impl GraphLayout {
    /// Places `keys` (sorted) on a deterministic row-major grid; pinned nodes keep their spot.
    pub fn place(&self, keys: &[NodeKey], _edges: &[(usize, usize)]) -> Vec<(f32, f32)> {
        let n = keys.len().max(1);
        let cols = (n as f32).sqrt().ceil() as usize;
        let cols = cols.max(1);
        let rows = n.div_ceil(cols);
        keys.iter()
            .enumerate()
            .map(|(i, k)| {
                if let Some(p) = self.pinned.get(k) {
                    return *p;
                }
                let (c, r) = (i % cols, i / cols);
                let x = (c as f32 + 0.5) / cols as f32;
                let y = (r as f32 + 0.5) / rows as f32;
                (0.05 + x * 0.9, 0.05 + y * 0.9)
            })
            .collect()
    }
}

/// Runtime state of the AGENT TRAFFIC window.
#[derive(Clone, Debug)]
pub struct AgentsWindowState {
    pub active: bool,
    /// Pinned windows stay open and only take the keyboard while `focused`.
    pub pinned: bool,
    pub focused: bool,
    /// User-chosen rect; `None` = default centred rect.
    pub rect: Option<Rect>,
    pub maximized: bool,
    /// Share of the inner width given to the graph pane, percent.
    pub splitter_pct: u16,
    pub selection: GraphSelection,
    pub layout: GraphLayout,
}

impl Default for AgentsWindowState {
    fn default() -> Self {
        Self {
            active: false,
            pinned: false,
            focused: false,
            rect: None,
            maximized: false,
            splitter_pct: DEFAULT_SPLITTER_PCT,
            selection: GraphSelection::default(),
            layout: GraphLayout::default(),
        }
    }
}

impl AgentsWindowState {
    /// `true` when key events belong to the window: open and either unpinned or focused.
    pub fn captures_input(&self) -> bool {
        self.active && (!self.pinned || self.focused)
    }
}

/// Aggregation record for one candidate node.
struct NodeAcc {
    label: String,
    live_pane: Option<PaneId>,
    last_seq: u64,
}

impl WindowManager {
    /// The agents window state.
    pub fn agents_window(&self) -> &AgentsWindowState {
        &self.agents_window
    }

    /// Mutable agents window state.
    pub fn agents_window_mut(&mut self) -> &mut AgentsWindowState {
        &mut self.agents_window
    }

    /// Opens the window (focused) and selects `focus` when it names a live pane.
    pub fn open_agents_window(&mut self, focus: Option<PaneId>) {
        self.agents_window.active = true;
        self.agents_window.focused = true;
        if let Some(id) = focus
            && self.panes.contains_key(&id)
        {
            self.agents_window.selection =
                GraphSelection { node: Some(NodeKey::pane(id)), edge: None };
        }
    }

    /// Closes the window and drops focus and pin.
    pub fn close_agents_window(&mut self) {
        self.agents_window.active = false;
        self.agents_window.focused = false;
        self.agents_window.pinned = false;
    }

    /// Window rect inside `workspace_area`, or `None` when inactive or the workspace is too small.
    pub fn agents_window_rect(&self, workspace_area: Rect) -> Option<Rect> {
        let w = &self.agents_window;
        if !w.active
            || workspace_area.width < MIN_WORKSPACE.0
            || workspace_area.height < MIN_WORKSPACE.1
        {
            return None;
        }
        if w.maximized {
            return Some(workspace_area);
        }
        if let Some(r) = w.rect {
            let clamped = r.intersection(workspace_area);
            if clamped.width >= 4 && clamped.height >= 3 {
                return Some(clamped);
            }
        }
        let width = (workspace_area.width * 80 / 100)
            .max(MIN_WINDOW.0)
            .min(workspace_area.width);
        let height = (workspace_area.height * 70 / 100)
            .max(MIN_WINDOW.1)
            .min(workspace_area.height);
        let x = workspace_area.x + (workspace_area.width - width) / 2;
        let y = workspace_area.y + (workspace_area.height - height) / 2;
        Some(Rect::new(x, y, width, height))
    }

    /// Builds the graph scene from the traffic bus plus one node per live pane.
    pub fn agents_graph_scene(&self) -> GraphScene {
        let mut acc: BTreeMap<NodeKey, NodeAcc> = BTreeMap::new();
        let mut edge_acc: BTreeMap<(NodeKey, NodeKey), (usize, u64)> = BTreeMap::new();

        let mut touch = |ep: &TrafficEndpoint, seq: u64, panes: &BTreeSet<PaneId>| -> NodeKey {
            let key = NodeKey::from_endpoint(ep);
            let slot = acc.entry(key.clone()).or_insert_with(|| NodeAcc {
                label: node_label(ep),
                live_pane: match ep {
                    TrafficEndpoint::Pane(id) if panes.contains(id) => Some(*id),
                    _ => None,
                },
                last_seq: 0,
            });
            slot.last_seq = slot.last_seq.max(seq);
            key
        };

        let live: BTreeSet<PaneId> = self.panes.keys().copied().collect();
        for e in self.traffic.iter() {
            let a = touch(&e.from, e.seq, &live);
            let b = touch(&e.to, e.seq, &live);
            if a == b {
                continue;
            }
            let pair = if a < b { (a, b) } else { (b, a) };
            let slot = edge_acc.entry(pair).or_insert((0, 0));
            slot.0 += 1;
            slot.1 = slot.1.max(e.seq);
        }
        for &id in &live {
            touch(&TrafficEndpoint::Pane(id), 0, &live);
        }

        let kept = cap_nodes(&acc);
        let keys: Vec<NodeKey> = kept.iter().cloned().collect();
        let index: BTreeMap<&NodeKey, usize> =
            keys.iter().enumerate().map(|(i, k)| (k, i)).collect();

        let edges_raw: Vec<((NodeKey, NodeKey), (usize, u64))> = edge_acc
            .into_iter()
            .filter(|((a, b), _)| index.contains_key(a) && index.contains_key(b))
            .collect();
        let idx_edges: Vec<(usize, usize)> =
            edges_raw.iter().map(|((a, b), _)| (index[a], index[b])).collect();
        let positions = self.agents_window.layout.place(&keys, &idx_edges);

        let sel = &self.agents_window.selection;
        let latest = self.traffic.latest_seq();
        let nodes = keys
            .iter()
            .zip(positions)
            .map(|(k, pos)| {
                let a = &acc[k];
                SceneNode {
                    key: k.clone(),
                    label: a.label.clone(),
                    pos,
                    state: a.live_pane.and_then(|id| self.agent_state(id)),
                    selected: sel.node.as_ref() == Some(k),
                }
            })
            .collect();
        let edges = edges_raw
            .into_iter()
            .map(|((from, to), (count, last_seq))| SceneEdge {
                selected: sel.edge.as_ref() == Some(&(from.clone(), to.clone())),
                active: last_seq + ACTIVE_EVENTS > latest && last_seq > 0,
                from,
                to,
                count,
                last_seq,
            })
            .collect();
        GraphScene { nodes, edges }
    }
}

/// Sanitised node label: `you`, `#<id>`, `@<session>`, agent name (at most [`LABEL_CAP`] chars).
fn node_label(ep: &TrafficEndpoint) -> String {
    sanitize_display(&ep.label(), LABEL_CAP)
}

/// Applies the node cap: every live pane is kept, the other endpoints newest first.
fn cap_nodes(acc: &BTreeMap<NodeKey, NodeAcc>) -> BTreeSet<NodeKey> {
    let mut kept: BTreeSet<NodeKey> = acc
        .iter()
        .filter(|(_, a)| a.live_pane.is_some())
        .map(|(k, _)| k.clone())
        .collect();
    let mut others: Vec<(&NodeKey, u64)> = acc
        .iter()
        .filter(|(_, a)| a.live_pane.is_none())
        .map(|(k, a)| (k, a.last_seq))
        .collect();
    others.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let room = NODE_CAP.saturating_sub(kept.len());
    kept.extend(others.into_iter().take(room).map(|(k, _)| k.clone()));
    kept
}
