//! Phase 50-01: classic slide-out drawer parity goldens (FLT-01, D-02).
//!
//! These snapshots were generated on the UNTOUCHED tree (before any Phase 50 `src/` edit) and must
//! never be regenerated in Phase 50. They pin cell symbols AND styles (via a style digest) of the
//! classic drawer for every edge and look, so any later change to classic output fails this suite.
//! `scripts/check_classic_goldens.sh` proves each golden was committed exactly once.

use std::sync::Mutex;

use bilterm::colon_command::{Action, parse_colon_command};
use bilterm::remote::RemoteHostProfile;
use bilterm::wm::SessionNodeKind;
use bilterm::ui::{WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!(
        "bilterm-drawer-parity-{}-{:?}",
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

/// Two-pane fixture with explicit OSC 2 titles (keeps cwd/home paths out of rendered text).
fn fixture(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    wm.pane_mut(1)
        .unwrap()
        .process_pty_output(b"\x1b]2;alpha\x07ALPHA_OUT\r\n");
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.pane_mut(p2)
        .unwrap()
        .process_pty_output(b"\x1b]2;beta\x07BETA_OUT\r\n");
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn render(wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

fn fnv1a64(bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn snapshot_text(buf: &Buffer) -> String {
    let mut bytes = Vec::new();
    for cell in buf.content() {
        bytes.extend_from_slice(
            format!(
                "{:?}|{:?}|{:?}|{:?};",
                cell.symbol(),
                cell.fg,
                cell.bg,
                cell.modifier
            )
            .as_bytes(),
        );
    }
    format!(
        "{}\nstyle-digest: {:016x}\n",
        buffer_to_snapshot_string(buf),
        fnv1a64(bytes)
    )
}

fn assert_no_host_paths(text: &str) {
    let tmp = std::env::temp_dir().to_string_lossy().to_string();
    for needle in ["/home/", "/tmp/", "/Users/", tmp.as_str()] {
        assert!(!text.contains(needle), "host path {needle:?} leaked into snapshot:\n{text}");
    }
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
    wm.drawer_mut().selected_index = 2;
}

fn finish(name: &str, wm: &WindowManager, area: Rect) {
    let text = snapshot_text(&render(wm, area));
    assert_no_host_paths(&text);
    insta::assert_snapshot!(name, text);
}

#[test]
fn legacy_left() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        open_drawer(&mut wm);
        finish("legacy_left", &wm, area);
    });
}

fn edge_case(name: &str, cmd: &str) {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        run_colon(&mut wm, cmd);
        open_drawer(&mut wm);
        finish(name, &wm, area);
    });
}

#[test]
fn legacy_right() {
    edge_case("legacy_right", ":drawer right");
}

#[test]
fn legacy_top() {
    edge_case("legacy_top", ":drawer top");
}

#[test]
fn legacy_bottom() {
    edge_case("legacy_bottom", ":drawer bottom");
}

#[test]
fn modern_left() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        run_colon(&mut wm, ":modern on");
        open_drawer(&mut wm);
        finish("modern_left", &wm, area);
    });
}

#[test]
fn liquid_glass_left() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        run_colon(&mut wm, ":liquid on");
        open_drawer(&mut wm);
        finish("liquid_glass_left", &wm, area);
    });
}

#[test]
fn maximized_left() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        open_drawer(&mut wm);
        wm.apply(Action::DrawerSize(u16::MAX)).unwrap();
        finish("maximized_left", &wm, area);
    });
}

#[test]
fn overlay_left() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        run_colon(&mut wm, ":drawer float");
        open_drawer(&mut wm);
        finish("overlay_left", &wm, area);
    });
}

#[test]
fn session_tree_left() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        wm.session_tree_mut()
            .upsert_session("child", SessionNodeKind::Independent, Some("default"));
        wm.session_tree_mut()
            .upsert_session("ops", SessionNodeKind::Independent, None);
        open_drawer(&mut wm);
        finish("session_tree_left", &wm, area);
    });
}

#[test]
fn quick_terminal_fleet_left() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        // The visible quake overlay occludes the whole left drawer (top 40% of the screen), so the
        // quick-terminal row cannot be pinned in a full-frame golden; toggle it on and off to
        // exercise the Action path and pin the fleet group with the overlay hidden.
        wm.apply(Action::QuakeToggle).unwrap();
        assert!(wm.is_quick_terminal_visible());
        wm.apply(Action::QuakeToggle).unwrap();
        assert!(!wm.is_quick_terminal_visible());
        wm.fleet_mut().register_profile(RemoteHostProfile {
            name: "prod-db".to_string(),
            destination: "ops@prod-db.example".to_string(),
            port: None,
            identity_file: None,
            control_master: false,
            remote_osc133_bootstrap: false,
            tags: vec!["db".to_string()],
            rpc_endpoint: None,
        });
        open_drawer(&mut wm);
        finish("quick_terminal_fleet_left", &wm, area);
    });
}

#[test]
fn gui_surface_left() {
    use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        open_drawer(&mut wm);
        wm.set_gui_surface(true);
        let mut renderer = GuiWorkspaceRenderer::new(8, 16);
        let mut surface = GuiFrameSurface::new(8, 16);
        renderer.render_workspace_with_cell_size(&mut wm, area, 8, 16, &mut surface);
        let (_, _, rgba) = surface.rasterize_rgba();
        let text = format!(
            "quads: {}\nglyphs: {}\nimages: {}\nrgba-digest: {:016x}\n",
            surface.quads.len(),
            surface.glyphs.len(),
            surface.images.len(),
            fnv1a64(rgba)
        );
        assert_no_host_paths(&text);
        insta::assert_snapshot!("gui_surface_left", text);
    });
}
