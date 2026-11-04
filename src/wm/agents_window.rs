//! Agent traffic zoom window (Phase 50-07, D-07/D-08): window state, graph scene aggregated from
//! the traffic bus, node layout and the selection model.
//!
//! The window is NOT a `Modal` variant: like the pinned `:mcp` console it can stay open without
//! owning the keyboard ([`AgentsWindowState::captures_input`]). Entry points (`:agents zoom`,
//! keys) are bound in later plans; everything here is reachable through [`WindowManager`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::Duration;

use ratatui::layout::Rect;

use crate::pane::PaneId;
use crate::vt::sanitize_display;
use crate::wm::agent_state::AgentState;
use crate::wm::agents_cmd::{KeywordEdit, SnoopFilterKind};
use crate::wm::{WindowManager, WindowResizeEdge};
use crate::wm::traffic::{TrafficEndpoint, TrafficEvent, TrafficKind};

/// Maximum number of graph nodes (live panes are always kept; oldest other endpoints drop first).
pub const NODE_CAP: usize = 64;
/// Maximum characters of a node label.
pub const LABEL_CAP: usize = 12;
/// Smallest workspace in which the window renders at all.
pub const MIN_WORKSPACE: (u16, u16) = (30, 10);
/// Smallest default window size (also the resize floor, 50-09).
pub const MIN_WINDOW: (u16, u16) = (40, 12);
/// Default share of the inner width given to the graph pane, in percent.
pub const DEFAULT_SPLITTER_PCT: u16 = 55;
/// An edge is "active" while it carried one of the newest [`ACTIVE_EVENTS`] bus events.
const ACTIVE_EVENTS: u64 = 3;
/// Maximum characters of a session / agent name inside a [`NodeKey`].
const KEY_NAME_CAP: usize = 64;

/// Node-key form of an untrusted session / agent name (WR-04).
///
/// A name that survives sanitising unchanged (and holds no `~`) is its own key, so ordinary keys
/// stay readable and typable. Any other name gets the sanitised, capped text plus `~<hash of the
/// RAW name>`; since only that class contains `~`, two distinct raw names can never share a key
/// (short of a 64-bit hash collision), whatever control, bidi, invisible or over-long text they
/// differ in. The visible label still comes from the sanitised text alone.
fn name_key(raw: &str) -> String {
    use std::hash::{Hash, Hasher};
    let clean = sanitize_display(raw, KEY_NAME_CAP);
    if clean == raw && !raw.contains('~') {
        return clean;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    raw.hash(&mut h);
    format!("{}~{:016x}", clean.replace('~', "-"), h.finish())
}

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
            TrafficEndpoint::Session(n) => format!("session:{}", name_key(n)),
            TrafficEndpoint::Agent(n) => format!("agent:{}", name_key(n)),
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

/// Pointer gesture in progress on the window (bookkeeping only, 50-09). Which drag is running is
/// not an operation: the motion that follows lowers to `SetRect`, `Split` or `PinNode`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentsDrag {
    /// Title-bar drag; the pointer keeps its offset inside the window.
    Move { grab_dx: u16, grab_dy: u16 },
    /// Edge or corner drag, relative to the rect at press time.
    Resize { edge: WindowResizeEdge, initial: Rect },
    /// Graph/conversation splitter drag.
    Splitter,
    /// Press on a node; `moved` tells a click (activate) from a drag (pin).
    Node { key: NodeKey, was_selected: bool, moved: bool },
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
    /// Pointer gesture in progress (bookkeeping only).
    pub drag: Option<AgentsDrag>,
    /// While on, the arrow keys nudge the selected node instead of moving the cursor.
    pub grab_mode: bool,
    /// Which part of the window owns the keyboard.
    pub pane_focus: WindowPane,
    /// Conversation (snoop) view state.
    pub snoop: SnoopState,
}

/// Part of the AGENT TRAFFIC window that owns the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowPane {
    Graph,
    Snoop,
    Compose,
}

impl WindowPane {
    /// Next pane in the Ctrl+O cycle: graph, conversation, compose, graph.
    pub fn next(self) -> Self {
        match self {
            Self::Graph => Self::Snoop,
            Self::Snoop => Self::Compose,
            Self::Compose => Self::Graph,
        }
    }
}

/// Whether the `send as me` compose line can write anywhere.
///
/// Phase 50 has no outward write path (D-09): the gate is always `Unavailable`. Phase 51 adds an
/// open variant behind its permission gate, rate limits, loop guard and logging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposeGate {
    /// Disabled; the text says why.
    Unavailable(&'static str),
}

/// Maximum characters of a keyword filter.
pub const KEYWORD_CAP: usize = 64;

/// State of the conversation (snoop) pane: read-only follow, filters, history scroll, copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnoopState {
    pub filter_agent: Option<NodeKey>,
    /// Unordered pair, stored `(smaller, larger)` by key.
    pub filter_pair: Option<(NodeKey, NodeKey)>,
    pub filter_text: Option<String>,
    /// Keep the newest line visible.
    pub follow: bool,
    /// Index (in the filtered lines, from the oldest) of the bottom visible line; used only while
    /// `follow` is off, so new traffic never moves the view.
    pub scroll: usize,
    /// Bus sequence number of the line `scroll` points at. The view is anchored on this so ring
    /// eviction (which shifts every index down) never makes a scrolled-back view creep forward
    /// (WR-03). Read through [`SnoopState::effective_scroll`].
    pub scroll_seq: Option<u64>,
    /// Selected line (bus sequence number); `None` = the newest line.
    pub selected_seq: Option<u64>,
    /// Keyword being typed (`Some` while the entry line is open).
    pub keyword_entry: Option<String>,
    /// Text of the last copied line.
    pub last_copy: Option<String>,
}

impl Default for SnoopState {
    fn default() -> Self {
        Self {
            filter_agent: None,
            filter_pair: None,
            filter_text: None,
            follow: true,
            scroll: 0,
            scroll_seq: None,
            selected_seq: None,
            keyword_entry: None,
            last_copy: None,
        }
    }
}

impl SnoopState {
    /// Bottom visible line index (while follow is off) resolved against the current `lines`: the
    /// anchored sequence number when it is still present, the oldest line when it was evicted,
    /// else the raw `scroll` index.
    pub fn effective_scroll(&self, lines: &[SnoopLine]) -> usize {
        match self.scroll_seq {
            Some(seq) if !self.follow => lines.iter().position(|l| l.seq == seq).unwrap_or(0),
            _ => self.scroll,
        }
    }

    /// Points the view at line `idx` of `lines`, remembering its sequence number.
    fn anchor_scroll(&mut self, lines: &[SnoopLine], idx: usize) {
        self.scroll = idx;
        self.scroll_seq = lines.get(idx).map(|l| l.seq);
    }
}

/// One conversation line, derived from a bus event.
#[derive(Clone, Debug)]
pub struct SnoopLine {
    pub seq: u64,
    /// Time since the event, measured against the agent tracker's last update (`None` before it ran).
    pub age: Option<Duration>,
    pub from_label: String,
    pub to_label: String,
    pub kind: TrafficKind,
    /// Sanitised preview (`[redacted]` when withheld).
    pub preview: String,
    pub redacted: bool,
}

/// Half-open range `[start, end)` of filtered-line indexes visible in `rows` rows.
///
/// Following shows the newest lines; otherwise `scroll` is the bottom visible index (clamped so the
/// oldest line is never pushed below the top edge).
pub fn snoop_view_range(len: usize, rows: usize, follow: bool, scroll: usize) -> (usize, usize) {
    if len == 0 || rows == 0 {
        return (0, 0);
    }
    let shown = rows.min(len);
    let bottom = if follow { len - 1 } else { scroll.min(len - 1).max(shown - 1) };
    (bottom + 1 - shown, bottom + 1)
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
            drag: None,
            grab_mode: false,
            pane_focus: WindowPane::Graph,
            snoop: SnoopState::default(),
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
        self.agents_window.drag = None;
        self.agents_window.grab_mode = false;
        self.agents_window.pane_focus = WindowPane::Graph;
        self.agents_window.snoop.keyword_entry = None;
    }

    /// Stores (or clears) the pointer-gesture bookkeeping of the window.
    pub fn set_agents_drag(&mut self, drag: Option<AgentsDrag>) {
        self.agents_window.drag = drag;
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

// ---- Conversation (snoop) view: lines, selection, copy (Phase 50-10, D-09) ----

/// Characters of a copied line.
const COPY_CAP: usize = 400;

impl WindowManager {
    fn snoop_matches(&self, e: &TrafficEvent) -> bool {
        let s = &self.agents_window.snoop;
        let (from, to) = (NodeKey::from_endpoint(&e.from), NodeKey::from_endpoint(&e.to));
        if let Some(a) = &s.filter_agent
            && from != *a
            && to != *a
        {
            return false;
        }
        if let Some((a, b)) = &s.filter_pair
            && !((from == *a && to == *b) || (from == *b && to == *a))
        {
            return false;
        }
        if let Some(t) = &s.filter_text
            && !e.preview.to_lowercase().contains(&t.to_lowercase())
        {
            return false;
        }
        true
    }

    /// Bus events that pass the active filters, oldest first (newest last).
    pub fn snoop_lines(&self) -> Vec<SnoopLine> {
        let now = self.agents.last_update();
        self.traffic
            .iter()
            .filter(|e| self.snoop_matches(e))
            .map(|e| SnoopLine {
                seq: e.seq,
                age: now.map(|n| n.saturating_duration_since(e.at)),
                from_label: e.from.label(),
                to_label: e.to.label(),
                kind: e.kind,
                preview: e.preview.clone(),
                redacted: e.redacted,
            })
            .collect()
    }

    /// Sequence number of the effective selection: the selected line while it is visible, else
    /// the newest line.
    pub fn snoop_selected_seq(&self) -> Option<u64> {
        let lines = self.snoop_lines();
        let wanted = self.agents_window.snoop.selected_seq;
        wanted
            .filter(|w| lines.iter().any(|l| l.seq == *w))
            .or_else(|| lines.last().map(|l| l.seq))
    }

    /// Selects the line `seq`. `false` when it is not in the filtered view.
    pub fn select_snoop_seq(&mut self, seq: u64) -> bool {
        let lines = self.snoop_lines();
        let Some(idx) = lines.iter().position(|l| l.seq == seq) else {
            return false;
        };
        self.agents_window.snoop.selected_seq = Some(seq);
        self.keep_snoop_selection_visible(idx, &lines);
        true
    }

    /// Moves the selection by `delta` lines (clamped). `false` when there are no lines.
    pub fn step_snoop_selection(&mut self, delta: i16) -> bool {
        let lines = self.snoop_lines();
        let Some(cur) = self.snoop_selected_seq() else {
            return false;
        };
        let at = lines.iter().position(|l| l.seq == cur).unwrap_or(lines.len() - 1);
        let to = (at as i64 + i64::from(delta)).clamp(0, lines.len() as i64 - 1) as usize;
        self.agents_window.snoop.selected_seq = Some(lines[to].seq);
        self.keep_snoop_selection_visible(to, &lines);
        true
    }

    /// Scrolls so line `idx` is inside the viewport; leaving the newest lines turns follow off.
    fn keep_snoop_selection_visible(&mut self, idx: usize, lines: &[SnoopLine]) {
        let rows = self.snoop_view_rows();
        if rows == 0 {
            return;
        }
        let s = &mut self.agents_window.snoop;
        let (start, end) = snoop_view_range(lines.len(), rows, s.follow, s.effective_scroll(lines));
        if idx < start {
            s.follow = false;
            s.anchor_scroll(lines, idx + rows - 1);
        } else if idx >= end {
            s.follow = false;
            s.anchor_scroll(lines, idx);
        }
    }

    /// `true` when the window owns the keyboard AND is actually drawn. A window hidden by a tiny
    /// workspace (below the minimum size) must not swallow keys the user cannot see it handle (WR-01).
    pub fn agents_window_captures_input(&self) -> bool {
        self.agents_window.captures_input() && self.agents_window_rect(self.agents_workspace()).is_some()
    }

    /// Rows available for conversation lines (0 when the window is not visible).
    fn snoop_view_rows(&self) -> usize {
        let ws = self.agents_workspace();
        self.agents_pane_rects(ws)
            .map_or(0, |r| usize::from(crate::ui::agents_ui::snoop_lines_area(self, r.snoop).height))
    }

    /// Copies the selected line (`<from> -> <to> [<kind>] <preview>`) to the clipboard and keeps
    /// it in `last_copy`. Redacted previews stay `[redacted]`. `None` when there is nothing to copy.
    pub fn copy_selected_snoop(&mut self) -> Option<String> {
        let seq = self.snoop_selected_seq()?;
        let line = self.snoop_lines().into_iter().find(|l| l.seq == seq)?;
        let raw = format!("{} -> {} [{}] {}", line.from_label, line.to_label, line.kind.as_str(), line.preview);
        let text = sanitize_display(&raw, COPY_CAP);
        crate::clipboard::set_system_clipboard(&text);
        self.agents_window.snoop.last_copy = Some(text.clone());
        Some(text)
    }

    /// State of the compose line. Always [`ComposeGate::Unavailable`] in Phase 50.
    pub fn compose_gate(&self) -> ComposeGate {
        ComposeGate::Unavailable("agent messaging requires Phase 51 ([api] allow_agent_messaging)")
    }

    /// Focuses `pane`, or the next one in the cycle (`None`). Leaves an open keyword entry.
    pub fn focus_agents_pane(&mut self, pane: Option<WindowPane>) {
        let w = &mut self.agents_window;
        w.pane_focus = pane.unwrap_or_else(|| w.pane_focus.next());
        w.snoop.keyword_entry = None;
        if w.pane_focus != WindowPane::Graph {
            w.grab_mode = false;
        }
    }
}

// ---- Conversation filters, follow and history scroll (Phase 50-10) ----

/// Human label of a node key: `you`, `#<id>`, `@<session>`, agent name.
pub fn key_label(key: &NodeKey) -> String {
    match key.as_str() {
        "user" => "you".to_string(),
        k => match k.split_once(':') {
            Some(("pane", id)) => format!("#{id}"),
            Some(("session", n)) => format!("@{n}"),
            Some(("agent", n)) => n.to_string(),
            _ => k.to_string(),
        },
    }
}

impl WindowManager {
    /// Back to the newest lines after a filter change; drops a selection that is filtered out.
    fn reset_snoop_view(&mut self) {
        let visible = self.snoop_selected_seq();
        let s = &mut self.agents_window.snoop;
        s.follow = true;
        s.scroll = 0;
        s.scroll_seq = None;
        if s.selected_seq.is_some() && s.selected_seq != visible {
            s.selected_seq = None;
        }
    }

    /// The node whose key or label is `raw` (`you`, `#2`, `pane:2`, `@session`, agent name).
    pub fn resolve_snoop_node(&self, raw: &str) -> Option<NodeKey> {
        self.agents_graph_scene()
            .nodes
            .into_iter()
            .find(|n| n.key.as_str() == raw || n.label == raw)
            .map(|n| n.key)
    }

    /// Endpoints `(from, to)` of the selected line.
    pub fn snoop_selected_endpoints(&self) -> Option<(NodeKey, NodeKey)> {
        let seq = self.snoop_selected_seq()?;
        self.traffic
            .iter()
            .find(|e| e.seq == seq)
            .map(|e| (NodeKey::from_endpoint(&e.from), NodeKey::from_endpoint(&e.to)))
    }

    /// Keeps only events sent or received by `key`.
    pub fn apply_agent_filter(&mut self, key: &NodeKey) {
        self.agents_window.snoop.filter_agent = Some(key.clone());
        self.reset_snoop_view();
    }

    /// Keeps only events between `a` and `b` (either direction).
    pub fn apply_pair_filter(&mut self, a: &NodeKey, b: &NodeKey) {
        let pair = if a < b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) };
        self.agents_window.snoop.filter_pair = Some(pair);
        self.reset_snoop_view();
    }

    /// Keeps only events whose preview contains `text` (case-insensitive substring, at most
    /// [`KEYWORD_CAP`] characters). Empty text removes the filter.
    pub fn apply_text_filter(&mut self, text: &str) {
        let t: String = text.trim().chars().filter(|c| !c.is_control()).take(KEYWORD_CAP).collect();
        self.agents_window.snoop.filter_text = (!t.is_empty()).then_some(t);
        self.reset_snoop_view();
    }

    /// Removes every filter.
    pub fn clear_snoop_filters(&mut self) {
        let s = &mut self.agents_window.snoop;
        (s.filter_agent, s.filter_pair, s.filter_text) = (None, None, None);
        self.reset_snoop_view();
    }

    /// Removes one filter.
    pub fn remove_snoop_filter(&mut self, kind: SnoopFilterKind) {
        let s = &mut self.agents_window.snoop;
        match kind {
            SnoopFilterKind::Agent => s.filter_agent = None,
            SnoopFilterKind::Pair => s.filter_pair = None,
            SnoopFilterKind::Text => s.filter_text = None,
        }
        self.reset_snoop_view();
    }

    /// Turns follow on (jumping to the newest line), off (keeping the view) or toggles it (`None`).
    pub fn set_snoop_follow(&mut self, mode: Option<bool>) {
        let lines = self.snoop_lines();
        let s = &mut self.agents_window.snoop;
        let on = mode.unwrap_or(!s.follow);
        if on {
            s.follow = true;
            s.scroll = 0;
            s.scroll_seq = None;
        } else {
            if s.follow {
                s.anchor_scroll(&lines, lines.len().saturating_sub(1));
            }
            s.follow = false;
        }
    }

    /// Scrolls the history: negative = towards older lines (turns follow off), positive = newer,
    /// `i16::MIN` = the oldest line. Scrolling down while following does nothing.
    pub fn scroll_snoop(&mut self, n: i16) {
        let lines = self.snoop_lines();
        let len = lines.len();
        if len == 0 {
            return;
        }
        let rows = self.snoop_view_rows();
        let s = &mut self.agents_window.snoop;
        if n == i16::MIN {
            s.follow = false;
            s.anchor_scroll(&lines, 0);
            return;
        }
        if n > 0 && s.follow {
            return;
        }
        // The bottom visible line as drawn (the view never shows less than the oldest lines).
        let cur = s.effective_scroll(&lines);
        let bottom = match snoop_view_range(len, rows, s.follow, cur) {
            (_, end) if rows > 0 => end - 1,
            _ if s.follow => len - 1,
            _ => cur.min(len - 1),
        };
        let step = usize::from(n.unsigned_abs());
        let to = if n < 0 { bottom.saturating_sub(step) } else { (bottom + step).min(len - 1) };
        s.anchor_scroll(&lines, to);
        s.follow = false;
    }

    /// One edit of the keyword entry line.
    pub fn edit_snoop_keyword(&mut self, edit: KeywordEdit) {
        match edit {
            KeywordEdit::Begin => {
                self.agents_window.pane_focus = WindowPane::Snoop;
                self.agents_window.snoop.keyword_entry = Some(String::new());
            }
            KeywordEdit::Push(c) => {
                if let Some(buf) = self.agents_window.snoop.keyword_entry.as_mut()
                    && !c.is_control()
                    && buf.chars().count() < KEYWORD_CAP
                {
                    buf.push(c);
                }
            }
            KeywordEdit::Pop => {
                if let Some(buf) = self.agents_window.snoop.keyword_entry.as_mut() {
                    buf.pop();
                }
            }
            KeywordEdit::Commit => {
                if let Some(buf) = self.agents_window.snoop.keyword_entry.take() {
                    self.apply_text_filter(&buf);
                }
            }
            KeywordEdit::Cancel => self.agents_window.snoop.keyword_entry = None,
        }
    }

    /// Opens the window on the conversation between `from` and `to`: selects the edge, filters to
    /// the pair and moves the keyboard to the conversation pane.
    pub fn open_agents_window_on_edge(&mut self, from: &NodeKey, to: &NodeKey) {
        self.open_agents_window(None);
        self.agents_select_edge(from, to);
        self.apply_pair_filter(from, to);
        self.focus_agents_pane(Some(WindowPane::Snoop));
    }
}
