//! Agent traffic zoom window (Phase 50-07, D-07/D-08): window state, graph scene aggregated from
//! the traffic bus, node layout and the selection model.
//!
//! The window is NOT a `Modal` variant: like the pinned `:mcp` console it can stay open without
//! owning the keyboard ([`AgentsWindowState::captures_input`]). Entry points (`:agents zoom`,
//! keys) are bound in later plans; everything here is reachable through [`WindowManager`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

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

/// Cold-start iteration count of the force-directed layout.
const COLD_ITERATIONS: usize = 60;
/// Warm-start iteration count (positions of known nodes are kept as the starting point).
const WARM_ITERATIONS: usize = 20;
const COLD_TEMPERATURE: f32 = 0.1;
const WARM_TEMPERATURE: f32 = 0.05;
/// Positions are kept inside `[POS_MIN, POS_MAX]` on both axes.
const POS_MIN: f32 = 0.05;
const POS_MAX: f32 = 0.95;
/// A node known from the previous layout never drifts further than this in one re-layout.
const MAX_WARM_DRIFT: f32 = 0.06;

type Pos = (f32, f32);

/// Memory of the last layout: positions by key and the topology they were computed for.
#[derive(Clone, Debug, Default)]
struct LayoutCache {
    prev: BTreeMap<NodeKey, Pos>,
    sig_keys: Vec<NodeKey>,
    sig_edges: Vec<(usize, usize)>,
    sig_pinned: BTreeMap<NodeKey, Pos>,
    result: Vec<Pos>,
}

/// Deterministic, stable node placement (Fruchterman-Reingold, `+ - * / sqrt` only).
///
/// The layout remembers the previous positions, so adding a node starts from the old picture
/// (warm start) and moves existing nodes by at most [`MAX_WARM_DRIFT`]. Asking again for an
/// unchanged topology returns the cached positions, so repeated frames never wobble. `pinned`
/// holds positions fixed by the user (node drag, 50-09).
#[derive(Debug, Default)]
pub struct GraphLayout {
    pub pinned: BTreeMap<NodeKey, (f32, f32)>,
    cache: Mutex<LayoutCache>,
}

impl Clone for GraphLayout {
    fn clone(&self) -> Self {
        Self { pinned: self.pinned.clone(), cache: Mutex::new(self.lock().clone()) }
    }
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Hash-derived unit-interval pair for `key`.
fn hash_unit(key: &NodeKey) -> (f32, f32, u64) {
    let h = fnv1a(key.as_str());
    let fx = ((h >> 8) & 0xffff) as f32 / 65535.0;
    let fy = ((h >> 24) & 0xffff) as f32 / 65535.0;
    (fx, fy, h)
}

fn clamp_pos(p: Pos) -> Pos {
    (p.0.clamp(POS_MIN, POS_MAX), p.1.clamp(POS_MIN, POS_MAX))
}

/// Row-major grid placement; used above [`NODE_CAP`] nodes where the O(n^2) layout is skipped.
fn grid_positions(n: usize) -> Vec<Pos> {
    let cols = ((n.max(1) as f32).sqrt().ceil() as usize).max(1);
    let rows = n.max(1).div_ceil(cols);
    (0..n)
        .map(|i| {
            let (c, r) = (i % cols, i / cols);
            let x = (c as f32 + 0.5) / cols as f32;
            let y = (r as f32 + 0.5) / rows as f32;
            (POS_MIN + x * (POS_MAX - POS_MIN), POS_MIN + y * (POS_MAX - POS_MIN))
        })
        .collect()
}

impl GraphLayout {
    fn lock(&self) -> std::sync::MutexGuard<'_, LayoutCache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Positions for `keys` (sorted, unique) connected by `edges` (index pairs into `keys`).
    pub fn place(&self, keys: &[NodeKey], edges: &[(usize, usize)]) -> Vec<(f32, f32)> {
        let mut cache = self.lock();
        if cache.sig_keys == keys && cache.sig_edges == edges && cache.sig_pinned == self.pinned {
            return cache.result.clone();
        }
        let result = if keys.len() > NODE_CAP {
            let mut grid = grid_positions(keys.len());
            for (i, k) in keys.iter().enumerate() {
                if let Some(p) = self.pinned.get(k) {
                    grid[i] = *p;
                }
            }
            grid
        } else {
            self.force_layout(&cache.prev, keys, edges)
        };
        cache.prev = keys.iter().cloned().zip(result.iter().copied()).collect();
        cache.sig_keys = keys.to_vec();
        cache.sig_edges = edges.to_vec();
        cache.sig_pinned = self.pinned.clone();
        cache.result = result.clone();
        result
    }

    fn force_layout(
        &self,
        prev: &BTreeMap<NodeKey, Pos>,
        keys: &[NodeKey],
        edges: &[(usize, usize)],
    ) -> Vec<Pos> {
        let n = keys.len();
        if n == 0 {
            return Vec::new();
        }
        let mut pos: Vec<Pos> = vec![(0.5, 0.5); n];
        let mut placed = vec![false; n];
        let mut warm = vec![false; n];
        let mut fixed = vec![false; n];
        for (i, k) in keys.iter().enumerate() {
            if let Some(p) = self.pinned.get(k) {
                pos[i] = (p.0.clamp(0.0, 1.0), p.1.clamp(0.0, 1.0));
                placed[i] = true;
                fixed[i] = true;
            } else if let Some(p) = prev.get(k) {
                pos[i] = clamp_pos(*p);
                placed[i] = true;
                warm[i] = true;
            }
        }
        let is_warm = warm.iter().any(|w| *w);
        for i in 0..n {
            if placed[i] {
                continue;
            }
            let (fx, fy, h) = hash_unit(&keys[i]);
            let neighbour = edges.iter().find_map(|&(a, b)| {
                if a == i && placed[b] {
                    Some(b)
                } else if b == i && placed[a] {
                    Some(a)
                } else {
                    None
                }
            });
            pos[i] = match neighbour {
                Some(j) => {
                    let dx = (0.04 + fx * 0.06) * if h & 1 == 1 { 1.0 } else { -1.0 };
                    let dy = (0.04 + fy * 0.06) * if h & 2 == 2 { 1.0 } else { -1.0 };
                    clamp_pos((pos[j].0 + dx, pos[j].1 + dy))
                }
                None => (0.1 + fx * 0.8, 0.1 + fy * 0.8),
            };
            placed[i] = true;
        }
        let start = pos.clone();

        let (iterations, t0) = if is_warm {
            (WARM_ITERATIONS, WARM_TEMPERATURE)
        } else {
            (COLD_ITERATIONS, COLD_TEMPERATURE)
        };
        let k = ((POS_MAX - POS_MIN) * (POS_MAX - POS_MIN) / n as f32).sqrt();
        for it in 0..iterations {
            let temp = t0 * (1.0 - it as f32 / iterations as f32);
            let mut disp: Vec<Pos> = vec![(0.0, 0.0); n];
            for i in 0..n {
                for j in (i + 1)..n {
                    let (dx, dy) = (pos[i].0 - pos[j].0, pos[i].1 - pos[j].1);
                    let d = (dx * dx + dy * dy).sqrt();
                    // Coincident nodes are pushed apart along +x / -x (deterministic).
                    let (ux, uy, d) = if d < 1e-3 { (1.0, 0.0, 1e-3) } else { (dx / d, dy / d, d) };
                    let f = k * k / d;
                    disp[i].0 += ux * f;
                    disp[i].1 += uy * f;
                    disp[j].0 -= ux * f;
                    disp[j].1 -= uy * f;
                }
            }
            for &(a, b) in edges {
                if a == b {
                    continue;
                }
                let (dx, dy) = (pos[a].0 - pos[b].0, pos[a].1 - pos[b].1);
                let d = (dx * dx + dy * dy).sqrt().max(1e-3);
                let f = d * d / k;
                let (ux, uy) = (dx / d, dy / d);
                disp[a].0 -= ux * f;
                disp[a].1 -= uy * f;
                disp[b].0 += ux * f;
                disp[b].1 += uy * f;
            }
            for i in 0..n {
                if fixed[i] {
                    continue;
                }
                let len = (disp[i].0 * disp[i].0 + disp[i].1 * disp[i].1).sqrt();
                if len > 1e-9 {
                    let step = len.min(temp);
                    pos[i] = clamp_pos((
                        pos[i].0 + disp[i].0 / len * step,
                        pos[i].1 + disp[i].1 / len * step,
                    ));
                }
            }
        }
        // Stability: nodes known from the previous picture stay close to where they were.
        for i in 0..n {
            if !warm[i] {
                continue;
            }
            let (dx, dy) = (pos[i].0 - start[i].0, pos[i].1 - start[i].1);
            let d = (dx * dx + dy * dy).sqrt();
            if d > MAX_WARM_DRIFT {
                let s = MAX_WARM_DRIFT / d;
                pos[i] = (start[i].0 + dx * s, start[i].1 + dy * s);
            }
        }
        pos
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

impl WindowManager {
    /// Selects the node with `key`; clears the edge selection. `false` when it is not in the scene.
    pub fn agents_select_node(&mut self, key: &NodeKey) -> bool {
        let scene = self.agents_graph_scene();
        if !scene.nodes.iter().any(|n| &n.key == key) {
            return false;
        }
        self.agents_window.selection = GraphSelection { node: Some(key.clone()), edge: None };
        true
    }

    /// Moves the selection to the nearest node in direction `dir` (dot-product filter, then
    /// Euclidean distance, ties by key). With nothing selected the leftmost node is selected.
    /// `false` (selection unchanged) when no node lies in that direction.
    pub fn agents_select_direction(&mut self, dir: GraphDir) -> bool {
        let scene = self.agents_graph_scene();
        let current = self
            .agents_window
            .selection
            .node
            .as_ref()
            .and_then(|k| scene.nodes.iter().find(|n| &n.key == k));
        let target = match current {
            None => scene.nodes.iter().min_by(|a, b| {
                a.pos
                    .0
                    .total_cmp(&b.pos.0)
                    .then_with(|| a.pos.1.total_cmp(&b.pos.1))
                    .then_with(|| a.key.cmp(&b.key))
            }),
            Some(cur) => {
                let (ux, uy): (f32, f32) = match dir {
                    GraphDir::Left => (-1.0, 0.0),
                    GraphDir::Right => (1.0, 0.0),
                    GraphDir::Up => (0.0, -1.0),
                    GraphDir::Down => (0.0, 1.0),
                };
                let dist2 = |n: &SceneNode| {
                    let (dx, dy) = (n.pos.0 - cur.pos.0, n.pos.1 - cur.pos.1);
                    dx * dx + dy * dy
                };
                scene
                    .nodes
                    .iter()
                    .filter(|n| {
                        n.key != cur.key
                            && (n.pos.0 - cur.pos.0) * ux + (n.pos.1 - cur.pos.1) * uy > 0.0
                    })
                    .min_by(|a, b| {
                        dist2(a).total_cmp(&dist2(b)).then_with(|| a.key.cmp(&b.key))
                    })
            }
        };
        let Some(target) = target else {
            return false;
        };
        let key = target.key.clone();
        self.agents_window.selection = GraphSelection { node: Some(key), edge: None };
        true
    }

    /// Cycles through the selected node's incident edges (sorted by the other endpoint's key),
    /// wrapping at both ends. `false` when no node is selected or it has no edges.
    pub fn agents_cycle_edge(&mut self, forward: bool) -> bool {
        let Some(node) = self.agents_window.selection.node.clone() else {
            return false;
        };
        let scene = self.agents_graph_scene();
        let mut incident: Vec<(NodeKey, (NodeKey, NodeKey))> = scene
            .edges
            .iter()
            .filter_map(|e| {
                let other = if e.from == node {
                    &e.to
                } else if e.to == node {
                    &e.from
                } else {
                    return None;
                };
                Some((other.clone(), (e.from.clone(), e.to.clone())))
            })
            .collect();
        if incident.is_empty() {
            return false;
        }
        incident.sort_by(|a, b| a.0.cmp(&b.0));
        let n = incident.len();
        let at = self
            .agents_window
            .selection
            .edge
            .as_ref()
            .and_then(|sel| incident.iter().position(|(_, pair)| pair == sel));
        let next = match (at, forward) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(i), true) => (i + 1) % n,
            (Some(i), false) => (i + n - 1) % n,
        };
        self.agents_window.selection.edge = Some(incident[next].1.clone());
        true
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
