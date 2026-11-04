//! Phase 50-11: GUI rendering of the agents graph and Navigator state icons (FLT-01, FLT-03, D-10, D-05).
//! `render_gui_` tests run on `GuiFrameSurface`; the cell glyph list proves labels stay crisp on top.

use std::time::{Duration, Instant};

use bilterm::ui::agents_gui::{GRAPH_TEXTURE_ID, ICON_TEXTURE_BASE};
use bilterm::ui::drawer::navigator::navigator_dot_cells;
use bilterm::ui::graph_raster::{
    GraphColours, bezier_point, curve_controls, node_px_hires, node_radius_hires, stroke_width_hires,
};
use bilterm::ui::renderer::{GuiFrameSurface, GuiImageTexture, GuiWorkspaceRenderer};
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::agents_window::NodeKey;
use bilterm::wm::drawer::DrawerStyle;
use bilterm::wm::traffic::{TrafficEndpoint, TrafficKind};
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

/// 64-bit FNV-1a digest of the pixel bytes.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

/// Advances the shared animation tick (`spinner_tick`) by `n`.
fn advance_ticks(wm: &mut WindowManager, n: usize) {
    for _ in 0..n {
        wm.update_drawer_telemetry();
    }
}

fn colours(wm: &WindowManager) -> GraphColours {
    GraphColours::from_palette(&wm.config().palette())
}

fn node_center(wm: &WindowManager, img: &GuiImageTexture, key: &NodeKey, cell: (f32, f32)) -> (f32, f32) {
    let scene = wm.agents_graph_scene();
    let node = scene.nodes.iter().find(|n| &n.key == key).expect("node in scene");
    node_px_hires(node.pos, img.width_px as f32, img.height_px as f32, cell)
}

fn pixel(img: &GuiImageTexture, x: i32, y: i32) -> [u8; 3] {
    let (x, y) = (x.clamp(0, img.width_px as i32 - 1) as usize, y.clamp(0, img.height_px as i32 - 1) as usize);
    let i = (y * img.width_px as usize + x) * 4;
    [img.rgba_bytes[i], img.rgba_bytes[i + 1], img.rgba_bytes[i + 2]]
}

fn l1(a: [u8; 3], b: (u8, u8, u8)) -> i32 {
    (i32::from(a[0]) - i32::from(b.0)).abs() + (i32::from(a[1]) - i32::from(b.1)).abs() + (i32::from(a[2]) - i32::from(b.2)).abs()
}

/// Three panes: an old edge `#1-#2` (inactive) and a fresh active edge `#1-#3`.
fn three_pane_traffic() -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(40, 120);
    let area = Rect::new(0, 0, 120, 40);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.resize_all(area).unwrap();
    let now = Instant::now();
    wm.record_traffic(TrafficEndpoint::Pane(1), TrafficEndpoint::Pane(2), TrafficKind::Broadcast, "old", now);
    for _ in 0..3 {
        wm.record_traffic(TrafficEndpoint::Pane(1), TrafficEndpoint::Pane(3), TrafficKind::Broadcast, "new", now);
    }
    wm.open_agents_window(None);
    (wm, area)
}

#[test]
fn render_gui_static_frame_deterministic() {
    let (mut wm, area) = fixture(120, 40);
    let a = gui_frame(&mut wm, area);
    let b = gui_frame(&mut wm, area);
    let (ia, ib) = (graph_texture(&a).unwrap(), graph_texture(&b).unwrap());
    assert_eq!(ia.rgba_bytes, ib.rgba_bytes, "two renders at tick 0 must be byte identical");
    insta::assert_snapshot!(
        "render_gui_static_frame",
        format!("{}x{} rgba-digest={:016x}", ia.width_px, ia.height_px, fnv1a(&ia.rgba_bytes))
    );
}

#[test]
fn render_gui_curved_edges_antialiased() {
    let (mut wm, area) = fixture(120, 40);
    let surface = gui_frame(&mut wm, area);
    let img = graph_texture(&surface).unwrap();
    let cols = colours(&wm);
    let cell = (8.0, 16.0);
    let scene = wm.agents_graph_scene();
    let edge = scene.edges.iter().find(|e| e.active).expect("active edge after a broadcast");
    let center = |k: &NodeKey| node_center(&wm, img, k, cell);
    let (a, b) = (center(&edge.from), center(&edge.to));
    let (c1, c2) = curve_controls(a, b);

    // The curve bows away from the straight chord.
    let mid = bezier_point(a, c1, c2, b, 0.5);
    let chord_mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let off = ((mid.0 - chord_mid.0).powi(2) + (mid.1 - chord_mid.1).powi(2)).sqrt();
    assert!(off > 2.0, "edge midpoint must be offset from the chord, got {off}");

    // Along the curve there are anti-aliased pixels strictly between background and edge colour.
    let (bg, fg) = (cols.bg, cols.edge_active);
    let between = |p: [u8; 3]| {
        (0..3).all(|c| {
            let (lo, hi) = ([bg.0, bg.1, bg.2][c].min([fg.0, fg.1, fg.2][c]), [bg.0, bg.1, bg.2][c].max([fg.0, fg.1, fg.2][c]));
            p[c] >= lo && p[c] <= hi
        }) && l1(p, bg) > 0
            && l1(p, fg) > 0
    };
    let sw = stroke_width_hires(cell);
    let mut aa = 0;
    for i in 0..=40 {
        let (x, y) = bezier_point(a, c1, c2, b, i as f32 / 40.0);
        for d in -(sw as i32 + 2)..=(sw as i32 + 2) {
            aa += usize::from(between(pixel(img, x as i32 + d, y as i32))) + usize::from(between(pixel(img, x as i32, y as i32 + d)));
        }
    }
    assert!(aa >= 8, "expected anti-aliased edge pixels, found {aa}");
    // The curve centre is solidly the edge colour (the chord midpoint is not).
    let (mx, my) = (mid.0.round() as i32, mid.1.round() as i32);
    let nearest = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| l1(pixel(img, mx + dx, my + dy), fg)).min().unwrap();
    assert!(nearest < 45, "no edge-coloured pixel at the curve midpoint (closest L1 distance {nearest})");
    assert!(l1(pixel(img, chord_mid.0.round() as i32, chord_mid.1.round() as i32), bg) < 20, "the chord midpoint is off the curve");
}

#[test]
fn render_gui_particles_move_with_tick() {
    let (mut wm, area) = three_pane_traffic();
    let t0 = gui_frame(&mut wm, area);
    advance_ticks(&mut wm, 7);
    let t7 = gui_frame(&mut wm, area);
    let (i0, i7) = (graph_texture(&t0).unwrap(), graph_texture(&t7).unwrap());
    assert_eq!(i0.rgba_bytes.len(), i7.rgba_bytes.len());

    let cell = (8.0, 16.0);
    let scene = wm.agents_graph_scene();
    let active: Vec<_> = scene.edges.iter().filter(|e| e.active).collect();
    assert_eq!(active.len(), 1, "exactly the fresh edge is active");
    let e = active[0];
    let center = |k: &NodeKey| node_center(&wm, i0, k, cell);
    let (a, b) = (center(&e.from), center(&e.to));
    let (c1, c2) = curve_controls(a, b);
    let near: Vec<(f32, f32)> = (0..=200).map(|i| bezier_point(a, c1, c2, b, i as f32 / 200.0)).collect();
    let reach = stroke_width_hires(cell) * 1.2 + 4.0;

    let mut diff = 0;
    for y in 0..i0.height_px as i32 {
        for x in 0..i0.width_px as i32 {
            if pixel(i0, x, y) != pixel(i7, x, y) {
                diff += 1;
                let close = near.iter().any(|p| (p.0 - x as f32).powi(2) + (p.1 - y as f32).powi(2) <= reach * reach);
                assert!(close, "pixel ({x},{y}) changed away from the active edge");
            }
        }
    }
    assert!(diff > 0, "particles must move between tick 0 and tick 7");
}

#[test]
fn render_gui_node_glow_state_colour() {
    let (mut wm, area) = fixture(120, 40);
    wm.report_agent_state(1, Some(AgentState::Blocked));
    wm.report_agent_state(2, Some(AgentState::Idle));
    wm.update_agent_states(Instant::now());
    let surface = gui_frame(&mut wm, area);
    let img = graph_texture(&surface).unwrap();
    let cols = colours(&wm);
    let cell = (8.0, 16.0);
    let r = node_radius_hires(cell, false);
    let ring = |key: &NodeKey| {
        let (cx, cy) = node_center(&wm, img, key, cell);
        (0..8)
            .map(|k| {
                let (dx, dy) = [(1.0, 0.0), (0.7, 0.7), (0.0, 1.0), (-0.7, 0.7), (-1.0, 0.0), (-0.7, -0.7), (0.0, -1.0), (0.7, -0.7)][k];
                pixel(img, (cx + dx * r * 1.4).round() as i32, (cy + dy * r * 1.4).round() as i32)
            })
            .collect::<Vec<_>>()
    };
    let blocked = ring(&NodeKey::pane(1));
    let toward = |p: &[u8; 3], rgb: (u8, u8, u8)| {
        let (b, w) = ([cols.bg.0, cols.bg.1, cols.bg.2], [rgb.0, rgb.1, rgb.2]);
        l1(*p, cols.bg) > 0 && (0..3).map(|c| (i32::from(p[c]) - i32::from(b[c])) * (i32::from(w[c]) - i32::from(b[c]))).sum::<i32>() > 0
    };
    let tinted = blocked.iter().filter(|p| toward(p, cols.blocked)).count();
    assert!(tinted >= 6, "blocked glow must tint the ring toward the warning colour ({tinted}/8)");
    let idle = ring(&NodeKey::pane(2));
    let bare = idle.iter().filter(|p| l1(**p, cols.bg) == 0).count();
    assert!(bare >= 6, "an idle node has no glow ({bare}/8 background pixels)");
}

#[test]
fn render_gui_hidpi_cell_size() {
    let (mut wm, area) = fixture(120, 40);
    let small = gui_frame_at(&mut wm, area, 8, 16);
    let big = gui_frame_at(&mut wm, area, 16, 32);
    let (s, b) = (graph_texture(&small).unwrap(), graph_texture(&big).unwrap());
    assert_eq!(b.width_px, s.width_px * 2);
    assert_eq!(b.height_px, s.height_px * 2);
    assert_eq!(u32::from(b.pixel_width), b.width_px, "stays 1:1 at hi-DPI");
    assert_eq!(u32::from(b.pixel_height), b.height_px);
    assert_eq!(b.x_px, s.x_px * 2);
}

#[test]
fn render_gui_scaled_window_texture_matches_scaled_rect() {
    let (mut wm, area) = fixture(120, 40);
    wm.set_gui_surface(true);
    wm.set_panel_scale_pct(150);
    let vws = wm.panel_virtual_area(area, bilterm::ui::typography::PanelTarget::Agents);
    let g = wm.agents_pane_rects(vws).expect("scaled window").graph;
    let surface = gui_frame(&mut wm, area);
    let img = graph_texture(&surface).expect("scaled graph texture");
    let (x0, x1) = ((f32::from(g.x) * 12.0).round() as u32, (f32::from(g.x + g.width) * 12.0).round() as u32);
    let (y0, y1) = ((f32::from(g.y) * 24.0).round() as u32, (f32::from(g.y + g.height) * 24.0).round() as u32);
    assert_eq!((img.x_px, img.y_px), (x0, y0));
    assert_eq!((img.width_px, img.height_px), (x1 - x0, y1 - y0));
    assert_eq!(u32::from(img.pixel_width), img.width_px);
    assert!(img.rgba_bytes.chunks_exact(4).all(|p| p[3] == 255));
    assert!(
        !surface.glyphs.iter().any(|gl| gl.x_px >= x0 as f32 && gl.x_px < x1 as f32 && gl.y_px >= y0 as f32 && gl.y_px < y1 as f32 && is_braille(&gl.text)),
        "no braille under the scaled texture"
    );
}

#[test]
fn render_gui_texture_size_capped() {
    let (mut wm, area) = fixture(120, 40);
    // 80000 px wide: over u16::MAX on one axis.
    let wide = gui_frame_at(&mut wm, area, 2000, 16);
    assert!(graph_texture(&wide).is_none(), "an axis above u16::MAX gets no texture");
    // Both axes fit u16 but the area is far above the pixel budget.
    let huge = gui_frame_at(&mut wm, area, 1500, 1500);
    assert!(graph_texture(&huge).is_none(), "an over-budget texture is skipped, not allocated");
    for img in wide.images.iter().chain(huge.images.iter()) {
        assert_eq!(img.rgba_bytes.len() as u64, u64::from(img.pixel_width) * u64::from(img.pixel_height) * 4);
    }
}

#[test]
fn render_gui_tick_redraws_while_edges_active() {
    let mut wm = WindowManager::new_headless(40, 120);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    let t0 = Instant::now();
    let _ = wm.tick_agent_ui(t0);
    wm.open_agents_window(None);
    assert!(!wm.tick_agent_ui(t0), "open window without traffic: no redraws");

    wm.record_traffic(TrafficEndpoint::Pane(1), TrafficEndpoint::Pane(2), TrafficKind::Broadcast, "x", t0);
    assert!(wm.tick_agent_ui(t0), "new traffic redraws");
    assert!(!wm.tick_agent_ui(t0 + Duration::from_millis(50)), "throttled inside 120 ms");
    assert!(wm.tick_agent_ui(t0 + Duration::from_millis(130)), "edge still flowing: animation frame");
    assert!(!wm.tick_agent_ui(t0 + Duration::from_millis(140)), "throttled again");
    assert!(wm.tick_agent_ui(t0 + Duration::from_millis(1900)), "still inside the 2 s flow window");
    assert!(!wm.tick_agent_ui(t0 + Duration::from_millis(2300)), "idle edges stop requesting redraws");
}

const STATES: [AgentState; 5] =
    [AgentState::Working, AgentState::Blocked, AgentState::Idle, AgentState::Done, AgentState::Errored];

/// Five panes reporting one state each, Navigator drawer open (no agents window).
fn navigator_wm() -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(40, 120);
    let area = Rect::new(0, 0, 120, 40);
    for _ in 0..4 {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(area).unwrap();
    wm.config_mut().drawer.style = DrawerStyle::Navigator;
    wm.toggle_drawer();
    for (i, st) in STATES.iter().enumerate() {
        wm.report_agent_state(i + 1, Some(*st));
    }
    wm.update_agent_states(Instant::now());
    (wm, area)
}

fn workspace_of(wm: &WindowManager, area: Rect) -> Rect {
    bilterm::ui::shell::ShellLayout::compute(area, wm.config(), wm.pane_count()).workspace
}

fn icon_images(surface: &GuiFrameSurface) -> Vec<&GuiImageTexture> {
    surface.images.iter().filter(|i| (ICON_TEXTURE_BASE..ICON_TEXTURE_BASE + 10_000).contains(&i.id)).collect()
}

#[test]
fn render_gui_navigator_icons_pushed() {
    let (mut wm, area) = navigator_wm();
    let ws = workspace_of(&wm, area);
    let dots = navigator_dot_cells(&wm, ws);
    assert_eq!(dots.len(), 5, "five reported states -> five dot cells");
    let surface = gui_frame(&mut wm, area);
    let icons = icon_images(&surface);
    assert_eq!(icons.len(), 5);
    for dot in &dots {
        let img = icons.iter().find(|i| i.id == ICON_TEXTURE_BASE + dot.row_index as u32).expect("icon for row");
        assert_eq!((img.x_px, img.y_px), (u32::from(dot.x) * 8, u32::from(dot.y) * 16));
        assert_eq!((img.width_px, img.height_px), (8, 16));
        assert_eq!((u32::from(img.pixel_width), u32::from(img.pixel_height)), (8, 16), "1:1");
        assert!(img.rgba_bytes.chunks_exact(4).all(|p| p[3] == 255), "icon must be opaque");
        assert!(
            !surface.glyphs.iter().any(|g| g.x_px == img.x_px as f32 && g.y_px == img.y_px as f32),
            "the dot cell must be cleared in the glyph list"
        );
    }
}

fn icon_by_state(wm: &WindowManager, area: Rect, surface: &GuiFrameSurface) -> Vec<(AgentState, Vec<u8>)> {
    let dots = navigator_dot_cells(wm, workspace_of(wm, area));
    icon_images(surface)
        .iter()
        .map(|img| {
            let dot = dots.iter().find(|d| ICON_TEXTURE_BASE + d.row_index as u32 == img.id).unwrap();
            (dot.state, img.rgba_bytes.clone())
        })
        .collect()
}

#[test]
fn render_gui_navigator_icon_shapes_differ_by_state() {
    let (mut wm, area) = navigator_wm();
    let surface = gui_frame(&mut wm, area);
    let icons = icon_by_state(&wm, area, &surface);
    assert_eq!(icons.len(), 5);
    for i in 0..icons.len() {
        for j in i + 1..icons.len() {
            assert_ne!(icons[i].1, icons[j].1, "{:?} and {:?} icons must differ", icons[i].0, icons[j].0);
        }
    }
}

#[test]
fn render_gui_navigator_icons_animate_working_only() {
    let (mut wm, area) = navigator_wm();
    let f0 = gui_frame(&mut wm, area);
    let a = icon_by_state(&wm, area, &f0);
    advance_ticks(&mut wm, 6);
    let f6 = gui_frame(&mut wm, area);
    let b = icon_by_state(&wm, area, &f6);
    for (state, bytes) in &a {
        let after = &b.iter().find(|(s, _)| s == state).unwrap().1;
        let animated = matches!(state, AgentState::Working | AgentState::Blocked);
        assert_eq!(bytes != after, animated, "{state:?}: animated={animated}");
    }
}

#[test]
fn render_gui_classic_style_pushes_no_icons() {
    let (mut wm, area) = navigator_wm();
    wm.config_mut().drawer.style = DrawerStyle::Classic;
    let surface = gui_frame(&mut wm, area);
    assert!(icon_images(&surface).is_empty(), "classic drawer must not get icon textures");
}

#[test]
fn render_tui_path_pushes_no_textures() {
    use ratatui::buffer::Buffer;
    use ratatui::widgets::Widget;
    let (wm, area) = navigator_wm();
    let mut buf = Buffer::empty(area);
    bilterm::ui::WorkspaceWidget::new(&wm).render(area, &mut buf);
    let dots = navigator_dot_cells(&wm, workspace_of(&wm, area));
    assert_eq!(dots.len(), 5);
    for dot in &dots {
        let sym = buf[(dot.x, dot.y)].symbol();
        assert_ne!(sym, " ", "TUI keeps the text dot for {:?}", dot.state);
    }
    let working = dots.iter().find(|d| d.state == AgentState::Working).unwrap();
    assert!(is_braille(buf[(working.x, working.y)].symbol()), "Working keeps its braille spinner in the terminal");
}

#[test]
fn render_gui_scaled_drawer_icons_follow_panel_scale() {
    let (mut wm, area) = navigator_wm();
    wm.set_gui_surface(true);
    wm.set_panel_scale_pct(150);
    let surface = gui_frame(&mut wm, area);
    let icons = icon_images(&surface);
    assert_eq!(icons.len(), 5, "scaled drawer still gets one icon per dot");
    for img in icons {
        assert_eq!((img.width_px, img.height_px), (12, 24), "icon cell is 150% of 8x16");
        assert_eq!((u32::from(img.pixel_width), u32::from(img.pixel_height)), (12, 24));
        assert!(img.rgba_bytes.chunks_exact(4).all(|p| p[3] == 255));
    }
}
