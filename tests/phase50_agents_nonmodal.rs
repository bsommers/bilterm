//! Agents window follow-ups: the window is a normal, non-modal window (keys follow focus, clicks on
//! other panes pass through), graph edges glow with recent communication, and the communication
//! topology is recorded for the session.

use std::sync::Mutex;

use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::wm::{SplitDirection, WindowManager, rect_contains};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Isolates config/state directories for tests that drive real key dispatch.
fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!(
        "bilterm-agents-nonmodal-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Serialized by ENV_LOCK.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_STATE_HOME", &dir);
        std::env::set_var("XDG_DATA_HOME", &dir);
    }
    let out = f();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn fixture() -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(40, 120);
    let area = Rect::new(0, 0, 120, 40);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn traffic_fixture() -> (WindowManager, Rect) {
    let (mut wm, area) = fixture();
    wm.broadcast_command("echo hi").unwrap();
    wm.open_agents_window(None);
    (wm, area)
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    wm.apply(parsed).unwrap();
}

fn press(wm: &mut WindowManager, code: KeyCode) {
    handle_wm_key_event(wm, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
}

fn mouse(wm: &mut WindowManager, area: Rect, kind: MouseEventKind, col: u16, row: u16) {
    let ev = MouseEvent { kind, column: col, row, modifiers: KeyModifiers::NONE };
    handle_wm_mouse_event(wm, ev, area).unwrap();
}

fn click(wm: &mut WindowManager, area: Rect, col: u16, row: u16) {
    mouse(wm, area, MouseEventKind::Down(MouseButton::Left), col, row);
    mouse(wm, area, MouseEventKind::Up(MouseButton::Left), col, row);
}

fn ws_of(wm: &WindowManager, area: Rect) -> Rect {
    bilterm::ui::shell::ShellLayout::compute(area, wm.config(), wm.pane_count()).workspace
}

fn win_rect(wm: &WindowManager, area: Rect) -> Rect {
    wm.agents_window_rect(ws_of(wm, area)).expect("window rect")
}

fn prompts(wm: &WindowManager) -> Vec<String> {
    wm.panes().values().map(|p| p.prompt().text()).collect()
}

/// A workspace cell outside the window, inside pane `1` (left) or `2` (right).
fn outside_cell(wm: &WindowManager, area: Rect, right: bool) -> (u16, u16) {
    let ws = ws_of(wm, area);
    let r = win_rect(wm, area);
    let col = if right { ws.x + ws.width - 1 } else { ws.x };
    let row = ws.y + ws.height - 1;
    assert!(!rect_contains(r, col, row), "cell must lie outside the window");
    (col, row)
}

// ---- Non-modal window ----

#[test]
fn nonmodal_click_on_other_pane_passes_through_and_blurs() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        wm.focus_and_raise(1);
        wm.agents_window_mut().focused = true;
        assert!(wm.agents_window().captures_input());
        let (c, r) = outside_cell(&wm, area, true);
        click(&mut wm, area, c, r);
        assert_eq!(wm.active_pane_id(), 2, "the click reached the pane under the pointer");
        assert!(wm.agents_window().active, "the window stays open");
        assert!(!wm.agents_window().focused, "the click took focus away from the window");
        assert!(!wm.agents_window().captures_input());
    });
}

#[test]
fn nonmodal_keys_go_to_pane_when_window_not_focused() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let (c, r) = outside_cell(&wm, area, true);
        click(&mut wm, area, c, r);
        let before = prompts(&wm);
        let maximized = wm.agents_window().maximized;
        press(&mut wm, KeyCode::Char('m'));
        assert_eq!(wm.agents_window().maximized, maximized, "`m` did not reach the window");
        assert_ne!(prompts(&wm), before, "`m` was typed into the active pane");
        press(&mut wm, KeyCode::Esc);
        assert!(wm.agents_window().active, "Esc is not the window's while it has no focus");
    });
}

#[test]
fn nonmodal_click_inside_window_takes_focus_back() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let (c, r) = outside_cell(&wm, area, false);
        click(&mut wm, area, c, r);
        assert!(!wm.agents_window().captures_input());
        let w = win_rect(&wm, area);
        click(&mut wm, area, w.x + w.width / 2, w.y + w.height / 2);
        assert!(wm.agents_window().captures_input(), "a click inside focuses the window");
        press(&mut wm, KeyCode::Char('m'));
        assert!(wm.agents_window().maximized, "keys go to the focused window");
    });
}

#[test]
fn nonmodal_pane_focus_change_releases_window_focus() {
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        assert!(wm.agents_window().captures_input());
        wm.focus_and_raise(1);
        assert!(wm.agents_window().active);
        assert!(!wm.agents_window().captures_input(), "focusing a pane (leader keys) blurs the window");
        run_colon(&mut wm, ":agents zoom");
        assert!(wm.agents_window().captures_input(), "`:agents zoom` focuses it again");
    });
}

#[test]
fn nonmodal_blur_command_releases_focus_without_closing() {
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        run_colon(&mut wm, ":agents blur");
        assert!(wm.agents_window().active && !wm.agents_window().focused);
        assert!(parse_colon_command(":agents blur extra").unwrap().is_err());
    });
}

#[test]
fn nonmodal_exit_guard_still_wins_over_focused_window() {
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        wm.open_exit_guard("test trigger");
        press(&mut wm, KeyCode::Esc);
        assert!(wm.exit_guard_state().is_none(), "Esc cancelled the guard");
        assert!(wm.agents_window().active, "the window was not touched");
    });
}
