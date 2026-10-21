---
phase: 03-window-manager-h-v-composite-splits-z-order-stacking-shuffling-guake-drop-down-terminal
status: passed
verified: 2026-10-20
requirements_verified:
  - WM-01
  - WM-02
  - WM-03
  - WM-04
  - WM-05
score: 5/5
---

# Phase 3 Verification: Window Manager — H/V Composite Splits, Z-Order Stacking/Shuffling & Guake Drop-Down Terminal

## Goal-Backward Verification Summary

**Phase Goal:** As a terminal multiplexer user, I want to split composite panes horizontally or vertically, stack and shuffle panes in z-order or raise them via keyboard/mouse, and toggle a persistent Guake-style drop-down quick terminal with wide-character-safe occlusion, so that I can manage multiple tiled, stacked, and pop-over terminal workspaces without visual corruption.

**Verdict:** `status: passed` (5/5 observable truths verified, 5/5 requirements verified, 36/36 automated tests passing, 0 clippy warnings).

## Observable Success Criteria Verification

| # | Success Criterion | Status | Evidence |
|---|-------------------|--------|----------|
| 1 | User can split any composite pane horizontally or vertically, spawning a new independent `CompositePane` with its own PTY `OutputCanvas` and its own movable `FloatingCommandPrompt`. | **PASSED** | Verified in `wm::split_tree::tests` and `snapshot_hv_composite_splits`. |
| 2 | User can stack composite panes in z-order, rotate/shuffle the pane deck top-to-bottom or bottom-to-top, and selectively raise any pane to the front via keyboard shortcuts (`Alt+1`..`Alt+9`) or mouse clicks. | **PASSED** | Verified in `wm::z_stack::tests` and `snapshot_z_order_stacked_cascade_and_deck_shuffle`. |
| 3 | User can press a dedicated hotkey to toggle a persistent Guake/Quake-style drop-down quick terminal overlay for quick commands that preserves its live PTY session when hidden. | **PASSED** | Verified in `wm::quick_terminal::tests` and `snapshot_guake_quick_terminal_overlay`. |
| 4 | User can move, resize, focus/raise, zoom/maximize, and close composite panes via keyboard chords, mouse dragging/clicking on window borders, or `:command` palette commands, while SGR (1006) mouse coordinates are accurately translated to the focused pane's local canvas coordinates. | **PASSED** | Verified in `wm::tests::window_manager_splits_stacks_shuffles_raises_and_translates_sgr_mouse`. |
| 5 | Overlapping floating prompts, stacked composite panes, drop-down overlays, and drop shadows sanitize 2-column wide Unicode/CJK/Emoji/Nerd Font graphemes at occlusion boundaries (`occlude_rect`) so multi-column characters never shift or corrupt borders. | **PASSED** | Verified in `ui::occlusion::tests` and `snapshot_wide_char_cjk_occlusion_boundary`. |

## Requirement Traceability

| Requirement | Description | Status | Verification Test |
|-------------|-------------|--------|-------------------|
| `WM-01` | H/V binary splits of `CompositePane`s | **VERIFIED** | `wm::split_tree::tests`, `snapshot_hv_composite_splits` |
| `WM-02` | Z-order stacking, top-to-bottom/bottom-to-top deck shuffling, and selective raise | **VERIFIED** | `wm::z_stack::tests`, `snapshot_z_order_stacked_cascade_and_deck_shuffle` |
| `WM-03` | Multi-pane keyboard/mouse/`:command` controls and SGR (1006) local mouse coordinate translation | **VERIFIED** | `wm::tests::window_manager_splits_stacks_shuffles_raises_and_translates_sgr_mouse` |
| `WM-04` | Wide-character (`unicode-width`) boundary sanitization (`occlude_rect`) | **VERIFIED** | `ui::occlusion::tests`, `snapshot_wide_char_cjk_occlusion_boundary` |
| `WM-05` | Persistent Guake/Quake-style drop-down quick terminal overlay | **VERIFIED** | `wm::quick_terminal::tests`, `snapshot_guake_quick_terminal_overlay` |
