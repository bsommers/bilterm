use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::pane::PaneId;

/// Split orientation for tiled `CompositePane` binary splits (`WM-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDirection {
    /// Splits a pane into Top (`first`) and Bottom (`second`) composite panes.
    Horizontal,
    /// Splits a pane into Left (`first`) and Right (`second`) composite panes.
    Vertical,
}

/// Recursive Binary Space Partitioning (BSP) tree node for tiled `CompositePane` layout.
#[derive(Clone, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
