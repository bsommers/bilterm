//! Phase 64-03: TRAFFIC MONITOR window state, geometry, dock, scale and deterministic rendering.

use bilterm::traffic::{GroupKey, Locality, Origin, Role, SortKey, TrafficTap};
use bilterm::ui::traffic_monitor_ui::{fmt_bytes, render_traffic_monitor};
use bilterm::wm::WindowManager;
use bilterm::wm::panel_dock::DockPanel;
use bilterm::wm::traffic_monitor::TrafficMonitorOp as Op;
use bilterm::wm::traffic_monitor_geometry::{TrafficBadge, TrafficChip, TrafficHit, TrafficLayout};
use bilterm::action::Action;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Mirrors `SyntheticTraffic::scenario_mixed` on the window manager's own meter.
pub struct Feed {
    pub taps: Vec<TrafficTap>,
    pub now: u64,
}

impl Feed {
    pub fn new(wm: &mut WindowManager) -> Self {
        let m = wm.traffic_meter();
        wm.traffic_tick(0);
        let ch = |o: Origin, p: Option<usize>, r: Role, l: &str| m.open_channel(o, l, p, r);
        let taps = vec![
            ch(Origin::LocalPty, Some(1), Role::Both, "local"),
            ch(Origin::Ssh { host: "prod-db".into(), port: 22 }, Some(2), Role::Both, "prod-db"),
            ch(Origin::Ssh { host: "web-1".into(), port: 2222 }, Some(3), Role::Both, "web-1"),
            ch(Origin::TmuxLink { dest: "bastion".into(), session: "work".into() }, None, Role::Wire, "bastion"),
            ch(Origin::Ws { client: "web-1".into() }, None, Role::Wire, "web-1"),
            ch(Origin::RpcUnix, None, Role::Wire, "rpc"),
        ];
        let mut f = Self { taps, now: 0 };
        for s in 0..5usize {
            f.taps[0].rx(1000 + 100 * s);
            f.taps[0].tx(50);
            f.taps[1].rx(20_000);
            f.taps[1].tx(400);
            f.taps[2].rx(5_000);
            f.second(wm);
        }
        for _ in 0..3 {
            f.taps[3].rx(9000);
            f.taps[4].tx(6000);
            f.taps[5].tx(800);
            f.second(wm);
        }
        f
    }
    pub fn second(&mut self, wm: &mut WindowManager) {
        self.now += 1000;
        wm.traffic_tick(self.now);
    }
}

pub fn wm(cols: u16, rows: u16) -> WindowManager {
    WindowManager::new_headless(rows, cols)
}

pub fn apply(wm: &mut WindowManager, op: Op) {
    wm.apply(Action::TrafficMonitor(op)).unwrap();
}

pub fn render(wm: &WindowManager, ws: Rect) -> Buffer {
    let cfg = wm.config().clone();
    let mut buf = Buffer::empty(Rect::new(0, 0, ws.x + ws.width, ws.y + ws.height));
    render_traffic_monitor(wm, ws, &cfg, &cfg.palette(), &cfg.glyph_set(), &mut buf);
    buf
}

pub fn text(buf: &Buffer) -> String {
    let a = buf.area;
    (a.y..a.y + a.height)
        .map(|y| (a.x..a.x + a.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn toggle_open_close_focus_and_input_capture() {
    let mut w = wm(120, 40);
    assert!(!w.traffic_monitor().captures_input());
    apply(&mut w, Op::Toggle);
    assert!(w.traffic_monitor().active() && w.traffic_monitor().focused && w.traffic_monitor().captures_input());
    apply(&mut w, Op::Blur);
    assert!(w.traffic_monitor().active() && !w.traffic_monitor().captures_input());
    apply(&mut w, Op::Toggle);
    assert!(!w.traffic_monitor().active());
    apply(&mut w, Op::Open);
    apply(&mut w, Op::Pin(Some(true)));
    apply(&mut w, Op::Dismiss);
    assert!(w.traffic_monitor().active() && !w.traffic_monitor().focused, "pinned dismiss only blurs");
    apply(&mut w, Op::Pin(Some(false)));
    apply(&mut w, Op::Open);
    apply(&mut w, Op::Dismiss);
    assert!(!w.traffic_monitor().active());
}

#[test]
fn opening_blurs_other_windows() {
    let mut w = wm(120, 40);
    w.apply(Action::ClipboardWindow(bilterm::wm::clipboard_window::ClipboardWindowOp::Open { scope: None })).unwrap();
    assert!(w.clipboard_window().focused);
    apply(&mut w, Op::Open);
    assert!(!w.clipboard_window().focused && w.traffic_monitor().focused);
}

#[test]
fn rect_defaults_clamps_and_minimums() {
    let mut w = wm(200, 60);
    apply(&mut w, Op::Open);
    let ws = w.dock_base_area();
    let r = w.traffic_monitor_rect(ws).unwrap();
    assert_eq!((r.width, r.height), (84, 22));
    apply(&mut w, Op::SetRect { x: 0, y: 0, w: 10, h: 3 });
    let r = w.traffic_monitor_rect(ws).unwrap();
    assert_eq!((r.width, r.height), (52, 12), "never below 52x12");
    apply(&mut w, Op::Move { dx: 5000, dy: 5000 });
    let r = w.traffic_monitor_rect(ws).unwrap();
    assert!(r.x + r.width <= ws.x + ws.width && r.y + r.height <= ws.y + ws.height);
    apply(&mut w, Op::SetRect { x: 1, y: 1, w: 500, h: 500 });
    let r = w.traffic_monitor_rect(ws).unwrap();
    assert!(r.width <= ws.width && r.height <= ws.height);
    apply(&mut w, Op::SizeAuto);
    assert_eq!(w.traffic_monitor_rect(ws).unwrap().width, 84);
    let mut tiny = wm(60, 14);
    apply(&mut tiny, Op::Open);
    let r = tiny.traffic_monitor_rect(tiny.dock_base_area()).unwrap();
    assert!(r.width >= 52.min(60) && r.height >= 1);
}

#[test]
fn geometry_hit_table() {
    let mut w = wm(120, 40);
    let _f = Feed::new(&mut w);
    apply(&mut w, Op::Open);
    let ws = w.dock_base_area();
    let l = TrafficLayout::compute(w.traffic_monitor_rect(ws).unwrap());
    let o = l.outer;
    let hit = |c, r| w.traffic_monitor_hit(ws, c, r);
    assert_eq!(hit(o.x, o.y), TrafficHit::Edge(bilterm::wm::WindowResizeEdge::TopLeft));
    assert_eq!(hit(o.x + o.width - 1, o.y + o.height - 1), TrafficHit::Edge(bilterm::wm::WindowResizeEdge::BottomRight));
    assert_eq!(hit(o.x + 3, o.y), TrafficHit::Title);
    for (b, x) in &l.badges {
        assert_eq!(hit(*x + 1, o.y), TrafficHit::Badge(*b));
    }
    assert!(l.badges.iter().any(|(b, _)| *b == TrafficBadge::Close));
    assert_eq!(l.tabs.len(), 7);
    for (g, x, _) in &l.tabs {
        assert_eq!(hit(*x + 1, l.tabs_y), TrafficHit::GroupTab(*g));
    }
    assert_eq!(hit(l.inner.x + 2, l.filter_y), TrafficHit::Filter);
    for (loc, x, _) in &l.chips {
        assert_eq!(hit(*x, l.filter_y), TrafficHit::LocalityChip(*loc));
    }
    for (c, x, wd) in &l.columns {
        if let Some(k) = c.sort_key() {
            assert_eq!(hit(*x + wd - 1, l.colhead_y), TrafficHit::Header(k));
        }
    }
    assert_eq!(hit(l.inner.x + 1, l.rows_y), TrafficHit::Row(0));
    assert_eq!(hit(l.inner.x + 1, l.rows_y + 2), TrafficHit::Row(2));
    let n = w.traffic_rows().rows.len();
    assert!(n > 2);
    assert_eq!(hit(l.inner.x + 1, l.rows_y + l.rows_h - 1), if n >= l.rows_h as usize { TrafficHit::Row(l.rows_h as usize - 1) } else { TrafficHit::Body });
    for (c, x, _) in &l.footer {
        assert_eq!(hit(*x, l.footer_y), TrafficHit::Chip(*c));
    }
    assert!(l.footer.iter().any(|(c, _, _)| *c == TrafficChip::Pause));
    assert_eq!(hit(o.x + o.width + 1, o.y), TrafficHit::Outside);
    assert_eq!(hit(l.inner.x + 1, l.header_y), TrafficHit::Body);
    // scroll maps rows through the scroll offset
    apply(&mut w, Op::Scroll(1));
    assert_eq!(w.traffic_monitor_hit(ws, l.inner.x + 1, l.rows_y), TrafficHit::Row(1));
}

#[test]
fn pause_freezes_rows_until_resume() {
    let mut w = wm(120, 40);
    let mut f = Feed::new(&mut w);
    apply(&mut w, Op::Open);
    let before = w.traffic_rows().rows.iter().map(|r| r.rx_total).collect::<Vec<_>>();
    apply(&mut w, Op::Pause(Some(true)));
    f.taps[0].rx(999_999);
    f.second(&mut w);
    let frozen = w.traffic_rows().rows.iter().map(|r| r.rx_total).collect::<Vec<_>>();
    assert_eq!(before, frozen);
    let buf = render(&w, w.dock_base_area());
    assert!(text(&buf).contains("PAUSED"));
    apply(&mut w, Op::Pause(Some(false)));
    assert_ne!(before, w.traffic_rows().rows.iter().map(|r| r.rx_total).collect::<Vec<_>>());
}

#[test]
fn group_sort_filter_drill_and_stack_depth() {
    let mut w = wm(120, 40);
    let _f = Feed::new(&mut w);
    apply(&mut w, Op::Open);
    apply(&mut w, Op::Step(2));
    apply(&mut w, Op::GroupBy(GroupKey::Origin));
    assert_eq!(w.traffic_monitor().selected, 0);
    apply(&mut w, Op::Sort(SortKey::Total, Some(true)));
    apply(&mut w, Op::Sort(SortKey::Total, None));
    assert!(!w.traffic_monitor().sort.descending, "second Sort on the active key flips direction");
    apply(&mut w, Op::GroupBy(GroupKey::Connection));
    let label = w.traffic_rows().rows[0].label.clone();
    apply(&mut w, Op::Drill);
    assert_eq!(w.traffic_monitor().group, GroupKey::Pane);
    assert_eq!(w.traffic_monitor().filter.text, label);
    apply(&mut w, Op::DrillUp);
    assert_eq!(w.traffic_monitor().group, GroupKey::Connection);
    assert_eq!(w.traffic_monitor().filter.text, "");
    for _ in 0..8 {
        apply(&mut w, Op::GroupBy(GroupKey::Connection));
        apply(&mut w, Op::Drill);
        apply(&mut w, Op::GroupBy(GroupKey::Origin));
        apply(&mut w, Op::Drill);
    }
    assert!(w.traffic_monitor().drill_stack.len() <= 4);
    apply(&mut w, Op::Filter(format!("{}\x07x", "a".repeat(100))));
    let f = &w.traffic_monitor().filter.text;
    assert_eq!(f.chars().count(), 64);
    assert!(f.chars().all(|c| !c.is_control()));
    apply(&mut w, Op::Locality(Some(Locality::Network)));
    assert_eq!(w.traffic_monitor().filter.locality, Some(Locality::Network));
}

#[test]
fn dock_manual_move_releases_and_idle_layout_is_identity() {
    let mut w = wm(160, 40);
    let area = w.dock_base_area();
    assert_eq!(w.subtract_docked_panels(area), area);
    apply(&mut w, Op::Open);
    apply(&mut w, Op::Dock(Some(true)));
    assert!(w.traffic_monitor().docked);
    assert_ne!(w.subtract_docked_panels(area), area);
    assert!(w.panel_dock_slot(area, DockPanel::Traffic).is_some());
    apply(&mut w, Op::Move { dx: 1, dy: 1 });
    assert!(!w.traffic_monitor().docked);
    assert_eq!(w.subtract_docked_panels(area), area);
    apply(&mut w, Op::Dock(Some(true)));
    apply(&mut w, Op::Close);
    assert!(!w.traffic_monitor().docked);
}

#[test]
fn scale_clamps_to_75_200() {
    let mut w = wm(120, 40);
    w.set_gui_surface(true);
    apply(&mut w, Op::Scale(500));
    assert_eq!(w.scale_pct(bilterm::ui::typography::PanelTarget::Traffic), 200);
    apply(&mut w, Op::Scale(10));
    assert_eq!(w.scale_pct(bilterm::ui::typography::PanelTarget::Traffic), 75);
}

fn snapshot_wm(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let mut w = wm(cols, rows);
    let _f = Feed::new(&mut w);
    apply(&mut w, Op::Open);
    let ws = w.dock_base_area();
    (w, ws)
}

#[test]
fn snapshot_120x40() {
    let (w, ws) = snapshot_wm(120, 40);
    insta::assert_snapshot!("traffic_120x40", text(&render(&w, ws)));
}

#[test]
fn snapshot_80x24() {
    let (w, ws) = snapshot_wm(80, 24);
    insta::assert_snapshot!("traffic_80x24", text(&render(&w, ws)));
}

#[test]
fn render_matches_layout_and_hit_regions() {
    let (w, ws) = snapshot_wm(120, 40);
    let buf = render(&w, ws);
    let l = TrafficLayout::compute(w.traffic_monitor_rect(ws).unwrap());
    let at = |x: u16, y: u16, n: usize| (0..n as u16).map(|i| buf[(x + i, y)].symbol().to_string()).collect::<String>();
    for (g, x, _) in &l.tabs {
        let label = bilterm::wm::traffic_monitor_geometry::TABS.iter().find(|(k, _)| k == g).unwrap().1;
        assert_eq!(at(*x, l.tabs_y, label.len() + 2), format!("[{label}]"));
        assert_eq!(w.traffic_monitor_hit(ws, *x, l.tabs_y), TrafficHit::GroupTab(*g));
    }
    for (c, x, _) in &l.footer {
        let t = bilterm::wm::traffic_monitor_geometry::FOOTER_CHIPS.iter().find(|(k, _)| k == c).unwrap().1;
        assert_eq!(at(*x, l.footer_y, t.len()), t);
        assert_eq!(w.traffic_monitor_hit(ws, *x, l.footer_y), TrafficHit::Chip(*c));
    }
    for (loc, x, _) in &l.chips {
        let t = bilterm::wm::traffic_monitor_geometry::LOC_CHIPS.iter().find(|(k, _)| k == loc).unwrap().1;
        assert_eq!(at(*x, l.filter_y, t.len()), t);
    }
    for (c, x, wd) in &l.columns {
        if c.sort_key().is_some() {
            let head = at(*x, l.colhead_y, *wd as usize);
            assert!(head.contains(c.title().split(' ').next().unwrap()), "{head:?} vs {:?}", c.title());
        }
    }
    // every cell of the window is classified (never Outside) and the cell right outside is Outside.
    for y in l.outer.y..l.outer.y + l.outer.height {
        for x in l.outer.x..l.outer.x + l.outer.width {
            assert_ne!(w.traffic_monitor_hit(ws, x, y), TrafficHit::Outside);
        }
    }
}

#[test]
fn closed_monitor_draws_nothing_and_labels_are_sanitised() {
    let mut w = wm(120, 40);
    let ws = w.dock_base_area();
    let empty = Buffer::empty(Rect::new(0, 0, 120, 40));
    assert_eq!(render(&w, ws), empty);
    let m = w.traffic_meter();
    w.traffic_tick(0);
    let t = m.open_channel(Origin::Ws { client: "bad\x1b[31mclient".into() }, "bad\x1b[31mclient", None, Role::Wire);
    t.rx(100);
    w.traffic_tick(1000);
    apply(&mut w, Op::Open);
    let out = text(&render(&w, ws));
    assert!(!out.contains('\x1b'));
    assert!(out.contains("TRAFFIC MONITOR"));
}

#[test]
fn direction_and_locality_row_counts_and_sparkline_levels() {
    let (mut w, ws) = snapshot_wm(120, 40);
    apply(&mut w, Op::GroupBy(GroupKey::Direction));
    assert_eq!(w.traffic_rows().rows.len(), 2);
    apply(&mut w, Op::GroupBy(GroupKey::Locality));
    assert!(w.traffic_rows().rows.len() <= 3);
    let _ = render(&w, ws);
    // 8-level sparkline for a known series
    let mut buf = Buffer::empty(Rect::new(0, 0, 8, 1));
    use ratatui::widgets::{Sparkline, Widget};
    Sparkline::default().data(&[0u64, 1, 2, 3, 4, 5, 6, 7]).max(7).render(Rect::new(0, 0, 8, 1), &mut buf);
    let s: String = (0..8).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    assert_eq!(s.chars().nth(7), Some('█'));
    assert_eq!(bilterm::ui::traffic_monitor_gui::bar_heights(&[0, 1, 2, 3, 4, 5, 6, 7], 7), vec![0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(fmt_bytes(1536), "1.5K");
}
