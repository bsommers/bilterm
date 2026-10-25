use ratatui::layout::{Constraint, Direction, Layout, Rect};
use serde::{Deserialize, Serialize};

use crate::pane::PaneId;

/// Split orientation for tiled `CompositePane` binary splits (`WM-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    /// Splits a pane into Top (`first`) and Bottom (`second`) composite panes.
    Horizontal,
    /// Splits a pane into Left (`first`) and Right (`second`) composite panes.
    Vertical,
}

/// Recursive Binary Space Partitioning (BSP) tree node for tiled `CompositePane` layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitNode {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        /// Percentage (`10..=90`) allocated to `first`; `second` receives `100 - ratio_pct`.
        ratio_pct: u16,
        first: Box<SplitNode>,
        second: Box<SplitNode>,
    },
}

/// Binary Split Tree managing tiled H/V `CompositePane` geometry (`WM-01`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitTree {
    root: SplitNode,
}

impl SplitTree {
    pub fn new(root_pane: PaneId) -> Self {
        Self {
            root: SplitNode::Leaf(root_pane),
        }
    }

    pub fn root(&self) -> &SplitNode {
        &self.root
    }

    /// Splits the leaf matching `target_pane` in `direction`, placing `new_pane` in the second child.
    pub fn split_leaf(
        &mut self,
        target_pane: PaneId,
        new_pane: PaneId,
        direction: SplitDirection,
    ) -> bool {
        Self::split_node_recursive(&mut self.root, target_pane, new_pane, direction)
    }

    fn split_node_recursive(
        node: &mut SplitNode,
        target_pane: PaneId,
        new_pane: PaneId,
        direction: SplitDirection,
    ) -> bool {
        match node {
            SplitNode::Leaf(id) if *id == target_pane => {
                let existing = *id;
                *node = SplitNode::Split {
                    direction,
                    ratio_pct: 50,
                    first: Box::new(SplitNode::Leaf(existing)),
                    second: Box::new(SplitNode::Leaf(new_pane)),
                };
                true
            }
            SplitNode::Leaf(_) => false,
            SplitNode::Split { first, second, .. } => {
                Self::split_node_recursive(first, target_pane, new_pane, direction)
                    || Self::split_node_recursive(second, target_pane, new_pane, direction)
            }
        }
    }

    /// Removes `target_pane` from the split tree, promoting its sibling node in place.
    /// Returns `false` if `target_pane` is the sole root leaf or was not found.
    pub fn remove_leaf(&mut self, target_pane: PaneId) -> bool {
        if matches!(self.root, SplitNode::Leaf(id) if id == target_pane) {
            return false;
        }
        Self::remove_node_recursive(&mut self.root, target_pane)
    }

    fn remove_node_recursive(node: &mut SplitNode, target_pane: PaneId) -> bool {
        match node {
            SplitNode::Leaf(_) => false,
            SplitNode::Split { first, second, .. } => {
                if matches!(**first, SplitNode::Leaf(id) if id == target_pane) {
                    let promoted = second.as_ref().clone();
                    *node = promoted;
                    return true;
                }
                if matches!(**second, SplitNode::Leaf(id) if id == target_pane) {
                    let promoted = first.as_ref().clone();
                    *node = promoted;
                    return true;
                }
                Self::remove_node_recursive(first, target_pane)
                    || Self::remove_node_recursive(second, target_pane)
            }
        }
    }

    /// Adjusts the split ratio (`10%..=90%`) of the immediate parent split containing `target_pane`.
    pub fn adjust_ratio(&mut self, target_pane: PaneId, delta_pct: i16) -> bool {
        Self::adjust_ratio_recursive(&mut self.root, target_pane, delta_pct)
    }

    /// Adjusts the nearest ancestor split of `target_pane` matching `direction` (or falls back to
    /// `adjust_ratio` if no split along `direction` exists). When `grow_pane_mode` is true and
    /// `target_pane` is in the `second` branch, inverts `delta_pct` so positive deltas grow `target_pane`.
    pub fn adjust_ratio_directional(
        &mut self,
        target_pane: PaneId,
        direction: SplitDirection,
        delta_pct: i16,
    ) -> bool {
        if Self::adjust_directional_recursive(&mut self.root, target_pane, direction, delta_pct) {
            return true;
        }
        self.adjust_ratio(target_pane, delta_pct)
    }

    fn adjust_directional_recursive(
        node: &mut SplitNode,
        target_pane: PaneId,
        target_dir: SplitDirection,
        delta_pct: i16,
    ) -> bool {
        match node {
            SplitNode::Leaf(_) => false,
            SplitNode::Split {
                direction,
                ratio_pct,
                first,
                second,
            } => {
                let in_first = Self::contains_pane(first, target_pane);
                let in_second = !in_first && Self::contains_pane(second, target_pane);
                if !in_first && !in_second {
                    return false;
                }
                // Try deeper matching split first (innermost matching ancestor)
                if in_first
                    && Self::adjust_directional_recursive(first, target_pane, target_dir, delta_pct)
                {
                    return true;
                }
                if in_second
                    && Self::adjust_directional_recursive(second, target_pane, target_dir, delta_pct)
                {
                    return true;
                }
                if *direction == target_dir {
                    let signed_delta = if in_second { -delta_pct } else { delta_pct };
                    let next =
                        (i32::from(*ratio_pct) + i32::from(signed_delta)).clamp(10, 90) as u16;
                    *ratio_pct = next;
                    return true;
                }
                false
            }
        }
    }

    /// Updates the split ratio (`10%..=90%`) of the nearest ancestor split of `target_pane` along
    /// `target_dir` so its divider aligns with mouse coordinate `(col, row)` inside `area`.
    pub fn set_split_ratio_at_point(
        &mut self,
        target_pane: PaneId,
        target_dir: SplitDirection,
        col: u16,
        row: u16,
        area: Rect,
    ) -> bool {
        Self::set_ratio_at_point_recursive(&mut self.root, target_pane, target_dir, col, row, area)
    }

    fn set_ratio_at_point_recursive(
        node: &mut SplitNode,
        target_pane: PaneId,
        target_dir: SplitDirection,
        col: u16,
        row: u16,
        area: Rect,
    ) -> bool {
        match node {
            SplitNode::Leaf(_) => false,
            SplitNode::Split {
                direction,
                ratio_pct,
                first,
                second,
            } => {
                let in_first = Self::contains_pane(first, target_pane);
                let in_second = !in_first && Self::contains_pane(second, target_pane);
                if !in_first && !in_second {
                    return false;
                }
                let dir = match direction {
                    SplitDirection::Horizontal => Direction::Vertical,
                    SplitDirection::Vertical => Direction::Horizontal,
                };
                let chunks = Layout::default()
                    .direction(dir)
                    .constraints([
                        Constraint::Percentage(*ratio_pct),
                        Constraint::Percentage(100u16.saturating_sub(*ratio_pct)),
                    ])
                    .split(area);
                if in_first
                    && Self::set_ratio_at_point_recursive(
                        first,
                        target_pane,
                        target_dir,
                        col,
                        row,
                        chunks[0],
                    )
                {
                    return true;
                }
                if in_second
                    && Self::set_ratio_at_point_recursive(
                        second,
                        target_pane,
                        target_dir,
                        col,
                        row,
                        chunks[1],
                    )
                {
                    return true;
                }
                if *direction == target_dir {
                    let pct = match direction {
                        SplitDirection::Vertical => {
                            let rel = col.saturating_sub(area.x);
                            ((u32::from(rel) * 100) / u32::from(area.width.max(1))) as i32
                        }
                        SplitDirection::Horizontal => {
                            let rel = row.saturating_sub(area.y);
                            ((u32::from(rel) * 100) / u32::from(area.height.max(1))) as i32
                        }
                    };
                    *ratio_pct = pct.clamp(10, 90) as u16;
                    return true;
                }
                false
            }
        }
    }

    /// Swaps the positions of two leaf panes (`pane_a` and `pane_b`) in the split tree.
    pub fn swap_leaves(&mut self, pane_a: PaneId, pane_b: PaneId) -> bool {
        if pane_a == pane_b
            || !Self::contains_pane(&self.root, pane_a)
            || !Self::contains_pane(&self.root, pane_b)
        {
            return false;
        }
        Self::swap_leaves_recursive(&mut self.root, pane_a, pane_b);
        true
    }

    fn swap_leaves_recursive(node: &mut SplitNode, pane_a: PaneId, pane_b: PaneId) {
        match node {
            SplitNode::Leaf(id) => {
                if *id == pane_a {
                    *id = pane_b;
                } else if *id == pane_b {
                    *id = pane_a;
                }
            }
            SplitNode::Split { first, second, .. } => {
                Self::swap_leaves_recursive(first, pane_a, pane_b);
                Self::swap_leaves_recursive(second, pane_a, pane_b);
            }
        }
    }

    /// Returns all leaf `PaneId`s in left-to-right / top-to-bottom tree order.
    pub fn leaves(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        Self::collect_leaves(&self.root, &mut out);
        out
    }

    fn collect_leaves(node: &SplitNode, out: &mut Vec<PaneId>) {
        match node {
            SplitNode::Leaf(id) => out.push(*id),
            SplitNode::Split { first, second, .. } => {
                Self::collect_leaves(first, out);
                Self::collect_leaves(second, out);
            }
        }
    }

    /// Resets all split ratios in the tree back to `50%`.
    pub fn reset_ratios(&mut self) {
        Self::reset_ratios_recursive(&mut self.root);
    }

    fn reset_ratios_recursive(node: &mut SplitNode) {
        if let SplitNode::Split {
            ratio_pct,
            first,
            second,
            ..
        } = node
        {
            *ratio_pct = 50;
            Self::reset_ratios_recursive(first);
            Self::reset_ratios_recursive(second);
        }
    }

    fn adjust_ratio_recursive(
        node: &mut SplitNode,
        target_pane: PaneId,
        delta_pct: i16,
    ) -> bool {
        match node {
            SplitNode::Leaf(_) => false,
            SplitNode::Split {
                ratio_pct,
                first,
                second,
                ..
            } => {
                if Self::contains_pane(first, target_pane)
                    && matches!(**first, SplitNode::Leaf(_))
                    || Self::contains_pane(second, target_pane)
                        && matches!(**second, SplitNode::Leaf(_))
                {
                    let next = (i32::from(*ratio_pct) + i32::from(delta_pct)).clamp(10, 90) as u16;
                    *ratio_pct = next;
                    return true;
                }
                Self::adjust_ratio_recursive(first, target_pane, delta_pct)
                    || Self::adjust_ratio_recursive(second, target_pane, delta_pct)
            }
        }
    }

    fn contains_pane(node: &SplitNode, target: PaneId) -> bool {
        match node {
            SplitNode::Leaf(id) => *id == target,
            SplitNode::Split { first, second, .. } => {
                Self::contains_pane(first, target) || Self::contains_pane(second, target)
            }
        }
    }

    /// Computes exact tiled `Rect` viewports for every leaf `PaneId` in tree order.
    pub fn compute_rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let mut out = Vec::new();
        Self::compute_rects_recursive(&self.root, area, &mut out);
        out
    }

    fn compute_rects_recursive(node: &SplitNode, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
        match node {
            SplitNode::Leaf(id) => out.push((*id, area)),
            SplitNode::Split {
                direction,
                ratio_pct,
                first,
                second,
            } => {
                let dir = match direction {
                    SplitDirection::Horizontal => Direction::Vertical,
                    SplitDirection::Vertical => Direction::Horizontal,
                };
                let chunks = Layout::default()
                    .direction(dir)
                    .constraints([
                        Constraint::Percentage(*ratio_pct),
                        Constraint::Percentage(100u16.saturating_sub(*ratio_pct)),
                    ])
                    .split(area);
                Self::compute_rects_recursive(first, chunks[0], out);
                Self::compute_rects_recursive(second, chunks[1], out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_horizontally_and_vertically_and_removes_leaves_cleanly() {
        let mut tree = SplitTree::new(1);
        assert!(tree.split_leaf(1, 2, SplitDirection::Vertical));
        assert!(tree.split_leaf(2, 3, SplitDirection::Horizontal));

        let rects = tree.compute_rects(Rect::new(0, 0, 100, 40));
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0], (1, Rect::new(0, 0, 50, 40)));
        assert_eq!(rects[1], (2, Rect::new(50, 0, 50, 20)));
        assert_eq!(rects[2], (3, Rect::new(50, 20, 50, 20)));

        assert!(tree.adjust_ratio(2, 10));
        let rects_adjusted = tree.compute_rects(Rect::new(0, 0, 100, 40));
        assert_eq!(rects_adjusted[1], (2, Rect::new(50, 0, 50, 24)));

        assert!(tree.remove_leaf(2));
        let rects_after_remove = tree.compute_rects(Rect::new(0, 0, 100, 40));
        assert_eq!(rects_after_remove.len(), 2);
        assert_eq!(rects_after_remove[0].0, 1);
        assert_eq!(rects_after_remove[1].0, 3);
    }
}
