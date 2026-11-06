//! Phase 51.1-10: the key and mouse paths of the tmux link banner (UNI-R01), GUI and TUI through the
//! shared handlers, plus the proofs that the global link chords lower to actions and that the
//! banner can never swallow a typed password.

use bilterm::action::Action;
use bilterm::input::keymap::{DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS};
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::input::{AppAction, handle_wm_key_event};
use bilterm::pane::LinkBanner;
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::link_cmd::RemoteLinkOp;
use bilterm::wm::{Focus, SplitDirection, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

const COLS: u16 = 100;
const ROWS: u16 = 30;
const AREA: Rect = Rect { x: 0, y: 0, width: COLS, height: ROWS };

const RECONNECT_STATUS: &str = "No tmux link to reconnect (use :ssh-tmux <dest> [session])";
const DISCONNECT_STATUS: &str = "No tmux link to disconnect";

fn wm_one() -> WindowManager {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.resize_all(AREA).unwrap();
    wm
}

fn banner(text: &str, interactive: bool) -> LinkBanner {
    LinkBanner { text: text.to_string(), interactive }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn ch(c: char) -> KeyEvent {
    key(KeyCode::Char(c))
}

fn status(wm: &WindowManager) -> String {
    wm.active_pane().status_message().unwrap_or("").to_string()
}

fn clear_status(wm: &mut WindowManager) {
    wm.active_pane_mut().set_status_message(None);
}

fn send(wm: &mut WindowManager, k: KeyEvent) -> AppAction {
    handle_wm_key_event(wm, k).unwrap()
}

fn leader(wm: &mut WindowManager, c: char) {
    send(wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));
    send(wm, ch(c));
}

// ---------------------------------------------------------------- Task 1: keys
mod keys {
    use super::*;

    #[test]
    fn banner_keys_reconnect_disconnect_detach() {
        for k in [key(KeyCode::Enter), ch('r'), ch('R')] {
            let mut wm = wm_one();
            wm.active_pane_mut().set_link_banner(Some(banner("link down", true)));
            assert_eq!(send(&mut wm, k), AppAction::Handled);
            assert_eq!(status(&wm), RECONNECT_STATUS, "{k:?}");
        }
        for k in [ch('x'), ch('X')] {
            let mut wm = wm_one();
            wm.active_pane_mut().set_link_banner(Some(banner("link down", true)));
            assert_eq!(send(&mut wm, k), AppAction::Handled);
            assert_eq!(status(&wm), DISCONNECT_STATUS, "{k:?}");
        }
        let mut wm = wm_one();
        wm.active_pane_mut().set_link_banner(Some(banner("link down", true)));
        assert_eq!(send(&mut wm, ch('d')), AppAction::Quit);
        assert!(wm.is_client_detached());

        // Other letters fall through: no status change, the prompt receives the character.
        let mut wm = wm_one();
        wm.active_pane_mut().set_link_banner(Some(banner("link down", true)));
        send(&mut wm, ch('q'));
        assert_eq!(wm.active_pane().prompt().text(), "q");
        assert_eq!(status(&wm), "");
        // Modified keys fall through too (Ctrl+R / Alt+X are not the banner keys).
        let mut wm = wm_one();
        wm.active_pane_mut().set_link_banner(Some(banner("link down", true)));
        send(&mut wm, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        send(&mut wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT));
        assert_ne!(status(&wm), RECONNECT_STATUS);
        assert_ne!(status(&wm), DISCONNECT_STATUS);
        assert!(!wm.is_client_detached());
    }

    #[test]
    fn non_interactive_banner_never_swallows_keys() {
        let mut wm = wm_one();
        {
            let p = wm.active_pane_mut();
            p.set_link_banner(Some(banner("Connecting...", false)));
            p.set_remote_input_tap(true);
            p.set_link_raw_input(true);
        }
        let mut written = Vec::new();
        for k in [ch('r'), ch('x'), ch('d'), key(KeyCode::Enter)] {
            assert_eq!(send(&mut wm, k), AppAction::Handled, "{k:?}");
            written.extend_from_slice(wm.active_pane().last_pty_write());
        }
        assert_eq!(written, b"rxd\r");
        assert_ne!(status(&wm), RECONNECT_STATUS);
        assert_ne!(status(&wm), DISCONNECT_STATUS);
        assert!(wm.active_pane().prompt().history.is_empty());
        assert!(wm.active_pane().prompt().text().is_empty());
        assert!(!wm.is_client_detached());
    }

    #[test]
    fn banner_keys_ignored_with_pending_leader_or_open_modal() {
        // After the leader key, `r`/`x` belong to the leader table, not the banner.
        let mut wm = wm_one();
        wm.active_pane_mut().set_link_banner(Some(banner("link down", true)));
        send(&mut wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));
        assert!(wm.is_leader_pending());
        send(&mut wm, ch('x'));
        assert_ne!(status(&wm), DISCONNECT_STATUS);
        assert!(!wm.is_leader_pending());

        // With the palette open, `x` types into the palette.
        let mut wm = wm_one();
        wm.active_pane_mut().set_link_banner(Some(banner("link down", true)));
        send(&mut wm, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::ALT));
        assert!(!matches!(wm.modal(), bilterm::wm::Modal::None), "palette should be open");
        send(&mut wm, ch('x'));
        assert_ne!(status(&wm), DISCONNECT_STATUS);
        assert!(!matches!(wm.modal(), bilterm::wm::Modal::None), "palette must stay open");
    }

    #[test]
    fn leader_and_global_chords_lower_to_actions() {
        let mut wm = wm_one();
        leader(&mut wm, 'u');
        assert_eq!(wm.active_pane().prompt().text(), ":ssh-tmux ");

        let mut wm = wm_one();
        leader(&mut wm, 'U');
        assert_eq!(status(&wm), RECONNECT_STATUS);

        let mut wm = wm_one();
        leader(&mut wm, 'Q');
        assert_eq!(status(&wm), DISCONNECT_STATUS);

        let ctrl_alt_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL | KeyModifiers::ALT);
        let mut wm = wm_one();
        send(&mut wm, ctrl_alt_r);
        assert_eq!(status(&wm), RECONNECT_STATUS);

        // Also in raw passthrough with the input tap on (`reserved_global_action`).
        let mut wm = wm_one();
        {
            let p = wm.active_pane_mut();
            p.set_remote_input_tap(true);
            p.set_link_raw_input(true);
        }
        clear_status(&mut wm);
        send(&mut wm, ctrl_alt_r);
        assert_eq!(status(&wm), RECONNECT_STATUS);
    }

    #[test]
    fn link_chords_are_unique() {
        let ops = [
            RemoteLinkOp::Prefill,
            RemoteLinkOp::Reconnect,
            RemoteLinkOp::Disconnect,
        ];
        // Leader table: each op has a chord, and no leader code is bound twice.
        for op in ops {
            let a = Action::RemoteLink(op);
            let codes: Vec<KeyCode> =
                DEFAULT_LEADER_BINDINGS.iter().filter(|(_, x)| *x == a).map(|(c, _)| *c).collect();
            assert!(!codes.is_empty(), "{a:?} has no leader chord");
            for c in codes {
                let n = DEFAULT_LEADER_BINDINGS.iter().filter(|(k, _)| *k == c).count();
                assert_eq!(n, 1, "leader chord {c:?} for {a:?} bound {n} times");
            }
        }
        // Global table: Reconnect has a chord, and (mods, code) belongs to one action only, in one table.
        let g: Vec<(KeyModifiers, KeyCode)> = DEFAULT_GLOBAL_BINDINGS
            .iter()
            .filter(|(_, _, a)| matches!(a, Action::RemoteLink(_)))
            .map(|(m, c, _)| (*m, *c))
            .collect();
        assert!(!g.is_empty(), "no global link chord");
        for (m, c) in g {
            let in_global = DEFAULT_GLOBAL_BINDINGS.iter().filter(|(m2, c2, _)| *m2 == m && *c2 == c).count();
            let in_pane = DEFAULT_PANE_BINDINGS.iter().filter(|(m2, c2, _)| *m2 == m && *c2 == c).count();
            assert_eq!(in_global, 1, "global chord {m:?}+{c:?} bound {in_global} times");
            assert_eq!(in_pane, 0, "global chord {m:?}+{c:?} also in the pane table");
        }
        // No link action lives in the pane table.
        assert!(DEFAULT_PANE_BINDINGS.iter().all(|(_, _, a)| !matches!(a, Action::RemoteLink(_))));
    }

    #[test]
    fn banner_does_not_change_other_panes() {
        let mut wm = wm_one();
        wm.split_focused(SplitDirection::Vertical).unwrap();
        wm.resize_all(AREA).unwrap();
        let ids: Vec<usize> = wm.panes().keys().copied().collect();
        let (first, second) = (ids[0], ids[1]);
        wm.focus_and_raise(first);
        wm.pane_by_focus_mut(Focus::Pane(second)).unwrap().set_link_banner(Some(banner("link down", true)));
        assert_eq!(wm.active_pane_id(), first);
        send(&mut wm, ch('r'));
        assert_ne!(status(&wm), RECONNECT_STATUS);
        assert_eq!(wm.active_pane().prompt().text(), "r");
    }

}

// ---------------------------------------------------------------- Task 2: mouse

fn render_buf(wm: &WindowManager) -> Buffer {
    let mut buf = Buffer::empty(AREA);
    WorkspaceWidget::new(wm).render(AREA, &mut buf);
    buf
}

/// Finds `needle` in the rendered buffer; returns the `(column, row)` of its first character.
fn find_text(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
    for y in 0..buf.area.height {
        let row: Vec<String> = (0..buf.area.width)
            .map(|x| buf.cell((x, y)).map(|c| c.symbol().to_string()).unwrap_or_default())
            .collect();
        let line = row.concat();
        if let Some(byte) = line.find(needle) {
            let col = line[..byte].chars().count() as u16;
            return Some((col, y));
        }
    }
    None
}

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent { kind, column: col, row, modifiers: KeyModifiers::NONE }
}

fn click(wm: &mut WindowManager, col: u16, row: u16) -> AppAction {
    handle_wm_mouse_event(wm, mouse(MouseEventKind::Down(MouseButton::Left), col, row), AREA).unwrap()
}

const BANNER_TEXT: &str = "LINKBANNER-XYZ";

#[test]
fn click_on_link_banner_reconnects() {
    let mut wm = wm_one();
    wm.active_pane_mut().set_link_banner(Some(banner(BANNER_TEXT, true)));
    wm.active_pane_mut().set_link_raw_input(true);
    let (col, row) = find_text(&render_buf(&wm), BANNER_TEXT).expect("banner rendered");
    assert_eq!(click(&mut wm, col, row), AppAction::Handled);
    assert_eq!(status(&wm), RECONNECT_STATUS);
}

#[test]
fn click_on_held_remote_banner_respawns() {
    let mut wm = wm_one();
    {
        let p = wm.active_pane_mut();
        p.set_remote_target(Some("me@box".to_string()));
        p.hold_exited_shell(Some(255));
        p.set_link_raw_input(true);
    }
    // The default banner starts with a wide emoji; search its ASCII tail so cell widths do not matter.
    let (col, row) = find_text(&render_buf(&wm), "Respawn Shell").expect("held banner rendered");
    assert_eq!(click(&mut wm, col, row), AppAction::Handled);
    assert!(wm.active_pane().held_exited_shell().is_none(), "hold must clear");
    assert_eq!(status(&wm), "Shell respawned cleanly");
}

#[test]
fn click_elsewhere_passes_through() {
    let mut wm = wm_one();
    wm.active_pane_mut().set_link_banner(Some(banner(BANNER_TEXT, true)));
    wm.active_pane_mut().set_link_raw_input(true);
    let (col, row) = find_text(&render_buf(&wm), BANNER_TEXT).unwrap();
    let cases = [
        mouse(MouseEventKind::Down(MouseButton::Left), col, row - 1),
        mouse(MouseEventKind::Down(MouseButton::Right), col, row),
        mouse(MouseEventKind::Down(MouseButton::Middle), col, row),
        mouse(MouseEventKind::Drag(MouseButton::Left), col, row),
        mouse(MouseEventKind::Up(MouseButton::Left), col, row),
        mouse(MouseEventKind::Moved, col, row),
    ];
    for ev in cases {
        let _ = handle_wm_mouse_event(&mut wm, ev, AREA).unwrap();
        assert_ne!(status(&wm), RECONNECT_STATUS, "{ev:?} must not reconnect");
    }

    // Normal routing still runs: a click in the pane keeps focus on it.
    let before = wm.active_pane_id();
    click(&mut wm, col, row - 1);
    assert_eq!(wm.active_pane_id(), before);
    assert_ne!(status(&wm), RECONNECT_STATUS);
}

#[test]
fn click_on_non_interactive_banner_does_nothing_special() {
    let mut wm = wm_one();
    wm.active_pane_mut().set_link_banner(Some(banner(BANNER_TEXT, false)));
    wm.active_pane_mut().set_link_raw_input(true);
    let (col, row) = find_text(&render_buf(&wm), BANNER_TEXT).unwrap();
    click(&mut wm, col, row);
    assert_ne!(status(&wm), RECONNECT_STATUS);
}

#[test]
fn click_focuses_the_clicked_pane() {
    let mut wm = wm_one();
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();
    let ids: Vec<usize> = wm.panes().keys().copied().collect();
    let (first, second) = (ids[0], ids[1]);
    {
        let p = wm.pane_by_focus_mut(Focus::Pane(second)).unwrap();
        p.set_link_banner(Some(banner(BANNER_TEXT, true)));
        p.set_link_raw_input(true);
    }
    // The renderer draws the banner for the active pane only: find its cell while `second` is active.
    wm.focus_and_raise(second);
    let (col, row) = find_text(&render_buf(&wm), BANNER_TEXT).unwrap();
    wm.focus_and_raise(first);
    assert_eq!(wm.active_pane_id(), first);
    assert_eq!(click(&mut wm, col, row), AppAction::Handled);
    assert_eq!(wm.active_pane_id(), second);
    assert_eq!(status(&wm), RECONNECT_STATUS);
}

#[test]
fn gui_and_tui_share_the_handler() {
    let mut results = Vec::new();
    for gui in [false, true] {
        let mut wm = wm_one();
        wm.set_gui_surface(gui);
        wm.active_pane_mut().set_link_banner(Some(banner(BANNER_TEXT, true)));
        wm.active_pane_mut().set_link_raw_input(true);
        let (col, row) = find_text(&render_buf(&wm), BANNER_TEXT).unwrap();
        results.push((click(&mut wm, col, row), status(&wm)));
    }
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0].1, RECONNECT_STATUS);
}
