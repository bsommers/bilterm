//! Phase 51-02: frozen geometry/hit contract of the AGENTS list window (D-03) and the identity
//! hooks for tiling (D-04) and demo state (D-01). Cells in `hit_table` are computed by hand from
//! the default rect: workspace 120x40, 3 panes -> window (83,1) 36x7 (height = rows + 4), so
//! title row y=1, header y=2, rows y=3..=5, footer y=6, right border x=118, bottom border y=7.

use bilterm::ui::typography::PanelTarget;
use bilterm::wm::agents_list_geometry::{ListBadge, ListChip, ListHit};
use bilterm::wm::panel_dock::DockPanel;
use bilterm::wm::split_tree::SplitTree;
use bilterm::wm::{SplitDirection, WindowManager, WindowResizeEdge};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

fn fixture(cols: u16, rows: u16, panes: usize) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    for _ in 1..panes {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn rects_of(wm: &WindowManager, area: Rect) -> Vec<(usize, Rect)> {
    let mut v = wm.compute_workspace_rects(area);
    v.sort_by_key(|(id, _)| *id);
    v
}

fn within(inner: Rect, outer: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

fn disjoint(a: Rect, b: Rect) -> bool {
    a.width == 0 || a.height == 0 || b.width == 0 || b.height == 0 || a.intersection(b).area() == 0
}

#[test]
fn hit_table() {
    let (mut wm, area) = fixture(120, 40, 3);
    wm.open_agents_list();
    let ids: Vec<usize> = wm.agents_list_rows().iter().map(|r| r.pane_id).collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(wm.agents_list_rect(area), Some(Rect::new(83, 1, 36, 7)));
    let hit = |c: u16, r: u16| wm.agents_list_hit(area, c, r);

    // Outside: above, left, right of the window and the workspace origin.
    assert_eq!(hit(0, 0), ListHit::Outside);
    assert_eq!(hit(83, 0), ListHit::Outside);
    assert_eq!(hit(82, 4), ListHit::Outside);
    assert_eq!(hit(119, 4), ListHit::Outside);
    assert_eq!(hit(100, 8), ListHit::Outside);
    // Title row: plain title cells, then the badges (pin at 106, trio [_][box][X] at 108..=116).
    assert_eq!(hit(90, 1), ListHit::Title);
    assert_eq!(hit(105, 1), ListHit::Title);
    assert_eq!(hit(106, 1), ListHit::Badge(ListBadge::Pin));
    assert_eq!(hit(107, 1), ListHit::Title);
    for c in 108..=110 {
        assert_eq!(hit(c, 1), ListHit::Badge(ListBadge::Minimize), "col {c}");
    }
    for c in 111..=113 {
        assert_eq!(hit(c, 1), ListHit::Badge(ListBadge::Dock), "col {c}");
    }
    for c in 114..=116 {
        assert_eq!(hit(c, 1), ListHit::Badge(ListBadge::Close), "col {c}");
    }
    assert_eq!(hit(117, 1), ListHit::Title);
    // Corners resize, the rest of the top row is the title.
    assert_eq!(hit(83, 1), ListHit::Border(WindowResizeEdge::TopLeft));
    assert_eq!(hit(118, 1), ListHit::Border(WindowResizeEdge::TopRight));
    // Borders.
    assert_eq!(hit(83, 4), ListHit::Border(WindowResizeEdge::Left));
    assert_eq!(hit(118, 4), ListHit::Border(WindowResizeEdge::Right));
    assert_eq!(hit(100, 7), ListHit::Border(WindowResizeEdge::Bottom));
    assert_eq!(hit(83, 7), ListHit::Border(WindowResizeEdge::BottomLeft));
    assert_eq!(hit(118, 7), ListHit::Border(WindowResizeEdge::BottomRight));
    // Header, rows (pane-id order), chips and footer.
    assert_eq!(hit(90, 2), ListHit::Header);
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(hit(90, 3 + i as u16), ListHit::Row(*id), "row {i}");
        assert_eq!(hit(84, 3 + i as u16), ListHit::Row(*id), "row {i} left edge");
        assert_eq!(hit(117, 3 + i as u16), ListHit::Row(*id), "row {i} right edge");
    }
    for c in 84..=91 {
        assert_eq!(hit(c, 6), ListHit::Chip(ListChip::Filter), "col {c}");
    }
    assert_eq!(hit(92, 6), ListHit::Footer);
    for c in 93..=98 {
        assert_eq!(hit(c, 6), ListHit::Chip(ListChip::Dock), "col {c}");
    }
    assert_eq!(hit(99, 6), ListHit::Footer);
    for c in 100..=105 {
        assert_eq!(hit(c, 6), ListHit::Chip(ListChip::Zoom), "col {c}");
    }
    assert_eq!(hit(110, 6), ListHit::Footer);

    // Body: a row below the last row (taller manual rect).
    wm.agents_list_mut().rect = Some(Rect::new(83, 1, 36, 12));
    assert_eq!(wm.agents_list_hit(area, 90, 6), ListHit::Body);
    assert_eq!(wm.agents_list_hit(area, 90, 3), ListHit::Row(ids[0]));
    assert_eq!(wm.agents_list_hit(area, 110, 11), ListHit::Footer);

    // A closed window hits nothing.
    wm.close_agents_list();
    assert_eq!(wm.agents_list_hit(area, 90, 3), ListHit::Outside);
}

#[test]
fn hit_scroll_maps_rows() {
    let (mut wm, _) = fixture(200, 60, 30);
    let area = Rect::new(0, 0, 200, 30);
    wm.open_agents_list();
    let ids: Vec<usize> = wm.agents_list_rows().iter().map(|r| r.pane_id).collect();
    assert_eq!(ids.len(), 30);
    let lay = wm.agents_list_layout(area).unwrap();
    assert!(lay.visible_rows < 30, "rows must overflow so scrolling matters");
    let (x, y0) = (lay.rows_area.x + 2, lay.rows_area.y);
    assert_eq!(wm.agents_list_hit(area, x, y0), ListHit::Row(ids[0]));
    wm.agents_list_mut().scroll = 5;
    assert_eq!(wm.agents_list_hit(area, x, y0), ListHit::Row(ids[5]));
    assert_eq!(wm.agents_list_hit(area, x, y0 + 2), ListHit::Row(ids[7]));
    // Scrolled past the end: the empty remainder is the body.
    wm.agents_list_mut().scroll = 29;
    assert_eq!(wm.agents_list_hit(area, x, y0), ListHit::Row(ids[29]));
    assert_eq!(wm.agents_list_hit(area, x, y0 + 1), ListHit::Body);
}

#[test]
fn rect_bounds() {
    for (cols, rows) in [(24u16, 6u16), (30, 8), (80, 24), (200, 60)] {
        let (mut wm, _) = fixture(cols.max(30), rows.max(10), 2);
        let ws = Rect::new(0, 0, cols, rows);
        assert_eq!(wm.agents_list_rect(ws), None, "closed window has no rect");
        wm.open_agents_list();
        let r = wm.agents_list_rect(ws).unwrap_or_else(|| panic!("no rect in {cols}x{rows}"));
        assert!(r.width > 0 && r.height > 0, "zero-size rect in {cols}x{rows}: {r:?}");
        assert!(within(r, ws), "{r:?} leaves {ws:?}");
        assert!(r.width >= 20.min(cols) && r.height >= 6.min(rows), "{r:?} too small in {cols}x{rows}");

        for manual in [Rect::new(0, 0, 500, 500), Rect::new(cols - 1, rows - 1, 40, 40), Rect::new(3, 2, 1, 1), Rect::new(0, 0, 0, 0)] {
            wm.agents_list_mut().rect = Some(manual);
            let r = wm.agents_list_rect(ws).unwrap();
            assert!(r.width > 0 && r.height > 0 && within(r, ws), "{manual:?} -> {r:?} in {cols}x{rows}");
            assert!(r.width >= 20.min(cols) && r.height >= 6.min(rows), "{manual:?} -> {r:?}");
        }
    }
    let (mut wm, _) = fixture(80, 24, 2);
    wm.open_agents_list();
    assert_eq!(wm.agents_list_rect(Rect::new(0, 0, 23, 6)), None);
    assert_eq!(wm.agents_list_rect(Rect::new(0, 0, 24, 5)), None);
    assert!(wm.agents_list_layout(Rect::new(0, 0, 23, 6)).is_none());
    assert_eq!(wm.agents_list_hit(Rect::new(0, 0, 24, 5), 3, 2), ListHit::Outside);
    // A workspace with a non-zero origin keeps the window inside it.
    let ws = Rect::new(5, 3, 60, 20);
    let r = wm.agents_list_rect(ws).unwrap();
    assert!(within(r, ws), "{r:?} leaves {ws:?}");
}

#[test]
fn layout_regions_partition() {
    for (cols, rows, panes) in [(24u16, 6u16, 2usize), (80, 24, 3), (120, 40, 30), (200, 60, 1)] {
        let (mut wm, _) = fixture(cols.max(30), rows.max(10), panes.min(8));
        wm.open_agents_list();
        let ws = Rect::new(0, 0, cols, rows);
        let lay = wm.agents_list_layout(ws).unwrap();
        assert_eq!(lay.outer, wm.agents_list_rect(ws).unwrap());
        assert!(within(lay.inner, lay.outer) && lay.inner.width + 2 == lay.outer.width);
        assert!(within(lay.title_row, lay.outer) && lay.title_row.height == 1);
        for part in [lay.header, lay.rows_area, lay.footer] {
            assert!(within(part, lay.inner), "{part:?} outside {:?}", lay.inner);
        }
        assert!(disjoint(lay.header, lay.rows_area));
        assert!(disjoint(lay.rows_area, lay.footer));
        assert!(disjoint(lay.header, lay.footer));
        assert!(disjoint(lay.title_row, lay.inner));
        assert_eq!(lay.visible_rows, usize::from(lay.rows_area.height));
        assert!(lay.rows_area.height >= 1, "{cols}x{rows}: no row visible");
        for (_, r) in &lay.badges {
            assert!(within(*r, lay.title_row), "badge {r:?} outside title row");
        }
        for (_, r) in &lay.chips {
            assert!(within(*r, lay.footer), "chip {r:?} outside footer {:?}", lay.footer);
        }
    }
}

#[test]
fn hit_scaled_space() {
    let (mut wm, area) = fixture(120, 40, 3);
    wm.set_gui_surface(true);
    wm.set_scale(PanelTarget::AgentsList, 150);
    wm.open_agents_list();
    let down = |c: u16, r: u16| MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: c,
        row: r,
        modifiers: KeyModifiers::NONE,
    };
    let (_, vws) = wm.panel_space(down(0, 0), area, PanelTarget::AgentsList);
    assert_eq!((vws.width, vws.height), (80, 26));
    let lay = wm.agents_list_layout(vws).unwrap();
    let ids: Vec<usize> = wm.agents_list_rows().iter().map(|r| r.pane_id).collect();
    let (close, _) = lay.badges.iter().find(|(b, _)| *b == ListBadge::Close).map(|(_, r)| (*r, ())).unwrap();
    let zoom_chip = lay.chips.iter().find(|(c, _)| *c == ListChip::Zoom).map(|(_, r)| *r).unwrap();
    let cases = [
        ((lay.title_row.x + 3, lay.title_row.y), ListHit::Title),
        ((close.x + 1, close.y), ListHit::Badge(ListBadge::Close)),
        ((lay.rows_area.x + 2, lay.rows_area.y + 1), ListHit::Row(ids[1])),
        ((zoom_chip.x + 1, zoom_chip.y), ListHit::Chip(ListChip::Zoom)),
        ((lay.header.x + 1, lay.header.y), ListHit::Header),
        ((lay.outer.x, lay.outer.y + 2), ListHit::Border(WindowResizeEdge::Left)),
    ];
    for ((vc, vr), want) in cases {
        // Real cell whose virtual image is (vc, vr): virtual = real * 100 / 150.
        let real = (vc * 3).div_ceil(2);
        let real_row = (vr * 3).div_ceil(2);
        let (m, ws2) = wm.panel_space(down(real, real_row), area, PanelTarget::AgentsList);
        assert_eq!((m.column, m.row), (vc, vr), "inverse mapping of ({vc},{vr})");
        assert_eq!(wm.agents_list_hit(ws2, m.column, m.row), want, "scaled real ({real},{real_row})");
        assert_eq!(wm.agents_list_hit(vws, vc, vr), want, "virtual ({vc},{vr})");
    }
}

#[test]
fn badges_hidden_when_narrow() {
    let (mut wm, _) = fixture(80, 24, 2);
    wm.open_agents_list();
    let ws = Rect::new(0, 0, 24, 8);
    let lay = wm.agents_list_layout(ws).unwrap();
    assert_eq!(lay.outer.width, 24);
    assert!(lay.badges.iter().all(|(b, _)| *b != ListBadge::Pin), "24-wide window must not carry a pin badge");
    assert_eq!(lay.badges.len(), 3, "the trio stays");
    for (_, r) in &lay.badges {
        assert_eq!((r.y, r.height), (lay.title_row.y, 1));
        assert!(r.x > lay.outer.x && r.x + r.width <= lay.outer.x + lay.outer.width - 1, "{r:?} leaves the title row");
    }
    // Wide window carries all four.
    let wide = wm.agents_list_layout(Rect::new(0, 0, 120, 40)).unwrap();
    assert_eq!(wide.badges.len(), 4);
    // A window under 24 columns (manual rect) has no badges at all and never panics.
    wm.agents_list_mut().rect = Some(Rect::new(0, 0, 20, 6));
    let tiny = wm.agents_list_layout(Rect::new(0, 0, 30, 10)).unwrap();
    assert!(tiny.badges.is_empty());
    for c in 0..30 {
        for r in 0..10 {
            let _ = wm.agents_list_hit(Rect::new(0, 0, 30, 10), c, r);
        }
    }
}

// ---------------------------------------------------------------- task 2: identity hooks

#[test]
fn seams_identity_without_panels() {
    let (mut wm, area) = fixture(120, 40, 2);
    for a in [area, Rect::new(0, 0, 24, 6), Rect::new(3, 2, 80, 30), Rect::new(0, 0, 1, 1)] {
        assert_eq!(wm.subtract_docked_panels(a), a);
        assert_eq!(wm.effective_workspace_area(a), a);
        assert_eq!(wm.panel_dock_slot(a, DockPanel::AgentsList), None);
        assert_eq!(wm.panel_dock_slot(a, DockPanel::AgentsTraffic), None);
    }
    wm.open_agents_list();
    assert_eq!(wm.subtract_docked_panels(area), area);
    let before = rects_of(&wm, area);
    wm.dock_panels_on_arrange();
    // 51-09: the hook docks the open list (pane rects are derived live, so they already shrink).
    assert!(wm.agents_list().docked);
    assert_ne!(rects_of(&wm, area), before);
    assert!(wm.set_panel_docked(DockPanel::AgentsList, false));
    assert_eq!(rects_of(&wm, area), before);
    // 51-09: the flag flips only for an open panel and every change re-flows the panes.
    assert!(wm.set_panel_docked(DockPanel::AgentsList, true));
    assert!(wm.agents_list().docked);
    assert!(!wm.set_panel_docked(DockPanel::AgentsList, true), "unchanged reports false");
    assert_ne!(rects_of(&wm, area), before, "docking re-flows the panes");
    assert!(wm.set_panel_docked(DockPanel::AgentsList, false));
    assert_eq!(rects_of(&wm, area), before, "release re-flows back");
    assert!(!wm.set_panel_docked(DockPanel::AgentsList, false));
    assert!(!wm.set_panel_docked(DockPanel::AgentsTraffic, true));
    wm.close_agents_list();
    assert!(!wm.set_panel_docked(DockPanel::AgentsList, true), "inactive list refuses");
    assert!(!wm.agents_list().docked);
}

#[test]
fn arrange_grid_output_unchanged_without_panels() {
    let (mut wm, area) = fixture(120, 40, 2);
    let ids: Vec<usize> = wm.ordered_pane_ids();
    assert_eq!(ids.len(), 2);
    let mut tree = SplitTree::new(ids[0]);
    tree.split_leaf(ids[0], ids[1], SplitDirection::Vertical);
    let mut expect = tree.compute_rects(area);
    expect.sort_by_key(|(id, _)| *id);

    assert_eq!(wm.auto_arrange_windows("grid"), 2);
    assert_eq!(rects_of(&wm, area), expect);
    // 51-09: an open list is docked by arrange; the panes tile the area left of the strip.
    wm.open_agents_list();
    assert_eq!(wm.auto_arrange_windows("grid"), 2);
    assert!(wm.agents_list().docked);
    assert_ne!(rects_of(&wm, area), expect);
}

#[test]
fn clear_demo_hook_runs() {
    let (mut wm, _) = fixture(120, 40, 2);
    assert!(!wm.agents_demo_summary().running);
    wm.seed_agents_demo_for_tests(vec![1, 2], vec![2]);
    let s = wm.agents_demo_summary();
    assert!(s.running && s.endpoints == vec![1, 2] && s.spawned == vec![2]);
    wm.clear_agents_demo();
    assert!(!wm.agents_demo_summary().running);

    // create_new_session resets it (pane ids restart at 2 there).
    wm.seed_agents_demo_for_tests(vec![1], vec![2]);
    wm.create_new_session(Some("seam-a"), true).unwrap();
    assert!(!wm.agents_demo_summary().running, "create_new_session must clear demo state");

    // apply_saved_session_snapshot (through a layout-profile restore) resets it too.
    wm.save_layout_profile(Some("seam-p")).unwrap();
    wm.switch_layout_profile("seam-q").unwrap();
    wm.seed_agents_demo_for_tests(vec![1], vec![2]);
    wm.switch_layout_profile("seam-p").unwrap();
    assert!(!wm.agents_demo_summary().running, "snapshot restore must clear demo state");
}

#[test]
fn summaries_read_only() {
    let (mut wm, _) = fixture(120, 40, 2);
    let s = wm.agents_list_summary();
    assert!(!s.active && !s.pinned && !s.focused && s.selected.is_none());
    assert!(!wm.agents_demo_summary().running);
    for cmd in [":agents list", ":agents list pin"] {
        let parsed = bilterm::colon_command::parse_colon_command(cmd).unwrap().unwrap();
        wm.apply(parsed).unwrap();
    }
    let s = wm.agents_list_summary();
    assert!(s.active && s.focused);
    assert_eq!(s.scale_pct, 100);
    assert_eq!(s.row_count, 2);
    wm.set_scale(PanelTarget::AgentsList, 125);
    assert_eq!(wm.agents_list_summary().scale_pct, 125, "summary reflects scale_setting");
    let again = wm.agents_list_summary();
    assert_eq!(again, wm.agents_list_summary(), "reading is idempotent");
}
