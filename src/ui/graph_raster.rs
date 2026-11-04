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

/// Cell (relative to the graph rect) holding the label of `node`, with the label text.
fn label_cell(node: &SceneNode, rect: Rect) -> Option<(u16, u16, String)> {
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

/// Terminal sink: clears `rect`, writes node labels, then flushes the braille graph around them.
pub fn render_graph_tui(scene: &GraphScene, rect: Rect, buf: &mut Buffer, palette: &ThemePalette) {
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

    let bg = [colours.bg.0, colours.bg.1, colours.bg.2, 0];
    let mut canvas =
        PixelCanvasBuffer::new(u32::from(rect.width) * 2, u32::from(rect.height) * 4, bg);
    draw_agent_graph(&mut canvas, scene, &colours, 0);
    canvas.flush_truecolor_subcells_to_ratatui(CanvasSubcellMode::Braille, rect, buf);
}
