# Phase 25 Research: Dynamic Terminal Text & Font Size Scaling (`FONT-01`)

**Phase:** 25 (`FONT-01`)
**Date:** 2026-10-24

## 1. Requirements & Success Criteria (`FONT-01`)

1. **`Action` & Control Surfaces**:
   - `Action::FontZoomIn`, `Action::FontZoomOut`, `Action::FontZoomReset`, and `Action::FontSetSize(u16)` (plus `:fontsize <pt|pct|+|-|reset>`) wired to:
     - Keyboard shortcuts: `Ctrl++` / `Ctrl+=`, `Ctrl+-`, `Ctrl+0` (in `DEFAULT_GLOBAL_BINDINGS` and `DEFAULT_LEADER_BINDINGS`)
     - Colon commands: `:fontsize [+|-|reset|<pt>|<pct>%]` (aliases `:font`, `:textsize`)
     - JSON-RPC 2.0 / MCP: `workspace.fontsize` (`bilterm_workspace_fontsize`)
     - `config.toml`: `font_size_pt` (default `14`), `font_scale_pct` (default `100`, range `50..=300`), `cell_width_px` (default `8`), `cell_height_px` (default `16`), `host_font_passthrough` (default `true`).
2. **Native Pixel / SVG Renderer Scaling (`GuiWorkspaceRenderer` / `GuiFrameSurface`)**:
   - Scales cell pixel dimensions (`cell_width_px`, `cell_height_px`) proportionally with `font_scale_pct` (`50%..=300%`) and `font_size_pt`.
   - Recalculates viewport `(rows, cols)` for a given pixel window size (`viewport_for_pixel_window(width_px, height_px) -> Rect`) and resizes all workspace panes accordingly (`wm.resize_all(viewport)`).
   - Scales SVG typography (`font-size`, `textLength` / horizontal scale for `DECDWL`/`DECDHL` lines) in `GuiFrameSurface::export_svg` and scales PPM bitmap glyph strokes in `GuiFrameSurface::rasterize_rgba`.
3. **TUI Mode (`Ratatui`) DEC Line Attributes & Host Font Passthrough**:
   - `VtParser` tracks per-row DEC line size attributes (`LineSizeAttr`: `Single` `\x1b#5`, `DoubleWidth` `\x1b#6` `DECDWL`, `DoubleHeightTop` `\x1b#3` `DECDHL`, `DoubleHeightBottom` `\x1b#4` `DECDHL`) and exposes `line_size_attr(row: u16) -> LineSizeAttr` on `VtParser`, `RowSnapshot`, and `TerminalBackend`.
   - `render_output_canvas` renders `DoubleWidth` / `DoubleHeightTop` / `DoubleHeightBottom` lines with 2-cell horizontal character spacing/expansion on the Ratatui cell grid, and pairs with the focal zoom lens (`render_focal_zoom_cursor`) and optional host-terminal `OSC 50` (`\x1b]50;SetProfile=...\x07` / `\x1b]50;fontsize=<pt>\x07`) + Kitty remote font-size passthrough queued via `drain_outbound_osc52`.

## 2. File-Size Budget Strategy (`<= 800` lines per file, `ARCH-R04`)

- `src/vt/mod.rs` is `798` lines -> move existing helper methods or place DEC line-attribute helpers in a new submodule `src/ui/typography.rs` (and compact `src/vt/mod.rs` slightly so it stays `< 785` lines).
- `src/colon_command/execute.rs` is `752` lines -> delegate `Action::Font*` execution to `wm.execute_font_action(action)` defined in `src/ui/typography.rs`.
- `src/api/rpc.rs` is `760` lines -> delegate `workspace.fontsize` handling to a concise helper in `src/ui/typography.rs`.
