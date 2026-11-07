//! `KeyEventKind::Repeat` handling: auto-repeat of a chord the WM consumes as a toggle is dropped,
//! while auto-repeat of keys forwarded to the pane (raw/alt-screen passthrough) is preserved.
//! Supersedes #20 (the filter there ran before the passthrough decision).

use bilterm::input::handle_wm_key_event;
use bilterm::pty::line_discipline::InputRoutingMode;
use bilterm::wm::{Modal, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

fn repeat(mut key: KeyEvent) -> KeyEvent {
    key.kind = KeyEventKind::Repeat;
    key
}

fn alt_screen_wm() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.active_pane_mut().process_pty_output(b"\x1b[?1049h");
    wm.active_pane_mut().refresh_line_discipline();
    assert_eq!(
        wm.active_pane().effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );
    wm
}

#[test]
fn repeat_of_wm_toggle_chord_fires_once() {
    let mut wm = WindowManager::new_headless(24, 80);
    let alt_f7 = KeyEvent::new(KeyCode::F(7), KeyModifiers::ALT);
    handle_wm_key_event(&mut wm, alt_f7).unwrap();
    assert!(wm.drawer_is_open());
    for _ in 0..5 {
        handle_wm_key_event(&mut wm, repeat(alt_f7)).unwrap();
        assert!(wm.drawer_is_open(), "Alt+F7 repeat must not flip the drawer");
    }

    let ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
    let mut wm = WindowManager::new_headless(24, 80);
    handle_wm_key_event(&mut wm, ctrl_p).unwrap();
    assert!(matches!(wm.modal(), Modal::Palette { .. }));
    handle_wm_key_event(&mut wm, repeat(ctrl_p)).unwrap();
    assert!(
        matches!(wm.modal(), Modal::Palette { .. }),
        "Ctrl+P repeat must not close the palette"
    );
}

#[test]
fn repeat_of_intercepted_toggle_in_raw_passthrough_fires_once() {
    // Alt+F7 is intercepted even over raw passthrough; its repeat must not flicker the drawer.
    let mut wm = alt_screen_wm();
    let alt_f7 = KeyEvent::new(KeyCode::F(7), KeyModifiers::ALT);
    handle_wm_key_event(&mut wm, alt_f7).unwrap();
    assert!(wm.drawer_is_open());
    handle_wm_key_event(&mut wm, repeat(alt_f7)).unwrap();
    assert!(wm.drawer_is_open());
}

#[test]
fn repeat_of_forwarded_keys_in_alt_screen_is_still_forwarded() {
    let keys = [
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE),
    ];
    for k in keys {
        let mut wm = alt_screen_wm();
        handle_wm_key_event(&mut wm, k).unwrap();
        let first = wm.active_pane().last_pty_write().to_vec();
        assert!(!first.is_empty(), "press of {k:?} should be forwarded");
        wm.active_pane_mut().write_to_pty(b"").unwrap();
        assert!(wm.active_pane().last_pty_write().is_empty());
        handle_wm_key_event(&mut wm, repeat(k)).unwrap();
        assert_eq!(
            wm.active_pane().last_pty_write(),
            first.as_slice(),
            "repeat of {k:?} must be forwarded exactly like the press"
        );
    }
}
