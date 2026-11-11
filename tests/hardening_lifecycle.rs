//! Regression tests for process-lifecycle and crash-containment hardening.

use bilterm::app::{App, MAX_INPUT_STREAM_ERRORS};
use bilterm::error::{BiltermError, TerminalRestoreGuard};
use bilterm::pane::CompositePane;
use bilterm::wm::agent_mail::{AgentDeliveryMode, AgentMailOp};
use bilterm::wm::{SplitDirection, WindowManager};

/// Item 1: a guard created before the TUI is entered (the panic-hook guard) must see, and be
/// able to clear, the active state set by the guard that entered it.
#[test]
fn hook_side_guard_sees_state_set_by_entering_guard() {
    let hook_guard = TerminalRestoreGuard::new();
    assert!(!hook_guard.is_active());
    let entering = TerminalRestoreGuard::new_simulated_host_active();
    assert!(hook_guard.is_active(), "hook guard must observe the entered host TUI");
    hook_guard.mark_restored_quiet();
    assert!(!hook_guard.is_active());
    entering.mark_restored_quiet();
    assert!(!entering.is_active());
    assert!(!TerminalRestoreGuard::new().is_active());
}

fn app() -> App {
    App::new(CompositePane::new_headless(1, 24, 80))
}

/// Item 3: a closed input stream quits instead of spinning.
#[test]
fn closed_input_stream_quits() {
    let mut a = app();
    let mut errs = 0u8;
    a.on_input_stream_item(None, &mut errs);
    assert!(a.should_quit);
}

/// Item 3: repeated read errors quit; a good event in between resets the count.
#[test]
fn repeated_input_errors_quit_and_reset_on_success() {
    let mut a = app();
    let mut errs = 0u8;
    let err = || Some(Err(std::io::Error::other("read failed")));
    for _ in 0..MAX_INPUT_STREAM_ERRORS - 1 {
        a.on_input_stream_item(err(), &mut errs);
    }
    assert!(!a.should_quit);
    a.on_input_stream_item(Some(Ok(crossterm::event::Event::FocusGained)), &mut errs);
    assert_eq!(errs, 0);
    for _ in 0..MAX_INPUT_STREAM_ERRORS {
        a.on_input_stream_item(err(), &mut errs);
    }
    assert!(a.should_quit);
}

/// Item 4: an input-handling error is reported in the status line and does not quit.
#[test]
fn event_handling_error_is_reported_not_fatal() {
    let mut a = app();
    a.report_event_error(&BiltermError::Pty("write failed on pane 3".to_string()));
    assert!(!a.should_quit);
    let status = a.wm.active_pane().status_message().unwrap_or("").to_string();
    assert!(status.contains("write failed on pane 3"), "{status}");
}

fn send_nudge(wm: &mut WindowManager, to: &str) -> Result<String, String> {
    wm.agent_mail_apply(&AgentMailOp::Send {
        from: Some("operator".to_string()),
        to: to.to_string(),
        body: "hello".to_string(),
        kind: "message".to_string(),
        delivery: AgentDeliveryMode::Nudge,
        reply_to: None,
    })
}

fn wm_with_named_second_pane() -> (WindowManager, usize) {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.agent_mail_mut().set_enabled(true);
    let id = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.agent_mail_apply(&AgentMailOp::SetName { pane_id: Some(id), name: "bob".to_string() }).unwrap();
    assert!(send_nudge(&mut wm, "bob").is_ok());
    (wm, id)
}

/// Item 5: a new session drops the old panes; their agent names must go with them, so a send
/// neither panics nor reaches an unrelated pane that later reuses the id.
#[test]
fn new_session_forgets_agent_names_of_dropped_panes() {
    let (mut wm, id) = wm_with_named_second_pane();
    wm.create_new_session(Some("fresh"), true).unwrap();
    assert!(!wm.panes().contains_key(&id));
    assert!(send_nudge(&mut wm, "bob").is_err());
    assert!(wm.agent_mail().name_for_pane(id).is_none());
    // An unrelated pane now reuses the id: it must not inherit the name.
    let reused = wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(reused, id);
    assert!(send_nudge(&mut wm, "bob").is_err());
    assert!(wm.agent_mail().name_for_pane(reused).is_none());
}

/// Item 5: layout/session restore replaces the panes wholesale too.
#[test]
fn snapshot_restore_forgets_agent_names_of_dropped_panes() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.agent_mail_mut().set_enabled(true);
    let single = wm.snapshot_session("single");
    let id = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.agent_mail_apply(&AgentMailOp::SetName { pane_id: Some(id), name: "bob".to_string() }).unwrap();
    wm.apply_saved_session_snapshot(&single);
    assert!(!wm.panes().contains_key(&id));
    assert!(send_nudge(&mut wm, "bob").is_err());
    assert!(wm.agent_mail().name_for_pane(id).is_none());
}

/// Item 6: six bytes that are not six ASCII hex digits must be rejected, not sliced.
#[test]
fn non_ascii_hex_colour_is_rejected_without_panic() {
    assert!(bilterm::ui::theme::parse_hex_color("#a€bc").is_none());
    assert!(bilterm::ui::theme::parse_hex_color("#+1+2+3").is_none());
    assert!(bilterm::ui::theme::parse_hex_color("#1A2b3C").is_some());
    for content in [
        "base00: \"a€bc\"\nbase05: \"ffffff\"\n",
        "background = \"#a€bc\"\nforeground = \"#ffffff\"\n",
        "[colors]\nbackground = \"a€bc\"\nforeground = \"0xa€bc\"\n",
        "[colors.primary]\nbackground = \"#a€bc\"\nforeground = \"#a€bc\"\n",
    ] {
        let _ = bilterm::ui::theme_import::parse_open_theme_str("t", content);
    }
}
