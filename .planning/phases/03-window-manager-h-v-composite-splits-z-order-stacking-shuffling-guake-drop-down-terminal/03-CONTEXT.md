# Phase 3: Window Manager — H/V Composite Splits, Z-Order Stacking/Shuffling & Guake Drop-Down Terminal - Context

**Gathered:** 2026-10-20
**Status:** Ready for planning
**Mode:** Autonomous

<domain>
## Phase Boundary

Phase 3 scales Bilterm from a single `CompositePane` to a full multi-pane TUI Window Manager (`src/wm/`) supporting:
1. **Horizontal & Vertical Binary Split Tree (`WM-01`)**: Any `CompositePane` can be split horizontally (top/bottom) or vertically (left/right), spawning a new independent `CompositePane` with its own full-height PTY `OutputCanvas` and its own movable `FloatingCommandPrompt`.
2. **Z-Order Pane Stacking, Deck Shuffling & Selective Raise (`WM-02`)**: Composite panes can be stacked in an ordered Z-Order deck (`z_stack: Vec<PaneId>`), shuffled/rotated top-to-bottom (`Alt+Tab` / `F6` / `:deck next`) or bottom-to-top (`Alt+Shift+Tab` / `:deck prev`), or selectively raised to the front via keyboard shortcuts (`Alt+1`..`Alt+9` window numbers) or mouse clicks anywhere inside an occluded pane.
3. **Triple-Modality Multi-Pane Controls & SGR (1006) Local Coordinate Translation (`WM-03`)**: Users can split, focus/raise, resize split ratios, zoom/maximize (`F5` / `Alt+Z` / `:zoom`), detach/stack (`:float-pane`), and close panes via keyboard chords, mouse clicks/drags on window borders/dividers, or `:command` execution, while translating global host SGR (1006) mouse coordinates to the target pane's local canvas coordinates.
4. **Wide-Character Boundary Sanitization (`WM-04`)**: Whenever a floating command prompt, stacked z-order window, drop shadow, or Guake drop-down terminal occludes an underlying pane buffer, `occlude_rect(buf, rect)` sanitizes 2-column wide Unicode/CJK/Emoji/Nerd Font graphemes (`unicode-width`) straddling the left (`rect.x`) or right (`rect.x + rect.width`) edges so multi-column characters never shift rows or corrupt box-drawing borders.
5. **Persistent Guake/Quake-Style Drop-Down Quick Terminal (`WM-05`)**: A dedicated global hotkey (`Alt+\`` / `Ctrl+Space` / `:quake`) toggles a top-anchored persistent quick terminal overlay (configurable height percentage, default 40% of screen height) that slides over the workspace for rapid "by-the-way" commands while keeping its underlying shell/PTY process alive when hidden.

</domain>

<decisions>
## Implementation Decisions

### Hybrid Tiled + Stacked Z-Order Window Manager (`src/wm/`)
- **Split Tree (`src/wm/split_tree.rs`)**: Binary Space Partitioning (BSP) tree (`SplitNode::Leaf(PaneId)` and `SplitNode::Split { direction: SplitDirection, ratio_pct: u16, first: Box<SplitNode>, second: Box<SplitNode> }`) computes exact `Rect` viewports for tiled composite panes and supports recursive H/V splitting, leaf removal, and ratio adjustment (`10%..=90%`).
- **Z-Order Stack & Layout Mode (`src/wm/z_stack.rs`)**: Maintains `order: Vec<PaneId>` where `order.last()` is the topmost/focused pane (`z-index = N-1`). Supports both **Tiled Split Mode** and **Overlapping Cascade/Stack Mode** (where panes overlap in classic Borland Turbo Vision cascade geometry or deck-stacked full/floating rectangles and can be shuffled top-to-bottom, bottom-to-top, or selectively raised to the top of `order` via `Alt+1..9` or mouse hit-testing from top of `order` downward).
- **Persistent Guake Drop-Down Quick Terminal (`src/wm/quick_terminal.rs`)**: Holds a dedicated persistent `CompositePane` (`id = 0`, title `"QUAKE QUICK TERM [~]"`) and `visible: bool`, `height_pct: u16` (default `40%`). Toggling `visible` never destroys or sends `SIGHUP` to its PTY; when visible, it renders last (highest z-index) at the top of the screen over all tiled/stacked panes and intercepts keyboard/mouse events.
- **Wide-Character Occlusion Sanitizer (`src/ui/occlusion.rs`)**: `occlude_rect(buf: &mut Buffer, rect: Rect)` inspects `rect.x` (if `rect.x > buf.area.x`, checks if `buf[(rect.x - 1, y)]` has `UnicodeWidthStr::width == 2` and replaces it with `" "` before clearing) and `rect.x + rect.width` (checks if `buf[(rect.x + rect.width - 1, y)]` was the lead cell of a 2-column grapheme and replaces `buf[(rect.x + rect.width, y)]` with `" "`), then runs `Clear.render(rect, buf)`.

</decisions>
