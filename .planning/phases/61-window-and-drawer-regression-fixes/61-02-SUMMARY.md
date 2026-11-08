# Phase 61-02 Summary: Derive Effective Zoom from Focus and True Zoom Toggle

## Delivered
- **`FIX-02` Derived Effective Zoom & True Toggle (`src/wm/mod.rs`)**:
  - Implemented private `fn effective_zoomed(&self) -> Option<PaneId>` deriving zoom state from `self.zoomed_pane.and(self.z_stack.top()).filter(|id| self.panes.contains_key(id))`.
  - Updated `pub fn zoomed_pane(&self)` to return `self.effective_zoomed()`.
  - Updated `compute_workspace_rects` to use `self.effective_zoomed()`, ensuring `hit_test_pane` and workspace rendering always target the active focused pane when zoomed.
  - Made `toggle_zoom` a true toggle flipping between `Some(top_id)` and `None` unconditionally based on `self.zoomed_pane.is_some()`.
  - Added `self.reflow_default_viewport()` calls in `focus_and_raise`, `shuffle_deck_next`, and `shuffle_deck_prev` when `self.zoomed_pane.is_some()`.
- **`tests/phase61_zoom_follows_focus.rs`**:
  - Added 8 independent integration tests covering `shuffle_while_zoomed_keeps_zoom_on_active_pane`, `toggle_after_focus_change_unzooms`, `keys_reach_visible_zoomed_pane`, `menu_window_zoom_toggles_via_keys`, `menu_window_zoom_via_mouse`, `mouse_click_when_zoomed_never_targets_hidden_pane`, `zoom_with_quick_terminal_visible_does_not_panic`, and `remove_zoomed_or_top_pane_clears_zoom`.
