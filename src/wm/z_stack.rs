use ratatui::layout::Rect;

use crate::pane::PaneId;

/// Z-Order Deck Stack Manager (`WM-02`).
///
/// Stores panes in ascending z-order (`order[0]` is at the bottom of the deck,
/// `order.last()` is at the top of the deck and holds active focus).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZStack {
    order: Vec<PaneId>,
}

impl ZStack {
    pub fn new(initial_pane: PaneId) -> Self {
        Self {
            order: vec![initial_pane],
        }
    }

    /// Returns panes ordered from bottom (`0`) to top (`len - 1`).
    pub fn order(&self) -> &[PaneId] {
        &self.order
    }

    /// Returns the topmost (active/focused) `PaneId`.
    pub fn top(&self) -> Option<PaneId> {
        self.order.last().copied()
    }

    /// Pushes a newly created `PaneId` to the top of the z-order stack.
    pub fn push(&mut self, pane_id: PaneId) {
        self.order.retain(|&id| id != pane_id);
        self.order.push(pane_id);
    }

    /// Removes a closed `PaneId` from the z-order stack.
    pub fn remove(&mut self, pane_id: PaneId) {
        self.order.retain(|&id| id != pane_id);
    }

    /// Selectively raises `pane_id` to the top of the z-order stack (`Alt+1..9` or mouse click).
    pub fn raise(&mut self, pane_id: PaneId) -> bool {
        if let Some(pos) = self.order.iter().position(|&id| id == pane_id) {
            let id = self.order.remove(pos);
            self.order.push(id);
            true
        } else {
            false
        }
    }

    /// Shuffles/rotates the deck top-to-bottom (`Alt+Tab` / `F6` / `:deck next`):
    /// sends the current top pane to the bottom of the deck (`index 0`) and promotes the next pane to the top.
    pub fn shuffle_top_to_bottom(&mut self) -> Option<PaneId> {
        if self.order.len() > 1 {
            let top = self.order.pop().unwrap();
            self.order.insert(0, top);
        }
        self.top()
    }

    /// Shuffles/rotates the deck bottom-to-top (`Alt+Shift+Tab` / `:deck prev`):
    /// pulls the bottom-most pane (`index 0`) to the top of the deck.
    pub fn shuffle_bottom_to_top(&mut self) -> Option<PaneId> {
        if self.order.len() > 1 {
            let bottom = self.order.remove(0);
            self.order.push(bottom);
        }
        self.top()
    }

    /// Returns `(1-based rank, total_panes)` where `rank == total_panes` is the topmost pane.
    pub fn rank_of(&self, pane_id: PaneId) -> Option<(usize, usize)> {
        self.order
            .iter()
            .position(|&id| id == pane_id)
            .map(|idx| (idx + 1, self.order.len()))
    }

    /// Computes overlapping Borland Turbo Vision cascade rectangles for each pane in ascending z-order.
    pub fn compute_cascade_rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let count = self.order.len();
        if count == 0 {
            return Vec::new();
        }
        if count == 1 {
            return vec![(self.order[0], area)];
        }

        let max_step_x = (area.width / 5).clamp(2, 4);
        let max_step_y = (area.height / 6).clamp(1, 2);
        let total_offset_x = max_step_x * (count.saturating_sub(1) as u16);
        let total_offset_y = max_step_y * (count.saturating_sub(1) as u16);

        let win_w = area.width.saturating_sub(total_offset_x).max(20).min(area.width);
        let win_h = area.height.saturating_sub(total_offset_y).max(6).min(area.height);

        self.order
            .iter()
            .enumerate()
            .map(|(idx, &pane_id)| {
                let x = (area.x + (idx as u16) * max_step_x)
                    .min(area.x + area.width.saturating_sub(win_w));
                let y = (area.y + (idx as u16) * max_step_y)
                    .min(area.y + area.height.saturating_sub(win_h));
                (pane_id, Rect::new(x, y, win_w, win_h))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn z_stack_shuffles_top_to_bottom_bottom_to_top_and_selectively_raises() {
        let mut stack = ZStack::new(1);
        stack.push(2);
        stack.push(3);
        assert_eq!(stack.order(), &[1, 2, 3]);
        assert_eq!(stack.top(), Some(3));

        // Shuffle top-to-bottom: 3 goes to bottom (0), 2 becomes top
        assert_eq!(stack.shuffle_top_to_bottom(), Some(2));
        assert_eq!(stack.order(), &[3, 1, 2]);

        // Shuffle bottom-to-top: 3 comes back from bottom to top
        assert_eq!(stack.shuffle_bottom_to_top(), Some(3));
        assert_eq!(stack.order(), &[1, 2, 3]);

        // Selective raise pane #1 to top
        assert!(stack.raise(1));
        assert_eq!(stack.order(), &[2, 3, 1]);
        assert_eq!(stack.rank_of(1), Some((3, 3)));
    }
}
