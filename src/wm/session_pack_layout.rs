//! Pure deterministic 2D treemap session packing layout engine (Phase 54, VIEW-01).
//!
//! Provides squarified treemap partitioning with terminal character-cell aspect
//! ratio correction (k ≈ 2.0). Produces bit-for-bit deterministic layouts,
//! strict containment, zero overlap, and graceful degradation on tiny areas.

use ratatui::layout::Rect;
use std::collections::{BTreeMap, BTreeSet};

/// Input descriptor for a window to be packed inside a session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowPackInput {
    pub pane_id: usize,
    pub title: String,
    pub weight: u32,
    pub focused: bool,
}

/// Input descriptor for a session container (can be root or nested).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPackInput {
    pub session_name: String,
    pub parent_session: Option<String>,
    pub windows: Vec<WindowPackInput>,
    pub active: bool,
}

/// Output layout box for an individual window card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowPackRect {
    pub pane_id: usize,
    pub session_name: String,
    pub title: String,
    pub rect: Rect,
    pub focused: bool,
}

/// Output layout box for a session container with borders and contained windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPackRect {
    pub session_name: String,
    pub outer: Rect,
    pub inner: Rect,
    pub windows: Vec<WindowPackRect>,
    pub active: bool,
}

impl SessionPackRect {
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn find_window(&self, pane_id: usize) -> Option<&WindowPackRect> {
        self.windows.iter().find(|w| w.pane_id == pane_id)
    }
}

/// Computes the usable inner content rectangle from an outer container rectangle.
pub fn compute_inner_rect(outer: Rect) -> Rect {
    if outer.width < 3 || outer.height < 3 {
        Rect::new(outer.x, outer.y, 0, 0)
    } else {
        Rect::new(
            outer.x.saturating_add(1),
            outer.y.saturating_add(1),
            outer.width.saturating_sub(2),
            outer.height.saturating_sub(2),
        )
    }
}

/// Computes the aggregate packing weight of a session including direct windows and children.
fn compute_session_weight(
    idx: usize,
    sessions: &[SessionPackInput],
    children_map: &BTreeMap<usize, Vec<usize>>,
    visited: &mut BTreeSet<usize>,
) -> u64 {
    if !visited.insert(idx) {
        return 1;
    }
    let input = &sessions[idx];
    let mut w: u64 = input.windows.iter().map(|win| win.weight.max(1) as u64).sum();
    if let Some(kids) = children_map.get(&idx) {
        for &kid_idx in kids {
            w = w.saturating_add(compute_session_weight(kid_idx, sessions, children_map, visited));
        }
    }
    visited.remove(&idx);
    w.max(1)
}

/// Pure deterministic squarified treemap partitioner for weighted rectangles.
///
/// Adapts recursive bisection with terminal character cell aspect ratio factor
/// (width : height ≈ 2 : 1). Always partitions along the visually longer axis.
pub fn pack_rects(area: Rect, weights: &[u64]) -> Vec<Rect> {
    let n = weights.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![area];
    }
    if area.width == 0 || area.height == 0 {
        return vec![Rect::new(area.x, area.y, 0, 0); n];
    }

    // Preserve indices to return in caller's exact order
    let mut items: Vec<(usize, u64)> = weights
        .iter()
        .enumerate()
        .map(|(i, &w)| (i, w.max(1)))
        .collect();

    // Deterministic sort: descending by weight, tie-break by original index
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut out = vec![Rect::default(); n];
    bisection_treemap(area, &items, &mut out);
    out
}

fn bisection_treemap(area: Rect, items: &[(usize, u64)], out: &mut [Rect]) {
    let n = items.len();
    if n == 0 {
        return;
    }
    if n == 1 {
        out[items[0].0] = area;
        return;
    }
    if area.width == 0 || area.height == 0 {
        for item in items {
            out[item.0] = Rect::new(area.x, area.y, 0, 0);
        }
        return;
    }

    let total_w: u64 = items.iter().map(|(_, w)| *w).sum();
    if total_w == 0 {
        for item in items {
            out[item.0] = Rect::new(area.x, area.y, 0, 0);
        }
        return;
    }

    // Find partition index closest to 50% of total weight
    let half_w = total_w / 2;
    let mut acc: u64 = 0;
    let mut split_idx = 1;
    let mut best_diff = u64::MAX;

    for (i, &(_, item_w)) in items[..n - 1].iter().enumerate() {
        acc = acc.saturating_add(item_w);
        let diff = acc.abs_diff(half_w);
        if diff < best_diff {
            best_diff = diff;
            split_idx = i + 1;
        }
    }

    let first_w: u64 = items[..split_idx].iter().map(|(_, w)| *w).sum();

    // Terminal aspect ratio correction: 1 line high ≈ 2 columns wide
    let split_vertical = if area.width >= 2 && area.height >= 2 {
        area.width >= area.height.saturating_mul(2)
    } else if area.width >= 2 {
        true
    } else if area.height >= 2 {
        false
    } else {
        out[items[0].0] = area;
        for item in &items[1..] {
            out[item.0] = Rect::new(area.x, area.y, 0, 0);
        }
        return;
    };

    let (r1, r2) = if split_vertical {
        let max_w = area.width.saturating_sub(1).max(1);
        let w1 = (((area.width as u64 * first_w + total_w / 2) / total_w) as u16).clamp(1, max_w);
        let w2 = area.width.saturating_sub(w1);
        (Rect::new(area.x, area.y, w1, area.height), Rect::new(area.x.saturating_add(w1), area.y, w2, area.height))
    } else {
        let max_h = area.height.saturating_sub(1).max(1);
        let h1 = (((area.height as u64 * first_w + total_w / 2) / total_w) as u16).clamp(1, max_h);
        let h2 = area.height.saturating_sub(h1);
        (Rect::new(area.x, area.y, area.width, h1), Rect::new(area.x, area.y.saturating_add(h1), area.width, h2))
    };

    bisection_treemap(r1, &items[..split_idx], out);
    bisection_treemap(r2, &items[split_idx..], out);
}

/// Packs sessions and their windows into a deterministic 2D squarified treemap layout.
///
/// Guaranteed invariants:
/// 1. Sibling session containers never overlap.
/// 2. Sibling window cards never overlap.
/// 3. Every window card is strictly contained within its session container's `inner` rect.
/// 4. Child session containers are strictly contained within their parent's `inner` rect.
/// 5. Never panics or overflows on degenerate/tiny input areas (`width < 8 || height < 4`).
pub fn pack_sessions_treemap(area: Rect, sessions: &[SessionPackInput]) -> Vec<SessionPackRect> {
    let n = sessions.len();
    if n == 0 {
        return Vec::new();
    }

    // Map session names to indices
    let mut name_to_idx = BTreeMap::new();
    for (i, s) in sessions.iter().enumerate() {
        name_to_idx.insert(s.session_name.as_str(), i);
    }

    // Build parent-to-children relationship
    let mut children_map: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut root_indices = Vec::new();

    for (i, s) in sessions.iter().enumerate() {
        let parent_idx = s
            .parent_session
            .as_deref()
            .and_then(|p| name_to_idx.get(p).copied())
            .filter(|&idx| idx != i);
        if let Some(p_idx) = parent_idx {
            children_map.entry(p_idx).or_default().push(i);
            continue;
        }
        root_indices.push(i);
    }

    // Result initialized with empty boxes
    let mut out: Vec<SessionPackRect> = sessions
        .iter()
        .map(|s| SessionPackRect {
            session_name: s.session_name.clone(),
            outer: Rect::default(),
            inner: Rect::default(),
            windows: Vec::new(),
            active: s.active,
        })
        .collect();

    // Compute weights for root sessions
    let mut visited = BTreeSet::new();
    let root_weights: Vec<u64> = root_indices
        .iter()
        .map(|&idx| compute_session_weight(idx, sessions, &children_map, &mut visited))
        .collect();

    // Pack root sessions into bounding area
    let root_rects = pack_rects(area, &root_weights);
    for (r_idx, &s_idx) in root_indices.iter().enumerate() {
        let outer = root_rects[r_idx];
        let inner = compute_inner_rect(outer);
        out[s_idx].outer = outer;
        out[s_idx].inner = inner;
        pack_session_interior(s_idx, sessions, &children_map, &mut out);
    }

    out
}

/// Recursively packs the interior of a session: child sessions and direct windows.
fn pack_session_interior(
    s_idx: usize,
    sessions: &[SessionPackInput],
    children_map: &BTreeMap<usize, Vec<usize>>,
    out: &mut [SessionPackRect],
) {
    let inner = out[s_idx].inner;
    let s_input = &sessions[s_idx];
    let child_indices = children_map.get(&s_idx).cloned().unwrap_or_default();

    let num_windows = s_input.windows.len();
    let num_children = child_indices.len();

    if num_windows == 0 && num_children == 0 {
        return;
    }

    if num_children == 0 {
        // Direct windows only
        let w_weights: Vec<u64> = s_input
            .windows
            .iter()
            .map(|w| w.weight.max(1) as u64)
            .collect();
        let w_rects = pack_rects(inner, &w_weights);
        out[s_idx].windows = s_input
            .windows
            .iter()
            .enumerate()
            .map(|(w_i, w)| WindowPackRect {
                pane_id: w.pane_id,
                session_name: s_input.session_name.clone(),
                title: w.title.clone(),
                rect: w_rects[w_i],
                focused: w.focused,
            })
            .collect();
        return;
    }

    // Both child sessions and windows: partition `inner` among both
    enum InteriorItem {
        Child(usize),
        Window(usize),
    }

    let mut visited = BTreeSet::new();
    let mut items: Vec<(InteriorItem, u64)> = Vec::new();

    for &c_idx in &child_indices {
        let w = compute_session_weight(c_idx, sessions, children_map, &mut visited);
        items.push((InteriorItem::Child(c_idx), w));
    }
    for (w_i, win) in s_input.windows.iter().enumerate() {
        items.push((InteriorItem::Window(w_i), win.weight.max(1) as u64));
    }

    let weights: Vec<u64> = items.iter().map(|(_, w)| *w).collect();
    let rects = pack_rects(inner, &weights);

    let mut win_results = Vec::new();
    for (i, (item, _)) in items.into_iter().enumerate() {
        match item {
            InteriorItem::Child(c_idx) => {
                let outer = rects[i];
                let c_inner = compute_inner_rect(outer);
                out[c_idx].outer = outer;
                out[c_idx].inner = c_inner;
                pack_session_interior(c_idx, sessions, children_map, out);
            }
            InteriorItem::Window(w_i) => {
                let win = &s_input.windows[w_i];
                win_results.push(WindowPackRect {
                    pane_id: win.pane_id,
                    session_name: s_input.session_name.clone(),
                    title: win.title.clone(),
                    rect: rects[i],
                    focused: win.focused,
                });
            }
        }
    }
    out[s_idx].windows = win_results;
}
