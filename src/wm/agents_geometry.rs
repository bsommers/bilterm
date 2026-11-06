//! Geometry, hit-testing and the move / resize / splitter / node operations of the AGENT TRAFFIC
//! window (Phase 50-09, D-07/D-08).
//!
//! [`WindowManager::agents_pane_rects`] is the single source of window geometry for the renderer,
//! the mouse handler and the GUI pass; [`WindowManager::agents_hit`] classifies a cell against it.
//! The mutating methods are reached only through `Action::Agents(..)` (the `:agents` executor), so
//! keys, mouse gestures and colon commands share one code path.

use ratatui::layout::Rect;
use unicode_width::UnicodeWidthStr;

use crate::ui::agents_ui::split_panes;
use crate::ui::graph_raster::{curve_controls, label_cell, node_px};
use crate::wm::panel_dock::DockPanel;
use crate::wm::agents_window::{GraphDir, GraphSelection, MIN_WINDOW, NodeKey, SceneNode};
use crate::wm::{WindowManager, WindowResizeEdge, rect_contains};

/// Splitter bounds, percent of the inner width given to the graph pane.
pub const SPLITTER_RANGE: (u16, u16) = (20, 80);
/// Fallback nudge when the window is not visible (permille).
const DEFAULT_NUDGE_PM: i32 = 20;

/// Window regions shared by rendering and hit-testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentsPaneRects {
    /// Whole window including the frame.
    pub outer: Rect,
    /// Area inside the frame.
    pub inner: Rect,
    /// Top frame row (title and badges).
    pub title_row: Rect,
    /// Node graph pane.
    pub graph: Rect,
    /// Column of the vertical divider.
    pub splitter_col: u16,
    /// Conversation (bus event) pane.
    pub snoop: Rect,
}

/// What lies under a cell of the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentsHit {
    Outside,
    Title,
    Border(WindowResizeEdge),
    Splitter,
    Node(NodeKey),
    Edge(NodeKey, NodeKey),
    Graph,
    Snoop,
}

/// Resize edge under a frame cell below the title row. The four corners (including the two top
/// ones) resize; the rest of the top row is the title bar, as for modal windows.
fn border_edge(outer: Rect, col: u16, row: u16) -> Option<WindowResizeEdge> {
    let right = outer.x + outer.width.saturating_sub(1);
    let bottom = outer.y + outer.height.saturating_sub(1);
    if row == outer.y {
        return if col == outer.x {
            Some(WindowResizeEdge::TopLeft)
        } else if col == right {
            Some(WindowResizeEdge::TopRight)
        } else {
            None
        };
    }
    let (on_bottom, on_left, on_right) = (row == bottom, col == outer.x, col == right);
    if !(on_bottom || on_left || on_right) {
        return None;
    }
    Some(
        if (on_bottom && col >= right.saturating_sub(1)) || (on_right && row >= bottom.saturating_sub(1)) {
            WindowResizeEdge::BottomRight
        } else if (on_bottom && col <= outer.x + 1) || (on_left && row >= bottom.saturating_sub(1)) {
            WindowResizeEdge::BottomLeft
        } else if on_bottom {
            WindowResizeEdge::Bottom
        } else if on_left {
            WindowResizeEdge::Left
        } else {
            WindowResizeEdge::Right
        },
    )
}

fn canvas_px(graph: Rect) -> (f32, f32) {
    (f32::from(graph.width) * 2.0, f32::from(graph.height) * 4.0)
}

/// Cell (relative to the graph rect) of the node dot.
fn node_dot_cell(node: &SceneNode, graph: Rect) -> (i32, i32) {
    let (w, h) = canvas_px(graph);
    let (px, py) = node_px(node.pos, w, h);
    (
        (px / 2.0).floor().clamp(0.0, f32::from(graph.width) - 1.0) as i32,
        (py / 4.0).floor().clamp(0.0, f32::from(graph.height) - 1.0) as i32,
    )
}

/// Cubic Bezier point at `t`.
fn bezier(p: [(f32, f32); 4], t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        w0 * p[0].0 + w1 * p[1].0 + w2 * p[2].0 + w3 * p[3].0,
        w0 * p[0].1 + w1 * p[1].1 + w2 * p[2].1 + w3 * p[3].1,
    )
}

/// Cells (relative to the graph rect) the curve of the edge between nodes `a` and `b` passes
/// through, sampled finer than one pixel the way the renderer draws it.
pub fn edge_curve_cells(a: &SceneNode, b: &SceneNode, graph: Rect) -> Vec<(i32, i32)> {
    let (w, h) = canvas_px(graph);
    let (pa, pb) = (node_px(a.pos, w, h), node_px(b.pos, w, h));
    let (c1, c2) = curve_controls(pa, pb);
    let chord = ((pb.0 - pa.0).powi(2) + (pb.1 - pa.1).powi(2)).sqrt();
    let steps = ((chord * 1.5).ceil() as usize).clamp(16, 1024);
    let mut cells: Vec<(i32, i32)> = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let (x, y) = bezier([pa, c1, c2, pb], i as f32 / steps as f32);
        let cell = ((x / 2.0).floor() as i32, (y / 4.0).floor() as i32);
        if cells.last() != Some(&cell) {
            cells.push(cell);
        }
    }
    cells
}

/// Permille position (0..=1000 on both axes) that puts a node label under the pointer cell.
pub fn graph_cell_permille(graph: Rect, col: u16, row: u16) -> (u16, u16) {
    let (w, h) = canvas_px(graph);
    let (x0, y0) = node_px((0.0, 0.0), w, h);
    let (x1, y1) = node_px((1.0, 1.0), w, h);
    let px = (i32::from(col) - i32::from(graph.x)) as f32 * 2.0 + 1.0;
    // Labels sit one row below their node dot.
    let py = (i32::from(row) - 1 - i32::from(graph.y)) as f32 * 4.0 + 2.0;
    let pm = |v: f32, lo: f32, hi: f32| (((v - lo) / (hi - lo).max(1.0)) * 1000.0).round().clamp(0.0, 1000.0) as u16;
    (pm(px, x0, x1), pm(py, y0, y1))
}

impl WindowManager {
    /// Regions of the open window, or `None` when it is closed or too small to show content.
    pub fn agents_pane_rects(&self, workspace_area: Rect) -> Option<AgentsPaneRects> {
        let outer = self.agents_window_rect(workspace_area)?;
        if outer.width < 8 || outer.height < 4 {
            return None;
        }
        let inner = Rect::new(outer.x + 1, outer.y + 1, outer.width - 2, outer.height - 2);
        if inner.width < 6 || inner.height < 1 {
            return None;
        }
        let (graph, splitter_col, snoop) = split_panes(inner, self.agents_window().splitter_pct);
        Some(AgentsPaneRects {
            outer,
            inner,
            title_row: Rect::new(outer.x, outer.y, outer.width, 1),
            graph,
            splitter_col,
            snoop,
        })
    }

    /// Classifies the cell `(col, row)` against the open window. Nodes win over edges.
    pub fn agents_hit(&self, workspace_area: Rect, col: u16, row: u16) -> AgentsHit {
        let Some(outer) = self.agents_window_rect(workspace_area) else {
            return AgentsHit::Outside;
        };
        if !rect_contains(outer, col, row) {
            return AgentsHit::Outside;
        }
        if let Some(edge) = border_edge(outer, col, row) {
            return AgentsHit::Border(edge);
        }
        if row == outer.y {
            return AgentsHit::Title;
        }
        let Some(rects) = self.agents_pane_rects(workspace_area) else {
            return AgentsHit::Graph;
        };
        if !rect_contains(rects.inner, col, row) {
            return AgentsHit::Graph;
        }
        if col == rects.splitter_col {
            return AgentsHit::Splitter;
        }
        if !rect_contains(rects.graph, col, row) {
            return AgentsHit::Snoop;
        }
        let scene = self.agents_graph_scene();
        let (rel_c, rel_r) = (i32::from(col - rects.graph.x), i32::from(row - rects.graph.y));
        for node in &scene.nodes {
            if node_dot_cell(node, rects.graph) == (rel_c, rel_r) {
                return AgentsHit::Node(node.key.clone());
            }
            if let Some((lx, ly, text)) = label_cell(node, rects.graph) {
                let width = UnicodeWidthStr::width(text.as_str()) as i32;
                if i32::from(ly) == rel_r && rel_c >= i32::from(lx) && rel_c < i32::from(lx) + width {
                    return AgentsHit::Node(node.key.clone());
                }
            }
        }
        let mut best: Option<(i32, &crate::wm::agents_window::SceneEdge)> = None;
        for edge in &scene.edges {
            let (Some(a), Some(b)) = (
                scene.nodes.iter().find(|n| n.key == edge.from),
                scene.nodes.iter().find(|n| n.key == edge.to),
            ) else {
                continue;
            };
            let dist = edge_curve_cells(a, b, rects.graph)
                .into_iter()
                .map(|(x, y)| (x - rel_c).abs().max((y - rel_r).abs()))
                .min();
            if let Some(d) = dist
                && d <= 1
                && best.is_none_or(|(bd, _)| d < bd)
            {
                best = Some((d, edge));
            }
        }
        match best {
            Some((_, e)) => AgentsHit::Edge(e.from.clone(), e.to.clone()),
            None => AgentsHit::Graph,
        }
    }
}

impl WindowManager {
    /// Workspace as the window sees it: the scaled (virtual) cell space the renderer, the mouse
    /// handler and the GUI path all use (identity at 100%). WR-02.
    pub(crate) fn agents_workspace(&self) -> Rect {
        let ws = self.current_workspace_area().1;
        self.panel_virtual_area(ws, crate::ui::typography::PanelTarget::Agents)
    }

    fn agents_closed_status(&mut self) {
        self.active_pane_mut().set_status_message(Some("Agent traffic window is not open".to_string()));
    }

    /// Sets the window rect, clamped to the workspace (min 40x12, or the workspace when smaller);
    /// leaves maximized mode. Returns the stored rect.
    pub fn set_agents_rect(&mut self, rect: Rect) -> Rect {
        self.set_panel_docked(DockPanel::AgentsTraffic, false);
        let ws = self.agents_workspace();
        let (ws_w, ws_h) = (ws.width.max(1), ws.height.max(1));
        let w = rect.width.clamp(MIN_WINDOW.0.min(ws_w), ws_w);
        let h = rect.height.clamp(MIN_WINDOW.1.min(ws_h), ws_h);
        let max_x = ws.x + ws.width.saturating_sub(w);
        let max_y = ws.y + ws.height.saturating_sub(h);
        let clamped = Rect::new(rect.x.clamp(ws.x, max_x), rect.y.clamp(ws.y, max_y), w, h);
        let state = self.agents_window_mut();
        state.maximized = false;
        state.rect = Some(clamped);
        clamped
    }

    /// Moves the window by `(dx, dy)` cells.
    pub fn move_agents_window(&mut self, dx: i16, dy: i16) {
        let ws = self.agents_workspace();
        let Some(cur) = self.agents_window_rect(ws) else {
            return self.agents_closed_status();
        };
        let x = (i32::from(cur.x) + i32::from(dx)).clamp(0, i32::from(u16::MAX)) as u16;
        let y = (i32::from(cur.y) + i32::from(dy)).clamp(0, i32::from(u16::MAX)) as u16;
        self.set_agents_rect(Rect::new(x, y, cur.width, cur.height));
    }

    /// Grows (positive) or shrinks the window by `(dw, dh)` cells, anchored at its top-left corner.
    pub fn resize_agents_window(&mut self, dw: i16, dh: i16) {
        let ws = self.agents_workspace();
        let Some(cur) = self.agents_window_rect(ws) else {
            return self.agents_closed_status();
        };
        let w = (i32::from(cur.width) + i32::from(dw)).clamp(0, i32::from(u16::MAX)) as u16;
        let h = (i32::from(cur.height) + i32::from(dh)).clamp(0, i32::from(u16::MAX)) as u16;
        self.set_agents_rect(Rect::new(cur.x, cur.y, w, h));
    }

    /// Sets the window size (`Some`, position kept) or returns to the default centred rect (`None`).
    pub fn set_agents_size(&mut self, size: Option<(u16, u16)>) {
        self.set_panel_docked(DockPanel::AgentsTraffic, false);
        let ws = self.agents_workspace();
        match size {
            None => {
                let state = self.agents_window_mut();
                state.rect = None;
                state.maximized = false;
            }
            Some((cols, rows)) => {
                let Some(cur) = self.agents_window_rect(ws) else {
                    return self.agents_closed_status();
                };
                self.set_agents_rect(Rect::new(cur.x, cur.y, cols, rows));
            }
        }
    }

    /// Toggles the maximized state.
    pub fn toggle_agents_maximize(&mut self) {
        self.set_panel_docked(DockPanel::AgentsTraffic, false);
        let state = self.agents_window_mut();
        state.maximized = !state.maximized;
    }

    /// Sets the splitter position, clamped to [`SPLITTER_RANGE`].
    pub fn set_agents_splitter_pct(&mut self, pct: u16) {
        self.agents_window_mut().splitter_pct = pct.clamp(SPLITTER_RANGE.0, SPLITTER_RANGE.1);
    }

    /// Pins `key` at the permille position `(x_pm, y_pm)` of the graph pane. `false` when the node
    /// is not in the scene.
    pub fn pin_agents_node(&mut self, key: &NodeKey, x_pm: u16, y_pm: u16) -> bool {
        if !self.agents_graph_scene().nodes.iter().any(|n| &n.key == key) {
            return false;
        }
        let pos = (f32::from(x_pm.min(1000)) / 1000.0, f32::from(y_pm.min(1000)) / 1000.0);
        self.agents_window_mut().layout.pinned.insert(key.clone(), pos);
        true
    }

    /// Releases a pinned node back to the automatic layout. `false` when it was not pinned.
    pub fn unpin_agents_node(&mut self, key: &NodeKey) -> bool {
        self.agents_window_mut().layout.pinned.remove(key).is_some()
    }

    /// Moves the selected node one terminal cell in `dir` and pins it there. `false` when no
    /// node is selected.
    pub fn nudge_agents_node(&mut self, dir: GraphDir) -> bool {
        let Some(key) = self.agents_window().selection.node.clone() else {
            return false;
        };
        let scene = self.agents_graph_scene();
        let Some(node) = scene.nodes.iter().find(|n| n.key == key) else {
            return false;
        };
        let ws = self.agents_workspace();
        let (step_x, step_y) = match self.agents_pane_rects(ws) {
            Some(r) => {
                let (w, h) = canvas_px(r.graph);
                let span_x = node_px((1.0, 0.0), w, h).0 - node_px((0.0, 0.0), w, h).0;
                let span_y = node_px((0.0, 1.0), w, h).1 - node_px((0.0, 0.0), w, h).1;
                (
                    ((2.0 / span_x.max(1.0)) * 1000.0).round().max(1.0) as i32,
                    ((4.0 / span_y.max(1.0)) * 1000.0).round().max(1.0) as i32,
                )
            }
            None => (DEFAULT_NUDGE_PM, DEFAULT_NUDGE_PM),
        };
        let (dx, dy) = match dir {
            GraphDir::Left => (-step_x, 0),
            GraphDir::Right => (step_x, 0),
            GraphDir::Up => (0, -step_y),
            GraphDir::Down => (0, step_y),
        };
        let pm = |v: f32, d: i32| ((v * 1000.0).round() as i32 + d).clamp(0, 1000) as u16;
        let (x, y) = (pm(node.pos.0, dx), pm(node.pos.1, dy));
        self.pin_agents_node(&key, x, y)
    }

    /// Turns grab mode on, off or toggles it (`None`). Grabbing needs a selected node.
    pub fn set_agents_grab(&mut self, mode: Option<bool>) {
        let want = mode.unwrap_or(!self.agents_window().grab_mode);
        if want && self.agents_window().selection.node.is_none() {
            self.active_pane_mut()
                .set_status_message(Some("Select a node before grabbing".to_string()));
            return;
        }
        self.agents_window_mut().grab_mode = want;
    }

    /// Selects the edge between `a` and `b` (either order). Keeps the selected node when it is an
    /// endpoint, otherwise selects the first endpoint. `false` when the edge is not in the scene.
    pub fn agents_select_edge(&mut self, a: &NodeKey, b: &NodeKey) -> bool {
        let pair = if a < b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) };
        let scene = self.agents_graph_scene();
        if !scene.edges.iter().any(|e| e.from == pair.0 && e.to == pair.1) {
            return false;
        }
        let keep = self
            .agents_window()
            .selection
            .node
            .clone()
            .filter(|n| *n == pair.0 || *n == pair.1);
        let node = keep.unwrap_or_else(|| pair.0.clone());
        self.agents_window_mut().selection = GraphSelection { node: Some(node), edge: Some(pair) };
        true
    }
}
