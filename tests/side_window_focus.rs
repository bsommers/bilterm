//! `FOCUS-01`: keyboard capture and focus of the side windows.
//!
//! One keyboard owner at a time; selecting a row hands the pane the keyboard in one step; Esc
//! releases a side window without closing it; `Ctrl+Alt+O` / `:focus cycle` walks the keyboard
//! pane -> open side windows -> pane; the owner is visible in the status row.

use std::sync::Mutex;

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::keymap::default_global_action;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::shell::ShellLayout;
use bilterm::wm::keyboard_focus::{FocusOp, KeyboardOwner};
use bilterm::wm::{SplitDirection, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

static ENV_LOCK: Mutex<()> = Mutex::new(());
const CTRL_ALT: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);
const NONE: KeyModifiers = KeyModifiers::NONE;
const AREA: Rect = Rect { x: 0, y: 0, width: 160, height: 44 };

/// Isolates config/state directories for tests that drive real key dispatch.
fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!(
        "bilterm-side-focus-{}-{:?}",
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

fn fixture(panes: usize) -> WindowManager {
    let mut wm = WindowManager::new_headless(AREA.height, AREA.width);
    for _ in 1..panes {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(AREA).unwrap();
    wm
}

fn colon(wm: &mut WindowManager, line: &str) {
    let action = parse_colon_command(line).unwrap_or_else(|| panic!("{line} parses")).unwrap_or_else(|e| panic!("{line}: {e}"));
    wm.apply(action).unwrap();
}

fn press(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap();
}

fn click(wm: &mut WindowManager, col: u16, row: u16) {
    let ev = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: col, row, modifiers: NONE };
    handle_wm_mouse_event(wm, ev, AREA).unwrap();
}

fn ws_of(wm: &WindowManager) -> Rect {
    ShellLayout::compute(AREA, wm.config(), wm.pane_count()).workspace
}

fn list_row_cell(wm: &WindowManager, i: usize) -> (u16, u16) {
    let l = wm.agents_list_layout(ws_of(wm)).expect("list layout");
    (l.rows_area.x + 2, l.rows_area.y + i as u16)
}

fn screen(wm: &WindowManager) -> Vec<String> {
    let mut buf = Buffer::empty(AREA);
    WorkspaceWidget::new(wm).render(AREA, &mut buf);
    (0..AREA.height)
        .map(|y| (0..AREA.width).map(|x| buf[(x, y)].symbol().to_string()).collect())
        .collect()
}

/// Every window that claims the keyboard, read from the public state.
fn claimants(wm: &WindowManager) -> Vec<&'static str> {
    let mut v = Vec::new();
    if wm.agents_list().captures_input() { v.push("agents-list"); }
    if wm.agents_window().captures_input() { v.push("agents"); }
    if wm.session_map().captures_input() { v.push("sessions"); }
    if wm.clipboard_window().captures_input() { v.push("clipboard"); }
    if wm.traffic_monitor().captures_input() { v.push("traffic"); }
    if wm.script_studio().captures_input() { v.push("script"); }
    if wm.command_window().captures_input() { v.push("commands"); }
    if wm.fonts_window().captures_input() { v.push("fonts"); }
    if wm.agents_hud().captures_input() { v.push("hud"); }
    if wm.drawer().focused { v.push("drawer"); }
    v
}

// ------------------------------------------------------------ 1. single keyboard owner

#[test]
fn opening_any_side_window_leaves_exactly_one_keyboard_owner() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        let steps: [(&str, KeyboardOwner, &str); 8] = [
            (":agents list open", KeyboardOwner::AgentsList, "agents-list"),
            (":traffic open", KeyboardOwner::TrafficMonitor, "traffic"),
            (":clipboard open", KeyboardOwner::Clipboard, "clipboard"),
            (":sessions map open", KeyboardOwner::SessionMap, "sessions"),
            (":drawer focus", KeyboardOwner::Drawer, "drawer"),
            (":agents zoom", KeyboardOwner::AgentsWindow, "agents"),
            (":script open", KeyboardOwner::ScriptStudio, "script"),
            (":agents list open", KeyboardOwner::AgentsList, "agents-list"),
        ];
        for (line, owner, name) in steps {
            colon(&mut wm, line);
            assert_eq!(wm.keyboard_owner(), owner, "after {line}");
            assert_eq!(claimants(&wm), vec![name], "after {line}: one owner, the newly focused window");
        }
        assert!(wm.traffic_monitor().active() && wm.clipboard_window().active, "blurred windows stay open");
        assert!(wm.session_map().active && wm.drawer().open && wm.script_studio().active);
    });
}

#[test]
fn focusing_a_pane_releases_every_side_window_and_the_drawer() {
    with_temp_xdg(|| {
        for line in [":focus 1", ":focus left", ":deck next", ":focus pane"] {
            let mut wm = fixture(2);
            colon(&mut wm, ":agents list open");
            colon(&mut wm, ":drawer focus");
            colon(&mut wm, line);
            assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane, "after {line}");
            assert!(claimants(&wm).is_empty(), "after {line}");
            assert!(wm.agents_list().active && wm.drawer().open, "{line} closes nothing");
        }
    });
}

#[test]
fn a_press_inside_a_side_window_takes_the_keyboard_from_the_drawer() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        colon(&mut wm, ":agents list open");
        colon(&mut wm, ":drawer focus");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer);
        let l = wm.agents_list_layout(ws_of(&wm)).expect("layout");
        // The column header is not a row: a press there only focuses the window.
        click(&mut wm, l.header.x + 2, l.header.y);
        assert_eq!(claimants(&wm), vec!["agents-list"]);
    });
}

// ------------------------------------------------------------ 2. selecting hands over the keyboard

#[test]
fn one_click_on_an_agents_list_row_gives_that_pane_the_keyboard() {
    with_temp_xdg(|| {
        for focused_first in [false, true] {
            let mut wm = fixture(3);
            colon(&mut wm, ":agents list open");
            if !focused_first {
                colon(&mut wm, ":agents list blur");
            }
            let target = wm.agents_list_rows()[0].pane_id;
            assert_ne!(wm.active_pane_id(), target, "the fixture starts on another pane");
            let (c, r) = list_row_cell(&wm, 0);
            click(&mut wm, c, r);
            assert_eq!(wm.active_pane_id(), target, "focused_first={focused_first}: one click jumps");
            assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane);
            assert!(wm.agents_list().active, "the list stays open");
            assert_eq!(wm.agents_list().selected, Some(target));
            press(&mut wm, KeyCode::Char('l'), NONE);
            press(&mut wm, KeyCode::Char('s'), NONE);
            assert_eq!(wm.active_pane().prompt().text(), "ls", "typing reaches the selected pane at once");
        }
    });
}

#[test]
fn arrows_browse_the_list_and_enter_hands_over_the_keyboard() {
    with_temp_xdg(|| {
        let mut wm = fixture(3);
        colon(&mut wm, ":agents list open");
        let start = wm.active_pane_id();
        press(&mut wm, KeyCode::Up, NONE);
        press(&mut wm, KeyCode::Up, NONE);
        let selected = wm.agents_list().selected.expect("a row is selected");
        assert_ne!(selected, start, "the selection moved");
        assert_eq!(wm.active_pane_id(), start, "browsing does not jump");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::AgentsList);
        press(&mut wm, KeyCode::Enter, NONE);
        assert_eq!(wm.active_pane_id(), selected);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane);
        assert!(wm.agents_list().active, "Enter leaves the unpinned list open");
    });
}

#[test]
fn the_drawer_keeps_the_keyboard_after_enter_and_tab_hands_it_to_the_selected_pane() {
    // The drawer's "stay focused in place" guarantee (tests/slide_out_drawer_and_telemetry.rs) is
    // kept: Enter raises the pane and keeps browsing; Tab is the hand-over.
    with_temp_xdg(|| {
        let mut wm = fixture(3);
        colon(&mut wm, ":drawer focus");
        let rows = wm.drawer_rows();
        let (idx, target) = rows
            .iter()
            .enumerate()
            .find_map(|(i, r)| match r.kind {
                bilterm::wm::DrawerRowKind::Pane(id) if id != wm.active_pane_id() => Some((i, id)),
                _ => None,
            })
            .expect("a row for another pane");
        wm.apply(Action::DrawerNav(bilterm::wm::drawer::DrawerNavOp::Row(idx))).unwrap();
        press(&mut wm, KeyCode::Enter, NONE);
        assert_eq!(wm.active_pane_id(), target);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer);
        press(&mut wm, KeyCode::Tab, NONE);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane);
        assert!(wm.drawer().open, "the drawer stays open");
        press(&mut wm, KeyCode::Char('x'), NONE);
        assert_eq!(wm.active_pane().prompt().text(), "x");
    });
}

// ------------------------------------------------------------ 3. Esc releases, it does not destroy

#[test]
fn esc_releases_a_side_window_and_leaves_it_open() {
    with_temp_xdg(|| {
        type Open = fn(&WindowManager) -> bool;
        let cases: [(&str, Open); 7] = [
            (":agents list open", |w| w.agents_list().active),
            (":agents zoom", |w| w.agents_window().active),
            (":sessions map open", |w| w.session_map().active),
            (":clipboard open", |w| w.clipboard_window().active),
            (":traffic open", |w| w.traffic_monitor().active()),
            (":script open", |w| w.script_studio().active),
            (":commands open", |w| w.command_window().active),
        ];
        for (line, is_open) in cases {
            let mut wm = fixture(2);
            colon(&mut wm, line);
            assert_ne!(wm.keyboard_owner(), KeyboardOwner::Pane, "{line} takes the keyboard");
            press(&mut wm, KeyCode::Esc, NONE);
            assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane, "Esc after {line} returns to the pane");
            assert!(is_open(&wm), "Esc after {line} leaves the window open");
            press(&mut wm, KeyCode::Char('x'), NONE);
            assert_eq!(wm.active_pane().prompt().text(), "x", "after {line} + Esc typing reaches the shell");
        }
    });
}

#[test]
fn q_still_closes_the_list_windows() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        colon(&mut wm, ":agents list open");
        press(&mut wm, KeyCode::Char('q'), NONE);
        assert!(!wm.agents_list().active, "q closes the agents list");
        colon(&mut wm, ":traffic open");
        press(&mut wm, KeyCode::Char('q'), NONE);
        assert!(!wm.traffic_monitor().active(), "q closes the traffic monitor");
        colon(&mut wm, ":clipboard open");
        press(&mut wm, KeyCode::Char('q'), NONE);
        assert!(!wm.clipboard_window().active, "q closes the clipboard viewer");
    });
}

#[test]
fn esc_in_the_command_window_only_blurs_when_nothing_is_typed() {
    with_temp_xdg(|| {
        let mut wm = fixture(1);
        colon(&mut wm, ":commands open");
        press(&mut wm, KeyCode::Char('z'), NONE);
        assert_eq!(wm.command_window().query, "z");
        press(&mut wm, KeyCode::Esc, NONE);
        assert!(!wm.command_window().active, "Esc with a query keeps dismissing the unpinned window");
    });
}

#[test]
fn the_modal_pickers_still_close_on_esc() {
    with_temp_xdg(|| {
        let mut wm = fixture(1);
        colon(&mut wm, ":fonts open");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Fonts);
        press(&mut wm, KeyCode::Esc, NONE);
        assert!(!wm.fonts_window().active);
        colon(&mut wm, ":agents hud open");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::AgentsHud);
        press(&mut wm, KeyCode::Esc, NONE);
        assert!(!wm.agents_hud().visible());
    });
}

// ------------------------------------------------------------ 4. a keyboard route

#[test]
fn ctrl_alt_o_walks_pane_then_side_windows_left_to_right_then_pane() {
    with_temp_xdg(|| {
        for code in [KeyCode::Char('o'), KeyCode::Char('O')] {
            assert_eq!(
                default_global_action(KeyEvent::new(code, CTRL_ALT), false),
                Some(Action::KeyboardFocus(FocusOp::Next))
            );
        }
        let mut wm = fixture(2);
        colon(&mut wm, ":agents list open"); // right-aligned
        colon(&mut wm, ":drawer focus"); // left edge
        colon(&mut wm, ":focus pane");
        assert_eq!(wm.keyboard_focus_ring(), vec![KeyboardOwner::Drawer, KeyboardOwner::AgentsList]);
        let mut seen = Vec::new();
        for _ in 0..4 {
            press(&mut wm, KeyCode::Char('o'), CTRL_ALT);
            seen.push(wm.keyboard_owner());
            assert!(claimants(&wm).len() <= 1);
        }
        assert_eq!(
            seen,
            vec![KeyboardOwner::Drawer, KeyboardOwner::AgentsList, KeyboardOwner::Pane, KeyboardOwner::Drawer]
        );
        colon(&mut wm, ":focus cycle prev");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane);
        colon(&mut wm, ":focus cycle prev");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::AgentsList);
        assert!(wm.agents_list().active && wm.drawer().open, "cycling closes nothing");
    });
}

#[test]
fn the_cycle_chord_works_in_raw_passthrough_and_does_nothing_without_side_windows() {
    with_temp_xdg(|| {
        let mut wm = fixture(1);
        colon(&mut wm, ":raw");
        press(&mut wm, KeyCode::Char('o'), CTRL_ALT);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane, "nothing to cycle to");
        assert!(
            wm.active_pane().status_message().is_none_or(|m| !m.contains("side window")),
            "with nothing to cycle the chord is left to the program in the pane"
        );
        colon(&mut wm, ":agents list open");
        colon(&mut wm, ":focus pane");
        press(&mut wm, KeyCode::Char('o'), CTRL_ALT);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::AgentsList, "the chord is reserved over raw passthrough");
    });
}

#[test]
fn focus_by_name_takes_an_open_window_and_reports_a_closed_one() {
    with_temp_xdg(|| {
        let mut wm = fixture(1);
        colon(&mut wm, ":focus traffic");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane);
        assert!(!wm.traffic_monitor().active(), ":focus never opens a window");
        let msg = wm.active_pane().status_message().unwrap_or_default().to_string();
        assert!(msg.contains("not open"), "{msg}");
        colon(&mut wm, ":traffic open");
        colon(&mut wm, ":agents list open");
        colon(&mut wm, ":focus traffic");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::TrafficMonitor);
        assert_eq!(claimants(&wm), vec!["traffic"]);
    });
}

// ------------------------------------------------------------ 5. ownership is visible

#[test]
fn the_status_row_names_the_window_that_holds_the_keyboard() {
    with_temp_xdg(|| {
        let mut wm = fixture(1);
        let plain = screen(&wm);
        assert!(!plain.iter().any(|l| l.contains("KBD:")), "no badge while the pane owns the keyboard");
        colon(&mut wm, ":agents list open");
        let owned = screen(&wm);
        assert!(owned.iter().any(|l| l.contains("KBD:AGENT-LIST")), "badge while the list owns the keyboard");
        colon(&mut wm, ":focus pane");
        assert!(wm.agents_list().active);
        let released = screen(&wm);
        assert!(!released.iter().any(|l| l.contains("KBD:")), "badge gone once the pane has the keyboard back");
        // The drawer is the exception: the classic drawer goldens pin the pane chrome with it focused.
        colon(&mut wm, ":drawer focus");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer);
        assert_eq!(wm.keyboard_owner_shown(), KeyboardOwner::Pane);
        assert!(!screen(&wm).iter().any(|l| l.contains("KBD:")));
    });
}

#[test]
fn the_pane_frame_is_drawn_inactive_while_a_side_window_holds_the_keyboard() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        // Classic chrome: the active frame is double-line (`╔`), an inactive one single-line (`┌`).
        let corner = |wm: &WindowManager| {
            let rects = wm.compute_workspace_rects(ws_of(wm));
            let (_, outer) = rects.iter().find(|(id, _)| *id == wm.active_pane_id()).expect("active pane rect");
            let mut buf = Buffer::empty(AREA);
            WorkspaceWidget::new(wm).render(AREA, &mut buf);
            buf[(outer.x, outer.y)].symbol().to_string()
        };
        let active = corner(&wm);
        colon(&mut wm, ":traffic open");
        colon(&mut wm, ":traffic rect 10 10 60 14");
        let owned = corner(&wm);
        assert_ne!(active, owned, "the pane frame changes while the traffic monitor owns the keyboard");
        colon(&mut wm, ":focus pane");
        assert_eq!(corner(&wm), active, "and is restored with the keyboard");
    });
}

#[test]
fn a_modal_mcp_console_takes_the_keyboard_once_and_ownership_stays_stable() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        colon(&mut wm, ":agents list open");
        colon(&mut wm, ":mcp");
        assert!(wm.mcp_console().captures_input(), "the console opened");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::McpConsole, "an opening modal console takes the keyboard");
        assert!(wm.agents_list().active && !wm.agents_list().focused);
        // A window focused over the open console keeps the keyboard across later settles.
        colon(&mut wm, ":agents list open");
        for _ in 0..3 {
            wm.settle_keyboard_owner();
            assert_eq!(wm.keyboard_owner(), KeyboardOwner::AgentsList);
        }
    });
}
