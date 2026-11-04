//! Agent traffic graph rasteriser (Phase 50-07, D-10): draws a [`GraphScene`] onto a
//! [`PixelCanvasBuffer`] and flushes it into a ratatui buffer as braille sub-cells.
//!
//! The pixel drawing ([`draw_agent_graph`]) is target independent so the GUI backend (50-11) can
//! reuse it; [`render_graph_tui`] is the terminal sink. Labels are written into the buffer first:
//! the sub-cell flush only fills cells that still hold a blank symbol, so text always wins.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthStr;

use crate::pane::CanvasSubcellMode;
use crate::ui::cursor::color_to_rgb;
use crate::ui::design::DesignTokens;
use crate::ui::pixel_canvas::PixelCanvasBuffer;
use crate::ui::theme::ThemePalette;
use crate::wm::agent_state::AgentState;
use crate::wm::agents_window::{GraphScene, SceneEdge, SceneNode};

/// Horizontal / top padding of the node area, canvas pixels.
const PAD: f32 = 4.0;
/// Bottom padding of the node area (leaves one terminal row for labels), canvas pixels.
const PAD_BOTTOM: f32 = 8.0;
/// Sideways bend of an edge curve as a fraction of its length.
const CURVE_BEND: f32 = 0.18;
const NODE_RADIUS: f32 = 3.0;
const NODE_RADIUS_SELECTED: f32 = 4.0;

type Rgb = (u8, u8, u8);

/// Colours used by the graph drawing.
#[derive(Clone, Copy, Debug)]
pub struct GraphColours {
    pub bg: Rgb,
    pub edge: Rgb,
    pub edge_active: Rgb,
    pub accent: Rgb,
    pub node: Rgb,
    pub working: Rgb,
    pub blocked: Rgb,
    pub idle: Rgb,
    pub done: Rgb,
    pub errored: Rgb,
}

impl GraphColours {
    /// Derives the graph colours from the active theme palette.
    pub fn from_palette(palette: &ThemePalette) -> Self {
        let t = DesignTokens::from_palette(palette);
        let c = |col: Color| color_to_rgb(col, (200, 200, 200));
        Self {
            bg: c(palette.window_frame_bg),
            edge: c(t.border_idle),
            // Traffic colour; distinct from `accent`, which marks the selection.
            edge_active: c(t.success),
            accent: c(t.accent),
            node: c(t.text_primary),
            working: c(t.success),
            blocked: c(t.warning),
            idle: c(t.text_muted),
            done: c(t.accent),
            errored: c(t.danger),
        }
    }

    /// Colour of a node by agent state (plain nodes use the neutral node colour).
    pub fn state_rgb(&self, state: Option<AgentState>) -> Rgb {
        match state {
            Some(AgentState::Working) => self.working,
            Some(AgentState::Blocked) => self.blocked,
            Some(AgentState::Idle) => self.idle,
            Some(AgentState::Done) => self.done,
            Some(AgentState::Errored) => self.errored,
            None => self.node,
        }
    }
}

/// Canvas pixel position of a normalised node position.
pub fn node_px(pos: (f32, f32), width: f32, height: f32) -> (f32, f32) {
    let x = PAD + pos.0.clamp(0.0, 1.0) * (width - 2.0 * PAD).max(1.0);
    let y = PAD + pos.1.clamp(0.0, 1.0) * (height - PAD - PAD_BOTTOM).max(1.0);
    (x, y)
}

fn edge_rgb(edge: &SceneEdge, colours: &GraphColours) -> Rgb {
    if edge.selected {
        colours.accent
    } else if edge.active {
        colours.edge_active
    } else {
        colours.edge
    }
}

/// Control points of the edge curve: the chord's thirds pushed sideways by a share of its length
/// (perpendicular via a 90 degree rotation, no trigonometry).
pub fn curve_controls(a: (f32, f32), b: (f32, f32)) -> ((f32, f32), (f32, f32)) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-3 {
        return (a, b);
    }
    let off = CURVE_BEND * len;
    let (nx, ny) = (-dy / len * off, dx / len * off);
    (
        (a.0 + dx / 3.0 + nx, a.1 + dy / 3.0 + ny),
        (a.0 + 2.0 * dx / 3.0 + nx, a.1 + 2.0 * dy / 3.0 + ny),
    )
}

/// Draws edges (selected edge last, so it stays on top) and nodes onto `canvas`.
///
/// `tick` animates active edges with a growing halo around their endpoints; `0` is the static frame.
pub fn draw_agent_graph(
    canvas: &mut PixelCanvasBuffer,
    scene: &GraphScene,
    colours: &GraphColours,
    tick: u32,
) {
    let (w, h) = (canvas.width as f32, canvas.height as f32);
    let px_of = |key: &crate::wm::agents_window::NodeKey| {
        scene
            .nodes
            .iter()
            .find(|n| &n.key == key)
            .map(|n| node_px(n.pos, w, h))
    };
    let mut ordered: Vec<&SceneEdge> = scene.edges.iter().collect();
    ordered.sort_by_key(|e| e.selected);
    for edge in ordered {
        let (Some(a), Some(b)) = (px_of(&edge.from), px_of(&edge.to)) else {
            continue;
        };
        let (c1, c2) = curve_controls(a, b);
        let rgb = edge_rgb(edge, colours);
        canvas.draw_bezier_aa(a, c1, c2, b, rgb);
        if edge.selected {
            // Selected edges are drawn two pixels thick so they stand out from plain traffic.
            canvas.draw_bezier_aa(
                (a.0, a.1 + 1.0),
                (c1.0, c1.1 + 1.0),
                (c2.0, c2.1 + 1.0),
                (b.0, b.1 + 1.0),
                rgb,
            );
        }
    }
    for node in &scene.nodes {
        let (x, y) = node_px(node.pos, w, h);
        let rgb = colours.state_rgb(node.state);
        let r = if node.selected { NODE_RADIUS_SELECTED } else { NODE_RADIUS };
        canvas.draw_circle_aa(x, y, r, true, rgb);
        if node.selected {
            canvas.draw_circle_aa(x, y, r + 2.0, false, colours.accent);
        }
        if tick > 0
            && scene
                .edges
                .iter()
                .any(|e| e.active && (e.from == node.key || e.to == node.key))
        {
            canvas.draw_circle_aa(x, y, r + 2.0 + (tick % 4) as f32, false, colours.edge_active);
        }
    }
}

/// Cell (relative to the graph rect) holding the label of `node`, with the label text (also the node hit area).
pub fn label_cell(node: &SceneNode, rect: Rect) -> Option<(u16, u16, String)> {
    if rect.width == 0 || rect.height == 0 {
        return None;
    }
    let (px, py) = node_px(
        node.pos,
        f32::from(rect.width) * 2.0,
        f32::from(rect.height) * 4.0,
    );
    let cx = ((px / 2.0).floor() as i64).clamp(0, i64::from(rect.width) - 1) as u16;
    let cy = ((py / 4.0).floor() as i64 + 1).clamp(0, i64::from(rect.height) - 1) as u16;
    let mut text = node.label.clone();
    while UnicodeWidthStr::width(text.as_str()) > usize::from(rect.width) {
        text.pop();
    }
    let tw = UnicodeWidthStr::width(text.as_str()) as u16;
    let x = cx
        .saturating_sub(tw / 2)
        .min(rect.width.saturating_sub(tw));
    Some((x, cy, text))
}

/// Clears `rect` to spaces on the window background and writes the node labels, selected nodes
/// reversed and bold. Shared by the terminal sink and the GUI texture pass, so both show the same
/// text; the GUI then draws the graph itself underneath as pixels.
pub fn render_graph_labels(scene: &GraphScene, rect: Rect, buf: &mut Buffer, palette: &ThemePalette) {
    let rect = rect.intersection(buf.area);
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let colours = GraphColours::from_palette(palette);
    let tokens = DesignTokens::from_palette(palette);
    let base = Style::default().fg(tokens.text_primary).bg(palette.window_frame_bg);
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(base);
            }
        }
    }

    for node in &scene.nodes {
        let Some((lx, ly, text)) = label_cell(node, rect) else {
            continue;
        };
        let rgb = colours.state_rgb(node.state);
        let mut style = Style::default()
            .fg(Color::Rgb(rgb.0, rgb.1, rgb.2))
            .bg(palette.window_frame_bg);
        if node.selected {
            style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
        }
        buf.set_stringn(rect.x + lx, rect.y + ly, &text, usize::from(rect.width - lx), style);
    }
}

/// Terminal sink: clears `rect`, writes node labels, then flushes the braille graph around them.
pub fn render_graph_tui(scene: &GraphScene, rect: Rect, buf: &mut Buffer, palette: &ThemePalette) {
    let rect = rect.intersection(buf.area);
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    render_graph_labels(scene, rect, buf, palette);
    let colours = GraphColours::from_palette(palette);
    let bg = [colours.bg.0, colours.bg.1, colours.bg.2, 0];
    let mut canvas =
        PixelCanvasBuffer::new(u32::from(rect.width) * 2, u32::from(rect.height) * 4, bg);
    draw_agent_graph(&mut canvas, scene, &colours, 0);
    canvas.flush_truecolor_subcells_to_ratatui(CanvasSubcellMode::Braille, rect, buf);
}

/// Node centre in canvas pixels for a graph pane of `cell` pixels per terminal cell.
///
/// Same layout as the braille path ([`node_px`] on a 2x4 sub-cell grid) scaled to the canvas, so a
/// node sits directly above its [`label_cell`] in both renderers.
pub fn node_px_hires(pos: (f32, f32), canvas_w: f32, canvas_h: f32, cell: (f32, f32)) -> (f32, f32) {
    let cols = (canvas_w / cell.0.max(1.0)).round().max(1.0);
    let rows = (canvas_h / cell.1.max(1.0)).round().max(1.0);
    let (bx, by) = node_px(pos, cols * 2.0, rows * 4.0);
    (bx * canvas_w / (cols * 2.0), by * canvas_h / (rows * 4.0))
}

/// Node radius in canvas pixels for a cell of `cell` pixels.
pub fn node_radius_hires(cell: (f32, f32), selected: bool) -> f32 {
    (cell.0 * 0.45).max(3.0) + if selected { 1.0 } else { 0.0 }
}

/// Point of the cubic Bezier `a, c1, c2, b` at parameter `t` (polynomial form, no trigonometry).
pub fn bezier_point(a: (f32, f32), c1: (f32, f32), c2: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        w0 * a.0 + w1 * c1.0 + w2 * c2.0 + w3 * b.0,
        w0 * a.1 + w1 * c1.1 + w2 * c2.1 + w3 * b.1,
    )
}

/// Stroke width in canvas pixels for a cell of `cell` pixels.
pub fn stroke_width_hires(cell: (f32, f32)) -> f32 {
    (cell.0 / 6.0).max(1.5)
}

/// Number of flow particles on an active edge.
const PARTICLES: u32 = 3;
/// Animation ticks per full particle revolution (`fract(tick * 0.04)`).
const PARTICLE_PERIOD: u64 = 25;
/// Glow radius as a multiple of the node radius.
const GLOW_RADIUS_FACTOR: f32 = 2.2;
/// Peak glow opacity.
const GLOW_PEAK: f32 = 0.6;
/// Opacity of an edge that carried no recent traffic.
const IDLE_EDGE_ALPHA: f32 = 0.55;

/// Strokes the polyline `pts` with a round-capped line of `width` pixels in `rgb` at opacity `alpha`.
///
/// Coverage is the per-pixel maximum over all segments (distance to the nearest segment), so joints
/// and self-overlaps never double-blend and the edge stays uniform.
pub fn stroke_polyline(canvas: &mut PixelCanvasBuffer, pts: &[(f32, f32)], width: f32, rgb: Rgb, alpha: f32) {
    if pts.len() < 2 {
        return;
    }
    let half = width * 0.5;
    let pad = half + 1.5;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in pts {
        min_x = min_x.min(p.0);
        min_y = min_y.min(p.1);
        max_x = max_x.max(p.0);
        max_y = max_y.max(p.1);
    }
    let bx0 = (min_x - pad).floor().max(0.0) as i32;
    let by0 = (min_y - pad).floor().max(0.0) as i32;
    let bx1 = (max_x + pad).ceil().min(canvas.width as f32 - 1.0) as i32;
    let by1 = (max_y + pad).ceil().min(canvas.height as f32 - 1.0) as i32;
    if bx1 < bx0 || by1 < by0 {
        return;
    }
    let (bw, bh) = ((bx1 - bx0 + 1) as usize, (by1 - by0 + 1) as usize);
    let mut cov = vec![0.0f32; bw * bh];
    for seg in pts.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len2 = dx * dx + dy * dy;
        let sx0 = (a.0.min(b.0) - pad).floor().max(bx0 as f32) as i32;
        let sy0 = (a.1.min(b.1) - pad).floor().max(by0 as f32) as i32;
        let sx1 = (a.0.max(b.0) + pad).ceil().min(bx1 as f32) as i32;
        let sy1 = (a.1.max(b.1) + pad).ceil().min(by1 as f32) as i32;
        for y in sy0..=sy1 {
            for x in sx0..=sx1 {
                let (px, py) = (x as f32 - a.0, y as f32 - a.1);
                let t = if len2 < 1e-6 { 0.0 } else { ((px * dx + py * dy) / len2).clamp(0.0, 1.0) };
                let (qx, qy) = (px - t * dx, py - t * dy);
                let d = (qx * qx + qy * qy).sqrt();
                let c = (half + 0.5 - d).clamp(0.0, 1.0);
                let slot = &mut cov[(y - by0) as usize * bw + (x - bx0) as usize];
                if c > *slot {
                    *slot = c;
                }
            }
        }
    }
    for y in by0..=by1 {
        for x in bx0..=bx1 {
            let c = cov[(y - by0) as usize * bw + (x - bx0) as usize];
            if c > 0.0 {
                canvas.blend_pixel(x, y, rgb, c * alpha);
            }
        }
    }
}

/// Soft radial glow around `(cx, cy)`: opacity `(1 - d / radius)^2 * peak`.
fn draw_glow(canvas: &mut PixelCanvasBuffer, cx: f32, cy: f32, radius: f32, rgb: Rgb, peak: f32) {
    let x0 = (cx - radius).floor().max(0.0) as i32;
    let x1 = (cx + radius).ceil().min(canvas.width as f32 - 1.0) as i32;
    let y0 = (cy - radius).floor().max(0.0) as i32;
    let y1 = (cy + radius).ceil().min(canvas.height as f32 - 1.0) as i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            let f = 1.0 - (dx * dx + dy * dy).sqrt() / radius;
            if f > 0.0 {
                canvas.blend_pixel(x, y, rgb, f * f * peak);
            }
        }
    }
}

/// Glow colour of a node state; `None` for Idle and plain nodes (no glow).
fn glow_rgb(state: Option<AgentState>, colours: &GraphColours) -> Option<Rgb> {
    match state {
        Some(AgentState::Working) => Some(colours.working),
        Some(AgentState::Blocked) => Some(colours.blocked),
        Some(AgentState::Done) => Some(colours.done),
        Some(AgentState::Errored) => Some(colours.errored),
        Some(AgentState::Idle) | None => None,
    }
}

/// High-resolution GUI drawing of the graph onto an opaque canvas (D-10): anti-aliased curved
/// stroked edges, flow particles on active edges and a state-coloured glow per node.
///
/// `cell` is the pixel size of one terminal cell (hi-DPI aware); `tick` is the animation tick, `0`
/// being the deterministic static frame. Only `+ - * / sqrt` and polynomial Bezier evaluation are
/// used. Labels are not drawn: they stay crisp cell glyphs on top.
pub fn draw_agent_graph_hires(
    canvas: &mut PixelCanvasBuffer,
    scene: &GraphScene,
    colours: &GraphColours,
    tick: u64,
    cell: (f32, f32),
) {
    let (w, h) = (canvas.width as f32, canvas.height as f32);
    let px_of = |key: &crate::wm::agents_window::NodeKey| {
        scene
            .nodes
            .iter()
            .find(|n| &n.key == key)
            .map(|n| node_px_hires(n.pos, w, h, cell))
    };
    let sw = stroke_width_hires(cell);
    let phase = (tick % PARTICLE_PERIOD) as f32 / PARTICLE_PERIOD as f32;
    let mut ordered: Vec<&SceneEdge> = scene.edges.iter().collect();
    ordered.sort_by_key(|e| e.selected);
    let mut curves = Vec::with_capacity(ordered.len());
    for edge in ordered {
        let (Some(a), Some(b)) = (px_of(&edge.from), px_of(&edge.to)) else {
            continue;
        };
        let (c1, c2) = curve_controls(a, b);
        let chord = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let steps = ((chord / 3.0) as usize).clamp(12, 96);
        let pts: Vec<(f32, f32)> =
            (0..=steps).map(|i| bezier_point(a, c1, c2, b, i as f32 / steps as f32)).collect();
        let (width, alpha) = if edge.selected {
            (sw * 1.6, 1.0)
        } else if edge.active {
            (sw, 1.0)
        } else {
            (sw, IDLE_EDGE_ALPHA)
        };
        stroke_polyline(canvas, &pts, width, edge_rgb(edge, colours), alpha);
        curves.push((edge.active, a, c1, c2, b));
    }
    for &(active, a, c1, c2, b) in &curves {
        if !active {
            continue;
        }
        for k in 0..PARTICLES {
            let t = (phase + k as f32 / PARTICLES as f32).fract();
            let (x, y) = bezier_point(a, c1, c2, b, t);
            canvas.draw_circle_aa(x, y, sw * 1.2 + 0.5, true, colours.node);
        }
    }
    for node in &scene.nodes {
        let (x, y) = node_px_hires(node.pos, w, h, cell);
        let r = node_radius_hires(cell, node.selected);
        if let Some(rgb) = glow_rgb(node.state, colours) {
            draw_glow(canvas, x, y, r * GLOW_RADIUS_FACTOR, rgb, GLOW_PEAK);
        }
    }
    for node in &scene.nodes {
        let (x, y) = node_px_hires(node.pos, w, h, cell);
        let r = node_radius_hires(cell, node.selected);
        canvas.draw_circle_aa(x, y, r, true, colours.state_rgb(node.state));
        if node.selected {
            canvas.draw_circle_aa(x, y, r + 2.0, false, colours.accent);
        }
    }
}
