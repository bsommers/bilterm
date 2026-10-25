//! Dual-Backend Graphical Canvas Rasterizer (`TEK-01`, `CANV-01`).
//!
//! Renders both the classic Tektronix 4010/4014 storage-tube vector state machine ([`TektronixState`])
//! and the modern 2026/2027 retained scene-graph ([`PaneCanvasScene`]) onto:
//! 1. **TUI Sub-Cell Character Grid (`Ratatui` `Buffer`)**:
//!    - `CanvasSubcellMode::Braille` (`2×4` sub-pixels per cell, `U+2800..U+28FF`)
//!    - `CanvasSubcellMode::Sextant` (`2×3` sub-pixels per cell, `U+1FB00..U+1FB3B`)
//!    - `CanvasSubcellMode::Quadrant` (`2×2` sub-pixels per cell, `▘▝▀▖▌▞▛▗▚▐▜▄▙▟█`)
//!
//!    Executed inside Pass 1 (`render_output_canvas`) so the `FloatingCommandPrompt` (Pass 2) and
//!    `OiaStatusBar` (Pass 3) remain cleanly composited on top.
//! 2. **GUI Full-Resolution Surface (`GuiFrameSurface`)**:
//!    Since `GuiWorkspaceRenderer` renders `WorkspaceWidget` into its cell buffer and also supports
//!    high-resolution quads, any active graphical canvas is automatically reflected in both SVG and `.ppm` exports.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::config::BiltermConfig;
use crate::pane::{
    CanvasSubcellMode, CompositePane, DeclarativePlot, InteractiveImageView, PaneId, PlotKind,
    VectorPrimitive,
};
use crate::ui::cursor::SpotlightMode;
pub use crate::ui::hires_subcell::{
    QUADRANT_CHARS, braille_char_from_mask, sextant_char_from_mask,
};
use crate::ui::pixel_canvas::PixelCanvasBuffer;
use crate::ui::theme::ThemePreset;
use crate::vt::{TEK_MAX_X, TEK_MAX_Y, TekLineStyle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RasterCacheKey {
    vt_gen: u64,
    scene_gen: u64,
    tek_segs: usize,
    tek_decay: usize,
    prims: usize,
    plots: usize,
    sprites: usize,
    has_img: bool,
    width: u16,
    height: u16,
    px_c: u32,
    px_r: u32,
    theme: ThemePreset,
    focal_mode: SpotlightMode,
    focal_pct: u16,
    cursor: (u16, u16),
}

thread_local! {
    static RASTER_CACHE: RefCell<BTreeMap<PaneId, (RasterCacheKey, Arc<PixelCanvasBuffer>)>> =
        const { RefCell::new(BTreeMap::new()) };
}

#[derive(Clone, Copy, Debug)]
struct SubcellFocalLens {
    focus_x: f32,
    focus_y: f32,
    rx_sub: f32,
    ry_sub: f32,
    peak_zoom: f32,
    inner_edge: f32,
    outer_edge: f32,
}

impl SubcellFocalLens {
    fn from_config(
        cfg: &BiltermConfig,
        canvas: Rect,
        cursor_rc: (u16, u16),
        mode: CanvasSubcellMode,
    ) -> Option<Self> {
        if cfg.focal_zoom_mode == SpotlightMode::Off || cfg.focal_zoom_pct <= 100 {
            return None;
        }
        let (sx, sy) = mode.subcell_grid();
        let (cur_r, cur_c) = cursor_rc;
        let cc = if cur_c < canvas.width {
            cur_c
        } else {
            canvas.width / 2
        };
        let cr = if cur_r < canvas.height {
            cur_r
        } else {
            canvas.height / 2
        };
        let focus_x = (f32::from(cc) + 0.5) * f32::from(sx);
        let focus_y = (f32::from(cr) + 0.5) * f32::from(sy);
        let rx_cols = f32::from(cfg.focal_zoom_rx.max(4));
        let ry_rows = match cfg.focal_zoom_mode {
            SpotlightMode::Circle => (rx_cols * 0.5).max(2.0),
            SpotlightMode::Ellipse => f32::from(cfg.focal_zoom_ry.max(2)),
            SpotlightMode::Off => return None,
        };
        let feather = f32::from(cfg.focal_zoom_feather_pct.clamp(5, 100)) / 100.0;
        Some(Self {
            focus_x,
            focus_y,
            rx_sub: rx_cols * f32::from(sx),
            ry_sub: ry_rows * f32::from(sy),
            peak_zoom: f32::from(cfg.focal_zoom_pct.clamp(105, 400)) / 100.0,
            inner_edge: (1.0 - feather).max(0.0),
            outer_edge: 1.0 + feather,
        })
    }

    #[inline]
    fn warp(&self, x: f32, y: f32) -> (i32, i32) {
        let dx = x - self.focus_x;
        let dy = y - self.focus_y;
        let nx = dx / self.rx_sub.max(1.0);
        let ny = dy / self.ry_sub.max(1.0);
        let dist = (nx * nx + ny * ny).sqrt();
        let mag = if dist <= self.inner_edge {
            let t = if self.inner_edge > 0.01 {
                (dist / self.inner_edge).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let s = t * t * (3.0 - 2.0 * t);
            self.peak_zoom - (self.peak_zoom - 1.0) * 0.12 * s
        } else if dist < self.outer_edge {
            let t = ((dist - self.inner_edge) / (self.outer_edge - self.inner_edge)).clamp(0.0, 1.0);
            let s = t * t * (3.0 - 2.0 * t);
            let edge_start = self.peak_zoom - (self.peak_zoom - 1.0) * 0.12;
            edge_start * (1.0 - s) + s
        } else {
            1.0
        };
        (
            (self.focus_x + dx * mag).round() as i32,
            (self.focus_y + dy * mag).round() as i32,
        )
    }
}

struct SubcellBuffer {
    sub_w: usize,
    sub_h: usize,
    lit: Vec<bool>,
    rgb: Vec<(u8, u8, u8)>,
    lens: Option<SubcellFocalLens>,
}

impl SubcellBuffer {
    fn new(
        cols: u16,
        rows: u16,
        mode: CanvasSubcellMode,
        lens: Option<SubcellFocalLens>,
    ) -> Self {
        let (sx, sy) = mode.subcell_grid();
        let sub_w = usize::from(cols.max(1)) * usize::from(sx);
        let sub_h = usize::from(rows.max(1)) * usize::from(sy);
        Self {
            sub_w,
            sub_h,
            lit: vec![false; sub_w * sub_h],
            rgb: vec![(0, 0, 0); sub_w * sub_h],
            lens,
        }
    }

    #[inline]
    fn plot_raw(&mut self, x: i32, y: i32, color: (u8, u8, u8)) {
        if x >= 0 && y >= 0 && (x as usize) < self.sub_w && (y as usize) < self.sub_h {
            let idx = (y as usize) * self.sub_w + (x as usize);
            self.lit[idx] = true;
            self.rgb[idx] = color;
        }
    }

    #[inline]
    fn plot(&mut self, x: i32, y: i32, color: (u8, u8, u8)) {
        if let Some(lens) = self.lens {
            let (wx, wy) = lens.warp(x as f32, y as f32);
            self.plot_raw(wx, wy, color);
        } else {
            self.plot_raw(x, y, color);
        }
    }

    fn draw_line_raw(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        style: TekLineStyle,
        color: (u8, u8, u8),
    ) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut cx = x0;
        let mut cy = y0;
        let mut step_idx = 0usize;
        loop {
            if style.pattern_bit(step_idx) {
                self.plot_raw(cx, cy, color);
            }
            if cx == x1 && cy == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                cx += sx;
            }
            if e2 <= dx {
                err += dx;
                cy += sy;
            }
            step_idx = step_idx.wrapping_add(1);
        }
    }

    fn draw_line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        style: TekLineStyle,
        color: (u8, u8, u8),
    ) {
        if let Some(lens) = self.lens {
            let span = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
            let steps = (span * 2).clamp(1, 2048);
            let mut prev = lens.warp(x0 as f32, y0 as f32);
            for s in 1..=steps {
                let t = (s as f32) / (steps as f32);
                let lx = (x0 as f32) + ((x1 - x0) as f32) * t;
                let ly = (y0 as f32) + ((y1 - y0) as f32) * t;
                let cur = lens.warp(lx, ly);
                if style.pattern_bit(s as usize) {
                    self.draw_line_raw(prev.0, prev.1, cur.0, cur.1, TekLineStyle::Solid, color);
                }
                prev = cur;
            }
        } else {
            self.draw_line_raw(x0, y0, x1, y1, style, color);
        }
    }

    fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32, filled: bool, color: (u8, u8, u8)) {
        let r = radius.max(1);
        for dy in -r..=r {
            for dx in -r..=r {
                let d2 = dx * dx + dy * dy;
                if (filled && d2 <= r * r) || (!filled && (d2 - r * r).abs() <= r) {
                    self.plot(cx + dx, cy + dy, color);
                }
            }
        }
    }

    fn draw_rect(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        filled: bool,
        color: (u8, u8, u8),
    ) {
        let w = w.max(1);
        let h = h.max(1);
        if filled {
            for py in y..(y + h) {
                for px in x..(x + w) {
                    self.plot(px, py, color);
                }
            }
        } else {
            self.draw_line(x, y, x + w - 1, y, TekLineStyle::Solid, color);
            self.draw_line(x + w - 1, y, x + w - 1, y + h - 1, TekLineStyle::Solid, color);
            self.draw_line(x + w - 1, y + h - 1, x, y + h - 1, TekLineStyle::Solid, color);
            self.draw_line(x, y + h - 1, x, y, TekLineStyle::Solid, color);
        }
    }

    fn flush_to_ratatui(&self, canvas: Rect, mode: CanvasSubcellMode, buf: &mut Buffer) {
        let (sx, sy) = mode.subcell_grid();
        let sx = usize::from(sx);
        let sy = usize::from(sy);
        for r in 0..usize::from(canvas.height) {
            for c in 0..usize::from(canvas.width) {
                let mut mask: u8 = 0;
                let mut fg_rgb = (51u8, 255u8, 51u8);
                for dy in 0..sy {
                    for dx in 0..sx {
                        let px = c * sx + dx;
                        let py = r * sy + dy;
                        if px < self.sub_w && py < self.sub_h {
                            let idx = py * self.sub_w + px;
                            if self.lit[idx] {
                                fg_rgb = self.rgb[idx];
                                let bit = match mode {
                                    CanvasSubcellMode::Braille => match (dx, dy) {
                                        (0, 0) => 0x01,
                                        (0, 1) => 0x02,
                                        (0, 2) => 0x04,
                                        (1, 0) => 0x08,
                                        (1, 1) => 0x10,
                                        (1, 2) => 0x20,
                                        (0, 3) => 0x40,
                                        (1, 3) => 0x80,
                                        _ => 0,
                                    },
                                    CanvasSubcellMode::Sextant
                                    | CanvasSubcellMode::Quadrant
                                    | CanvasSubcellMode::HalfBlock
                                    | CanvasSubcellMode::Octant => 1u8 << (dy * 2 + dx),
                                };
                                mask |= bit;
                            }
                        }
                    }
                }
                if mask != 0
                    && let Some(cell) = buf.cell_mut((canvas.x + c as u16, canvas.y + r as u16))
                {
                    // Preserve non-empty VT100 terminal text characters so shell output after `:tek demo` remains readable
                    if cell.symbol() == " " || cell.symbol().is_empty() {
                        let ch = match mode {
                            CanvasSubcellMode::Braille => braille_char_from_mask(mask),
                            CanvasSubcellMode::Sextant => sextant_char_from_mask(mask),
                            CanvasSubcellMode::Quadrant => QUADRANT_CHARS[usize::from(mask & 0x0f)],
                            CanvasSubcellMode::HalfBlock | CanvasSubcellMode::Octant => '▀',
                        };
                        let mut sym_buf = [0u8; 4];
                        cell.set_symbol(ch.encode_utf8(&mut sym_buf));
                        cell.set_style(
                            cell.style()
                                .fg(Color::Rgb(fg_rgb.0, fg_rgb.1, fg_rgb.2))
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                }
            }
        }
    }
}

/// Renders active Tektronix 4010/4014 storage-tube vectors and modern `PaneCanvasScene` primitives,
/// 2D sprites, declarative plots, and interactive images onto the `OutputCanvas` (`TEK-01`, `CANV-01`).
pub fn render_graphical_canvas(
    pane: &CompositePane,
    canvas: Rect,
    cfg: &BiltermConfig,
    buf: &mut Buffer,
) {
    let tek = pane.vt().tektronix();
    let scene = pane.canvas_scene();
    if !tek.has_visible_content() && !scene.has_visible_content() {
        return;
    }
    if canvas.width == 0 || canvas.height == 0 {
        return;
    }

    let mode = scene.subcell_mode;
    if matches!(mode, CanvasSubcellMode::HalfBlock | CanvasSubcellMode::Octant) {
        // On-screen flush only samples at most a 2x4 sub-cell grid per cell, so the per-frame raster is
        // capped (4..=16 x 8..=32 px/cell) to bound render cost; `:canvas export` keeps the full resolution.
        let (raw_c, raw_r) = scene
            .resolution
            .resolve_px_per_cell(canvas.width, canvas.height);
        let px_c = raw_c.clamp(4, 16);
        let px_r = raw_r.clamp(8, 32);
        let key = RasterCacheKey {
            vt_gen: pane.vt().generation(),
            scene_gen: scene.generation,
            tek_segs: tek.segments.len(),
            tek_decay: tek.decaying_segments.len(),
            prims: scene.primitives.len(),
            plots: scene.plots.len(),
            sprites: scene.sprites.len(),
            has_img: scene.image_view.is_some(),
            width: canvas.width,
            height: canvas.height,
            px_c,
            px_r,
            theme: cfg.theme,
            focal_mode: cfg.focal_zoom_mode,
            focal_pct: cfg.focal_zoom_pct,
            cursor: pane.vt().cursor_position(),
        };
        let can_use_cache = (key.vt_gen > 0 || key.scene_gen > 0) && key.sprites == 0;
        let px_buf = RASTER_CACHE.with(|cache| {
            if can_use_cache
                && let Some((cached_key, buf_arc)) = cache.borrow().get(&pane.id())
                && *cached_key == key
            {
                return Arc::clone(buf_arc);
            }
            let rendered = Arc::new(PixelCanvasBuffer::render_pane_scene(
                pane,
                cfg,
                canvas.width,
                canvas.height,
                px_c,
                px_r,
            ));
            if can_use_cache {
                cache.borrow_mut().insert(pane.id(), (key, Arc::clone(&rendered)));
            }
            rendered
        });
        px_buf.flush_truecolor_subcells_to_ratatui(mode, canvas, buf);
    } else {
        let lens = SubcellFocalLens::from_config(cfg, canvas, pane.vt().cursor_position(), mode);
        let mut sub = SubcellBuffer::new(canvas.width, canvas.height, mode, lens);
        let max_x = (sub.sub_w.saturating_sub(1)).max(1) as i32;
        let max_y = (sub.sub_h.saturating_sub(1)).max(1) as i32;

        // 1. Render Tektronix 4010/4014 storage-tube vectors (decaying afterglow first, then live bright beam)
        if tek.has_visible_content() {
            let is_amber = cfg.theme == ThemePreset::Ibm3279Amber;
            let bright_color = if is_amber {
                (255, 176, 0)
            } else {
                (51, 255, 51)
            };
            let decay_color = if is_amber {
                (130, 82, 0)
            } else {
                (20, 130, 20)
            };

            for seg in tek.decaying_segments.iter().chain(tek.segments.iter()) {
                let color = if seg.phosphor_age == 0 {
                    bright_color
                } else {
                    decay_color
                };
                let x0 = ((i32::from(seg.x0) * max_x) / i32::from(TEK_MAX_X - 1)).clamp(0, max_x);
                let y0 = max_y
                    - ((i32::from(seg.y0) * max_y) / i32::from(TEK_MAX_Y - 1)).clamp(0, max_y);
                let x1 = ((i32::from(seg.x1) * max_x) / i32::from(TEK_MAX_X - 1)).clamp(0, max_x);
                let y1 = max_y
                    - ((i32::from(seg.y1) * max_y) / i32::from(TEK_MAX_Y - 1)).clamp(0, max_y);
                if seg.point_only {
                    sub.plot(x0, y0, color);
                } else {
                    sub.draw_line(x0, y0, x1, y1, seg.style, color);
                }
            }
        }

        // 2. Render modern retained scene-graph (images, plots, vectors, and z-sorted 2D sprites)
        if scene.has_visible_content() {
            if let Some(img) = &scene.image_view {
                render_image_to_subcells(&mut sub, img);
            }
            for plot in &scene.plots {
                render_plot_to_subcells(&mut sub, plot);
            }
            for prim in &scene.primitives {
                render_primitive_to_subcells(&mut sub, prim);
            }
            let mut sorted_sprites: Vec<_> = scene.sprites.iter().filter(|s| s.visible).collect();
            sorted_sprites.sort_by_key(|s| s.z_index);
            for sp in sorted_sprites {
                let sx0 = sp.x.round() as i32;
                let sy0 = sp.y.round() as i32;
                let frame = sp.frames.get(sp.current_frame);
                for dy in 0..i32::from(sp.height) {
                    for dx in 0..i32::from(sp.width) {
                        let opaque = frame
                            .and_then(|f| {
                                f.get((dy as usize) * usize::from(sp.width) + (dx as usize))
                            })
                            .copied()
                            .unwrap_or(1)
                            != 0;
                        if opaque {
                            sub.plot(sx0 + dx, sy0 + dy, sp.rgb);
                        }
                    }
                }
            }
        }

        sub.flush_to_ratatui(canvas, mode, buf);
    }

    // 3. Overlay Tektronix Alpha labels and Scene TextLabels at cell coordinates
    for lbl in &tek.labels {
        let col = ((u32::from(lbl.x) * u32::from(canvas.width.saturating_sub(1)))
            / u32::from(TEK_MAX_X - 1)) as u16;
        let row = canvas.height.saturating_sub(1)
            - (((u32::from(lbl.y) * u32::from(canvas.height.saturating_sub(1)))
                / u32::from(TEK_MAX_Y - 1)) as u16);
        draw_canvas_label(buf, canvas, col, row, &lbl.text, (51, 255, 51));
    }
    if scene.has_visible_content() {
        let (sx, sy) = mode.subcell_grid();
        for prim in &scene.primitives {
            if let VectorPrimitive::TextLabel { x, y, text, rgb } = prim {
                let col = (x.max(0.0) as u16) / sx.max(1);
                let row = (y.max(0.0) as u16) / sy.max(1);
                draw_canvas_label(buf, canvas, col, row, text, *rgb);
            }
        }
    }
}

fn draw_canvas_label(
    buf: &mut Buffer,
    canvas: Rect,
    col: u16,
    row: u16,
    text: &str,
    rgb: (u8, u8, u8),
) {
    if row >= canvas.height {
        return;
    }
    for (i, ch) in text.chars().enumerate() {
        let dst_c = col.saturating_add(i as u16);
        if dst_c >= canvas.width {
            break;
        }
        if let Some(cell) = buf.cell_mut((canvas.x + dst_c, canvas.y + row))
            && (cell.symbol() == " " || cell.symbol().is_empty())
        {
            let mut sym = [0u8; 4];
            cell.set_symbol(ch.encode_utf8(&mut sym));
            cell.set_style(
                Style::default()
                    .fg(Color::Rgb(rgb.0, rgb.1, rgb.2))
                    .add_modifier(Modifier::BOLD),
            );
        }
    }
}

fn render_primitive_to_subcells(sub: &mut SubcellBuffer, prim: &VectorPrimitive) {
    match prim {
        VectorPrimitive::Line {
            x0,
            y0,
            x1,
            y1,
            rgb,
        } => {
            sub.draw_line(
                x0.round() as i32,
                y0.round() as i32,
                x1.round() as i32,
                y1.round() as i32,
                TekLineStyle::Solid,
                *rgb,
            );
        }
        VectorPrimitive::Polyline {
            points,
            rgb,
            closed,
        } => {
            for pair in points.windows(2) {
                sub.draw_line(
                    pair[0].0.round() as i32,
                    pair[0].1.round() as i32,
                    pair[1].0.round() as i32,
                    pair[1].1.round() as i32,
                    TekLineStyle::Solid,
                    *rgb,
                );
            }
            if *closed
                && let (Some(first), Some(last)) = (points.first(), points.last())
            {
                sub.draw_line(
                    last.0.round() as i32,
                    last.1.round() as i32,
                    first.0.round() as i32,
                    first.1.round() as i32,
                    TekLineStyle::Solid,
                    *rgb,
                );
            }
        }
        VectorPrimitive::CubicBezier {
            p0,
            p1,
            p2,
            p3,
            rgb,
        } => {
            let steps = 24;
            let mut prev = (p0.0.round() as i32, p0.1.round() as i32);
            for s in 1..=steps {
                let t = (s as f32) / (steps as f32);
                let u = 1.0 - t;
                let bx = u * u * u * p0.0
                    + 3.0 * u * u * t * p1.0
                    + 3.0 * u * t * t * p2.0
                    + t * t * t * p3.0;
                let by = u * u * u * p0.1
                    + 3.0 * u * u * t * p1.1
                    + 3.0 * u * t * t * p2.1
                    + t * t * t * p3.1;
                let cur = (bx.round() as i32, by.round() as i32);
                sub.draw_line(prev.0, prev.1, cur.0, cur.1, TekLineStyle::Solid, *rgb);
                prev = cur;
            }
        }
        VectorPrimitive::Circle {
            cx,
            cy,
            radius,
            rgb,
            filled,
        } => {
            sub.draw_circle(
                cx.round() as i32,
                cy.round() as i32,
                radius.round() as i32,
                *filled,
                *rgb,
            );
        }
        VectorPrimitive::RectShape {
            x,
            y,
            width,
            height,
            rgb,
            filled,
        } => {
            sub.draw_rect(
                x.round() as i32,
                y.round() as i32,
                width.round() as i32,
                height.round() as i32,
                *filled,
                *rgb,
            );
        }
        VectorPrimitive::Polygon { vertices, rgb } => {
            for pair in vertices.windows(2) {
                sub.draw_line(
                    pair[0].0.round() as i32,
                    pair[0].1.round() as i32,
                    pair[1].0.round() as i32,
                    pair[1].1.round() as i32,
                    TekLineStyle::Solid,
                    *rgb,
                );
            }
            if let (Some(first), Some(last)) = (vertices.first(), vertices.last()) {
                sub.draw_line(
                    last.0.round() as i32,
                    last.1.round() as i32,
                    first.0.round() as i32,
                    first.1.round() as i32,
                    TekLineStyle::Solid,
                    *rgb,
                );
            }
        }
        VectorPrimitive::TextLabel { .. } => {}
    }
}

fn render_plot_to_subcells(sub: &mut SubcellBuffer, plot: &DeclarativePlot) {
    let w = (sub.sub_w.saturating_sub(4)).max(2) as f32;
    let h = (sub.sub_h.saturating_sub(4)).max(2) as f32;
    match &plot.kind {
        PlotKind::Line(vals) if !vals.is_empty() => {
            let min_v = vals.iter().copied().fold(f32::INFINITY, f32::min);
            let max_v = vals.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let span = (max_v - min_v).abs().max(1.0);
            let n = (vals.len().saturating_sub(1)).max(1) as f32;
            let mut prev = None;
            for (i, &v) in vals.iter().enumerate() {
                let x = 2 + ((i as f32 / n) * w).round() as i32;
                let y = 2 + ((1.0 - (v - min_v) / span) * h).round() as i32;
                if let Some((px, py)) = prev {
                    sub.draw_line(px, py, x, y, TekLineStyle::Solid, plot.rgb);
                } else {
                    sub.plot(x, y, plot.rgb);
                }
                prev = Some((x, y));
            }
        }
        PlotKind::Scatter(pts) => {
            for &(x, y) in pts {
                sub.plot(2 + (x.round() as i32), 2 + (y.round() as i32), plot.rgb);
            }
        }
        PlotKind::Bar(vals) if !vals.is_empty() => {
            let max_v = vals.iter().copied().fold(1.0f32, f32::max);
            let bar_step = (w / (vals.len() as f32)).max(2.0);
            for (i, &v) in vals.iter().enumerate() {
                let bx = 2 + ((i as f32) * bar_step).round() as i32;
                let bh = ((v.max(0.0) / max_v) * h).round().max(1.0) as i32;
                let by = 2 + (h as i32) - bh;
                sub.draw_rect(bx, by, (bar_step as i32 - 1).max(1), bh, true, plot.rgb);
            }
        }
        PlotKind::Candlestick(bars) if !bars.is_empty() => {
            let min_v = bars.iter().map(|b| b.low).fold(f32::INFINITY, f32::min);
            let max_v = bars.iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max);
            let span = (max_v - min_v).abs().max(1.0);
            let step = (w / (bars.len() as f32)).max(3.0);
            for (i, bar) in bars.iter().enumerate() {
                let cx = 2 + ((i as f32 + 0.5) * step).round() as i32;
                let y_hi = 2 + ((1.0 - (bar.high - min_v) / span) * h).round() as i32;
                let y_lo = 2 + ((1.0 - (bar.low - min_v) / span) * h).round() as i32;
                let y_op = 2 + ((1.0 - (bar.open - min_v) / span) * h).round() as i32;
                let y_cl = 2 + ((1.0 - (bar.close - min_v) / span) * h).round() as i32;
                let color = if bar.close >= bar.open {
                    (85, 255, 85)
                } else {
                    (255, 85, 85)
                };
                sub.draw_line(cx, y_hi, cx, y_lo, TekLineStyle::Solid, color);
                let top = y_op.min(y_cl);
                let bot = y_op.max(y_cl).max(top + 1);
                sub.draw_rect(cx - 1, top, 3, bot - top, true, color);
            }
        }
        PlotKind::Heatmap { rows, cols, values } if *rows > 0 && *cols > 0 => {
            let max_v = values.iter().copied().fold(1.0f32, f32::max);
            for r in 0..*rows {
                for c in 0..*cols {
                    let v = values.get(r * cols + c).copied().unwrap_or(0.0);
                    let norm = (v / max_v).clamp(0.0, 1.0);
                    let color = (
                        (norm * 255.0) as u8,
                        ((1.0 - (norm - 0.5).abs() * 2.0) * 200.0) as u8,
                        ((1.0 - norm) * 255.0) as u8,
                    );
                    sub.plot(2 + c as i32, 2 + r as i32, color);
                }
            }
        }
        _ => {}
    }
}

fn render_image_to_subcells(sub: &mut SubcellBuffer, img: &InteractiveImageView) {
    let zoom = img.zoom.max(0.25);
    let dst_w = ((f32::from(img.width) * zoom).round() as i32).max(1);
    let dst_h = ((f32::from(img.height) * zoom).round() as i32).max(1);
    for dy in 0..dst_h {
        let sx = (((dy * i32::from(img.height)) / dst_h).clamp(0, i32::from(img.height) - 1)) as u16;
        for dx in 0..dst_w {
            let sy_col =
                (((dx * i32::from(img.width)) / dst_w).clamp(0, i32::from(img.width) - 1)) as u16;
            if let Some([r, g, b, a]) = img.inspect_pixel(sy_col, sx)
                && a > 0
            {
                sub.plot(img.pan_x + dx, img.pan_y + dy, (r, g, b));
            }
        }
    }
}
