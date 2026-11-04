//! Phase 50-02: herdr-style drawer tree (FLT-01, D-02). `style_` = config seam, `tree_` = row model + renderer.

use std::sync::Mutex;

use bilterm::colon_command::parse_colon_command;
use bilterm::ui::WorkspaceWidget;
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
