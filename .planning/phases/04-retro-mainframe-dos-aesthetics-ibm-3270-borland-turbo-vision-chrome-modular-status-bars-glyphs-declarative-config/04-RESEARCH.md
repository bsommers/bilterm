# Phase 4: Retro Mainframe & DOS Aesthetics — IBM 3270 + Borland Turbo Vision Chrome, Modular Status Bars, Glyphs & Declarative Config - Research

**Researched:** 2026-10-20
**Status:** Complete

## 1. Translucent Character-Cell Drop Shadows (`render_drop_shadow`)
- Classic Borland Turbo Vision (1990) did not overwrite shadowed screen cells with opaque `░` blocks; instead, it mutated the VGA attribute byte of the 2 columns to the right and 1 row below every floating window to `0x08` (DarkGray on Black), allowing the underlying text to remain legible through the shadow!
- In Ratatui's `Buffer`, `render_drop_shadow(buf, rect, bounds)` iterates over the right 2 columns (`rect.x + rect.width .. rect.x + rect.width + 2`, `y = rect.y + 1 .. rect.y + rect.height + 1`) and bottom 1 row (`x = rect.x + 2 .. rect.x + rect.width + 2`, `y = rect.y + rect.height`), clamping to `bounds`, and updates each cell's `Style` to `fg(Color::DarkGray).bg(Color::Black)`.

## 2. IBM 3270 Full-Pane Cross-Hair Cursor (`render_crosshair_cursor`)
- Hardware terminal cursors can only highlight a single `(x, y)` cell.
- `render_crosshair_cursor(buf, pane_rect, cursor_x, cursor_y, tint_bg, caret_style)` iterates over all cells `(x, cursor_y)` across `pane_rect` (horizontal hair) and `(cursor_x, y)` down `pane_rect` (vertical hair) and sets their background to `tint_bg` (`#002B55` in Turbo Blue, `#003300` in 3278 Green, `#331900` in 3279 Amber) while applying the user's chosen `CaretStyle` (`SolidBlock`, `BlinkingHollowBlock` `▯`, `Underscore` `_`, or `VerticalBar` `▏`) at `(cursor_x, cursor_y)`.

## 3. Declarative TOML & INI Configuration (`src/config/mod.rs`)
- `BiltermConfig` uses `serde::Deserialize` and `serde::Serialize` with `#[serde(default)]` on every section (`[theme]`, `[prompt]`, `[cursor]`, `[status_bar]`, `[quick_terminal]`) so partial `~/.config/bilterm/config.toml` files load cleanly with sensible defaults.
- Also provides `parse_ini_theme(ini_text: &str) -> Result<ThemePalette>` so users can import classic `.ini` color theme definitions (`[colors] surface_bg = #001C38 ...`).
