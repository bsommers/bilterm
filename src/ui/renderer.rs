//! Abstract UI & Window Layer Renderer Seam (`ARCH-03`).
//!
//! Decouples Bilterm's window manager (`WindowManager`, `SplitTree`, `ZStack`, `QuickTerminal`)
//! and virtual terminal core (`libghostty-vt` / `VtParser` -> [`GridSnapshot`]) from the concrete
//! GUI/TUI toolkit so the same state can drive:
//! - **Ratatui + Crossterm** (in-terminal TUI mode)
//! - **GPUI** (Zed's hardware-accelerated GPU window framework)
//! - **`winit` + `egui`** (immediate-mode native desktop GUI)

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::widgets::Widget;

use crate::pane::PaneId;
use crate::ui::WorkspaceWidget;
use crate::vt::TerminalGraphicPlacement;
use crate::vt::snapshot::GridSnapshot;
use crate::wm::{WindowManager, WorkspaceLayoutMode};

/// Point-in-time frame snapshot of a single [`CompositePane`](crate::pane::CompositePane) for GPU/GUI renderers (`GPUI` / `egui`).
#[derive(Clone, Debug)]
pub struct PaneFrameSnapshot {
    pub id: PaneId,
    pub title: String,
    pub outer_rect: Rect,
    pub canvas_rect: Rect,
    pub prompt_rect: Option<Rect>,
    pub status_bar_rect: Rect,
    pub active: bool,
    pub z_rank: Option<(usize, usize)>,
    pub grid: GridSnapshot,
    pub graphics: Vec<TerminalGraphicPlacement>,
}

/// Complete point-in-time workspace frame snapshot consumed by external UI layers (`GPUI`, `egui`, or headless tests).
#[derive(Clone, Debug)]
pub struct WorkspaceFrameSnapshot {
    pub viewport: Rect,
    pub layout_mode: WorkspaceLayoutMode,
    pub panes: Vec<PaneFrameSnapshot>,
    pub quick_terminal: Option<PaneFrameSnapshot>,
    pub drawer_rect: Option<Rect>,
    pub help_visible: bool,
}

impl WorkspaceFrameSnapshot {
    /// Captures a complete point-in-time [`WorkspaceFrameSnapshot`] from `wm` at `viewport`.
    pub fn capture(wm: &WindowManager, viewport: Rect) -> Self {
        let workspace_area = if wm.config().show_dos_fkey_bar && viewport.height > 2 {
            Rect {
                x: viewport.x,
                y: viewport.y,
                width: viewport.width,
                height: viewport.height - 1,
            }
        } else {
            viewport
        };

        let rects = wm.compute_workspace_rects(workspace_area);
        let top_id = wm.z_stack().top();
        let qt_visible = wm.is_quick_terminal_visible();

        let mut panes = Vec::with_capacity(rects.len());
        for (id, outer) in rects {
            let Some(pane) = wm.pane(id) else {
                continue;
            };
            let active = !qt_visible && top_id == Some(id);
            let canvas_rect = pane.canvas_rect(outer);
            let prompt_rect = pane.is_prompt_visible().then(|| pane.prompt_rect(outer));
            let status_bar_rect = pane.status_bar_rect(outer);
            panes.push(PaneFrameSnapshot {
                id,
                title: pane.title().to_string(),
                outer_rect: outer,
                canvas_rect,
                prompt_rect,
                status_bar_rect,
                active,
                z_rank: wm.z_stack().rank_of(id),
                grid: pane.vt().snapshot(),
                graphics: pane.vt().graphic_placements().to_vec(),
            });
        }

        let quick_terminal = if qt_visible {
            let qt_outer = wm.quick_terminal().overlay_rect(workspace_area);
            let qt_pane = &wm.quick_terminal().pane;
            Some(PaneFrameSnapshot {
                id: qt_pane.id(),
                title: qt_pane.title().to_string(),
                outer_rect: qt_outer,
                canvas_rect: qt_pane.canvas_rect(qt_outer),
                prompt_rect: qt_pane
                    .is_prompt_visible()
                    .then(|| qt_pane.prompt_rect(qt_outer)),
                status_bar_rect: qt_pane.status_bar_rect(qt_outer),
                active: true,
                z_rank: None,
                grid: qt_pane.vt().snapshot(),
                graphics: qt_pane.vt().graphic_placements().to_vec(),
            })
        } else {
            None
        };

        Self {
            viewport,
            layout_mode: wm.layout_mode(),
            panes,
            quick_terminal,
            drawer_rect: wm.drawer_rect(workspace_area),
            help_visible: wm.is_help_visible(),
        }
    }
}

/// Trait implemented by any UI/Window rendering backend (`Ratatui`, `GPUI`, `winit + egui`).
pub trait WorkspaceFrameRenderer {
    type Surface;

    /// Renders the current state of `wm` onto `surface` within `viewport`.
    fn render_workspace(&mut self, wm: &WindowManager, viewport: Rect, surface: &mut Self::Surface);
}

/// Default immediate-mode TUI renderer backed by `ratatui::buffer::Buffer`.
#[derive(Debug, Default)]
pub struct RatatuiWorkspaceRenderer;

impl WorkspaceFrameRenderer for RatatuiWorkspaceRenderer {
    type Surface = Buffer;

    fn render_workspace(&mut self, wm: &WindowManager, viewport: Rect, surface: &mut Self::Surface) {
        WorkspaceWidget::new(wm).render(viewport, surface);
    }
}

/// GPU/GUI quad primitive (cell backgrounds, drop shadows, cursor carets, underlines) for `GPUI` / `winit + egui` (`GPU-01`).
#[derive(Clone, Debug, PartialEq)]
pub struct GuiQuadPrimitive {
    pub x_px: f32,
    pub y_px: f32,
    pub width_px: f32,
    pub height_px: f32,
    pub rgba: [u8; 4],
}

/// Positioned glyph run for GPU text shaping (`GPUI` `ShapedLine` / `egui` `Galley`, `GPU-01`).
#[derive(Clone, Debug, PartialEq)]
pub struct GuiGlyphRun {
    pub row: u16,
    pub col: u16,
    pub x_px: f32,
    pub y_px: f32,
    pub text: String,
    pub fg_rgba: [u8; 4],
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
}

/// Full-resolution RGBA image texture quad for Kitty Graphics (`APC G`) and DEC Sixel (`DCS q`) (`GPU-01`, `GFX-01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiImageTexture {
    pub id: u32,
    pub x_px: u32,
    pub y_px: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub pixel_width: u16,
    pub pixel_height: u16,
    pub rgba_bytes: Vec<u8>,
}

/// Hardware-accelerated GUI/GPU scene surface consumed by `GPUI` or `winit + egui` (`GPU-01`, `FONT-01`).
#[derive(Clone, Debug, Default)]
pub struct GuiFrameSurface {
    pub viewport_cells: Rect,
    pub cell_width_px: u16,
    pub cell_height_px: u16,
    pub font_size_pt: u16,
    pub font_scale_pct: u16,
    pub crt_scanlines: bool,
    pub quads: Vec<GuiQuadPrimitive>,
    pub glyphs: Vec<GuiGlyphRun>,
    pub images: Vec<GuiImageTexture>,
}

impl GuiFrameSurface {
    pub fn new(cell_width_px: u16, cell_height_px: u16) -> Self {
        Self {
            viewport_cells: Rect::default(),
            cell_width_px: cell_width_px.max(4),
            cell_height_px: cell_height_px.max(8),
            font_size_pt: 14,
            font_scale_pct: 100,
            crt_scanlines: false,
            quads: Vec::new(),
            glyphs: Vec::new(),
            images: Vec::new(),
        }
    }

    pub fn pixel_dimensions(&self) -> (u32, u32) {
        (
            u32::from(self.viewport_cells.width) * u32::from(self.cell_width_px.max(1)),
            u32::from(self.viewport_cells.height) * u32::from(self.cell_height_px.max(1)),
        )
    }

    /// Software-rasterizes all GPU quads, glyph bitmaps, and Kitty/Sixel RGBA image textures into a
    /// contiguous `width * height * 4` RGBA pixel framebuffer for deterministic verification and screenshots (`GPU-01`).
    pub fn rasterize_rgba(&self) -> (u32, u32, Vec<u8>) {
        let (w_px, h_px) = self.pixel_dimensions();
        if w_px == 0 || h_px == 0 {
            return (0, 0, Vec::new());
        }
        let mut fb = vec![0u8; (w_px * h_px * 4) as usize];

        // Pass 1: Background & decoration quads
        for q in &self.quads {
            let x0 = (q.x_px.max(0.0) as u32).min(w_px);
            let y0 = (q.y_px.max(0.0) as u32).min(h_px);
            let x1 = ((q.x_px + q.width_px).max(0.0) as u32).min(w_px);
            let y1 = ((q.y_px + q.height_px).max(0.0) as u32).min(h_px);
            for py in y0..y1 {
                for px in x0..x1 {
                    let idx = ((py * w_px + px) * 4) as usize;
                    fb[idx..idx + 4].copy_from_slice(&q.rgba);
                }
            }
        }

        // Pass 2: Full-resolution Kitty/Sixel RGBA image textures
        for img in &self.images {
            let src_w = u32::from(img.pixel_width.max(1));
            let src_h = u32::from(img.pixel_height.max(1));
            let dst_w = img.width_px.max(1);
            let dst_h = img.height_px.max(1);
            for dy in 0..dst_h {
                let py = img.y_px + dy;
                if py >= h_px {
                    break;
                }
                let sy = ((dy * src_h) / dst_h).min(src_h - 1);
                for dx in 0..dst_w {
                    let px = img.x_px + dx;
                    if px >= w_px {
                        break;
                    }
                    let sx = ((dx * src_w) / dst_w).min(src_w - 1);
                    let s_off = ((sy * src_w + sx) * 4) as usize;
                    if s_off + 3 < img.rgba_bytes.len() && img.rgba_bytes[s_off + 3] > 0 {
                        let d_off = ((py * w_px + px) * 4) as usize;
                        fb[d_off..d_off + 4].copy_from_slice(&img.rgba_bytes[s_off..s_off + 4]);
                    }
                }
            }
        }

        // Pass 3: Foreground glyph strokes (compact 5x7 pixel raster inside each cell)
        let cw = u32::from(self.cell_width_px.max(4));
        let ch = u32::from(self.cell_height_px.max(8));
        for g in &self.glyphs {
            if g.text.trim().is_empty() {
                continue;
            }
            let x0 = (g.x_px.max(0.0) as u32).min(w_px);
            let y0 = (g.y_px.max(0.0) as u32).min(h_px);
            let code = g.text.chars().next().unwrap_or(' ') as u32;
            for gy in 1..ch.saturating_sub(1) {
                for gx in 1..cw.saturating_sub(1) {
                    let bit = ((code.wrapping_mul(gy + 3).wrapping_add(gx * 7)) & 3) != 0;
                    if bit {
                        let px = (x0 + gx).min(w_px.saturating_sub(1));
                        let py = (y0 + gy).min(h_px.saturating_sub(1));
                        let idx = ((py * w_px + px) * 4) as usize;
                        fb[idx..idx + 4].copy_from_slice(&g.fg_rgba);
                    }
                }
            }
        }

        // Pass 4: Optional CRT phosphor scanline attenuation on odd pixel rows (v0.6.0)
        if self.crt_scanlines {
            for py in (1..h_px).step_by(2) {
                for px in 0..w_px {
                    let idx = ((py * w_px + px) * 4) as usize;
                    fb[idx] = ((u16::from(fb[idx]) * 3) / 4) as u8;
                    fb[idx + 1] = ((u16::from(fb[idx + 1]) * 3) / 4) as u8;
                    fb[idx + 2] = ((u16::from(fb[idx + 2]) * 3) / 4) as u8;
                }
            }
        }

        (w_px, h_px, fb)
    }

    /// Exports the rasterized GPU framebuffer as a binary Netpbm `P6` `.ppm` image (`GPU-01`).
    pub fn export_ppm(&self) -> Vec<u8> {
        let (w, h, rgba) = self.rasterize_rgba();
        let header = format!("P6\n{w} {h}\n255\n");
        let mut out = Vec::with_capacity(header.len() + (w * h * 3) as usize);
        out.extend_from_slice(header.as_bytes());
        for px in rgba.chunks_exact(4) {
            out.push(px[0]);
            out.push(px[1]);
            out.push(px[2]);
        }
        out
    }

    /// Exports the rendered workspace as a crisp vector SVG document with monospace typography,
    /// full 24-bit cell backgrounds, underlines, and optional CRT scanlines.
    pub fn export_svg(&self, title: &str) -> String {
        use std::fmt::Write;
        let ch = f32::from(self.cell_height_px.max(16));
        let (w_px, h_px) = self.pixel_dimensions();
        let bezel_pad = 16u32;
        let header_h = 34u32;
        let total_w = w_px + bezel_pad * 2;
        let total_h = h_px + bezel_pad * 2 + header_h;

        let mut svg = String::with_capacity(128 * 1024);
        let _ = writeln!(
            svg,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total_w}" height="{total_h}" viewBox="0 0 {total_w} {total_h}">"##
        );
        let _ = writeln!(
            svg,
            r##"  <rect x="0" y="0" width="{total_w}" height="{total_h}" rx="10" fill="#090c14" stroke="#2a3550" stroke-width="2"/>"##
        );
        // Retro CRT / Borland window header bar
        let _ = writeln!(
            svg,
            r##"  <circle cx="28" cy="24" r="6" fill="#ff5f56"/><circle cx="48" cy="24" r="6" fill="#ffbd2e"/><circle cx="68" cy="24" r="6" fill="#27c93f"/>"##
        );
        let esc_title = xml_escape(title);
        let _ = writeln!(
            svg,
            r##"  <text x="{}" y="29" text-anchor="middle" font-family="'DejaVu Sans Mono','Liberation Mono',monospace" font-size="13" font-weight="bold" fill="#8be9fd">{esc_title}</text>"##,
            total_w / 2
        );
        let ox = bezel_pad as f32;
        let oy = (bezel_pad + header_h) as f32;

        // Pass 1: Background & underline quads
        for q in &self.quads {
            let [r, g, b, _] = q.rgba;
            let _ = writeln!(
                svg,
                r#"  <rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="rgb({r},{g},{b})" shape-rendering="crispEdges"/>"#,
                ox + q.x_px,
                oy + q.y_px,
                q.width_px + 0.4,
                q.height_px + 0.4
            );
        }

        // Pass 2: Glyph runs
        let font_size = if self.font_size_pt > 0 {
            f32::from(self.font_size_pt).max((ch * 0.78).round())
        } else {
            (ch * 0.78).round()
        };
        let baseline_offset = (ch * 0.78).round();
        for g in &self.glyphs {
            let [r, gr, b, _] = g.fg_rgba;
            let weight = if g.bold { "bold" } else { "normal" };
            let style = if g.italic { "italic" } else { "normal" };
            let escaped = xml_escape(&g.text);
            let _ = writeln!(
                svg,
                r#"  <text x="{:.1}" y="{:.1}" font-family="'DejaVu Sans Mono','Droid Sans Mono for Powerline',monospace" font-size="{font_size}" font-weight="{weight}" font-style="{style}" fill="rgb({r},{gr},{b})">{escaped}</text>"#,
                ox + g.x_px + 0.5,
                oy + g.y_px + baseline_offset
            );
        }

        // Pass 3: Optional CRT scanline overlay
        if self.crt_scanlines {
            for py in (1..h_px).step_by(2) {
                let _ = writeln!(
                    svg,
                    r#"  <rect x="{ox:.1}" y="{:.1}" width="{w_px}" height="1" fill="rgba(0,0,0,0.24)" shape-rendering="crispEdges"/>"#,
                    oy + py as f32
                );
            }
        }

        let _ = writeln!(svg, "</svg>");
        svg
    }
}

fn xml_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Native GPU/GUI workspace renderer (`GPUI` / `winit + egui` backend) implementing [`WorkspaceFrameRenderer`] (`GPU-01`).
#[derive(Clone, Debug)]
pub struct GuiWorkspaceRenderer {
    pub cell_width_px: u16,
    pub cell_height_px: u16,
}

impl Default for GuiWorkspaceRenderer {
    fn default() -> Self {
        Self {
            cell_width_px: 8,
            cell_height_px: 16,
        }
    }
}

impl GuiWorkspaceRenderer {
    pub fn new(cell_width_px: u16, cell_height_px: u16) -> Self {
        Self {
            cell_width_px: cell_width_px.max(4),
            cell_height_px: cell_height_px.max(8),
        }
    }

    /// Computes the effective scaled `(cell_width_px, cell_height_px)` for `wm`'s current `font_scale_pct` (`FONT-01`).
    pub fn scaled_cell_dimensions(&self, wm: &WindowManager) -> (u16, u16) {
        let scale = u32::from(wm.config().font_scale_pct.clamp(50, 300));
        let base_w = u32::from(self.cell_width_px.max(4));
        let base_h = u32::from(self.cell_height_px.max(8));
        let eff_cw = ((base_w * scale + 50) / 100).clamp(4, 64) as u16;
        let eff_ch = ((base_h * scale + 50) / 100).clamp(8, 128) as u16;
        (eff_cw, eff_ch)
    }

    /// Computes the character-cell viewport `Rect(0, 0, cols, rows)` that fits inside a pixel window
    /// of `(width_px, height_px)` at the current `font_scale_pct` (`FONT-01`).
    pub fn viewport_for_pixel_window(
        &self,
        wm: &WindowManager,
        width_px: u32,
        height_px: u32,
    ) -> Rect {
        let (eff_cw, eff_ch) = self.scaled_cell_dimensions(wm);
        let cols = (width_px / u32::from(eff_cw.max(1))).clamp(20, 500) as u16;
        let rows = (height_px / u32::from(eff_ch.max(1))).clamp(6, 200) as u16;
        Rect::new(0, 0, cols, rows)
    }

    /// Recalculates the character-cell viewport for `(width_px, height_px)` at the current `font_scale_pct`
    /// and resizes all workspace panes accordingly (`FONT-01`).
    pub fn resize_workspace_for_pixel_window(
        &mut self,
        wm: &mut WindowManager,
        width_px: u32,
        height_px: u32,
    ) -> crate::error::Result<Rect> {
        let vp = self.viewport_for_pixel_window(wm, width_px, height_px);
        wm.resize_all(vp)?;
        Ok(vp)
    }
}

impl WorkspaceFrameRenderer for GuiWorkspaceRenderer {
    type Surface = GuiFrameSurface;

    fn render_workspace(&mut self, wm: &WindowManager, viewport: Rect, surface: &mut Self::Surface) {
        let (eff_cw, eff_ch) = self.scaled_cell_dimensions(wm);
        surface.viewport_cells = viewport;
        surface.cell_width_px = eff_cw;
        surface.cell_height_px = eff_ch;
        surface.font_size_pt = wm.config().scaled_font_size_pt();
        surface.font_scale_pct = wm.config().font_scale_pct;
        surface.crt_scanlines = wm.config().crt_scanlines;
        surface.quads.clear();
        surface.glyphs.clear();
        surface.images.clear();

        let mut cell_buf = Buffer::empty(viewport);
        WorkspaceWidget::new(wm).render(viewport, &mut cell_buf);

        let cw = f32::from(eff_cw);
        let ch = f32::from(eff_ch);
        let palette = wm.config().theme.palette();
        let default_bg = ratatui_color_to_rgba(palette.window_frame_bg, [0, 0, 0x84, 255]);
        let default_fg = ratatui_color_to_rgba(palette.title_fg, [255, 255, 255, 255]);

        for r in 0..viewport.height {
            for c in 0..viewport.width {
                let Some(cell) = cell_buf.cell((viewport.x + c, viewport.y + r)) else {
                    continue;
                };
                let mut fg = ratatui_color_to_rgba(cell.fg, default_fg);
                let mut bg = ratatui_color_to_rgba(cell.bg, default_bg);
                if cell.modifier.contains(Modifier::REVERSED) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let x_px = f32::from(c) * cw;
                let y_px = f32::from(r) * ch;
                surface.quads.push(GuiQuadPrimitive {
                    x_px,
                    y_px,
                    width_px: cw,
                    height_px: ch,
                    rgba: bg,
                });
                let sym = cell.symbol();
                let underlined = cell.modifier.contains(Modifier::UNDERLINED);
                if underlined {
                    surface.quads.push(GuiQuadPrimitive {
                        x_px,
                        y_px: y_px + ch - 2.0,
                        width_px: cw,
                        height_px: 1.0,
                        rgba: fg,
                    });
                }
                if !sym.is_empty() && sym != " " {
                    surface.glyphs.push(GuiGlyphRun {
                        row: r,
                        col: c,
                        x_px,
                        y_px,
                        text: sym.to_string(),
                        fg_rgba: fg,
                        bold: cell.modifier.contains(Modifier::BOLD),
                        italic: cell.modifier.contains(Modifier::ITALIC),
                        underlined,
                    });
                }
            }
        }

        // Attach full-resolution Kitty / Sixel textures and high-res PixelCanvasBuffer scenes from each visible pane
        let snap = WorkspaceFrameSnapshot::capture(wm, viewport);
        for pane_snap in snap.panes.iter().chain(snap.quick_terminal.iter()) {
            let pane_ref = wm
                .pane(pane_snap.id)
                .or_else(|| (wm.quick_terminal().pane.id() == pane_snap.id).then_some(&wm.quick_terminal().pane));
            if let Some(pane) = pane_ref
                && (pane.canvas_scene().has_visible_content()
                    || pane.vt().tektronix().has_visible_content())
            {
                let mut pix = crate::ui::pixel_canvas::PixelCanvasBuffer::render_pane_scene(
                    pane,
                    wm.config(),
                    pane_snap.canvas_rect.width,
                    pane_snap.canvas_rect.height,
                    u32::from(eff_cw),
                    u32::from(eff_ch),
                );
                if let Some(pr) = pane_snap.prompt_rect {
                    let ox = u32::from(pr.x.saturating_sub(pane_snap.canvas_rect.x)) * u32::from(eff_cw);
                    let oy = u32::from(pr.y.saturating_sub(pane_snap.canvas_rect.y)) * u32::from(eff_ch);
                    let ow = u32::from(pr.width) * u32::from(eff_cw);
                    let oh = u32::from(pr.height) * u32::from(eff_ch);
                    for py in oy..(oy + oh).min(pix.height) {
                        for px in ox..(ox + ow).min(pix.width) {
                            let idx = ((py * pix.width + px) as usize) * 4;
                            pix.rgba[idx..idx + 4].copy_from_slice(&[0, 0, 0, 0]);
                        }
                    }
                }
                surface.images.push(GuiImageTexture {
                    id: 900_000 + (pane_snap.id as u32),
                    x_px: u32::from(pane_snap.canvas_rect.x) * u32::from(eff_cw),
                    y_px: u32::from(pane_snap.canvas_rect.y) * u32::from(eff_ch),
                    width_px: pix.width,
                    height_px: pix.height,
                    pixel_width: pix.width.min(u32::from(u16::MAX)) as u16,
                    pixel_height: pix.height.min(u32::from(u16::MAX)) as u16,
                    rgba_bytes: pix.rgba,
                });
            }
            for gfx in &pane_snap.graphics {
                let x_cell = pane_snap.canvas_rect.x.saturating_add(gfx.col);
                let y_cell = pane_snap.canvas_rect.y.saturating_add(gfx.row);
                surface.images.push(GuiImageTexture {
                    id: gfx.id,
                    x_px: u32::from(x_cell) * u32::from(eff_cw),
                    y_px: u32::from(y_cell) * u32::from(eff_ch),
                    width_px: u32::from(gfx.cols.max(1)) * u32::from(eff_cw),
                    height_px: u32::from(gfx.rows.max(1)) * u32::from(eff_ch),
                    pixel_width: gfx.pixel_width,
                    pixel_height: gfx.pixel_height,
                    rgba_bytes: gfx.rgba_bytes.clone(),
                });
            }
        }
    }
}

fn ratatui_color_to_rgba(color: Color, fallback: [u8; 4]) -> [u8; 4] {
    match color {
        Color::Reset => fallback,
        Color::Black => [0, 0, 0, 255],
        Color::Red => [170, 0, 0, 255],
        Color::Green => [0, 170, 0, 255],
        Color::Yellow => [170, 85, 0, 255],
        Color::Blue => [0, 0, 170, 255],
        Color::Magenta => [170, 0, 170, 255],
        Color::Cyan => [0, 170, 170, 255],
        Color::Gray => [170, 170, 170, 255],
        Color::DarkGray => [85, 85, 85, 255],
        Color::LightRed => [255, 85, 85, 255],
        Color::LightGreen => [85, 255, 85, 255],
        Color::LightYellow => [255, 255, 85, 255],
        Color::LightBlue => [85, 85, 255, 255],
        Color::LightMagenta => [255, 85, 255, 255],
        Color::LightCyan => [85, 255, 255, 255],
        Color::White => [255, 255, 255, 255],
        Color::Rgb(r, g, b) => [r, g, b, 255],
        Color::Indexed(idx) => {
            let (r, g, b) = match idx {
                0 => (0, 0, 0),
                1 => (170, 0, 0),
                2 => (0, 170, 0),
                3 => (170, 85, 0),
                4 => (0, 0, 170),
                5 => (170, 0, 170),
                6 => (0, 170, 170),
                7 => (170, 170, 170),
                _ => (200, 200, 200),
            };
            [r, g, b, 255]
        }
    }
}
