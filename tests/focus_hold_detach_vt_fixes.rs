//! Regression tests for five audited defects: floating-window focus exclusivity (drawer open and
//! pane raise), held ssh pane respawn, the stale client-detached latch, and a `CSI ?1049l` split
//! across two PTY reads.

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, AppAction};
use bilterm::pane::ExitProtectionPolicy;
use bilterm::vt::VtParser;
use bilterm::wm::{SplitDirection, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn colon(wm: &mut WindowManager, cmd: &str) -> AppAction {
    let action: Action = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("{cmd} is not a colon command"))
        .unwrap_or_else(|e| panic!("{cmd}: {e}"));
    wm.apply(action).unwrap()
}

#[test]
fn opening_the_drawer_takes_the_keyboard_from_every_floating_window() {
    for open in [":clipboard", ":agents hud"] {
        let mut wm = WindowManager::new_headless(24, 80);
        colon(&mut wm, open);
        assert!(
            wm.clipboard_window().captures_input() || wm.agents_hud().captures_input(),
            "{open} should focus its window"
        );
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)).unwrap();
        assert!(wm.drawer().focused, "{open}: F7 focuses the drawer");
        assert!(!wm.clipboard_window().captures_input(), "{open}: clipboard blurred");
        assert!(!wm.agents_hud().captures_input(), "{open}: HUD blurred");
    }
}

#[test]
fn raising_a_pane_blurs_the_hud_and_the_replay_view_without_closing_them() {
    let mut wm = WindowManager::new_headless(24, 80);
    let first = wm.active_pane_id();
    let second = wm.split_focused(SplitDirection::Vertical).unwrap();

    colon(&mut wm, ":agents hud");
    assert!(wm.agents_hud().captures_input());
    wm.focus_and_raise(first);
    assert!(!wm.agents_hud().captures_input());
    assert!(wm.agents_hud().visible(), "the HUD stays on screen");

    colon(&mut wm, ":agent replay");
    assert!(wm.agents_hud().captures_input(), "opening the replay view focuses it");
    wm.focus_and_raise(second);
    assert!(!wm.agents_hud().captures_input());
    assert!(wm.agents_hud().replay_pane().is_some(), "the replay view stays open");

    colon(&mut wm, ":agents hud");
    colon(&mut wm, ":agents hud");
    wm.shuffle_deck_next();
    assert!(!wm.agents_hud().captures_input(), "deck shuffle blurs too");
}

#[cfg(unix)]
#[test]
fn respawning_a_held_remote_pane_spawns_a_new_pty() {
    use bilterm::pane::CompositePane;
    use bilterm::pty::PtyConfig;

    let cfg = PtyConfig {
        rows: 24,
        cols: 80,
        program: "sh".to_string(),
        args: vec!["-c".to_string(), "sleep 30".to_string()],
        ..PtyConfig::default()
    };
    let mut pane = CompositePane::spawn(1, cfg).expect("spawn sh");
    pane.set_remote_target(Some("me@box".to_string()));
    assert!(!pane.is_headless());

    pane.hold_remote_exit(Some(255), Some("Connection closed"));
    assert!(pane.is_headless(), "the hold drops the PTY");
    assert!(pane.held_exited_shell().is_some_and(|h| h.had_pty));

    pane.respawn_shell_in_place().expect("respawn");
    assert!(pane.held_exited_shell().is_none());
    assert!(!pane.is_headless(), "the respawn must own a live PTY again");
}

#[test]
fn respawning_a_held_headless_pane_stays_headless() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.active_pane_mut().hold_remote_exit(Some(1), None);
    wm.active_pane_mut().respawn_shell_in_place().unwrap();
    assert!(wm.active_pane().is_headless());
}

#[test]
fn an_earlier_detach_does_not_let_ctrl_d_skip_the_exit_guard() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().exit_protection = ExitProtectionPolicy::Guard;
    // What a `:detach` sent over the live RPC socket leaves behind: the latch, and a running TUI.
    wm.detach_client();
    assert!(wm.is_client_detached());

    let out = handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL))
        .unwrap();
    assert_ne!(out, AppAction::Quit, "Ctrl+D must wait for the guard's answer");
    assert!(wm.exit_guard_state().is_some(), "the exit guard is open");
    assert!(!wm.is_client_detached());
}

#[test]
fn an_earlier_detach_does_not_let_quit_skip_the_exit_guard() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().exit_protection = ExitProtectionPolicy::Guard;
    wm.detach_client();

    assert_ne!(colon(&mut wm, ":quit"), AppAction::Quit);
    assert!(wm.exit_guard_state().is_some());
}

#[test]
fn alt_screen_exit_split_across_reads_clears_the_alt_screen_flag() {
    for (head, tail) in [
        (&b"\x1b[?1049"[..], &b"l"[..]),
        (b"\x1b[", b"?1049l"),
        (b"\x1b[?10", b"49l"),
        (b"\x1b", b"[?1049l"),
    ] {
        let mut vt = VtParser::new(24, 80, 100);
        vt.process(b"\x1b[?1049h");
        assert!(vt.is_alternate_screen());
        vt.process(head);
        vt.process(tail);
        assert!(!vt.is_alternate_screen(), "split {head:?} | {tail:?}");
        assert_eq!(vt.osc52_carry_len(), 0, "carry drained after {head:?} | {tail:?}");
    }
}

#[test]
fn a_complete_csi_is_not_carried() {
    let mut vt = VtParser::new(24, 80, 100);
    vt.process(b"\x1b[?1049h\x1b[2J");
    assert_eq!(vt.osc52_carry_len(), 0);
    assert!(vt.is_alternate_screen());
}
