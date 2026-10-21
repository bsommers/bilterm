# ADR-004: Two-Column Wide-Grapheme Occlusion Boundary Sanitization

- **Status:** Accepted
- **Date:** 2026-10-20

## Context
When an overlay rectangle (`FloatingCommandPrompt`, stacked `CompositePane`, or Guake `QuickTerminal`) is drawn over an `OutputCanvas` containing 2-column wide CJK characters, Emoji, or Nerd Font icons, a wide character whose lead cell sits at `rect.x - 1` or `rect.x + rect.width - 1` has half of its 2-cell footprint overwritten, causing host terminal emulators to shift or corrupt the row's box-drawing borders.

## Decision
`occlude_rect(buf: &mut Buffer, rect: Rect)` (`src/ui/occlusion.rs`) runs before every overlay render pass:
1. For each row `y`, if `rect.x > buf.area.x` and `UnicodeWidthStr::width(buf[(rect.x - 1, y)].symbol()) >= 2`, `buf[(rect.x - 1, y)]` is replaced with `" "`.
2. If `rect.x + rect.width < buf.area.right()` and `UnicodeWidthStr::width(buf[(rect.x + rect.width - 1, y)].symbol()) >= 2`, `buf[(rect.x + rect.width, y)]` is replaced with `" "`.
3. `Clear.render(rect, buf)` then clears the interior rectangle.

## Consequences
- Box-drawing borders (`╔═╗`, `┌─┐`) remain pixel-aligned even when overlaid directly over dense CJK/Emoji terminal output.
