# Phase 25 Plan 01 Summary — Typography Config, DEC Line-Size Attributes & Multi-Cell Rendering (`FONT-01`)

**Status:** Complete
**Wave:** 1
**Requirements Covered:** `FONT-01` (Config model, DEC `DECSWL`/`DECDWL`/`DECDHL` parser & `RowSnapshot`, Ratatui multi-cell row rendering, `OSC 50` host passthrough)

## Key Changes
- Created [`src/ui/typography.rs`](file:///home/bill/src/ai/bilterm/src/ui/typography.rs) implementing `LineSizeAttr` (`Single`, `DoubleWidth`, `DoubleHeightTop`, `DoubleHeightBottom`), `scan_dec_line_size_escapes`, `encode_host_font_passthrough`, `render_double_width_row`, and `WindowManager` font scaling methods (`font_zoom_in`, `font_zoom_out`, `font_zoom_reset`, `font_set_size`, `resize_for_pixel_window`).
- Extended [`BiltermConfig`](file:///home/bill/src/ai/bilterm/src/config/mod.rs) with `font_size_pt` (`14`), `font_scale_pct` (`100`), `cell_width_px` (`8`), `cell_height_px` (`16`), `host_font_passthrough` (`true`), `scaled_cell_dimensions_px`, `scaled_font_size_pt`, `step_font_scale`, `reset_font_scale`, and `set_font_size_or_scale`.
- Extended [`RowSnapshot`](file:///home/bill/src/ai/bilterm/src/vt/snapshot.rs), [`TerminalBackend`](file:///home/bill/src/ai/bilterm/src/vt/backend.rs), [`VtParser`](file:///home/bill/src/ai/bilterm/src/vt/mod.rs), and [`render_output_canvas`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs) to track and render DEC double-width/double-height rows across 2 horizontal cells per logical character.

## Verification
- `cargo test --lib` (`148 passed; 0 failed`)
