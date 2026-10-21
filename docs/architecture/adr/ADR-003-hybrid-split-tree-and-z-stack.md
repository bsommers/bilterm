# ADR-003: Hybrid Binary Split Tree & Z-Order Deck Stack Window Manager

- **Status:** Accepted
- **Date:** 2026-10-20

## Context
Bilterm must support both modern terminal multiplexer H/V tiled splits (`tmux`-style) and classic Borland Turbo Vision overlapping z-order window stacking/shuffling (`Alt+Tab` deck rotation, `Alt+1..9` selective raise, mouse click-to-raise), plus a persistent Guake-style drop-down quick terminal (`Alt+\`` / `Ctrl+Space`).

## Decision
`WindowManager` (`src/wm/mod.rs`) pairs a recursive Binary Space Partitioning tree (`SplitTree` in `src/wm/split_tree.rs`) with an explicit Z-Order Deck (`ZStack` in `src/wm/z_stack.rs`) and a persistent `QuickTerminal` (`src/wm/quick_terminal.rs`, `PaneId = 0`):
- In `WorkspaceLayoutMode::Tiled`, `SplitTree` computes non-overlapping rectangles while `ZStack` tracks focus order and deck rotation.
- In `WorkspaceLayoutMode::Stacked`, `ZStack` computes overlapping Borland cascade rectangles and renders panes in ascending z-order (`order[0]` to `order[N-1]`).
- Mouse hit-testing (`hit_test_pane`) traverses visible panes in **reverse z-order** (`order.iter().rev()`), raising clicked panes to the top of `ZStack` and translating global SGR (1006) mouse coordinates to pane-local coordinates.

## Consequences
- Users can seamlessly toggle between Tiled Splits and Stacked Z-Order Cascade windows (`Alt+M` or `:layout tile|stack`) without losing any pane state or PTY sessions.
