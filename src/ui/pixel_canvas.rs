//! High-Resolution Anti-Aliased RGBA Pixel Canvas & Adaptive SSH/GUI Graphics Engine (`CANV-02`, `SSH-GFX-01`).
//!
//! Provides a true floating-point anti-aliased RGBA pixel rasterizer ([`PixelCanvasBuffer`]) that
//! renders [`PaneCanvasScene`](crate::pane::PaneCanvasScene) and [`TektronixState`](crate::vt::tektronix::TektronixState)
//! across both **Native GUI mode** (`GuiWorkspaceRenderer`) and **Pure SSH / TUI sessions**:
//! - **Xiaolin Wu Anti-Aliased Vector Lines, Beziers, SDF Circles & Polygons**
//! - **High-DPI Multi-Series Data Plots (axes, gridlines, AA lines + area fill, scatter, bars, candlesticks, heatmaps)**
//! - **Bilinear RGBA Image Resampling & Smooth Pixel-Space Focal Zoom Lens**
//! - **24-bit TrueColor `1×2` Half-Block (`▀`/`▄`) TUI Renderer** (universal over any SSH / `tmux` session)
//! - **In-Band Pixel Protocol Encoders (`Kitty APC G`, `DEC Sixel DCS q`, `iTerm2 OSC 1337`, and `tmux` DCS passthrough)**

use std::fmt::Write as _;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::config::BiltermConfig;
use crate::pane::{
    CompositePane, DeclarativePlot, InteractiveImageView, PlotKind, VectorPrimitive,
};
use crate::ui::cursor::SpotlightMode;
use crate::ui::theme::ThemePreset;
use crate::vt::{TEK_MAX_X, TEK_MAX_Y};

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes raw bytes as standard RFC 4648 Base64.
pub fn base64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(BASE64_ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(BASE64_ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(BASE64_ALPHABET[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Wraps an escape sequence in a `tmux` DCS passthrough envelope (`\x1bPtmux;\x1b...\x1b\\`)
/// so Kitty/Sixel/iTerm2 pixel streams traverse an outer `$TMUX` session over SSH (`SSH-GFX-01`).
pub fn wrap_tmux_dcs_passthrough(seq: &str) -> String {
    let mut out = String::with_capacity(seq.len() + 16);
    out.push_str("\x1bPtmux;");
    for ch in seq.chars() {
        if ch == '\x1b' {
            out.push_str("\x1b\x1b");
        } else {
            out.push(ch);
        }
    }
    out.push_str("\x1b\\");
    out
}

/// High-resolution anti-aliased RGBA pixel framebuffer (`CANV-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PixelCanvasBuffer {
    pub width: u32,
    pub height: u32,
    /// Row-major `[R, G, B, A]` pixels (`width * height * 4` bytes).
    pub rgba: Vec<u8>,
}

impl PixelCanvasBuffer {
    pub fn new(width: u32, height: u32, bg: [u8; 4]) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let total = (w as usize) * (h as usize);
        let mut rgba = Vec::with_capacity(total * 4);
        for _ in 0..total {
            rgba.extend_from_slice(&bg);
        }
        Self {
            width: w,
            height: h,
            rgba,
        }
    }

    #[inline]
    pub fn pixel_at(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let idx = ((y * self.width + x) as usize) * 4;
        [
            self.rgba[idx],
            self.rgba[idx + 1],
            self.rgba[idx + 2],
            self.rgba[idx + 3],
        ]
    }

    /// Alpha-blends `(r, g, b)` with coverage `alpha` (`0.0..=1.0`) onto `(x, y)`.
    #[inline]
    pub fn blend_pixel(&mut self, x: i32, y: i32, rgb: (u8, u8, u8), alpha: f32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.01 {
            return;
        }
        let idx = (((y as u32) * self.width + (x as u32)) as usize) * 4;
        let inv = 1.0 - a;
        let dst_r = f32::from(self.rgba[idx]);
        let dst_g = f32::from(self.rgba[idx + 1]);
        let dst_b = f32::from(self.rgba[idx + 2]);
        let dst_a = f32::from(self.rgba[idx + 3]) / 255.0;
        self.rgba[idx] = (f32::from(rgb.0) * a + dst_r * inv).round().clamp(0.0, 255.0) as u8;
        self.rgba[idx + 1] =
            (f32::from(rgb.1) * a + dst_g * inv).round().clamp(0.0, 255.0) as u8;
        self.rgba[idx + 2] =
            (f32::from(rgb.2) * a + dst_b * inv).round().clamp(0.0, 255.0) as u8;
        let out_a = (a + dst_a * inv).clamp(0.0, 1.0);
        self.rgba[idx + 3] = (out_a * 255.0).round() as u8;
    }

    /// Draws a Xiaolin Wu anti-aliased line segment between `(x0, y0)` and `(x1, y1)` (`CANV-02`).
    pub fn draw_line_aa(
        &mut self,
        mut x0: f32,
        mut y0: f32,
        mut x1: f32,
        mut y1: f32,
        rgb: (u8, u8, u8),
        alpha: f32,
    ) {
        let steep = (y1 - y0).abs() > (x1 - x0).abs();
        if steep {
            std::mem::swap(&mut x0, &mut y0);
            std::mem::swap(&mut x1, &mut y1);
        }
        if x0 > x1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }
        let dx = x1 - x0;
        let dy = y1 - y0;
        let gradient = if dx.abs() < 1e-5 { 1.0 } else { dy / dx };

        let x_start = x0.round() as i32;
        let x_end = x1.round() as i32;
        let mut intery = y0 + gradient * ((x_start as f32) - x0);

        for x in x_start..=x_end {
            let y_int = intery.floor() as i32;
            let fpart = intery - (y_int as f32);
            let rfpart = 1.0 - fpart;
            if steep {
                self.blend_pixel(y_int, x, rgb, rfpart * alpha);
                self.blend_pixel(y_int + 1, x, rgb, fpart * alpha);
            } else {
                self.blend_pixel(x, y_int, rgb, rfpart * alpha);
                self.blend_pixel(x, y_int + 1, rgb, fpart * alpha);
            }
            intery += gradient;
        }
    }

    /// Draws an anti-aliased cubic Bezier curve (`CANV-02`).
    pub fn draw_bezier_aa(
        &mut self,
        p0: (f32, f32),
        p1: (f32, f32),
        p2: (f32, f32),
        p3: (f32, f32),
        rgb: (u8, u8, u8),
    ) {
        let steps = 48;
        let mut prev = p0;
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
            self.draw_line_aa(prev.0, prev.1, bx, by, rgb, 1.0);
            prev = (bx, by);
        }
    }

    /// Draws a signed-distance-field anti-aliased circle or filled disc (`CANV-02`).
    pub fn draw_circle_aa(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        filled: bool,
        rgb: (u8, u8, u8),
    ) {
        let r = radius.max(1.0);
        let min_x = (cx - r - 2.0).floor().max(0.0) as i32;
        let max_x = (cx + r + 2.0).ceil().min(self.width as f32 - 1.0) as i32;
        let min_y = (cy - r - 2.0).floor().max(0.0) as i32;
        let max_y = (cy + r + 2.0).ceil().min(self.height as f32 - 1.0) as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dist = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                let cov = if filled {
                    (r + 0.5 - dist).clamp(0.0, 1.0)
                } else {
                    (1.2 - (dist - r).abs()).clamp(0.0, 1.0)
                };
                if cov > 0.0 {
                    self.blend_pixel(x, y, rgb, cov);
                }
            }
        }
    }

    /// Draws a filled or outlined rectangle (`CANV-02`).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_rect_aa(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        filled: bool,
        rgb: (u8, u8, u8),
        alpha: f32,
    ) {
        if filled {
            let x0 = x.round().max(0.0) as i32;
            let y0 = y.round().max(0.0) as i32;
            let x1 = (x + w).round().min(self.width as f32) as i32;
            let y1 = (y + h).round().min(self.height as f32) as i32;
            for py in y0..y1 {
                for px in x0..x1 {
                    self.blend_pixel(px, py, rgb, alpha);
                }
            }
        } else {
            self.draw_line_aa(x, y, x + w, y, rgb, alpha);
            self.draw_line_aa(x + w, y, x + w, y + h, rgb, alpha);
            self.draw_line_aa(x + w, y + h, x, y + h, rgb, alpha);
            self.draw_line_aa(x, y + h, x, y, rgb, alpha);
        }
    }

    /// Renders a declarative data plot at high pixel resolution with axes, gridlines, and AA geometry (`CANV-02`).
    pub fn render_plot_hires(&mut self, plot: &DeclarativePlot, scale_x: f32, scale_y: f32) {
        let pad_x = (4.0 * scale_x).max(4.0);
        let pad_y = (3.0 * scale_y).max(3.0);
        let w = ((self.width as f32) - pad_x * 2.0).max(4.0);
        let h = ((self.height as f32) - pad_y * 2.0).max(4.0);

        // Subtle coordinate gridlines & axes
        for g in 0..=4 {
            let gy = pad_y + (g as f32 / 4.0) * h;
            self.draw_line_aa(pad_x, gy, pad_x + w, gy, (40, 75, 95), 0.35);
        }
        self.draw_line_aa(pad_x, pad_y, pad_x, pad_y + h, (90, 140, 170), 0.75);
        self.draw_line_aa(pad_x, pad_y + h, pad_x + w, pad_y + h, (90, 140, 170), 0.75);

        match &plot.kind {
            PlotKind::Line(vals) if !vals.is_empty() => {
                let min_v = vals.iter().copied().fold(f32::INFINITY, f32::min);
                let max_v = vals.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let span = (max_v - min_v).abs().max(1.0);
                let n = (vals.len().saturating_sub(1)).max(1) as f32;
                let mut prev: Option<(f32, f32)> = None;
                let base_y = pad_y + h;
                for (i, &v) in vals.iter().enumerate() {
                    let x = pad_x + (i as f32 / n) * w;
                    let y = pad_y + (1.0 - (v - min_v) / span) * h;
                    if let Some((px, py)) = prev {
                        // Soft translucent area fill under the curve
                        let x_start = px.round() as i32;
                        let x_end = x.round() as i32;
                        let dx = (x - px).max(1.0);
                        for ix in x_start..=x_end {
                            let t = ((ix as f32 - px) / dx).clamp(0.0, 1.0);
                            let iy_top = (py + t * (y - py)).round() as i32;
                            for iy in iy_top..(base_y.round() as i32) {
                                self.blend_pixel(ix, iy, plot.rgb, 0.16);
                            }
                        }
                        self.draw_line_aa(px, py, x, y, plot.rgb, 1.0);
                    }
                    self.draw_circle_aa(x, y, 1.5, true, plot.rgb);
                    prev = Some((x, y));
                }
            }
            PlotKind::Scatter(pts) => {
                for &(x, y) in pts {
                    self.draw_circle_aa(
                        pad_x + x * scale_x,
                        pad_y + y * scale_y,
                        2.2,
                        true,
                        plot.rgb,
                    );
                }
            }
            PlotKind::Bar(vals) if !vals.is_empty() => {
                let max_v = vals.iter().copied().fold(1.0f32, f32::max);
                let step = (w / (vals.len() as f32)).max(2.0);
                for (i, &v) in vals.iter().enumerate() {
                    let bx = pad_x + (i as f32) * step;
                    let bh = ((v.max(0.0) / max_v) * h).max(1.0);
                    let by = pad_y + h - bh;
                    self.draw_rect_aa(bx, by, (step - 1.5).max(1.0), bh, true, plot.rgb, 0.88);
                }
            }
            PlotKind::Candlestick(bars) if !bars.is_empty() => {
                let min_v = bars.iter().map(|b| b.low).fold(f32::INFINITY, f32::min);
                let max_v = bars.iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max);
                let span = (max_v - min_v).abs().max(1.0);
                let step = (w / (bars.len() as f32)).max(3.0);
                for (i, bar) in bars.iter().enumerate() {
                    let cx = pad_x + (i as f32 + 0.5) * step;
                    let y_hi = pad_y + (1.0 - (bar.high - min_v) / span) * h;
                    let y_lo = pad_y + (1.0 - (bar.low - min_v) / span) * h;
                    let y_op = pad_y + (1.0 - (bar.open - min_v) / span) * h;
                    let y_cl = pad_y + (1.0 - (bar.close - min_v) / span) * h;
                    let color = if bar.close >= bar.open {
                        (85, 255, 110)
                    } else {
                        (255, 85, 95)
                    };
                    self.draw_line_aa(cx, y_hi, cx, y_lo, color, 1.0);
                    let top = y_op.min(y_cl);
                    let bot = y_op.max(y_cl).max(top + 1.5);
                    let bw = (step * 0.6).clamp(2.0, 12.0);
                    self.draw_rect_aa(cx - bw * 0.5, top, bw, bot - top, true, color, 0.95);
                }
            }
            PlotKind::Heatmap { rows, cols, values } if *rows > 0 && *cols > 0 => {
                let max_v = values.iter().copied().fold(1.0f32, f32::max);
                let cell_w = (w / (*cols as f32)).max(1.0);
                let cell_h = (h / (*rows as f32)).max(1.0);
                for r in 0..*rows {
                    for c in 0..*cols {
                        let v = values.get(r * cols + c).copied().unwrap_or(0.0);
                        let norm = (v / max_v).clamp(0.0, 1.0);
                        let rgb = (
                            (norm * 255.0) as u8,
                            ((1.0 - (norm - 0.5).abs() * 2.0) * 210.0) as u8,
                            ((1.0 - norm) * 255.0) as u8,
                        );
                        self.draw_rect_aa(
                            pad_x + (c as f32) * cell_w,
                            pad_y + (r as f32) * cell_h,
                            cell_w.ceil(),
                            cell_h.ceil(),
                            true,
                            rgb,
                            0.9,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    /// Blits an [`InteractiveImageView`] with bilinear RGBA interpolation (`CANV-02`).
    pub fn render_image_bilinear(
        &mut self,
        img: &InteractiveImageView,
        scale_x: f32,
        scale_y: f32,
    ) {
        if img.width == 0 || img.height == 0 {
            return;
        }
        let zoom = img.zoom.max(0.25);
        let dst_w = ((f32::from(img.width) * zoom * scale_x).round() as i32).max(1);
        let dst_h = ((f32::from(img.height) * zoom * scale_y).round() as i32).max(1);
        let ox = (img.pan_x as f32 * scale_x).round() as i32;
        let oy = (img.pan_y as f32 * scale_y).round() as i32;

        for dy in 0..dst_h {
            let fy = ((dy as f32) / (dst_h as f32)) * f32::from(img.height.saturating_sub(1));
            let y0 = (fy.floor() as u16).min(img.height - 1);
            let y1 = (y0 + 1).min(img.height - 1);
            let wy = fy - f32::from(y0);
            for dx in 0..dst_w {
                let fx = ((dx as f32) / (dst_w as f32)) * f32::from(img.width.saturating_sub(1));
                let x0 = (fx.floor() as u16).min(img.width - 1);
                let x1 = (x0 + 1).min(img.width - 1);
                let wx = fx - f32::from(x0);

                let p00 = img.inspect_pixel(x0, y0).unwrap_or([0; 4]);
                let p10 = img.inspect_pixel(x1, y0).unwrap_or([0; 4]);
                let p01 = img.inspect_pixel(x0, y1).unwrap_or([0; 4]);
                let p11 = img.inspect_pixel(x1, y1).unwrap_or([0; 4]);

                let bilerp = |ch: usize| -> u8 {
                    let top = f32::from(p00[ch]) * (1.0 - wx) + f32::from(p10[ch]) * wx;
                    let bot = f32::from(p01[ch]) * (1.0 - wx) + f32::from(p11[ch]) * wx;
                    (top * (1.0 - wy) + bot * wy).round().clamp(0.0, 255.0) as u8
                };
                let a = bilerp(3);
                if a > 0 {
                    self.blend_pixel(
                        ox + dx,
                        oy + dy,
                        (bilerp(0), bilerp(1), bilerp(2)),
                        f32::from(a) / 255.0,
                    );
                }
            }
        }
    }

    /// Applies a smooth pixel-space radial focal zoom lens around `cursor_rc` when `focal_zoom_mode != Off` (`CANV-02`).
    pub fn apply_focal_lens(
        &self,
        cfg: &BiltermConfig,
        cursor_rc: (u16, u16),
        cell_cols: u16,
        cell_rows: u16,
    ) -> Self {
        if cfg.focal_zoom_mode == SpotlightMode::Off || cfg.focal_zoom_pct <= 100 {
            return self.clone();
        }
        let cols_f = f32::from(cell_cols.max(1));
        let rows_f = f32::from(cell_rows.max(1));
        let px_per_col = (self.width as f32) / cols_f;
        let px_per_row = (self.height as f32) / rows_f;

        let cc = if cursor_rc.1 < cell_cols {
            cursor_rc.1
        } else {
            cell_cols / 2
        };
        let cr = if cursor_rc.0 < cell_rows {
            cursor_rc.0
        } else {
            cell_rows / 2
        };
        let focus_x = (f32::from(cc) + 0.5) * px_per_col;
        let focus_y = (f32::from(cr) + 0.5) * px_per_row;
        let rx_px = f32::from(cfg.focal_zoom_rx.max(4)) * px_per_col;
        let ry_px = match cfg.focal_zoom_mode {
            SpotlightMode::Circle => (f32::from(cfg.focal_zoom_rx.max(4)) * 0.5).max(2.0) * px_per_row,
            SpotlightMode::Ellipse => f32::from(cfg.focal_zoom_ry.max(2)) * px_per_row,
            SpotlightMode::Off => return self.clone(),
        };
        let peak_zoom = f32::from(cfg.focal_zoom_pct.clamp(105, 400)) / 100.0;
        let feather = f32::from(cfg.focal_zoom_feather_pct.clamp(5, 100)) / 100.0;
        let inner = (1.0 - feather).max(0.0);
        let outer = 1.0 + feather;

        let mut out = self.clone();
        for y in 0..self.height {
            for x in 0..self.width {
                let dx = (x as f32) - focus_x;
                let dy = (y as f32) - focus_y;
                let dist = ((dx / rx_px.max(1.0)).powi(2) + (dy / ry_px.max(1.0)).powi(2)).sqrt();
                if dist < outer {
                    let t = ((dist - inner) / (outer - inner).max(0.001)).clamp(0.0, 1.0);
                    let falloff = 1.0 - t * t * (3.0 - 2.0 * t);
                    let mag = 1.0 + (peak_zoom - 1.0) * falloff;
                    let src_x = (focus_x + dx / mag).round().clamp(0.0, (self.width - 1) as f32) as u32;
                    let src_y = (focus_y + dy / mag).round().clamp(0.0, (self.height - 1) as f32) as u32;
                    let px = self.pixel_at(src_x, src_y);
                    let idx = ((y * self.width + x) as usize) * 4;
                    out.rgba[idx..idx + 4].copy_from_slice(&px);
                }
            }
        }
        out
    }

    /// Renders `pane`'s active `TektronixState` and `PaneCanvasScene` into a `PixelCanvasBuffer` (`CANV-02`).
    pub fn render_pane_scene(
        pane: &CompositePane,
        cfg: &BiltermConfig,
        cell_cols: u16,
        cell_rows: u16,
        px_per_col: u32,
        px_per_row: u32,
    ) -> Self {
        let w = (u32::from(cell_cols.max(1))) * px_per_col.max(1);
        let h = (u32::from(cell_rows.max(1))) * px_per_row.max(1);
        let mut buf = Self::new(w, h, [0, 0, 0, 0]);
        let sx = (px_per_col as f32) / 2.0;
        let sy = (px_per_row as f32) / 4.0;

        let tek = pane.vt().tektronix();
        if tek.has_visible_content() {
            let (bright_rgb, decay_rgb) = match cfg.theme {
                ThemePreset::Ibm3279Amber => ((255, 176, 0), (130, 82, 0)),
                _ => ((0, 255, 102), (0, 115, 45)),
            };
            let map_x = |tx: u16| -> f32 {
                (f32::from(tx.min(TEK_MAX_X)) / f32::from(TEK_MAX_X)) * ((w - 1) as f32)
            };
            let map_y = |ty: u16| -> f32 {
                (1.0 - (f32::from(ty.min(TEK_MAX_Y)) / f32::from(TEK_MAX_Y))) * ((h - 1) as f32)
            };
            for seg in &tek.decaying_segments {
                buf.draw_line_aa(map_x(seg.x0), map_y(seg.y0), map_x(seg.x1), map_y(seg.y1), decay_rgb, 0.55);
            }
            for seg in &tek.segments {
                buf.draw_line_aa(map_x(seg.x0), map_y(seg.y0), map_x(seg.x1), map_y(seg.y1), bright_rgb, 1.0);
            }
        }

        let scene = pane.canvas_scene();
        if scene.has_visible_content() {
            if let Some(img) = &scene.image_view {
                buf.render_image_bilinear(img, sx, sy);
            }
            for plot in &scene.plots {
                buf.render_plot_hires(plot, sx, sy);
            }
            for prim in &scene.primitives {
                match prim {
                    VectorPrimitive::Line { x0, y0, x1, y1, rgb } => {
                        buf.draw_line_aa(x0 * sx, y0 * sy, x1 * sx, y1 * sy, *rgb, 1.0);
                    }
                    VectorPrimitive::Polyline { points, rgb, closed } => {
                        for pair in points.windows(2) {
                            buf.draw_line_aa(
                                pair[0].0 * sx,
                                pair[0].1 * sy,
                                pair[1].0 * sx,
                                pair[1].1 * sy,
                                *rgb,
                                1.0,
                            );
                        }
                        if *closed
                            && let (Some(f), Some(l)) = (points.first(), points.last())
                        {
                            buf.draw_line_aa(l.0 * sx, l.1 * sy, f.0 * sx, f.1 * sy, *rgb, 1.0);
                        }
                    }
                    VectorPrimitive::CubicBezier { p0, p1, p2, p3, rgb } => {
                        buf.draw_bezier_aa(
                            (p0.0 * sx, p0.1 * sy),
                            (p1.0 * sx, p1.1 * sy),
                            (p2.0 * sx, p2.1 * sy),
                            (p3.0 * sx, p3.1 * sy),
                            *rgb,
                        );
                    }
                    VectorPrimitive::Circle { cx, cy, radius, rgb, filled } => {
                        buf.draw_circle_aa(cx * sx, cy * sy, radius * sx.max(1.0), *filled, *rgb);
                    }
                    VectorPrimitive::RectShape { x, y, width, height, rgb, filled } => {
                        buf.draw_rect_aa(x * sx, y * sy, width * sx, height * sy, *filled, *rgb, 0.95);
                    }
                    VectorPrimitive::Polygon { vertices, rgb } => {
                        for pair in vertices.windows(2) {
                            buf.draw_line_aa(
                                pair[0].0 * sx,
                                pair[0].1 * sy,
                                pair[1].0 * sx,
                                pair[1].1 * sy,
                                *rgb,
                                1.0,
                            );
                        }
                        if let (Some(f), Some(l)) = (vertices.first(), vertices.last()) {
                            buf.draw_line_aa(l.0 * sx, l.1 * sy, f.0 * sx, f.1 * sy, *rgb, 1.0);
                        }
                    }
                    VectorPrimitive::TextLabel { .. } => {}
                }
            }
            let mut sprites: Vec<_> = scene.sprites.iter().filter(|s| s.visible).collect();
            sprites.sort_by_key(|s| s.z_index);
            for sp in sprites {
                buf.draw_rect_aa(
                    sp.x * sx,
                    sp.y * sy,
                    f32::from(sp.width) * sx,
                    f32::from(sp.height) * sy,
                    true,
                    sp.rgb,
                    1.0,
                );
            }
        }

        buf.apply_focal_lens(cfg, pane.vt().cursor_position(), cell_cols, cell_rows)
    }

    /// Flushes this pixel buffer into `Ratatui` `buf` as 24-bit TrueColor `1×2` Half-Blocks (`▀` / `▄`) (`SSH-GFX-01`).
    ///
    /// Preserves non-space VT100 terminal text characters so shell output is never overwritten.
    pub fn flush_halfblocks_to_ratatui(&self, canvas: Rect, buf: &mut Buffer) {
        for r in 0..canvas.height {
            let y_top = ((u32::from(r) * 2) * self.height) / (u32::from(canvas.height.max(1)) * 2);
            let y_bot =
                ((u32::from(r) * 2 + 1) * self.height) / (u32::from(canvas.height.max(1)) * 2);
            for c in 0..canvas.width {
                let x = (u32::from(c) * self.width) / u32::from(canvas.width.max(1));
                let top = self.pixel_at(x, y_top);
                let bot = self.pixel_at(x, y_bot);
                if top[3] <= 16 && bot[3] <= 16 {
                    continue;
                }
                if let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + r))
                    && cell.symbol() == " "
                {
                    if top[3] > 16 && bot[3] > 16 {
                        cell.set_symbol("▀");
                        cell.set_fg(Color::Rgb(top[0], top[1], top[2]));
                        cell.set_bg(Color::Rgb(bot[0], bot[1], bot[2]));
                    } else if top[3] > 16 {
                        cell.set_symbol("▀");
                        cell.set_fg(Color::Rgb(top[0], top[1], top[2]));
                    } else {
                        cell.set_symbol("▄");
                        cell.set_fg(Color::Rgb(bot[0], bot[1], bot[2]));
                    }
                }
            }
        }
    }

    /// Exports this RGBA framebuffer as a Netpbm P6 `.ppm` image (`CANV-02`).
    pub fn export_ppm(&self) -> Vec<u8> {
        let header = format!("P6\n{} {}\n255\n", self.width, self.height);
        let mut out = Vec::with_capacity(header.len() + (self.width * self.height * 3) as usize);
        out.extend_from_slice(header.as_bytes());
        for px in self.rgba.as_chunks::<4>().0 {
            let a = u16::from(px[3]);
            let bg_r = 10u16;
            let bg_g = 16u16;
            let bg_b = 28u16;
            out.push((((u16::from(px[0]) * a) + bg_r * (255 - a)) / 255) as u8);
            out.push((((u16::from(px[1]) * a) + bg_g * (255 - a)) / 255) as u8);
            out.push((((u16::from(px[2]) * a) + bg_b * (255 - a)) / 255) as u8);
        }
        out
    }

    /// Exports this pixel buffer as a standalone vector/raster SVG document (`CANV-02`).
    pub fn export_svg(&self, title: &str) -> String {
        let mut svg = String::with_capacity(4096);
        let _ = writeln!(
            svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
            self.width, self.height, self.width, self.height
        );
        let _ = writeln!(svg, "  <title>{title}</title>");
        let _ = writeln!(
            svg,
            "  <rect width=\"100%\" height=\"100%\" fill=\"#0a101c\"/>"
        );
        for y in 0..self.height {
            let mut x = 0;
            while x < self.width {
                let px = self.pixel_at(x, y);
                if px[3] <= 16 {
                    x += 1;
                    continue;
                }
                let mut run = 1;
                while x + run < self.width && self.pixel_at(x + run, y) == px {
                    run += 1;
                }
                let _ = writeln!(
                    svg,
                    r#"  <rect x="{x}" y="{y}" width="{run}" height="1" fill="rgb({},{},{})" fill-opacity="{:.2}"/>"#,
                    px[0],
                    px[1],
                    px[2],
                    f32::from(px[3]) / 255.0
                );
                x += run;
            }
        }
        svg.push_str("</svg>\n");
        svg
    }

    /// Encodes this RGBA buffer as a Kitty Graphics Protocol (`APC G`) sequence (`SSH-GFX-01`).
    pub fn encode_kitty_apc(&self, cols: u16, rows: u16, wrap_tmux: bool) -> String {
        let b64 = base64_encode(&self.rgba);
        let raw = format!(
            "\x1b_Ga=T,f=32,s={},v={},c={},r={},q=2;{}\x1b\\",
            self.width,
            self.height,
            cols.max(1),
            rows.max(1),
            b64
        );
        if wrap_tmux {
            wrap_tmux_dcs_passthrough(&raw)
        } else {
            raw
        }
    }

    /// Encodes this buffer as an iTerm2 inline image (`OSC 1337;File=inline=1:...`) (`SSH-GFX-01`).
    pub fn encode_iterm2_osc1337(&self, cols: u16, rows: u16, wrap_tmux: bool) -> String {
        let ppm = self.export_ppm();
        let b64 = base64_encode(&ppm);
        let raw = format!(
            "\x1b]1337;File=inline=1;size={};width={};height={}:{}\x07",
            ppm.len(),
            cols.max(1),
            rows.max(1),
            b64
        );
        if wrap_tmux {
            wrap_tmux_dcs_passthrough(&raw)
        } else {
            raw
        }
    }

    /// Encodes this RGBA buffer as a DEC Sixel (`DCS q`) stream (`SSH-GFX-01`).
    pub fn encode_sixel_dcs(&self, wrap_tmux: bool) -> String {
        let mut out = String::with_capacity((self.width * self.height) as usize + 128);
        let _ = write!(
            out,
            "\x1bP0;1;q\"1;1;{};{}#0;2;0;0;0#1;2;0;100;65#2;2;33;100;100#3;2;100;100;33#4;2;100;33;33",
            self.width, self.height
        );
        let bands = self.height.div_ceil(6);
        for band in 0..bands {
            for reg in 1u8..=4u8 {
                let _ = write!(out, "#{reg}");
                for x in 0..self.width {
                    let mut mask = 0u8;
                    for dy in 0..6 {
                        let y = band * 6 + dy;
                        let px = self.pixel_at(x, y);
                        if px[3] > 32 {
                            let bucket = if px[0] > 180 && px[1] > 140 {
                                3
                            } else if px[0] > 180 {
                                4
                            } else if px[2] > 160 {
                                2
                            } else {
                                1
                            };
                            if bucket == reg {
                                mask |= 1 << dy;
                            }
                        }
                    }
                    out.push((b'?' + mask) as char);
                }
                out.push('$');
            }
            out.push('-');
        }
        out.push_str("\x1b\\");
        if wrap_tmux {
            wrap_tmux_dcs_passthrough(&out)
        } else {
            out
        }
    }
}
