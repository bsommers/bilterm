//! Phase 50-11: GUI rendering of the agents graph and Navigator state icons (FLT-01, FLT-03, D-10, D-05).
//! `render_gui_` tests run on `GuiFrameSurface`; the cell glyph list proves labels stay crisp on top.

use bilterm::ui::agents_gui::GRAPH_TEXTURE_ID;
use bilterm::ui::renderer::{GuiFrameSurface, GuiImageTexture, GuiWorkspaceRenderer};
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::layout::Rect;

/// Two-pane headless fixture with traffic and the agents window open.
fn fixture(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(area).unwrap();
    wm.broadcast_command("echo hi").unwrap();
    wm.open_agents_window(None);
    (wm, area)
}

fn gui_frame_at(wm: &mut WindowManager, area: Rect, cw: u16, ch: u16) -> GuiFrameSurface {
    wm.set_gui_surface(true);
    let mut renderer = GuiWorkspaceRenderer::new(cw, ch);
    let mut surface = GuiFrameSurface::new(cw, ch);
    renderer.render_workspace_with_cell_size(wm, area, cw, ch, &mut surface);
    surface
}

fn gui_frame(wm: &mut WindowManager, area: Rect) -> GuiFrameSurface {
    gui_frame_at(wm, area, 8, 16)
}

fn graph_texture(surface: &GuiFrameSurface) -> Option<&GuiImageTexture> {
    surface.images.iter().find(|i| i.id == GRAPH_TEXTURE_ID)
}

fn is_braille(sym: &str) -> bool {
    sym.chars().next().is_some_and(|c| ('\u{2800}'..='\u{28FF}').contains(&c))
}

#[test]
fn render_gui_tracer_pushes_opaque_graph_texture() {
    let (mut wm, area) = fixture(120, 40);
    let rects = wm.agents_pane_rects(area).expect("window open");
    let surface = gui_frame(&mut wm, area);
    let img = graph_texture(&surface).expect("graph texture 950_000");

    let g = rects.graph;
    assert_eq!(img.x_px, u32::from(g.x) * 8);
    assert_eq!(img.y_px, u32::from(g.y) * 16);
    assert_eq!(img.width_px, u32::from(g.width) * 8);
    assert_eq!(img.height_px, u32::from(g.height) * 16);
    assert_eq!(u32::from(img.pixel_width), img.width_px, "1:1 horizontally");
    assert_eq!(u32::from(img.pixel_height), img.height_px, "1:1 vertically");
    assert_eq!(img.rgba_bytes.len(), (img.width_px * img.height_px * 4) as usize);
    assert!(img.rgba_bytes.chunks_exact(4).all(|p| p[3] == 255), "texture must be opaque");

    let in_graph = |x: f32, y: f32| {
        x >= img.x_px as f32
            && x < (img.x_px + img.width_px) as f32
            && y >= img.y_px as f32
            && y < (img.y_px + img.height_px) as f32
    };
    assert!(
        !surface.glyphs.iter().any(|gl| in_graph(gl.x_px, gl.y_px) && is_braille(&gl.text)),
        "no braille glyph may remain under the texture"
    );
    let labels: String = surface
        .glyphs
        .iter()
        .filter(|gl| in_graph(gl.x_px, gl.y_px))
        .map(|gl| gl.text.as_str())
        .collect();
    assert!(labels.contains("you"), "labels stay cell glyphs on top, got {labels:?}");
}
