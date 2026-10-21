# Phase 3: Window Manager — H/V Composite Splits, Z-Order Stacking/Shuffling & Guake Drop-Down Terminal - Research

**Researched:** 2026-10-20
**Status:** Complete

## 1. Binary Space Partitioning (`SplitTree`)
- `SplitTree` represents tiled H/V splits of `CompositePane`s using recursive binary tree nodes:
  - `SplitDirection::Horizontal` splits a region into Top (`first`) and Bottom (`second`) sub-rectangles.
  - `SplitDirection::Vertical` splits a region into Left (`first`) and Right (`second`) sub-rectangles.
- Computing layout rectangles (`compute_rects(area: Rect) -> Vec<(PaneId, Rect)>`) uses `ratatui::layout::{Layout, Direction, Constraint}` at each internal split node with `Constraint::Percentage(ratio_pct)` and `Constraint::Percentage(100 - ratio_pct)`.

## 2. Z-Order Deck Stacking & Cascade Geometry (`ZStack`)
- `ZStack` maintains a `Vec<PaneId>` where the last element (`last()`) is the highest z-order (focused) pane.
- In **Tiled mode**, `SplitTree` determines each pane's `Rect`, while `ZStack` tracks focus history and deck order.
- In **Stacked / Cascade mode** (`Alt+S` or `:layout stack|tile`), each pane in `ZStack` is assigned an overlapping cascaded `Rect` (or floating `Rect`) and rendered in ascending z-order (`z_stack.iter()`) using `occlude_rect` before drawing each pane so higher-z panes cleanly occlude lower-z panes.
- Shuffling the deck (`shuffle_top_to_bottom` / `shuffle_bottom_to_top`) or selectively raising any pane (`raise(pane_id)` via `Alt+1..9` or mouse click) immediately brings that pane to the top of `ZStack` and focuses it.

## 3. SGR (1006) Local Mouse Coordinate Translation
- When a mouse event arrives at global terminal coordinates `(col, row)`:
  - If the Guake drop-down quick terminal is visible and `(col, row)` falls inside its `Rect`, the event is routed to the quick terminal pane.
  - Otherwise, hit-testing iterates through visible panes in **reverse z-order** (topmost pane first). The first pane whose `Rect` contains `(col, row)` is raised to the top of `ZStack` (on `MouseEventKind::Down`), and if the pane is in `RawPtyPassthrough` mode with mouse tracking enabled, global `(col, row)` is translated to pane-local `1`-based SGR 1006 coordinates `(col - pane_rect.x + 1, row - pane_rect.y + 1)` (`\x1b[<btn;local_col;local_rowM`).
