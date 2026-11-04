//! GUI texture pass for the agents window graph and the Navigator state icons (Phase 50-11, D-10 / D-05).
//!
//! In GUI mode the cell grid is turned into quads and glyphs; this pass runs on the finished cell
//! buffer first and replaces some cells by opaque, 1:1, high-resolution RGBA textures. The GUI
//! presenters paint quads, then images (copy where alpha > 0, no blending, nearest neighbour), then
//! glyphs, so a texture must be fully opaque, match the screen pixels one to one, and the cells it
//! covers must be cleared to spaces or braille / half-block glyphs would paint over it. Labels stay
//! cell glyphs and are drawn on top.
//!
//! Texture id ranges (all above the per-pane canvas range `900_000 + pane id`):
//! - [`GRAPH_TEXTURE_ID`] (`950_000`): the agents graph.
//! - [`ICON_TEXTURE_BASE`] (`960_000`) `+ row index` (row index < [`ICON_ROW_LIMIT`]): Navigator icons.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::ui::graph_raster::{GraphColours, draw_agent_graph_hires, render_graph_labels};
use crate::ui::pixel_canvas::PixelCanvasBuffer;
use crate::ui::renderer::{GuiFrameSurface, GuiImageTexture};
use crate::ui::typography::PanelTarget;
use crate::wm::WindowManager;

/// Texture id of the agents graph.
pub const GRAPH_TEXTURE_ID: u32 = 950_000;
/// First texture id of the Navigator state icons (`+ row index`).
pub const ICON_TEXTURE_BASE: u32 = 960_000;
/// Icon row indices at or above this value get no texture.
pub const ICON_ROW_LIMIT: usize = 10_000;
/// Upper bound on the pixel count of one texture (T-50-26): 16 Mpx = 64 MiB of RGBA.
const MAX_TEXTURE_PIXELS: u64 = 16 * 1024 * 1024;

/// `true` for the texture ids owned by this pass; the renderer keeps them last so they stay on top
/// of pane canvases.
pub fn is_agents_texture(id: u32) -> bool {
    (GRAPH_TEXTURE_ID..ICON_TEXTURE_BASE + ICON_ROW_LIMIT as u32).contains(&id)
}

/// Maps panel cells to screen pixels. A scaled panel is drawn in a virtual cell grid that starts at
/// the workspace origin (exactly like the renderer's `draw_panel`).
#[derive(Clone, Copy, Debug)]
pub struct PanelFrame {
    ws_area: Rect,
    viewport: Rect,
    base: (f32, f32),
    cell: (f32, f32),
}

impl PanelFrame {
    /// Frame of a panel drawn at the effective cell size.
    pub fn unscaled(ws_area: Rect, viewport: Rect, cell_w: u16, cell_h: u16) -> Self {
        Self::scaled(ws_area, viewport, cell_w, cell_h, 100)
    }

    /// Frame of a panel drawn at `scale_pct` percent of the effective cell size.
    pub fn scaled(ws_area: Rect, viewport: Rect, cell_w: u16, cell_h: u16, scale_pct: u16) -> Self {
        let base = (f32::from(cell_w), f32::from(cell_h));
        let k = f32::from(scale_pct) / 100.0;
        Self { ws_area, viewport, base, cell: (base.0 * k, base.1 * k) }
    }

    /// Pixel position of the top-left corner of panel cell `(col, row)`.
    fn px(&self, col: u16, row: u16) -> (f32, f32) {
        let ox = f32::from(self.ws_area.x.saturating_sub(self.viewport.x)) * self.base.0;
        let oy = f32::from(self.ws_area.y.saturating_sub(self.viewport.y)) * self.base.1;
        (
            ox + f32::from(col.saturating_sub(self.ws_area.x)) * self.cell.0,
            oy + f32::from(row.saturating_sub(self.ws_area.y)) * self.cell.1,
        )
    }

    /// Pixel rectangle `(x, y, w, h)` of the panel cell rectangle `rect`, or `None` when it is empty
    /// or would exceed the texture size limits (T-50-26: at most `u16::MAX` per axis and 16 Mpx).
    fn pixel_rect(&self, rect: Rect) -> Option<(u32, u32, u32, u32)> {
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        let (x0, y0) = self.px(rect.x, rect.y);
        let (x1, y1) = self.px(rect.x + rect.width, rect.y + rect.height);
        let (x0, y0) = (x0.round().max(0.0) as u32, y0.round().max(0.0) as u32);
        let (x1, y1) = (x1.round().max(0.0) as u32, y1.round().max(0.0) as u32);
        let (w, h) = (x1.checked_sub(x0)?, y1.checked_sub(y0)?);
        let fits = w > 0
            && h > 0
            && w <= u32::from(u16::MAX)
            && h <= u32::from(u16::MAX)
            && u64::from(w) * u64::from(h) <= MAX_TEXTURE_PIXELS;
        fits.then_some((x0, y0, w, h))
    }

    /// Pixel rectangle of a rectangle given in real (unscaled) cells; used for occlusion tests.
    fn real_pixel_rect(&self, r: Rect) -> (u32, u32, u32, u32) {
        let px = |c: u16, o: u16, b: f32| (f32::from(c.saturating_sub(o)) * b) as u32;
        (
            px(r.x, self.viewport.x, self.base.0),
            px(r.y, self.viewport.y, self.base.1),
            (f32::from(r.width) * self.base.0) as u32,
            (f32::from(r.height) * self.base.1) as u32,
        )
    }
}

fn pixel_rects_overlap(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// Rectangles drawn above the agents window and the drawer (`:mcp` console, modal, menu dropdown,
/// quick terminal), in real cells.
fn overlay_rects(wm: &WindowManager, ws_area: Rect) -> Vec<Rect> {
    [
        wm.mcp_console().panel_rect(ws_area),
        wm.active_modal_rect(ws_area),
        wm.menubar_state().map(|(cat, _)| crate::ui::modals::menubar_dropdown_rect(cat, ws_area)),
        wm.is_quick_terminal_visible().then(|| wm.quick_terminal().overlay_rect(ws_area)),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Replaces the agents window graph pane by one opaque 1:1 texture and keeps only the node labels as
/// cell glyphs. `buf` is the cell buffer the panel was rendered into, in the panel's own cell grid
/// (`vws`). Does nothing (the braille graph stays) when the window is closed, covered by an overlay,
/// or the texture would exceed the size limits.
fn push_graph(
    wm: &WindowManager,
    vws: Rect,
    frame: &PanelFrame,
    overlays: &[Rect],
    buf: &mut Buffer,
    images: &mut Vec<GuiImageTexture>,
    tick: u64,
) {
    if !wm.agents_window().active {
        return;
    }
    let Some(rects) = wm.agents_pane_rects(vws) else { return };
    let graph = rects.graph.intersection(buf.area);
    let Some(area) = frame.pixel_rect(graph) else { return };
    if overlays.iter().any(|o| pixel_rects_overlap(area, frame.real_pixel_rect(*o))) {
        return;
    }
    let (x, y, w, h) = area;
    let palette = wm.config().palette();
    let colours = GraphColours::from_palette(&palette);
    let scene = wm.agents_graph_scene();
    render_graph_labels(&scene, graph, buf, &palette);

    let bg = [colours.bg.0, colours.bg.1, colours.bg.2, 255];
    let mut canvas = PixelCanvasBuffer::new(w, h, bg);
    let cell = (w as f32 / f32::from(graph.width), h as f32 / f32::from(graph.height));
    draw_agent_graph_hires(&mut canvas, &scene, &colours, tick, cell);
    images.push(GuiImageTexture {
        id: GRAPH_TEXTURE_ID,
        x_px: x,
        y_px: y,
        width_px: w,
        height_px: h,
        pixel_width: w as u16,
        pixel_height: h as u16,
        rgba_bytes: canvas.rgba,
    });
}

/// Pushes the GUI textures of every panel drawn at 100 percent: the agents graph and, in the
/// Navigator drawer style, the state icons. `cell_buf` is the fully rendered workspace cell buffer;
/// covered cells are cleared in place. Panels with their own scale are handled from the
/// renderer's panel pass.
#[allow(clippy::too_many_arguments)]
pub fn push_agents_gui_textures(
    wm: &WindowManager,
    ws_area: Rect,
    viewport: Rect,
    cell_w: u16,
    cell_h: u16,
    cell_buf: &mut Buffer,
    surface: &mut GuiFrameSurface,
    tick: u64,
) {
    let frame = PanelFrame::unscaled(ws_area, viewport, cell_w, cell_h);
    let overlays = overlay_rects(wm, ws_area);
    if wm.scale_pct(PanelTarget::Agents) == 100 {
        push_graph(wm, ws_area, &frame, &overlays, cell_buf, &mut surface.images, tick);
    }
}
