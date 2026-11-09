//! Integration tests for the floating `DAEMON SUPERVISOR` window (TUI render, hit-testing,
//! keyboard/mouse routing, and the `:daemon` colon / top-menu entry points).
//!
//! Isolation: no test here starts a daemon. Any path that reads the runtime dir (`:daemon list`,
//! `:daemon status`, footer Refresh) runs under `isolated_runtime()`, which points
//! `XDG_RUNTIME_DIR` and `TMPDIR` at a fresh `TempDir` while holding a process-wide lock.

use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard};

use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::config::BiltermConfig;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::daemon_window_ui::render_daemon_window;
use bilterm::ui::glyphs::GlyphSet;
use bilterm::ui::modals::menubar_item_command;
use bilterm::ui::shell::ShellLayout;
use bilterm::ui::theme::ThemePalette;
use bilterm::wm::daemon_window::{
    DaemonMenuEntry, DaemonWindowButton, DaemonWindowHit, DaemonWindowLayout,
};
use bilterm::wm::{Modal, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

const COLS: u16 = 100;
const ROWS: u16 = 30;

/// Serializes every test that mutates process-global env vars.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Holds the env lock and points the runtime dir at a private temp dir until dropped.
struct IsolatedRuntime {
    _lock: MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
    prev: Vec<(&'static str, Option<OsString>)>,
}

impl Drop for IsolatedRuntime {
    fn drop(&mut self) {
        for (k, v) in &self.prev {
            // SAFETY: serialized by ENV_LOCK, which this guard still holds during restore.
            unsafe {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
    }
}

fn isolated_runtime() -> IsolatedRuntime {
    let lock = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let keys = ["XDG_RUNTIME_DIR", "TMPDIR"];
    let prev = keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
    // SAFETY: serialized by ENV_LOCK; restored in Drop.
    unsafe {
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        std::env::set_var("TMPDIR", dir.path());
    }
    IsolatedRuntime { _lock: lock, _dir: dir, prev }
}

fn full_area() -> Rect {
    Rect::new(0, 0, COLS, ROWS)
}

/// Headless WM (note: `new_headless` takes `(rows, cols)`) with the window open and focused.
fn wm_open() -> WindowManager {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    let d = wm.daemon_window_mut();
    d.active = true;
    d.focused = true;
    wm
}

fn entries_three() -> Vec<DaemonMenuEntry> {
    ["alpha", "beta", "gamma"]
        .iter()
        .map(|n| DaemonMenuEntry { name: (*n).into(), ..Default::default() })
        .collect()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn left_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: col, row, modifiers: KeyModifiers::NONE }
}

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

fn cell_text(buf: &Buffer, x: u16, y: u16, len: usize) -> String {
    (x..x + len as u16).map(|cx| buf[(cx, y)].symbol().to_string()).collect()
}

#[test]
fn tui_render_shows_title_rows_badges_and_footer() {
    let mut wm = wm_open();
    {
        let d = wm.daemon_window_mut();
        d.entries = vec![
            DaemonMenuEntry { name: "default".into(), ..Default::default() },
            DaemonMenuEntry {
                name: "worker-1".into(),
                running: true,
                pid: Some(4242),
                sessions: 1,
                panes: 3,
                socket_path: "/run/x/worker-1.sock".into(),
                ..Default::default()
            },
        ];
        d.status_line = "2 daemons, 1 running".into();
    }
    let ws = full_area();
    let mut buf = Buffer::empty(ws);
    render_daemon_window(&wm, ws, &BiltermConfig::default(), &ThemePalette::default(), &GlyphSet::default(), &mut buf);

    let outer = wm.daemon_window_rect(ws).expect("rect when active");
    let lay = DaemonWindowLayout::new(outer);

    assert!(row_text(&buf, outer.y).contains(" DAEMON SUPERVISOR "), "title missing");
    assert_eq!(cell_text(&buf, lay.close_x, outer.y, 3), "[X]");

    let rows: String = (lay.rows.y..lay.rows.y + 2).map(|y| row_text(&buf, y)).collect();
    assert!(rows.contains("● RUNNING"), "running badge missing: {rows}");
    assert!(rows.contains("○ STOPPED"), "stopped badge missing: {rows}");
    assert!(rows.contains("4242"), "pid missing: {rows}");

    for (b, x, _w) in lay.button_spans() {
        assert_eq!(cell_text(&buf, x, lay.footer_y, b.label().chars().count()), b.label());
    }
    assert_eq!(DaemonWindowButton::ALL.len(), lay.button_spans().len());
}

#[test]
fn gui_overlay_rect_matches_tui_rect() {
    // The GUI path (renderer.rs) takes its overlay rect from `wm.daemon_window_rect(ws)`, the same
    // accessor the TUI renderer uses, so the rect is checked directly rather than through pixels.
    let mut wm = wm_open();
    let ws = full_area();
    let outer = wm.daemon_window_rect(ws).expect("Some when active");
    assert!(outer.x + outer.width <= ws.x + ws.width && outer.y + outer.height <= ws.y + ws.height);

    // Same rect drives the rendered frame at a non-origin workspace.
    let ws2 = Rect::new(5, 3, COLS, ROWS);
    let mut buf = Buffer::empty(ws2);
    render_daemon_window(&wm, ws2, &BiltermConfig::default(), &ThemePalette::default(), &GlyphSet::default(), &mut buf);
    let outer2 = wm.daemon_window_rect(ws2).expect("Some when active");
    let lay2 = DaemonWindowLayout::new(outer2);
    assert_eq!(cell_text(&buf, lay2.close_x, outer2.y, 3), "[X]");

    wm.close_daemon_window();
    assert_eq!(wm.daemon_window_rect(ws), None, "None when closed");
    wm.daemon_window_mut().active = true;
    assert_eq!(wm.daemon_window_rect(Rect::new(0, 0, 39, ROWS)), None, "None when too narrow");
    assert_eq!(wm.daemon_window_rect(Rect::new(0, 0, COLS, 9)), None, "None when too short");
}

#[test]
fn refresh_lists_default_when_registry_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut wm = wm_open();
    wm.refresh_daemon_window_from(dir.path());
    let w = wm.daemon_window();
    assert_eq!(w.entries.len(), 1);
    assert_eq!(w.entries[0].name, "default");
    assert!(!w.entries[0].running);
    assert!(w.entries[0].socket_path.ends_with("daemon.sock"), "{}", w.entries[0].socket_path);
    assert!(w.status_line.contains("0 running"), "{}", w.status_line);
}

#[test]
fn keyboard_navigation_j_k_up_down_and_esc_closes() {
    let mut wm = wm_open();
    wm.daemon_window_mut().entries = entries_three();
    let sel = |wm: &WindowManager| wm.daemon_window().selected;

    handle_wm_key_event(&mut wm, key(KeyCode::Char('j'))).unwrap();
    assert_eq!(sel(&wm), 1);
    handle_wm_key_event(&mut wm, key(KeyCode::Down)).unwrap();
    assert_eq!(sel(&wm), 2);
    handle_wm_key_event(&mut wm, key(KeyCode::Down)).unwrap();
    assert_eq!(sel(&wm), 2, "clamps at last entry");

    handle_wm_key_event(&mut wm, key(KeyCode::Char('k'))).unwrap();
    assert_eq!(sel(&wm), 1);
    handle_wm_key_event(&mut wm, key(KeyCode::Up)).unwrap();
    assert_eq!(sel(&wm), 0);
    handle_wm_key_event(&mut wm, key(KeyCode::Up)).unwrap();
    assert_eq!(sel(&wm), 0, "clamps at first entry");

    handle_wm_key_event(&mut wm, key(KeyCode::Esc)).unwrap();
    assert!(!wm.daemon_window().active, "Esc closes the window");
    assert!(!wm.daemon_window().captures_input());
}

#[test]
fn mouse_row_click_selects_and_close_badge_closes() {
    let mut wm = wm_open();
    wm.daemon_window_mut().entries = entries_three();
    // Dispatch through the real mouse handler using the same workspace rect the app computes.
    let ws = ShellLayout::compute(full_area(), wm.config(), wm.pane_count()).workspace;
    let outer = wm.daemon_window_rect(ws).expect("rect");
    let lay = DaemonWindowLayout::new(outer);

    // Row 2 (index 1) of the list.
    let row2 = lay.rows.y + 1;
    assert_eq!(wm.daemon_window_hit(ws, lay.rows.x + 1, row2), DaemonWindowHit::Row(1));
    handle_wm_mouse_event(&mut wm, left_down(lay.rows.x + 1, row2), full_area()).unwrap();
    assert_eq!(wm.daemon_window().selected, 1);
    assert!(wm.daemon_window().active);

    // [X] badge on the title row.
    handle_wm_mouse_event(&mut wm, left_down(lay.close_x + 1, outer.y), full_area()).unwrap();
    assert!(!wm.daemon_window().active, "[X] click closes the window");
}

#[test]
fn mouse_buttons_hit_test() {
    let mut wm = wm_open();
    wm.daemon_window_mut().entries = entries_three();
    let ws = full_area();
    let outer = wm.daemon_window_rect(ws).unwrap();
    let lay = DaemonWindowLayout::new(outer);

    for (b, x, w) in lay.button_spans() {
        assert_eq!(wm.daemon_window_hit(ws, x, lay.footer_y), DaemonWindowHit::Button(b), "{b:?} start");
        assert_eq!(wm.daemon_window_hit(ws, x + w - 1, lay.footer_y), DaemonWindowHit::Button(b), "{b:?} end");
    }

    // Refresh only re-reads the runtime dir, so it runs against an isolated temp runtime.
    let (_, refresh_x, _) = lay.button_spans().into_iter().find(|(b, _, _)| *b == DaemonWindowButton::Refresh).unwrap();
    {
        let _rt = isolated_runtime();
        let hit = wm.daemon_window_click(ws, refresh_x, lay.footer_y);
        assert_eq!(hit, DaemonWindowHit::Button(DaemonWindowButton::Refresh));
    }
    assert!(wm.daemon_window().active, "Refresh keeps the window open");
    assert!(wm.daemon_window().status_line.contains("daemon"));

    // Close button closes the window. Stop/Start/Switch are intentionally never clicked here.
    let (_, close_x, _) = lay.button_spans().into_iter().find(|(b, _, _)| *b == DaemonWindowButton::Close).unwrap();
    assert_eq!(wm.daemon_window_click(ws, close_x, lay.footer_y), DaemonWindowHit::Button(DaemonWindowButton::Close));
    assert!(!wm.daemon_window().active, "Close button closes the window");
}

#[test]
fn daemon_list_colon_opens_floating_window() {
    let rt = isolated_runtime();
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    let cmd = parse_colon_command(":daemon list").expect("parses").expect("valid");
    execute_wm_colon_command(&mut wm, cmd);
    assert!(wm.daemon_window().active, "window active after :daemon list");
    assert!(wm.daemon_window().focused, "window focused after :daemon list");
    assert!(wm.daemon_window().entries.iter().any(|e| e.name == "default"));
    drop(rt);
}

#[test]
fn daemon_status_colon_opens_floating_window() {
    let rt = isolated_runtime();
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    let cmd = parse_colon_command(":daemon status").expect("parses").expect("valid");
    execute_wm_colon_command(&mut wm, cmd);
    assert!(wm.daemon_window().active, "window active after :daemon status");
    assert!(wm.daemon_window().focused, "window focused after :daemon status");
    assert!(wm.daemon_window().entries.iter().any(|e| e.name == "default"));
    drop(rt);
}

#[test]
fn top_menu_daemon_items_open_floating_window() {
    // Approach: the menubar modal is opened through the public `set_menubar_open_category` (the
    // same state a mouse click on the Daemon title sets), then Down/Enter go through the real
    // `handle_wm_key_event` path, which runs `menubar_item_command` -> parse -> execute.
    // Category 7 is "Daemon" in `MENUBAR_CATEGORIES`. Keyboard Left/Right cannot reach it (see the
    // ignored test below), so the category is selected directly.
    const DAEMON_CATEGORY: usize = 7;
    assert_eq!(menubar_item_command(DAEMON_CATEGORY, 0), Some(":daemon list"));
    assert_eq!(menubar_item_command(DAEMON_CATEGORY, 1), Some(":daemon status"));

    let rt = isolated_runtime();

    // Item 0: List Daemons.
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.set_menubar_open_category(Some(DAEMON_CATEGORY));
    handle_wm_key_event(&mut wm, key(KeyCode::Enter)).unwrap();
    assert!(!matches!(wm.modal(), Modal::Menubar { .. }), "menu closes on Enter");
    assert!(wm.daemon_window().active && wm.daemon_window().focused, "List Daemons opens window");

    // Item 1: Daemon Status.
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.set_menubar_open_category(Some(DAEMON_CATEGORY));
    handle_wm_key_event(&mut wm, key(KeyCode::Down)).unwrap();
    handle_wm_key_event(&mut wm, key(KeyCode::Enter)).unwrap();
    assert!(wm.daemon_window().active && wm.daemon_window().focused, "Daemon Status opens window");
    drop(rt);
}

/// BUG (documented, not fixed): `Modal::Menubar` Left/Right in `src/input/modal_palette.rs` use
/// `% 7` while `MENUBAR_CATEGORIES` has 9 entries, so keyboard navigation never reaches the
/// Daemon (index 7) or Web (index 8) categories.
#[test]
fn menubar_keyboard_right_reaches_daemon_category() {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.set_menubar_open_category(Some(6)); // Help
    handle_wm_key_event(&mut wm, key(KeyCode::Right)).unwrap();
    assert_eq!(wm.menubar_state().map(|(c, _)| c), Some(7), "Right from Help should reach Daemon");
}
