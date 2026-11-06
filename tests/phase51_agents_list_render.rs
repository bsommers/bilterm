//! Phase 51-06: rendering of the AGENTS list window (UIT-03). `render_` tests cover the TUI cell
//! path (explicit text and style assertions, no snapshots) and the GUI scaled-panel path.

use std::time::Instant;

use bilterm::ui::WorkspaceWidget;
use bilterm::ui::agents_gui::{GRAPH_TEXTURE_ID, ICON_TEXTURE_BASE};
use bilterm::ui::design::DesignTokens;
use bilterm::ui::drawer::navigator::{dot_color, navigator_dot_cells};
use bilterm::ui::glyphs::GlyphPack;
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};
use bilterm::ui::typography::{PanelTarget, scaled_panel_real_rect};
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::agents_list_cmd::ListFilter;
use bilterm::wm::agents_list_geometry::{ListBadge, ListChip};
use bilterm::wm::drawer::DrawerStyle;
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::Widget;

/// Headless workspace with `panes` panes (alternating splits) in a `cols` x `rows` area.
fn fixture_n(cols: u16, rows: u16, panes: usize) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    for i in 1..panes {
        let dir = if i % 2 == 1 { SplitDirection::Vertical } else { SplitDirection::Horizontal };
        wm.split_focused(dir).unwrap();
    }
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn fixture(cols: u16, rows: u16) -> (WindowManager, Rect) {
    fixture_n(cols, rows, 4)
}

fn render(wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

fn line(buf: &Buffer, r: Rect, y: u16) -> String {
    (r.x..r.x + r.width).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

fn all_text(buf: &Buffer) -> String {
    let a = buf.area;
    (a.y..a.y + a.height).map(|y| line(buf, a, y)).collect::<Vec<_>>().join("\n")
}

fn tokens(wm: &WindowManager) -> DesignTokens {
    DesignTokens::from_palette(&wm.config().palette())
}

fn report(wm: &mut WindowManager, states: &[(usize, AgentState)]) {
    for (id, st) in states {
        wm.report_agent_state(*id, Some(*st));
    }
    wm.update_agent_states(Instant::now());
}

/// Cells of `buf` outside `r` grown by `margin` that differ between the two buffers.
fn diff_outside(a: &Buffer, b: &Buffer, r: Rect, margin: u16) -> usize {
    let grown = Rect::new(
        r.x.saturating_sub(margin),
        r.y.saturating_sub(margin),
        r.width + 2 * margin,
        r.height + 2 * margin,
    );
    let area = a.area;
    (area.y..area.y + area.height)
        .flat_map(|y| (area.x..area.x + area.width).map(move |x| (x, y)))
        .filter(|&(x, y)| !grown.contains((x, y).into()) && a[(x, y)] != b[(x, y)])
        .count()
}

#[test]
fn render_rows_and_states() {
    let (mut wm, area) = fixture(120, 40);
    report(
        &mut wm,
        &[(1, AgentState::Working), (2, AgentState::Blocked), (3, AgentState::Idle), (4, AgentState::Done)],
    );
    wm.open_agents_list();
    let l = wm.agents_list_layout(area).expect("layout");
    let buf = render(&wm, area);
    let t = tokens(&wm);
    let expect = [
        (1, AgentState::Working, "working"),
        (2, AgentState::Blocked, "blocked"),
        (3, AgentState::Idle, "idle"),
        (4, AgentState::Done, "done"),
    ];
    for (i, (id, st, word)) in expect.iter().enumerate() {
        let y = l.rows_area.y + i as u16;
        let text = line(&buf, l.rows_area, y);
        assert!(text.contains(word), "row {i} missing state word {word}: {text:?}");
        assert!(text.contains(&format!("#{id}")), "row {i} missing #{id}: {text:?}");
        assert!(text.contains("0s"), "row {i} missing age text: {text:?}");
        assert_eq!(buf[(l.rows_area.x + 1, y)].fg, dot_color(*st, &t), "dot colour of row {i}");
    }
    // Pane-id order: the `#id` column grows down the window.
    let ids: Vec<usize> = (0..4)
        .map(|i| {
            let text = line(&buf, l.rows_area, l.rows_area.y + i);
            text.rsplit('#').next().unwrap().split_whitespace().next().unwrap().parse().unwrap()
        })
        .collect();
    assert_eq!(ids, vec![1, 2, 3, 4]);
    let header = line(&buf, l.header, l.header.y);
    for col in ["STATE", "NAME", "PANE", "SINCE"] {
        assert!(header.contains(col), "header missing {col}: {header:?}");
    }
    assert!(line(&buf, l.footer, l.footer.y).contains("4/4"), "footer shows the shown/total count");
}

#[test]
fn render_selection_and_active_marker() {
    let (mut wm, area) = fixture(120, 40);
    wm.open_agents_list();
    wm.agents_list_mut().selected = Some(2);
    let l = wm.agents_list_layout(area).unwrap();
    let buf = render(&wm, area);
    let reversed = |buf: &Buffer, i: u16| {
        buf[(l.rows_area.x + 4, l.rows_area.y + i)].modifier.contains(Modifier::REVERSED)
    };
    assert!(reversed(&buf, 1), "selected row is reverse video");
    assert!(!reversed(&buf, 0) && !reversed(&buf, 2), "other rows are not");
    wm.agents_list_mut().selected = Some(3);
    let buf = render(&wm, area);
    assert!(reversed(&buf, 2) && !reversed(&buf, 1), "the highlight follows the selection");

    let active = wm.active_pane_id();
    for (i, row) in wm.agents_list_rows().iter().enumerate() {
        let sym = buf[(l.rows_area.x, l.rows_area.y + i as u16)].symbol().to_string();
        if row.pane_id == active {
            assert_eq!(sym, "›", "active pane row carries the marker");
        } else {
            assert_eq!(sym, " ", "other rows carry none");
        }
    }
}

#[test]
fn render_focus_and_pin_visuals() {
    let (mut wm, area) = fixture(120, 40);
    wm.open_agents_list();
    let l = wm.agents_list_layout(area).unwrap();
    let focused = render(&wm, area);
    wm.agents_list_mut().blur();
    let unfocused = render(&wm, area);
    assert_ne!(focused, unfocused, "focus must be visible");
    let corner = (l.outer.x, l.outer.y + 1);
    assert_ne!(focused[corner].fg, unfocused[corner].fg, "border colour differs with focus");

    let pin = l.badges.iter().find(|(b, _)| *b == ListBadge::Pin).expect("pin badge").1;
    assert_eq!(unfocused[(pin.x, pin.y)].symbol(), "◇");
    wm.agents_list_mut().pinned = true;
    let pinned = render(&wm, area);
    assert_eq!(pinned[(pin.x, pin.y)].symbol(), "◆");
    let diffs: Vec<(u16, u16)> = (area.y..area.y + area.height)
        .flat_map(|y| (area.x..area.x + area.width).map(move |x| (x, y)))
        .filter(|&(x, y)| pinned[(x, y)] != unfocused[(x, y)])
        .collect();
    assert_eq!(diffs, vec![(pin.x, pin.y)], "pinned and unpinned differ by the pin badge only");

    wm.config_mut().glyphs = GlyphPack::Ascii;
    assert_eq!(render(&wm, area)[(pin.x, pin.y)].symbol(), "*");
    wm.agents_list_mut().pinned = false;
    assert_eq!(render(&wm, area)[(pin.x, pin.y)].symbol(), "o");

    // Trio badges sit at the geometry's cells.
    let g = wm.config().glyph_set();
    let buf = render(&wm, area);
    for (b, want) in [
        (ListBadge::Minimize, g.minimize_badge),
        (ListBadge::Dock, g.zoom_badge),
        (ListBadge::Close, g.close_badge),
    ] {
        let r = l.badges.iter().find(|(k, _)| *k == b).unwrap().1;
        assert_eq!(line(&buf, r, r.y), want, "badge {b:?}");
    }
}

#[test]
fn render_footer_chips_at_geometry_rects() {
    let (mut wm, area) = fixture(120, 40);
    wm.open_agents_list();
    let l = wm.agents_list_layout(area).unwrap();
    let buf = render(&wm, area);
    assert_eq!(l.chips.len(), 3);
    for ((chip, r), want) in l.chips.iter().zip(["[filter]", "[dock]", "[zoom]"]) {
        assert_eq!(line(&buf, *r, r.y), want, "chip {chip:?}");
    }
    assert_eq!(
        l.chips.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        vec![ListChip::Filter, ListChip::Dock, ListChip::Zoom]
    );

    // Too narrow for the chips: none drawn, nothing outside the window changes.
    let closed = {
        let mut other = fixture(120, 40).0;
        other.open_agents_list();
        other.close_agents_list();
        render(&other, area)
    };
    wm.agents_list_mut().rect = Some(Rect::new(5, 3, 20, 8));
    let l = wm.agents_list_layout(area).unwrap();
    assert!(l.chips.is_empty());
    let buf = render(&wm, area);
    let text = all_text(&buf);
    assert!(!text.contains("[filter]") && !text.contains("[zoom]"), "no chips in a narrow footer");
    assert_eq!(diff_outside(&buf, &closed, l.outer, 3), 0, "nothing drawn outside the window");
}

#[test]
fn render_scroll_and_empty() {
    let (mut wm, area) = fixture_n(200, 60, 12);
    wm.open_agents_list();
    wm.agents_list_mut().rect = Some(Rect::new(10, 5, 36, 8));
    let l = wm.agents_list_layout(area).unwrap();
    assert!(l.visible_rows < 12 && l.visible_rows >= 3);
    let vis = l.visible_rows as u16;
    let x = l.rows_area.x + l.rows_area.width - 1;
    let buf = render(&wm, area);
    assert_eq!(buf[(x, l.rows_area.y)].symbol(), "█", "thumb at the top, no up arrow at scroll 0");
    assert_eq!(buf[(x, l.rows_area.y + vis - 1)].symbol(), "▼", "more rows below");

    wm.agents_list_mut().scroll = 3;
    let buf = render(&wm, area);
    for i in 0..vis {
        let text = line(&buf, l.rows_area, l.rows_area.y + i);
        assert!(text.contains(&format!("#{}", 4 + i)), "row {i} shows pane {}: {text:?}", 4 + i);
        assert_ne!(buf[(x, l.rows_area.y + i)].symbol(), " ", "scroll indicator column");
    }
    assert_eq!(buf[(x, l.rows_area.y)].symbol(), "▲", "rows hidden above");
    let shown: Vec<String> = (0..vis).map(|i| line(&buf, l.rows_area, l.rows_area.y + i)).collect();
    assert!(!shown.iter().any(|s| s.contains("#3 ") || s.contains("#2 ")), "rows above scroll are not drawn");

    // Filter on detected agents with none detected: the empty-state line.
    wm.agents_list_mut().scroll = 0;
    wm.agents_list_mut().filter = ListFilter::Agents;
    let l = wm.agents_list_layout(area).unwrap();
    let buf = render(&wm, area);
    assert!(line(&buf, l.rows_area, l.rows_area.y).contains("No agents detected"));
    assert!(line(&buf, l.footer, l.footer.y).contains("0/12"));
}

#[test]
fn render_sanitises_names() {
    let (mut wm, area) = fixture(120, 40);
    wm.pane_mut(1).unwrap().set_title("ev\u{1b}[31mil\u{202e}x\u{200b}y");
    wm.update_agent_states(Instant::now());
    wm.open_agents_list();
    let outer = wm.agents_list_layout(area).unwrap().outer;
    let buf = render(&wm, area);
    // Only the list window: pane frames draw their own title through another path.
    let text = (outer.y..outer.bottom()).map(|y| line(&buf, outer, y)).collect::<Vec<_>>().join("\n");
    assert!(text.contains("milxy"), "sanitised name is drawn:\n{text}");
    for bad in ['\u{1b}', '\u{202e}', '\u{200b}', '\u{7}'] {
        assert!(!text.contains(bad), "control or bidi character {bad:?} reached the screen");
    }
}

#[test]
fn render_tiny_workspaces_do_not_panic() {
    for (cols, rows) in [(24u16, 6u16), (30, 8), (80, 24)] {
        let (mut wm, area) = fixture_n(cols, rows, 3);
        let base = render(&wm, area);
        wm.open_agents_list();
        for rect in [None, Some(Rect::new(0, 0, 1, 1)), Some(Rect::new(1000, 1000, 500, 500))] {
            wm.agents_list_mut().rect = rect;
            let buf = render(&wm, area);
            if let Some(l) = wm.agents_list_layout(area) {
                assert!(area.contains(l.outer.as_position()), "{cols}x{rows}: window starts inside the workspace");
                assert!(l.outer.right() <= area.right() && l.outer.bottom() <= area.bottom());
                assert_eq!(diff_outside(&buf, &base, l.outer, 3), 0, "{cols}x{rows}: stays inside its rect");
            }
        }
    }
}

#[test]
fn render_above_traffic_window() {
    let (mut wm, area) = fixture(120, 40);
    wm.open_agents_window(None);
    let tr = wm.agents_window_rect(area).expect("traffic rect");
    wm.open_agents_list();
    wm.agents_list_mut().rect = Some(Rect::new(tr.x + 2, tr.y + 2, 30, 10));
    let l = wm.agents_list_layout(area).unwrap();
    assert!(tr.intersects(l.outer), "the windows overlap");
    let both = render(&wm, area);
    wm.close_agents_window();
    let only_list = render(&wm, area);
    for y in l.outer.y..l.outer.y + l.outer.height {
        assert_eq!(
            line(&both, l.outer, y),
            line(&only_list, l.outer, y),
            "overlap row {y} belongs to the list"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// GUI path
// ---------------------------------------------------------------------------------------------

fn gui_frame(wm: &mut WindowManager, area: Rect) -> GuiFrameSurface {
    wm.set_gui_surface(true);
    let mut renderer = GuiWorkspaceRenderer::new(8, 16);
    let mut surface = GuiFrameSurface::new(8, 16);
    renderer.render_workspace_with_cell_size(wm, area, 8, 16, &mut surface);
    surface
}

fn workspace_of(wm: &WindowManager, area: Rect) -> Rect {
    bilterm::ui::shell::ShellLayout::compute(area, wm.config(), wm.pane_count()).workspace
}

fn digest(s: &GuiFrameSurface) -> u64 {
    let text = format!("{:?}|{:?}|{:?}", s.quads, s.glyphs, s.images);
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

/// Pixel box `(x0, y0, x1, y1)` of a cell rect drawn at 8x16 px cells.
fn px_box(r: Rect) -> (u32, u32, u32, u32) {
    (u32::from(r.x) * 8, u32::from(r.y) * 16, u32::from(r.right()) * 8, u32::from(r.bottom()) * 16)
}

fn overlaps(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

#[test]
fn render_gui_list_at_100_matches_tui_cells() {
    let (mut wm, area) = fixture(120, 40);
    wm.open_agents_list();
    let ws = workspace_of(&wm, area);
    let rect = wm.agents_list_rect(ws).unwrap();
    let surface = gui_frame(&mut wm, area);
    let inside: Vec<_> = surface
        .glyphs
        .iter()
        .filter(|g| g.col >= rect.x && g.col < rect.right() && g.row >= rect.y && g.row < rect.bottom())
        .collect();
    let mut title: Vec<_> = inside.iter().filter(|g| g.row == rect.y).collect();
    title.sort_by_key(|g| g.col);
    let title: String = title.iter().map(|g| g.text.as_str()).collect();
    assert!(title.contains("AGENTS"), "title glyphs in the list rect: {title:?}");
    let mut cells: Vec<(u16, u16)> = inside.iter().map(|g| (g.row, g.col)).collect();
    let n = cells.len();
    cells.sort_unstable();
    cells.dedup();
    assert_eq!(cells.len(), n, "no pane glyph under a list cell");
    assert!(inside.iter().all(|g| g.cell_width_px == 8), "100% scale: normal cells");
}

#[test]
fn render_gui_scaled_list_wider_cells() {
    let (mut wm, area) = fixture(120, 40);
    wm.open_agents_list();
    wm.set_gui_surface(true);
    wm.set_scale(PanelTarget::AgentsList, 150);
    let ws = workspace_of(&wm, area);
    let vws = wm.panel_virtual_area(ws, PanelTarget::AgentsList);
    let vr = wm.agents_list_rect(vws).expect("virtual rect");
    let real = scaled_panel_real_rect(ws, vr, 150);
    assert!(!all_text(&render(&wm, area)).contains("AGENTS"), "no unscaled copy: that would be a second window");

    let surface = gui_frame(&mut wm, area);
    let b = px_box(real);
    let inside: Vec<_> = surface
        .glyphs
        .iter()
        .filter(|g| {
            let cx = g.x_px + f32::from(g.cell_width_px) / 2.0;
            let cy = g.y_px + f32::from(g.cell_height_px) / 2.0;
            cx >= b.0 as f32 && cx < b.2 as f32 && cy >= b.1 as f32 && cy < b.3 as f32
        })
        .collect();
    assert!(!inside.is_empty());
    assert!(inside.iter().all(|g| g.cell_width_px == 12), "every glyph there is a 150% cell (panes masked)");
    assert!(inside.iter().any(|g| g.text == "A"), "the AGENTS title is drawn");
}

/// Graph texture of the traffic window overlapped by the list.
#[test]
fn render_gui_textures_do_not_cover_list() {
    let (mut wm, area) = fixture(120, 40);
    wm.broadcast_command("echo hi").unwrap();
    wm.open_agents_window(None);
    let ws = workspace_of(&wm, area);
    let graph = wm.agents_pane_rects(ws).expect("traffic window").graph;
    let base = gui_frame(&mut wm, area);
    assert!(base.images.iter().any(|i| i.id == GRAPH_TEXTURE_ID), "baseline has the graph texture");

    wm.open_agents_list();
    wm.agents_list_mut().rect = Some(Rect::new(graph.x + 1, graph.y + 1, 30, 10));
    let list = px_box(wm.agents_list_rect(ws).unwrap());
    let surface = gui_frame(&mut wm, area);
    for img in surface.images.iter().filter(|i| i.id == GRAPH_TEXTURE_ID) {
        let b = (img.x_px, img.y_px, img.x_px + img.width_px, img.y_px + img.height_px);
        assert!(!overlaps(b, list), "graph texture painted over the list");
    }

    // Navigator icons.
    let (mut wm, area) = fixture(120, 40);
    wm.config_mut().drawer.style = DrawerStyle::Navigator;
    wm.toggle_drawer();
    report(&mut wm, &[(1, AgentState::Working), (2, AgentState::Idle), (3, AgentState::Done), (4, AgentState::Blocked)]);
    let ws = workspace_of(&wm, area);
    let dots = navigator_dot_cells(&wm, ws);
    assert!(dots.len() >= 4);
    let icons = |s: &GuiFrameSurface| s.images.iter().filter(|i| (ICON_TEXTURE_BASE..ICON_TEXTURE_BASE + 10_000).contains(&i.id)).count();
    let before = icons(&gui_frame(&mut wm, area));
    assert_eq!(before, dots.len());
    wm.open_agents_list();
    wm.agents_list_mut().rect = Some(Rect::new(dots[0].x, dots[0].y, 30, 10));
    let list = px_box(wm.agents_list_rect(ws).unwrap());
    let surface = gui_frame(&mut wm, area);
    let kept: Vec<_> = surface
        .images
        .iter()
        .filter(|i| (ICON_TEXTURE_BASE..ICON_TEXTURE_BASE + 10_000).contains(&i.id))
        .collect();
    assert!(kept.len() < before, "icons under the list are suppressed");
    for img in kept {
        let b = (img.x_px, img.y_px, img.x_px + img.width_px, img.y_px + img.height_px);
        assert!(!overlaps(b, list), "icon painted over the list");
    }
}

#[test]
fn render_gui_scaled_list_geometry_matches_hit() {
    let (mut wm, area) = fixture(120, 40);
    wm.open_agents_list();
    wm.set_gui_surface(true);
    wm.set_scale(PanelTarget::AgentsList, 150);
    let ws = workspace_of(&wm, area);
    let vws = wm.panel_virtual_area(ws, PanelTarget::AgentsList);
    let vr = wm.agents_list_rect(vws).unwrap();
    let real = scaled_panel_real_rect(ws, vr, 150);
    let surface = gui_frame(&mut wm, area);
    // Corner glyphs of the frame sit on the 150% cell grid of the virtual rect.
    let at = |col: u16, row: u16| {
        let x = f32::from(ws.x) * 8.0 + f32::from(col - ws.x) * 12.0;
        let y = f32::from(ws.y) * 16.0 + f32::from(row - ws.y) * 24.0;
        surface.glyphs.iter().any(|g| (g.x_px - x).abs() < 0.5 && (g.y_px - y).abs() < 0.5)
    };
    assert!(at(vr.x, vr.y), "top-left frame cell");
    assert!(at(vr.x + vr.width - 1, vr.y + vr.height - 1), "bottom-right frame cell");
    // The glyph grid and `scaled_panel_real_rect` (what `panel_space` maps clicks into) agree to
    // within the rounding of one real cell.
    let left = f32::from(ws.x) * 8.0 + f32::from(vr.x - ws.x) * 12.0;
    let right = f32::from(ws.x) * 8.0 + f32::from(vr.x + vr.width - ws.x) * 12.0;
    assert!((left - (f32::from(real.x) * 8.0)).abs() <= 8.0, "left edge {left} vs {}", real.x);
    assert!((right - (f32::from(real.right()) * 8.0)).abs() <= 8.0, "right edge {right} vs {}", real.right());
}

#[test]
fn render_gui_list_closed_is_identical() {
    let (mut never, area) = fixture(120, 40);
    let a = digest(&gui_frame(&mut never, area));

    let (mut reopened, _) = fixture(120, 40);
    reopened.open_agents_list();
    reopened.set_gui_surface(true);
    reopened.set_scale(PanelTarget::AgentsList, 150);
    let _ = gui_frame(&mut reopened, area);
    reopened.close_agents_list();
    let b = digest(&gui_frame(&mut reopened, area));
    assert_eq!(a, b, "a closed list leaves no trace in the GUI frame (even with a stored scale)");

    let (mut other, _) = fixture(120, 40);
    assert_eq!(a, digest(&gui_frame(&mut other, area)), "frames are deterministic");
}
