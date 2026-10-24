# Phase 25 Plan 02 Summary — GUI Renderer Scaling, Action/Keymap/Colon & RPC/MCP Wiring (`FONT-01`)

**Status:** Complete
**Wave:** 2
**Requirements Covered:** `FONT-01` (`GuiWorkspaceRenderer` / `GuiFrameSurface` dynamic cell & SVG/PPM scaling, pixel-window grid recalculation, `Action::Font*`, `Ctrl++`/`Ctrl+=`/`Ctrl+-`/`Ctrl+0`, `:fontsize`, `workspace.fontsize` RPC & MCP)

## Key Changes
- Extended [`GuiFrameSurface`](file:///home/bill/src/ai/bilterm/src/ui/renderer.rs) and [`GuiWorkspaceRenderer`](file:///home/bill/src/ai/bilterm/src/ui/renderer.rs) to scale `cell_width_px`, `cell_height_px`, `font_size_pt`, `font_scale_pct`, SVG `<text font-size="...">`, and `.ppm` raster glyph dimensions dynamically, and added `viewport_for_pixel_window` and `resize_workspace_for_pixel_window`.
- Added `Action::FontZoomIn`, `Action::FontZoomOut`, `Action::FontZoomReset`, and `Action::FontSetSize(u16)` to [`src/action.rs`](file:///home/bill/src/ai/bilterm/src/action.rs), [`src/colon_command/parse.rs`](file:///home/bill/src/ai/bilterm/src/colon_command/parse.rs), [`src/colon_command/execute.rs`](file:///home/bill/src/ai/bilterm/src/colon_command/execute.rs), and [`src/input/keymap.rs`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs).
- Wired `workspace.fontsize` JSON-RPC method and `bilterm_workspace_fontsize` MCP tool in [`src/api/rpc.rs`](file:///home/bill/src/ai/bilterm/src/api/rpc.rs) and [`src/api/mcp.rs`](file:///home/bill/src/ai/bilterm/src/api/mcp.rs).

## Verification
- `cargo test --lib` (`148 passed; 0 failed`, including `action::tests::every_action_is_reachable_from_keymap_colon_or_rpc`)
