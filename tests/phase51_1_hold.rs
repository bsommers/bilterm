//! Phase 51.1 Plan 07: Pane hold + exit-guard banner integration tests (PST-03, PST-04).

use std::sync::Mutex;
use bilterm::pane::{CompositePane, ExitProtectionPolicy, HeldExitedShellState, LinkBanner};
use bilterm::pty::PtyEvent;
use bilterm::ui::render_link_banner;
use bilterm::ui::theme::ThemePreset;
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn remote_state_banner_texts() {
    // 1. With code and reason
    let held = HeldExitedShellState::remote(Some(255), Some("Connection to box closed."));
    assert_eq!(
        held.banner_message,
        "REMOTE SESSION ENDED (code 255): Connection to box closed. | Enter/r: reconnect | d: detach | :quit!: exit"
    );
    assert_eq!(held.exit_code, Some(255));

    // 2. Without code
    let held_no_code = HeldExitedShellState::remote(None, Some("Connection to box closed."));
    assert_eq!(
        held_no_code.banner_message,
        "REMOTE SESSION ENDED: Connection to box closed. | Enter/r: reconnect | d: detach | :quit!: exit"
    );
    assert_eq!(held_no_code.exit_code, None);

    // 3. Without reason
    let held_no_reason = HeldExitedShellState::remote(Some(255), None);
    assert_eq!(
        held_no_reason.banner_message,
        "REMOTE SESSION ENDED (code 255) | Enter/r: reconnect | d: detach | :quit!: exit"
    );

    // 4. Without code and without reason
    let held_neither = HeldExitedShellState::remote(None, None);
    assert_eq!(
        held_neither.banner_message,
        "REMOTE SESSION ENDED | Enter/r: reconnect | d: detach | :quit!: exit"
    );

    // 5. Empty or whitespace-only reason is omitted
    let held_empty = HeldExitedShellState::remote(Some(1), Some("   "));
    assert_eq!(
        held_empty.banner_message,
        "REMOTE SESSION ENDED (code 1) | Enter/r: reconnect | d: detach | :quit!: exit"
    );

    // 6. ESC-laden, 300-character reason yields single-line banner <= 160 chars
    let noisy_reason = format!("bad\x1b[31;1mANSI\x1b[0m\x07payload\r\nline2{}", "X".repeat(300));
    let held_noisy = HeldExitedShellState::remote(Some(255), Some(&noisy_reason));
    assert!(held_noisy.banner_message.chars().count() <= 160);
    assert!(!held_noisy.banner_message.contains('\r'));
    assert!(!held_noisy.banner_message.contains('\n'));
    assert!(!held_noisy.banner_message.contains('\x1b'));
    assert!(!held_noisy.banner_message.contains('\x07'));
}

#[test]
fn last_output_line_ignores_blank_rows() {
    let mut pane = CompositePane::new_headless(1, 24, 80);

    // Trailing blank rows ignored
    pane.process_pty_output(b"one\r\ntwo\r\n\r\n\r\n");
    assert_eq!(pane.last_output_line().as_deref(), Some("two"));

    // Empty screen gives None
    let empty_pane = CompositePane::new_headless(2, 24, 80);
    assert_eq!(empty_pane.last_output_line(), None);

    // Whitespace-only rows give None
    let mut ws_pane = CompositePane::new_headless(3, 24, 80);
    ws_pane.process_pty_output(b"   \r\n  \t  \r\n");
    assert_eq!(ws_pane.last_output_line(), None);

    // ANSI colors are not part of the output text
    let mut color_pane = CompositePane::new_headless(4, 24, 80);
    color_pane.process_pty_output(b"\x1b[32;1mcolored text\x1b[0m\r\n\r\n");
    assert_eq!(color_pane.last_output_line().as_deref(), Some("colored text"));

    // Long row capped at 80 chars
    let mut long_pane = CompositePane::new_headless(5, 24, 120);
    let long_str = "A".repeat(100);
    long_pane.process_pty_output(format!("{long_str}\r\n").as_bytes());
    let line = long_pane.last_output_line().expect("has output");
    assert!(line.chars().count() <= 80);
}

#[test]
fn exited_event_stores_code() {
    // Remote pane stores exit code on Exited(Some(c))
    let mut pane = CompositePane::new_headless(1, 24, 80);
    pane.set_remote_target(Some("user@box".to_string()));
    let changed = pane.ingest_pty_events(vec![PtyEvent::Exited(Some(255))]);
    assert!(changed);
    assert_eq!(pane.exit_code(), Some(255));
    assert!(pane.has_exited());

    // Headless remote pane with Exited(None) keeps None (no live PTY to probe)
    let mut pane2 = CompositePane::new_headless(2, 24, 80);
    pane2.set_remote_target(Some("user@box".to_string()));
    pane2.ingest_pty_events(vec![PtyEvent::Exited(None)]);
    assert_eq!(pane2.exit_code(), None);
    assert!(pane2.has_exited());

    // Local pane never probes PTY on Exited(None)
    let mut local_pane = CompositePane::new_headless(3, 24, 80);
    local_pane.ingest_pty_events(vec![PtyEvent::Exited(None)]);
    assert_eq!(local_pane.exit_code(), None);
    assert!(local_pane.has_exited());
}

#[test]
fn hold_remote_exit_sets_state() {
    let mut pane = CompositePane::new_headless(1, 24, 80);
    pane.set_remote_target(Some("remote-host".to_string()));
    pane.hold_remote_exit(Some(3), Some("x"));

    assert!(!pane.has_exited());
    let held = pane.held_exited_shell().expect("held state must be present");
    assert_eq!(held.exit_code, Some(3));
    assert_eq!(
        held.banner_message,
        "REMOTE SESSION ENDED (code 3): x | Enter/r: reconnect | d: detach | :quit!: exit"
    );
    assert_eq!(pane.status_message(), Some(held.banner_message.as_str()));
    assert_eq!(pane.exit_code(), Some(3));
}

#[test]
fn link_banner_roundtrip_and_render() {
    let mut pane = CompositePane::new_headless(1, 24, 80);
    assert!(pane.link_banner().is_none());

    pane.set_link_banner(Some(LinkBanner {
        text: "Connecting to host...".to_string(),
        interactive: false,
    }));
    let banner = pane.link_banner().expect("banner set");
    assert_eq!(banner.text, "Connecting to host...");
    assert!(!banner.interactive);

    let palette = ThemePreset::TurboBlue.palette();
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    render_link_banner(&mut buf, area, &palette, banner);

    // Drawn on the bottom row of area (y = 9)
    let last_row: String = (0..40).map(|x| buf[(x, 9)].symbol()).collect();
    assert!(last_row.contains("Connecting to host..."), "got: {last_row}");

    // Render nothing when banner text is empty
    let empty_banner = LinkBanner { text: String::new(), interactive: true };
    let mut buf2 = Buffer::empty(area);
    render_link_banner(&mut buf2, area, &palette, &empty_banner);
    let all_spaces = (0..40).all(|x| buf2[(x, 9)].symbol() == " ");
    assert!(all_spaces);

    // Panic-free sweep across dimensions
    for w in 0..=80 {
        for h in 0..=24 {
            let r = Rect::new(0, 0, w, h);
            let mut b = Buffer::empty(r);
            render_link_banner(&mut b, r, &palette, banner);
        }
    }
}

#[test]
fn link_banner_sanitization() {
    let mut pane = CompositePane::new_headless(1, 24, 80);
    pane.set_link_banner(Some(LinkBanner {
        text: "bad\x1b\x00\x07\r\n\ttext".to_string(),
        interactive: true,
    }));
    let b = pane.link_banner().expect("banner set");
    assert_eq!(b.text, "bad   text");
    assert!(!b.text.contains('\x1b'));
    assert!(!b.text.contains('\x00'));
    assert!(!b.text.contains('\x07'));
    assert!(!b.text.contains('\r'));
    assert!(!b.text.contains('\n'));
    assert!(b.interactive);
}

#[test]
fn exited_ssh_pane_is_held_with_code_and_reason() {
    let mut wm = WindowManager::new_headless(24, 80);
    let ssh_pane_id = wm.spawn_ssh_pane("me@box", None, vec![]).expect("spawn ssh pane");

    let pane = wm.pane_mut(ssh_pane_id).expect("pane exists");
    pane.process_pty_output(b"client_loop: send disconnect: Broken pipe\r\n");
    pane.ingest_pty_events(vec![PtyEvent::Exited(Some(255))]);

    let changed = wm.poll_all_ptys();
    assert!(changed);
    assert_eq!(wm.panes().len(), 2, "held pane must remain in workspace");

    let ssh_pane = wm.pane(ssh_pane_id).expect("pane exists");
    assert!(!ssh_pane.has_exited(), "held pane has_exited() must be false");
    let held = ssh_pane.held_exited_shell().expect("pane must be held");
    assert_eq!(held.exit_code, Some(255));
    assert!(held.banner_message.contains("255"));
    assert!(held.banner_message.contains("Broken pipe"));

    // Fleet node connected is marked false
    let node = wm
        .fleet()
        .nodes
        .values()
        .find(|n| matches!(n.kind, bilterm::remote::FleetNodeKind::SshPane { pane_id, .. } if pane_id == ssh_pane_id))
        .expect("fleet node exists");
    assert!(!node.connected, "fleet node must be marked disconnected");
}

#[test]
fn last_pane_is_held_not_quit() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.active_pane_mut().set_remote_target(Some("me@singlebox".to_string()));
    wm.active_pane_mut().ingest_pty_events(vec![PtyEvent::Exited(Some(1))]);

    let changed = wm.poll_all_ptys();
    assert!(changed);
    assert_eq!(wm.panes().len(), 1);
    assert!(!wm.all_panes_exited(), "all_panes_exited must be false for held last pane");
    assert!(!wm.active_pane().has_exited());
    assert!(wm.active_pane().held_exited_shell().is_some());
}

#[test]
fn local_pane_exit_is_unchanged() {
    // 1. Two local panes: one exits -> reaped (count becomes 1)
    let mut wm = WindowManager::new_headless(24, 80);
    let p2 = wm.split_focused(SplitDirection::Vertical).expect("split");
    assert_eq!(wm.panes().len(), 2);
    wm.pane_mut(p2).unwrap().ingest_pty_events(vec![PtyEvent::Exited(Some(0))]);
    wm.poll_all_ptys();
    assert_eq!(wm.panes().len(), 1);

    // 2. Single local pane exits -> remains, has_exited stays true, all_panes_exited is true
    let mut wm_single = WindowManager::new_headless(24, 80);
    wm_single.active_pane_mut().ingest_pty_events(vec![PtyEvent::Exited(Some(0))]);
    wm_single.poll_all_ptys();
    assert_eq!(wm_single.panes().len(), 1);
    assert!(wm_single.active_pane().has_exited());
    assert!(wm_single.all_panes_exited());
}

#[test]
fn policy_off_keeps_legacy_for_remote_panes() {
    // With ExitProtectionPolicy::Off, remote pane is reaped like a local pane
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().exit_protection = ExitProtectionPolicy::Off;
    let ssh_id = wm.spawn_ssh_pane("host", None, vec![]).expect("spawn ssh");
    assert_eq!(wm.panes().len(), 2);

    wm.pane_mut(ssh_id).unwrap().ingest_pty_events(vec![PtyEvent::Exited(Some(0))]);
    wm.poll_all_ptys();
    assert_eq!(wm.panes().len(), 1, "Off policy reaps remote pane among multiple");

    // With ExitProtectionPolicy::Respawn, remote pane is held
    let mut wm_respawn = WindowManager::new_headless(24, 80);
    wm_respawn.config_mut().exit_protection = ExitProtectionPolicy::Respawn;
    let ssh_id2 = wm_respawn.spawn_ssh_pane("host", None, vec![]).expect("spawn ssh");
    wm_respawn.pane_mut(ssh_id2).unwrap().ingest_pty_events(vec![PtyEvent::Exited(Some(0))]);
    wm_respawn.poll_all_ptys();
    assert_eq!(wm_respawn.panes().len(), 2, "Respawn policy holds remote pane");
    assert!(wm_respawn.pane(ssh_id2).unwrap().held_exited_shell().is_some());

    // With ExitProtectionPolicy::Detach, remote pane is held
    let mut wm_detach = WindowManager::new_headless(24, 80);
    wm_detach.config_mut().exit_protection = ExitProtectionPolicy::Detach;
    let ssh_id3 = wm_detach.spawn_ssh_pane("host", None, vec![]).expect("spawn ssh");
    wm_detach.pane_mut(ssh_id3).unwrap().ingest_pty_events(vec![PtyEvent::Exited(Some(0))]);
    wm_detach.poll_all_ptys();
    assert_eq!(wm_detach.panes().len(), 2, "Detach policy holds remote pane");
    assert!(wm_detach.pane(ssh_id3).unwrap().held_exited_shell().is_some());
}

#[test]
fn held_pane_reconnect_marks_node_connected() {
    let mut wm = WindowManager::new_headless(24, 80);
    let ssh_id = wm.spawn_ssh_pane("me@box", None, vec![]).expect("spawn ssh");
    wm.pane_mut(ssh_id).unwrap().ingest_pty_events(vec![PtyEvent::Exited(Some(255))]);
    wm.poll_all_ptys();

    // Node is disconnected
    let is_connected = |wm: &WindowManager| {
        wm.fleet()
            .nodes
            .values()
            .find_map(|n| match n.kind {
                bilterm::remote::FleetNodeKind::SshPane { pane_id, .. } if pane_id == ssh_id => {
                    Some(n.connected)
                }
                _ => None,
            })
            .expect("node found")
    };
    assert!(!is_connected(&wm));

    // Focus ssh pane and reconnect
    wm.focus_and_raise(ssh_id);
    wm.respawn_active_pane();

    // Hold cleared, node connected again, status set
    assert!(wm.pane(ssh_id).unwrap().held_exited_shell().is_none());
    assert!(is_connected(&wm));
    assert_eq!(
        wm.pane(ssh_id).unwrap().status_message(),
        Some("Reconnecting ssh: me@box")
    );

    // Held local pane takes legacy respawn
    let mut wm_local = WindowManager::new_headless(24, 80);
    wm_local.active_pane_mut().hold_exited_shell(Some(0));
    wm_local.respawn_active_pane();
    assert_eq!(
        wm_local.active_pane().status_message(),
        Some("Shell respawned cleanly")
    );
}

static TRACE_LOCK: Mutex<()> = Mutex::new(());
static LOG_BUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static INIT_TRACING: std::sync::Once = std::sync::Once::new();

#[derive(Clone, Default)]
struct GlobalTestWriter;

impl std::io::Write for GlobalTestWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        LOG_BUFFER.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for GlobalTestWriter {
    type Writer = GlobalTestWriter;
    fn make_writer(&'a self) -> Self::Writer {
        GlobalTestWriter
    }
}

fn init_test_tracing() {
    INIT_TRACING.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_writer(GlobalTestWriter)
            .with_max_level(tracing::Level::INFO)
            .try_init();
    });
}

#[test]
fn held_exit_emits_pane_exit_event() {
    let _guard = TRACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    init_test_tracing();
    LOG_BUFFER.lock().unwrap().clear();

    let mut wm = WindowManager::new_headless(24, 80);
    let ssh_id = wm.spawn_ssh_pane("me@box", None, vec![]).expect("spawn ssh");
    let pane = wm.pane_mut(ssh_id).unwrap();
    pane.process_pty_output(b"Connection reset by peer\r\n");
    pane.ingest_pty_events(vec![PtyEvent::Exited(Some(255))]);
    wm.poll_all_ptys();

    let logs = String::from_utf8(LOG_BUFFER.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("pane.exit"), "expected pane.exit in logs: {logs}");
    assert!(logs.contains("code=Some(255)") || logs.contains("code = Some(255)") || logs.contains("255"), "{logs}");
    assert!(logs.contains("remote_target=me@box") || logs.contains("me@box"), "{logs}");
}
