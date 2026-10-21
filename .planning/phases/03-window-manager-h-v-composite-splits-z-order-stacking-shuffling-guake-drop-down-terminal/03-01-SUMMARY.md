---
phase: 03-window-manager-h-v-composite-splits-z-order-stacking-shuffling-guake-drop-down-terminal
plan: 01
subsystem: wm-split-tree-zstack-occlusion
tags: [rust, ratatui, bsp-split-tree, z-order-stack, unicode-width, occlusion]
provides:
  - Binary Space Partitioning (`SplitTree`) for H/V splits of `CompositePane`s (`src/wm/split_tree.rs`)
  - Z-Order Deck Stack Manager (`ZStack`) for top-to-bottom/bottom-to-top shuffling and selective raise (`src/wm/z_stack.rs`)
  - 2-column wide Unicode/CJK/Emoji/Nerd Font boundary sanitizer (`occlude_rect` in `src/ui/occlusion.rs`)
affects: [src/wm/split_tree.rs, src/wm/z_stack.rs, src/ui/occlusion.rs]
tech-stack:
  added: []
  patterns: [Binary Split Tree Layout, Z-Order Painter's Stack, Wide-Grapheme Boundary Sanitization]
key-files:
  created: [src/wm/split_tree.rs, src/wm/z_stack.rs, src/ui/occlusion.rs]
  modified: []
key-decisions:
  - "Implemented `occlude_rect` to inspect `clamped.x - 1` and `clamped.x + clamped.width - 1` for 2-column graphemes (`UnicodeWidthStr::width >= 2`) and blank split halves before clearing"
requirements-completed: [WM-01, WM-02, WM-04]
duration: 3min
completed: 2026-10-20
---

# Phase 3 Plan 01: H/V Binary Split Tree, Z-Order Stack & Wide-Character Occlusion Summary

**Implemented `SplitTree` (`src/wm/split_tree.rs`), `ZStack` (`src/wm/z_stack.rs`), and `occlude_rect` (`src/ui/occlusion.rs`).**
