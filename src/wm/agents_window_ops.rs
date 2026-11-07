//! Operations on the AGENT TRAFFIC window: graph navigation, snoop view, conversation filtering and compose.

use crate::vt::sanitize_display;
use crate::wm::agents_cmd::{KeywordEdit, SnoopFilterKind};
use crate::wm::agents_window::{
    ComposeGate, GraphDir, GraphSelection, KEYWORD_CAP, NodeKey, SceneNode, SnoopLine,
    WindowPane, snoop_view_range,
};
use crate::wm::traffic::TrafficEvent;
use crate::wm::WindowManager;

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
