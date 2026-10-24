# Phase 26 Research — Tektronix 4010/4014 × Modern 2026/2027 Vector, Sprite, Plotting & Image Graphical Canvas (`TEK-01`, `CANV-01`)

**Date:** 2026-10-24
**Phase:** 26
**Requirements:** `TEK-01`, `CANV-01`

---

## 1. Architectural Fit & File-Size Budget (`ARCH-R01..05`)

To keep every source file under `src/**/*.rs` strictly `<= 800` lines (`ARCH-R04`):
1. **[`src/vt/tektronix.rs`](file:///home/bill/src/ai/bilterm/src/vt/tektronix.rs)**:
   - Classic Tektronix 4010/4014 storage-tube vector terminal state machine (`TektronixState`, `TekMode`, `TekLineStyle`, `TekVectorSegment`).
   - Decodes `DECSET ?38h` (`\x1b[?38h` enter Tek mode), `DECRST ?38l` (`\x1b[?38l`) / `\x1b\x03` (return to VT100 mode), `\x1b\x0c` (storage-tube erase with phosphor persistence decay), `GS` (`0x1D` Graph dark/bright vector mode), `FS` (`0x1C` Point Plot mode), `RS` (`0x1E` Incremental Plot mode), `US` (`0x1F` Alpha text mode), `\x1b` + `` ` ``..`d` (`Solid`, `Dotted`, `DotDash`, `ShortDash`, `LongDash` line styles), and 10-bit (`1024×780`) / 12-bit (`4096×3120`) Hi-Y, Extra-Byte, Lo-Y, Hi-X, Lo-X coordinate state latches.
2. **[`src/pane/canvas_scene.rs`](file:///home/bill/src/ai/bilterm/src/pane/canvas_scene.rs)**:
   - Modern retained scene-graph (`PaneCanvasScene`), `VectorPrimitive` (`Line`, `Polyline`, `CubicBezier`, `Circle`, `RectShape`, `Polygon`, `TextLabel`), `Sprite2D` (`id`, `x`, `y`, `vx`, `vy`, `width`, `height`, `z_index`, `frames`, `step_physics`, `collides_with` AABB collision), `DeclarativePlot` / `PlotKind` (`Line`, `Scatter`, `Bar`, `Candlestick`, `Heatmap`), and `InteractiveImageView` (`pan_x`, `pan_y`, `zoom`, `inspect_pixel`).
   - In-band `OSC 1337;Canvas=...` parser so programs inside a PTY can stream vectors, sprites, plots, and images directly.
3. **[`src/ui/canvas_raster.rs`](file:///home/bill/src/ai/bilterm/src/ui/canvas_raster.rs)**:
   - Dual-backend rasterizer:
     - **TUI Sub-Cell Rasterizer**: Renders Tektronix 4010/4014 vectors (with green `3278` / amber `3279` phosphor persistence glow) and `PaneCanvasScene` primitives/sprites/plots/images into sub-cell Unicode 2×4 Braille (`U+2800..U+28FF`), 2×3 Sextant (`U+1FB00..U+1FB3B`), or Quadrant blocks with 24-bit TrueColor inside Pass 1 (`render_output_canvas`), preserving Pass 2 (`FloatingCommandPrompt`) and Pass 3 (`OiaStatusBar`) compositing on top.
     - **GUI Full-Resolution Renderer**: Renders crisp vector/pixel primitives into `GuiFrameSurface` (SVG and `.ppm`).
   - Houses `WindowManager::execute_canvas_action` and `WindowManager::handle_rpc_canvas` so `src/colon_command/execute.rs` and `src/api/rpc.rs` remain well under 800 lines.

---

## 2. Wave Breakdown

- **Wave 1 (`26-01-PLAN.md`)**: Create `src/vt/tektronix.rs` (`TEK-01`) and `src/pane/canvas_scene.rs` (`CANV-01`), and wire into `VtParser` and `CompositePane`.
- **Wave 2 (`26-02-PLAN.md`)**: Create `src/ui/canvas_raster.rs` (Braille/Sextant/Quadrant TUI rasterizer + GUI renderer + `WindowManager` handlers) and wire `Action::CanvasCommand(String)`, `:canvas <subcommand>`, JSON-RPC `canvas.*`, and MCP `bilterm_canvas`.
- **Wave 3 (`26-03-PLAN.md`)**: Create `tests/tektronix_and_graphical_canvas.rs`, update documentation and `.planning/` state, and verify all quality gates.
