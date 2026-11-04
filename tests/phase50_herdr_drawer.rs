//! Phase 50-02: herdr-style drawer tree (FLT-01, D-02). `style_` = config seam, `tree_` = row model + renderer.

use std::sync::Mutex;

use bilterm::colon_command::parse_colon_command;
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::drawer::{DrawerConfig, DrawerStyle};
use bilterm::wm::{DrawerRowKind, SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!(
        "bilterm-herdr-drawer-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY-equivalent: serialized by ENV_LOCK; every test in this file goes through here.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_STATE_HOME", &dir);
    }
    let out = f();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// Two-pane fixture with explicit OSC 2 titles.
fn fixture(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    wm.pane_mut(1).unwrap().process_pty_output(b"\x1b]2;alpha\x07ALPHA_OUT\r\n");
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.pane_mut(p2).unwrap().process_pty_output(b"\x1b]2;beta\x07BETA_OUT\r\n");
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    wm.apply(parsed).unwrap();
}

fn open_drawer(wm: &mut WindowManager) {
    wm.toggle_drawer();
    wm.drawer_mut().focused = true;
}

fn render(wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

fn buffer_lines(buf: &Buffer) -> Vec<String> {
    let a = buf.area;
    (a.y..a.y + a.height)
        .map(|y| (a.x..a.x + a.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
        .collect()
}

#[test]
fn style_default_is_classic() {
    let d = DrawerConfig::default();
    assert_eq!(d.style, DrawerStyle::Classic);
    assert_eq!(d.herdr_width_cols, 28);
}

#[test]
fn style_set_via_colon_set_switches_rows() {
    with_temp_xdg(|| {
        let (mut wm, _area) = fixture(100, 30);
        run_colon(&mut wm, ":set drawer.style herdr");
        assert_eq!(wm.drawer_config().style, DrawerStyle::Herdr);
        let rows = wm.drawer_rows();
        assert!(matches!(rows[0].kind, DrawerRowKind::SessionHeader(_)));
        assert!(rows[0].herdr.is_some());
        assert_eq!(rows[1].kind, DrawerRowKind::Pane(1));
        assert_eq!(rows[2].kind, DrawerRowKind::Pane(2));
    });
}

#[test]
fn tree_tracer_renders_one_line_per_pane() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        run_colon(&mut wm, ":set drawer.style herdr");
        open_drawer(&mut wm);
        let lines = buffer_lines(&render(&wm, area));
        assert!(lines.iter().any(|l| l.contains("WORKSPACES")), "title missing:\n{}", lines.join("\n"));
        let alpha = lines.iter().position(|l| l.contains("#1 alpha"));
        let beta = lines.iter().position(|l| l.contains("#2 beta"));
        assert!(alpha.is_some() && beta.is_some(), "pane lines missing:\n{}", lines.join("\n"));
        assert_ne!(alpha, beta, "panes must be on separate lines");
        assert_eq!(wm.drawer_rect(area).unwrap().width, 28);
    });
}

// ---- Task 2: full tree semantics ----

fn herdr_wm(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let (mut wm, area) = fixture(cols, rows);
    run_colon(&mut wm, ":set drawer.style herdr");
    (wm, area)
}

fn kinds_depths(wm: &WindowManager) -> Vec<(DrawerRowKind, u8)> {
    wm.drawer_rows()
        .into_iter()
        .map(|r| (r.kind.clone(), r.herdr.as_ref().expect("herdr meta").depth))
        .collect()
}

fn session(name: &str) -> DrawerRowKind {
    DrawerRowKind::SessionHeader(name.to_string())
}

/// Tree main > build > default, where "default" is the active session (headless default name).
fn nested_tree(wm: &mut WindowManager) {
    let t = wm.session_tree_mut();
    t.branch("main", "build").unwrap();
    t.branch("build", "default").unwrap();
}

/// Lines of the drawer interior only (inside the frame), for glyph checks.
fn drawer_interior(wm: &WindowManager, area: Rect) -> Vec<String> {
    let buf = render(wm, area);
    let r = wm.drawer_rect(area).unwrap();
    (r.y + 1..r.y + r.height - 1)
        .map(|y| (r.x + 1..r.x + r.width - 1).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
        .collect()
}

#[test]
fn tree_nested_sessions_indent_by_depth() {
    with_temp_xdg(|| {
        let (mut wm, _a) = herdr_wm(100, 30);
        nested_tree(&mut wm);
        let got = kinds_depths(&wm);
        assert_eq!(
            got,
            vec![
                (session("main"), 0),
                (session("build"), 1),
                (session("default"), 2),
                (DrawerRowKind::Pane(1), 3),
                (DrawerRowKind::Pane(2), 3),
            ]
        );
        let rows = wm.drawer_rows();
        assert_eq!(rows[0].herdr.as_ref().unwrap().level, bilterm::wm::drawer::HerdrLevel::Workspace);
        assert_eq!(rows[1].herdr.as_ref().unwrap().level, bilterm::wm::drawer::HerdrLevel::Session);
        assert!(rows[2].herdr.as_ref().unwrap().live);
        assert!(!rows[0].herdr.as_ref().unwrap().live);
    });
}

#[test]
fn tree_root_without_children_lists_panes_at_depth_1() {
    with_temp_xdg(|| {
        let (wm, _a) = herdr_wm(100, 30);
        assert_eq!(
            kinds_depths(&wm),
            vec![(session("default"), 0), (DrawerRowKind::Pane(1), 1), (DrawerRowKind::Pane(2), 1)]
        );
    });
}

#[test]
fn tree_fold_hides_descendants() {
    with_temp_xdg(|| {
        let (mut wm, _a) = herdr_wm(100, 30);
        nested_tree(&mut wm);
        assert!(wm.session_tree_mut().toggle_fold("main"));
        let rows = wm.drawer_rows();
        assert_eq!(rows.len(), 1, "only the collapsed workspace remains: {:?}", rows.iter().map(|r| &r.kind).collect::<Vec<_>>());
        let meta = rows[0].herdr.as_ref().unwrap();
        assert!(meta.collapsed && meta.has_children);
    });
}

#[test]
fn tree_saved_session_rollup() {
    with_temp_xdg(|| {
        let (mut wm, area) = herdr_wm(100, 30);
        wm.session_tree_mut().upsert_session("archive", bilterm::wm::SessionNodeKind::Independent, None);
        wm.session_tree_mut().node_mut("archive").unwrap().pane_count = 3;
        let rows = wm.drawer_rows();
        let saved = rows.iter().find(|r| r.kind == session("archive")).expect("saved root row");
        let meta = saved.herdr.as_ref().unwrap();
        assert!(!meta.live);
        assert_eq!(meta.pane_count, 3);
        open_drawer(&mut wm);
        let inner = drawer_interior(&wm, area);
        assert!(inner.iter().any(|l| l.contains("archive") && l.trim_end().ends_with("3p -")), "{}", inner.join("\n"));
    });
}

#[test]
fn tree_marks_current_pane() {
    with_temp_xdg(|| {
        let (mut wm, area) = herdr_wm(100, 30);
        open_drawer(&mut wm);
        let rows = wm.drawer_rows();
        let focused = rows.iter().find(|r| r.is_focused_pane).expect("a focused pane row");
        let DrawerRowKind::Pane(fid) = focused.kind else { panic!("focused row is not a pane") };
        let inner = drawer_interior(&wm, area);
        for r in rows.iter().filter(|r| matches!(r.kind, DrawerRowKind::Pane(_))) {
            let DrawerRowKind::Pane(id) = r.kind else { unreachable!() };
            let line = inner.iter().find(|l| l.contains(&format!("#{id} "))).expect("pane line");
            assert_eq!(line.starts_with('›'), id == fid, "marker mismatch for #{id}: {line:?}");
        }
    });
}

#[test]
fn tree_one_line_per_pane_in_liquid_glass() {
    with_temp_xdg(|| {
        let (mut wm, area) = herdr_wm(100, 30);
        run_colon(&mut wm, ":liquid on");
        open_drawer(&mut wm);
        let lines = buffer_lines(&render(&wm, area));
        let a = lines.iter().position(|l| l.contains("#1 alpha")).expect("alpha line");
        let b = lines.iter().position(|l| l.contains("#2 beta")).expect("beta line");
        assert_eq!(b, a + 1, "rows must be single-line (no 2-row cards)");
    });
}

#[test]
fn tree_sanitizes_titles() {
    with_temp_xdg(|| {
        let (mut wm, area) = herdr_wm(100, 30);
        wm.pane_mut(1).unwrap().set_title("ev\u{202E}il\u{009B}title");
        wm.pane_mut(2).unwrap().set_title("L".repeat(500));
        open_drawer(&mut wm);
        for r in wm.drawer_rows() {
            assert!(!r.title.contains('\u{202E}') && !r.title.contains('\u{009B}'), "{:?}", r.title);
            assert!(r.title.chars().count() <= 64);
        }
        let inner = drawer_interior(&wm, area);
        let text = inner.join("\n");
        assert!(text.contains("#1 eviltitle"), "sanitised title missing:\n{text}");
        assert!(!text.contains('\u{202E}') && !text.contains('\u{009B}'));
        let long = inner.iter().find(|l| l.contains("#2 LLL")).expect("long title line");
        assert!(long.contains('…'), "truncated title needs an ellipsis: {long:?}");
    });
}

#[test]
fn tree_ascii_glyph_pack() {
    with_temp_xdg(|| {
        let (mut wm, area) = herdr_wm(100, 30);
        run_colon(&mut wm, ":set glyphs ascii");
        open_drawer(&mut wm);
        let inner = drawer_interior(&wm, area);
        assert!(inner.iter().any(|l| l.contains("#1 alpha")));
        for l in &inner {
            assert!(l.is_ascii(), "non-ASCII glyph in ascii pack: {l:?}");
        }
        assert!(inner.iter().any(|l| l.starts_with('>')), "ascii current marker missing");
    });
}

#[test]
fn tree_width_resize_writes_herdr_width() {
    with_temp_xdg(|| {
        let (mut wm, area) = herdr_wm(100, 30);
        run_colon(&mut wm, ":drawer size 30");
        open_drawer(&mut wm);
        assert_eq!(wm.drawer_config().herdr_width_cols, 30);
        assert_eq!(wm.drawer_config().width_cols, 34);
        assert_eq!(wm.drawer_rect(area).unwrap().width, 30);
    });
}

#[test]
fn tree_top_edge_renders_vertical_list() {
    with_temp_xdg(|| {
        let (mut wm, area) = herdr_wm(100, 30);
        run_colon(&mut wm, ":drawer top");
        open_drawer(&mut wm);
        let r = wm.drawer_rect(area).unwrap();
        assert_eq!(r.width, 100);
        let lines = buffer_lines(&render(&wm, area));
        let a = lines.iter().position(|l| l.contains("#1 alpha")).expect("alpha line");
        let b = lines.iter().position(|l| l.contains("#2 beta")).expect("beta line");
        assert_eq!(b, a + 1);
        assert!(a as u16 >= r.y && (b as u16) < r.y + r.height, "pane lines must sit inside the top drawer rect");
    });
}

// ---- Phase 50-04: state dots, counts, attention navigation (FLT-01, D-04/D-05) ----

/// Three-pane herdr fixture (ids 1..=3), titles alpha/beta/gamma.
fn three_pane_herdr(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let (mut wm, area) = herdr_wm(cols, rows);
    let p3 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.pane_mut(p3).unwrap().process_pty_output(b"\x1b]2;gamma\x07GAMMA_OUT\r\n");
    wm.resize_all(area).unwrap();
    (wm, area)
}

#[test]
fn attention_tracer_blocked_dot_and_jump() {
    with_temp_xdg(|| {
        let (mut wm, area) = three_pane_herdr(100, 30);
        wm.focus_and_raise(1);
        wm.report_agent_state(3, Some(AgentState::Blocked));
        wm.update_agent_states(std::time::Instant::now());
        open_drawer(&mut wm);
        let inner = drawer_interior(&wm, area);
        let line = inner.iter().find(|l| l.contains("#3 ")).expect("#3 line");
        assert!(line.contains('▲'), "blocked dot missing: {line:?}");
        run_colon(&mut wm, ":drawer attention");
        assert_eq!(wm.active_pane_id(), 3);
    });
}
