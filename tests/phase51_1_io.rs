//! Phase 51.1-13: I/O pumps (`pump_tmux_input`, `pump_tmux_outbound`, `pump_tmux_replies`),
//! outbound greeting gate, `send-keys -H` chunking, `capture-pane` hydrate, window closure
//! removals, fleet pane-id synchronization, and resize propagation (PST-02).
//! Headless: transport is `TmuxControlSubprocess::from_channels` + `FakeDriver`.

use std::io;
use std::sync::{Arc, Mutex};

use ratatui::layout::Rect;

use bilterm::pane::PaneId;
use bilterm::remote::{FleetNodeKind, FleetSelector};
use bilterm::tmux::TmuxControlSubprocess;
use bilterm::tmux::hydrate::{HydratePhase, ReplyKind};
use bilterm::tmux::link::{
    DisconnectCause, DriverPoll, ExitClass, HYDRATE_REPLY_LINES_CAP, LinkDriver, LinkEvent,
    LinkState, OUTBOUND_CAP, SEND_KEYS_CHUNK, TmuxDisconnect,
};
use bilterm::wm::WindowManager;

type LineTx = tokio::sync::mpsc::UnboundedSender<String>;
type CmdRx = tokio::sync::mpsc::UnboundedReceiver<String>;

#[derive(Default)]
struct FakeDriverState {
    written: Vec<u8>,
    resizes: Vec<(u16, u16)>,
    killed: bool,
}

#[derive(Clone, Default)]
struct FakeDriver {
    state: Arc<Mutex<FakeDriverState>>,
}

impl FakeDriver {
    fn new() -> (Self, Arc<Mutex<FakeDriverState>>) {
        let state = Arc::new(Mutex::new(FakeDriverState::default()));
        (Self { state: Arc::clone(&state) }, state)
    }
}

impl LinkDriver for FakeDriver {
    fn pump(&mut self) -> DriverPoll { DriverPoll::default() }
    fn write_raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.state.lock().unwrap().written.extend_from_slice(bytes);
        Ok(())
    }
    fn resize(&mut self, rows: u16, cols: u16) {
        self.state.lock().unwrap().resizes.push((rows, cols));
    }
    fn kill(&mut self) { self.state.lock().unwrap().killed = true; }
}

fn attach(wm: &mut WindowManager) -> (LineTx, CmdRx) {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    wm.attach_tmux_control(TmuxControlSubprocess::from_channels("main", cmd_tx, line_rx));
    (line_tx, cmd_rx)
}

fn send(tx: &LineTx, lines: &[&str]) {
    for l in lines {
        tx.send((*l).to_string()).unwrap();
    }
}

fn drain_cmds(rx: &mut CmdRx) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(cmd) = rx.try_recv() {
        out.push(cmd);
    }
    out
}

fn pane_for_tmux(wm: &WindowManager, tmux_pane: usize) -> Option<PaneId> {
    wm.tmux_link_summary()
        .panes
        .into_iter()
        .find_map(|(p, t)| (t == tmux_pane).then_some(p))
}

fn unhex_send_keys(cmd: &str, expected_tmux_pane: usize) -> Vec<u8> {
    let prefix = format!("send-keys -t %{expected_tmux_pane} -H ");
    let rest = cmd
        .strip_prefix(&prefix)
        .expect("send-keys prefix")
        .trim_end_matches('\n');
    rest.split_whitespace()
        .map(|tok| u8::from_str_radix(tok, 16).expect("hex byte"))
        .collect()
}

// ---- Task 1: outbound gating, input to send-keys, resize ----

mod input {
    use super::*;

    #[test]
    fn outbound_waits_for_greeting_then_flushes() {
        let mut wm = WindowManager::new_headless(24, 80);
        let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        let (tx, mut cmd_rx) = attach(&mut wm);

        // Map %3 before the attach greeting finishes (%begin / %end not sent yet).
        send(&tx, &["%layout-change @1 b25d,160x48,0,0,3"]);
        wm.tick_tmux_link(10);
        assert_eq!(pane_for_tmux(&wm, 3), Some(primary));
        assert_eq!(wm.link_hydrate_phase(), HydratePhase::AwaitingGreeting);

        // Submit two commands across ticks while still awaiting the greeting.
        wm.pane_mut(primary).unwrap().submit_command("ls").unwrap();
        wm.tick_tmux_link(15);
        wm.pane_mut(primary).unwrap().submit_command("pwd").unwrap();
        wm.tick_tmux_link(20);
        assert!(
            drain_cmds(&mut cmd_rx).is_empty(),
            "commands must wait in the bridge until the attach greeting completes"
        );

        // Greeting arrives; after one tick, both commands flush in order.
        send(&tx, &["%begin 1 1 0", "%end 1 1 0"]);
        wm.tick_tmux_link(30);
        let cmds = drain_cmds(&mut cmd_rx);
        assert_eq!(
            cmds,
            vec![
                "send-keys -t %3 -H 6c 73 0a\n".to_string(),
                "send-keys -t %3 -H 70 77 64 0a\n".to_string(),
            ]
        );
    }

    #[test]
    fn typed_line_becomes_send_keys_hex() {
        let mut wm = WindowManager::new_headless(24, 80);
        let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        let (tx, mut cmd_rx) = attach(&mut wm);
        send(
            &tx,
            &[
                "%begin 1 1 0",
                "%end 1 1 0",
                "%layout-change @1 b25d,160x48,0,0,3",
            ],
        );
        wm.tick_tmux_link(10);
        assert!(drain_cmds(&mut cmd_rx).is_empty());

        wm.pane_mut(primary).unwrap().submit_command("ls").unwrap();
        wm.tick_tmux_link(20);
        assert_eq!(
            drain_cmds(&mut cmd_rx),
            vec!["send-keys -t %3 -H 6c 73 0a\n".to_string()]
        );

        // 1500-byte paste splits into 3 chunks (512 + 512 + 476) whose hex equals the input.
        let payload: Vec<u8> = (0..1500).map(|i| b'a' + (i % 26) as u8).collect();
        wm.pane_mut(primary).unwrap().write_to_pty(&payload).unwrap();
        wm.tick_tmux_link(30);
        let chunks = drain_cmds(&mut cmd_rx);
        assert_eq!(chunks.len(), 3);
        let mut reassembled = Vec::new();
        for c in &chunks {
            let bytes = unhex_send_keys(c, 3);
            assert!(bytes.len() <= SEND_KEYS_CHUNK, "chunk len {}", bytes.len());
            reassembled.extend_from_slice(&bytes);
        }
        assert_eq!(reassembled, payload);
    }

    #[test]
    fn primary_input_goes_raw_while_awaiting_control() {
        let mut wm = WindowManager::new_headless(24, 80);
        let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        let (driver, drv_state) = FakeDriver::new();
        wm.set_link_driver(Some(Box::new(driver)));

        // Enable tap via initial tick while Connecting { awaiting_control: true }.
        wm.tick_tmux_link(0);
        assert!(matches!(
            wm.tmux_link_state(),
            LinkState::Connecting {
                awaiting_control: true,
                ..
            }
        ));

        wm.pane_mut(primary).unwrap().write_to_pty(b"yes\r").unwrap();
        wm.tick_tmux_link(10);
        assert_eq!(drv_state.lock().unwrap().written, b"yes\r");
        assert_eq!(wm.tmux_outbound_queued(), 0);
    }

    #[test]
    fn input_is_dropped_when_not_connected() {
        let mut wm = WindowManager::new_headless(24, 80);
        let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        let (tx, mut cmd_rx) = attach(&mut wm);
        send(
            &tx,
            &[
                "%begin 1 1 0",
                "%end 1 1 0",
                "%layout-change @1 b25d,160x48,0,0,3",
            ],
        );
        wm.tick_tmux_link(10);

        // Transition to Backoff.
        wm.feed_link_event(
            LinkEvent::Down(TmuxDisconnect {
                cause: DisconnectCause::Eof,
                reason: None,
                code: None,
                tail: Vec::new(),
                at_ms: 20,
                class: ExitClass::Transient,
            }),
            20,
        );
        assert!(matches!(wm.tmux_link_state(), LinkState::Backoff { .. }));

        wm.pane_mut(primary).unwrap().write_to_pty(b"ls\n").unwrap();
        wm.pane_mut(primary).unwrap().write_to_pty(b"pwd\n").unwrap();
        wm.tick_tmux_link(30);
        assert!(drain_cmds(&mut cmd_rx).is_empty());
        assert_eq!(wm.tmux_outbound_queued(), 0);
        assert_eq!(
            wm.pane(primary).unwrap().status_message(),
            Some("Link down: input not delivered (Enter/r reconnects)")
        );

        // Transition to Detached and verify the same drop + status behaviour.
        wm.pane_mut(primary).unwrap().set_status_message(None);
        wm.feed_link_event(LinkEvent::UserDisconnect, 40);
        assert_eq!(*wm.tmux_link_state(), LinkState::Detached);
        wm.pane_mut(primary).unwrap().write_to_pty(b"whoami\n").unwrap();
        wm.tick_tmux_link(50);
        assert_eq!(wm.tmux_outbound_queued(), 0);
        assert_eq!(
            wm.pane(primary).unwrap().status_message(),
            Some("Link down: input not delivered (Enter/r reconnects)")
        );
        // Subsequent tick without input produces nothing (tap buffer was drained).
        wm.pane_mut(primary).unwrap().set_status_message(None);
        wm.tick_tmux_link(60);
        assert_eq!(wm.pane(primary).unwrap().status_message(), None);
    }

    #[test]
    fn tap_is_enabled_for_mapped_panes() {
        let mut wm = WindowManager::new_headless(24, 80);
        let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        let (tx, _cmd_rx) = attach(&mut wm);
        send(&tx, &["%output %3 a", "%output %9 x"]);
        wm.tick_tmux_link(10);

        let p9 = pane_for_tmux(&wm, 9).expect("pane for %9");
        assert!(wm.pane(primary).unwrap().remote_input_tap());
        assert!(wm.pane(p9).unwrap().remote_input_tap());
        assert!(
            !wm.pane(1).unwrap().remote_input_tap(),
            "unmapped local pane 1 keeps remote_input_tap false"
        );
    }

    #[test]
    fn nothing_is_written_without_control() {
        let mut wm = WindowManager::new_headless(24, 80);
        for i in 0..10 {
            wm.queue_tmux_command(format!("send-keys -t %1 -H {:02x}", i), ReplyKind::Ignore);
        }
        wm.tick_tmux_link(10);
        assert_eq!(wm.tmux_outbound_queued(), 10);
    }

    #[test]
    fn outbound_queue_is_bounded() {
        let mut wm = WindowManager::new_headless(24, 80);
        let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        for i in 0..1000 {
            wm.queue_tmux_command(format!("cmd-{i}"), ReplyKind::Ignore);
        }
        wm.tick_tmux_link(10);
        assert_eq!(wm.tmux_outbound_queued(), OUTBOUND_CAP);
        let status = wm.pane(primary).unwrap().status_message().unwrap_or("");
        assert!(
            status.to_lowercase().contains("input") && status.to_lowercase().contains("dropped"),
            "expected drop status, got: {status}"
        );
    }

    #[test]
    fn resize_queues_one_refresh_per_distinct_size() {
        let mut wm = WindowManager::new_headless(24, 80);
        let _primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        let (driver, drv_state) = FakeDriver::new();

        // Before the PTY/control exists (Connecting, no driver/control): resize calls nothing.
        wm.resize_all(Rect::new(0, 0, 90, 28)).unwrap();
        wm.tick_tmux_link(5);
        assert_eq!(wm.tmux_outbound_queued(), 0);
        assert!(drv_state.lock().unwrap().resizes.is_empty());

        // Attach control + driver and move to Connected.
        wm.set_link_driver(Some(Box::new(driver)));
        let (tx, mut cmd_rx) = attach(&mut wm);
        send(&tx, &["%begin 1 1 0", "%end 1 1 0"]);
        wm.tick_tmux_link(10);
        assert!(matches!(wm.tmux_link_state(), LinkState::Connected { .. }));
        // An idle session (greeting only) now triggers pane discovery; nothing else is sent.
        assert_eq!(
            drain_cmds(&mut cmd_rx),
            vec!["list-panes -s -F \"#{window_id} #{pane_id} #{window_name}\"\n".to_string()]
        );
        assert!(drv_state.lock().unwrap().resizes.is_empty());

        // Changing default_size twice with the same value queues one refresh-client and one resize.
        // The remote client is as large as the linked pane (not the whole window), so the
        // expected sizes are read from the pane. Changing the size twice with the same value
        // queues one refresh-client and one resize.
        wm.resize_all(Rect::new(0, 0, 100, 31)).unwrap();
        wm.tick_tmux_link(20);
        wm.resize_all(Rect::new(0, 0, 100, 31)).unwrap();
        wm.tick_tmux_link(30);
        let (r1, c1) = wm.pane(_primary).unwrap().vt().size();
        assert!(c1 < 100, "the linked pane is narrower than the window ({c1})");
        assert_eq!(drain_cmds(&mut cmd_rx), vec![format!("refresh-client -C {c1}x{r1}\n")]);
        assert_eq!(drv_state.lock().unwrap().resizes, vec![(r1, c1)]);

        // A different size queues another and calls driver.resize once more.
        wm.resize_all(Rect::new(0, 0, 120, 41)).unwrap();
        wm.tick_tmux_link(40);
        let (r2, c2) = wm.pane(_primary).unwrap().vt().size();
        assert_eq!(drain_cmds(&mut cmd_rx), vec![format!("refresh-client -C {c2}x{r2}\n")]);
        assert_eq!(drv_state.lock().unwrap().resizes, vec![(r1, c1), (r2, c2)]);
    }
}

// ---- Task 2: hydrate with capture-pane, held output, replies, reconcile finish ----

#[test]
fn hydrate_sends_refresh_then_captures_after_settle() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, mut cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%session-changed $1 main",
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
        ],
    );
    wm.tick_tmux_link(100);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Ready);
    assert!(drain_cmds(&mut cmd_rx).is_empty());

    // At +299 ms after the last new mapping nothing is hydrated.
    wm.tick_tmux_link(399);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Ready);
    assert!(drain_cmds(&mut cmd_rx).is_empty());

    // At +300 ms refresh-client then capture-pane %3 and %4 are sent.
    wm.tick_tmux_link(400);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Sent);
    assert_hydrate_batch(drain_cmds(&mut cmd_rx), &[3, 4]);
}

#[test]
fn capture_reply_is_applied_and_output_resumes() {
    let mut wm = WindowManager::new_headless(24, 80);
    let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, mut cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0,3",
        ],
    );
    wm.tick_tmux_link(0);
    wm.tick_tmux_link(300);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Sent);
    let _ = drain_cmds(&mut cmd_rx);

    // Output while capture of %3 is pending is dropped; reply block 6 is refresh-client (Ignore),
    // reply block 7 is capture-pane %3 with three body lines.
    send(&tx, &[
        "%output %3 lost\\015\\012",
        "%begin 2 6 1", "%end 2 6 1",
        "%begin 2 7 1", "first line", "second line", "third line", "%end 2 7 1",
    ]);
    wm.tick_tmux_link(310);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Done);

    // Live output resumes after the capture reply.
    send(&tx, &["%output %3 _kept"]);
    wm.tick_tmux_link(320);

    let contents = wm.pane(primary).unwrap().vt().contents();
    assert!(contents.contains("first line"), "{contents}");
    assert!(contents.contains("second line"), "{contents}");
    assert!(contents.contains("third line_kept"), "{contents}");
    assert!(!contents.contains("lost"), "held output must be dropped: {contents}");
    let (cur_r, cur_c) = wm.pane(primary).unwrap().vt().cursor_position();
    assert_eq!(cur_r, 2);
    assert_eq!(cur_c, "third line_kept".len() as u16);
}

#[test]
fn error_reply_releases_the_hold() {
    let mut wm = WindowManager::new_headless(24, 80);
    let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm);
    send(&tx, &[
        "%begin 1 1 0", "%end 1 1 0",
        "%layout-change @1 b25d,160x48,0,0,3",
        "%output %3 before_hydrate",
    ]);
    wm.tick_tmux_link(0);
    wm.tick_tmux_link(300);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Sent);

    // %output while held is dropped; refresh-client ok, capture-pane fails with %error.
    send(&tx, &[
        "%output %3 dropped_during_hold",
        "%begin 2 6 1", "%end 2 6 1",
        "%begin 2 7 1", "can't find pane: %3", "%error 2 7 1",
    ]);
    wm.tick_tmux_link(310);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Done);
    let contents = wm.pane(primary).unwrap().vt().contents();
    assert!(contents.contains("before_hydrate"), "{contents}");
    assert!(!contents.contains("can't find pane"), "{contents}");
    assert!(!contents.contains("dropped_during_hold"), "{contents}");

    // Hold was released: new output arrives cleanly.
    send(&tx, &["%output %3 _after_error"]);
    wm.tick_tmux_link(320);
    assert!(wm.pane(primary).unwrap().vt().contents().contains("before_hydrate_after_error"));
}

#[test]
fn fifo_alignment_with_interleaved_keystrokes() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p3 = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, mut cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
        ],
    );
    wm.tick_tmux_link(0);
    let p4 = pane_for_tmux(&wm, 4).expect("pane for %4");

    // Queue capture %3, then an interleaved send-keys command, then capture %4.
    wm.queue_tmux_command(
        "capture-pane -p -e -J -S -2000 -t %3",
        ReplyKind::CapturePane { tmux_pane: 3 },
    );
    wm.pane_mut(p3).unwrap().submit_command("echo hi").unwrap();
    wm.tick_tmux_link(10); // flushes capture %3 and send-keys in FIFO order
    wm.queue_tmux_command(
        "capture-pane -p -e -J -S -2000 -t %4",
        ReplyKind::CapturePane { tmux_pane: 4 },
    );
    wm.tick_tmux_link(20); // flushes capture %4
    let cmds = drain_cmds(&mut cmd_rx);
    assert_eq!(cmds.len(), 3);

    // Deliver the three reply blocks: capture %3, empty send-keys block, capture %4.
    send(
        &tx,
        &[
            "%begin 2 10 1",
            "PANE_THREE_BODY",
            "%end 2 10 1",
            "%begin 2 11 1",
            "%end 2 11 1",
            "%begin 2 12 1",
            "PANE_FOUR_BODY",
            "%end 2 12 1",
        ],
    );
    wm.tick_tmux_link(30);
    assert!(wm.pane(p3).unwrap().vt().contents().contains("PANE_THREE_BODY"));
    assert!(!wm.pane(p3).unwrap().vt().contents().contains("PANE_FOUR_BODY"));
    assert!(wm.pane(p4).unwrap().vt().contents().contains("PANE_FOUR_BODY"));
    assert!(!wm.pane(p4).unwrap().vt().contents().contains("PANE_THREE_BODY"));
}

#[test]
fn done_reports_orphans_with_banner() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p3 = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
        ],
    );
    wm.tick_tmux_link(0);
    let p4 = pane_for_tmux(&wm, 4).expect("pane for %4");
    let count_before = wm.pane_count();

    // Reconnect where tmux %4 is gone (only %3 remains).
    wm.bump_link_generation();
    let (tx2, _cmd_rx2) = attach(&mut wm);
    send(
        &tx2,
        &[
            "%begin 2 1 0",
            "%end 2 1 0",
            "%layout-change @1 b25d,160x48,0,0,3",
        ],
    );
    wm.tick_tmux_link(1000);
    wm.tick_tmux_link(1300); // sends refresh-client + capture %3
    send(
        &tx2,
        &[
            "%begin 2 2 1",
            "%end 2 2 1",
            "%begin 2 3 1",
            "three alive",
            "%end 2 3 1",
        ],
    );
    wm.tick_tmux_link(1310);

    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Done);
    assert_eq!(wm.pane_count(), count_before, "orphan pane is never auto-removed");
    assert!(wm.pane(p3).unwrap().link_banner().is_none());
    let banner = wm
        .pane(p4)
        .unwrap()
        .link_banner()
        .expect("orphan banner on p4");
    assert_eq!(banner.text, "REMOTE PANE GONE | close with :close");
    assert!(wm.link_orphans().contains(&p4));
}

#[test]
fn generation_change_restarts_hydrate() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, mut cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
        ],
    );
    wm.tick_tmux_link(0);
    let p4 = pane_for_tmux(&wm, 4).unwrap();
    wm.tick_tmux_link(300);
    let _ = drain_cmds(&mut cmd_rx);
    send(
        &tx,
        &[
            "%begin 1 2 1",
            "%end 1 2 1",
            "%begin 1 3 1",
            "a",
            "%end 1 3 1",
            "%begin 1 4 1",
            "b",
            "%end 1 4 1",
        ],
    );
    wm.tick_tmux_link(310);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Done);

    // Close %4 via layout diff so we can also make an orphan on generation 2, then bump to gen 3.
    wm.bump_link_generation();
    let (tx2, mut cmd_rx2) = attach(&mut wm);
    send(
        &tx2,
        &[
            "%begin 2 1 0",
            "%end 2 1 0",
            "%layout-change @1 b25d,160x48,0,0,3",
        ],
    );
    wm.tick_tmux_link(500);
    wm.tick_tmux_link(800);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Sent);
    assert_hydrate_batch(drain_cmds(&mut cmd_rx2), &[3]);
    send(
        &tx2,
        &[
            "%begin 2 2 1",
            "%end 2 2 1",
            "%begin 2 3 1",
            "c",
            "%end 2 3 1",
        ],
    );
    wm.tick_tmux_link(810);
    assert!(wm.link_orphans().contains(&p4));

    // Generation 3 resets the planner to AwaitingGreeting and clears old orphans.
    wm.bump_link_generation();
    let (_tx3, _cmd_rx3) = attach(&mut wm);
    wm.tick_tmux_link(900);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::AwaitingGreeting);
    assert!(wm.link_orphans().is_empty());
    assert!(wm.pane(p4).unwrap().link_banner().is_none());
}

#[test]
fn hydrate_is_bounded() {
    let mut wm = WindowManager::new_headless(24, 80);
    let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0,3",
        ],
    );
    wm.tick_tmux_link(0);
    wm.tick_tmux_link(300);

    // Reply for refresh-client, then a 10,000-line reply for capture-pane %3.
    send(&tx, &["%begin 1 2 1", "%end 1 2 1", "%begin 1 3 1"]);
    for i in 0..10_000 {
        tx.send(format!("LINE_{i}")).unwrap();
    }
    send(&tx, &["%end 1 3 1"]);
    wm.tick_tmux_link(310);

    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Done);
    let contents = wm.pane(primary).unwrap().vt().contents();
    let last_kept = format!("LINE_{}", HYDRATE_REPLY_LINES_CAP - 1);
    let first_dropped = format!("LINE_{HYDRATE_REPLY_LINES_CAP}");
    assert!(contents.contains(&last_kept), "missing {last_kept}");
    assert!(
        !contents.contains(&first_dropped),
        "line past HYDRATE_REPLY_LINES_CAP was applied"
    );
}

#[derive(Clone)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl io::Write for LogBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuffer {
    type Writer = LogBuffer;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[test]
fn hydrate_event_is_logged_at_debug() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();
    });

    let buf = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(LogBuffer(Arc::clone(&buf)))
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_target(true)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        let (tx, _cmd_rx) = attach(&mut wm);
        send(
            &tx,
            &[
                "%begin 1 1 0",
                "%end 1 1 0",
                "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
            ],
        );
        wm.tick_tmux_link(0);
        wm.tick_tmux_link(300);
        send(
            &tx,
            &[
                "%begin 1 2 1",
                "%end 1 2 1",
                "%begin 1 3 1",
                "l1",
                "l2",
                "%end 1 3 1",
                "%begin 1 4 1",
                "l3",
                "%end 1 4 1",
            ],
        );
        wm.tick_tmux_link(310);
    });

    let log = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    assert!(log.contains("bilterm::link"), "missing target in log: {log}");
    assert!(log.contains("link.hydrate"), "missing link.hydrate in log: {log}");
    assert!(log.contains("panes=2"), "missing panes=2 in log: {log}");
    assert!(log.contains("lines=3"), "missing lines=3 in log: {log}");
}

// ---- Task 3: window closures, fleet pane ids, boundaries ----

#[test]
fn window_close_removes_mapped_panes() {
    let mut wm = WindowManager::new_headless(24, 80);
    let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0,3",
            "%layout-change @2 b25d,160x48,0,0{80x48,0,0,5,79x48,81,0,6}",
        ],
    );
    wm.tick_tmux_link(10);
    let p5 = pane_for_tmux(&wm, 5).expect("pane for %5");
    let p6 = pane_for_tmux(&wm, 6).expect("pane for %6");
    let count_before = wm.pane_count();

    send(&tx, &["%window-close @2"]);
    wm.tick_tmux_link(20);

    assert_eq!(wm.pane_count(), count_before - 2);
    assert!(wm.pane(p5).is_none());
    assert!(wm.pane(p6).is_none());
    assert!(wm.pane(primary).is_some());
    assert!(!wm.z_stack().order().contains(&p5));
    assert!(!wm.z_stack().order().contains(&p6));

    let node = wm.fleet().nodes.get("tmux-me@box-main").expect("fleet node");
    assert_eq!(
        node.kind,
        FleetNodeKind::SshTmux {
            destination: "me@box".to_string(),
            session_name: "main".to_string(),
            pane_ids: vec![primary],
        }
    );
}

#[test]
fn primary_and_last_pane_are_kept() {
    let mut wm = WindowManager::new_headless(24, 80);
    let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0,3",
        ],
    );
    wm.tick_tmux_link(10);

    // Closing the window of the primary keeps it with the orphan banner.
    send(&tx, &["%window-close @1"]);
    wm.tick_tmux_link(20);
    assert!(wm.pane(primary).is_some());
    assert_eq!(
        wm.pane(primary).unwrap().link_banner().unwrap().text,
        "REMOTE PANE GONE | close with :close"
    );

    // If only one pane remains in the WM, closing its window never leaves the WM empty.
    let mut single = WindowManager::new_headless(24, 80);
    let (tx_s, _rx_s) = attach(&mut single);
    send(&tx_s, &["%layout-change @1 b25d,160x48,0,0,8"]);
    single.tick_tmux_link(10);
    let p8 = single
        .panes()
        .keys()
        .copied()
        .find(|&id| id != 1)
        .expect("pane for %8");
    assert!(single.remove_pane(1));
    assert_eq!(single.pane_count(), 1);

    send(&tx_s, &["%window-close @1"]);
    single.tick_tmux_link(20);
    assert_eq!(single.pane_count(), 1);
    assert!(single.pane(p8).is_some());
    assert_eq!(
        single.pane(p8).unwrap().link_banner().unwrap().text,
        "REMOTE PANE GONE | close with :close"
    );
}

#[test]
fn layout_diff_removes_closed_split() {
    let mut wm = WindowManager::new_headless(24, 80);
    let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
        ],
    );
    wm.tick_tmux_link(10);
    let p4 = pane_for_tmux(&wm, 4).expect("pane for %4");
    assert!(wm.pane(p4).is_some());

    // Layout change without %4 removes p4.
    send(&tx, &["%layout-change @1 b25d,160x48,0,0,3"]);
    wm.tick_tmux_link(20);
    assert!(wm.pane(p4).is_none());
    assert!(wm.pane(primary).is_some());
}

#[test]
fn fleet_pane_ids_follow_mapping() {
    let mut wm = WindowManager::new_headless(24, 80);
    let primary = wm.connect_ssh_tmux("me@box", "main", vec!["ops".into()]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm);

    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
        ],
    );
    wm.tick_tmux_link(10);
    let p4 = pane_for_tmux(&wm, 4).unwrap();
    wm.fleet_mut()
        .nodes
        .get_mut("tmux-me@box-main")
        .unwrap()
        .last_dispatched_command = Some("hostname".to_string());

    // Add a third pane (%5).
    send(
        &tx,
        &["%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,40x48,81,0,4,39x48,122,0,5}"],
    );
    wm.tick_tmux_link(20);
    let p5 = pane_for_tmux(&wm, 5).unwrap();

    assert_eq!(wm.fleet().nodes.len(), 1);
    let node = wm.fleet().nodes.get("tmux-me@box-main").unwrap();
    assert!(node.connected);
    assert_eq!(node.last_dispatched_command.as_deref(), Some("hostname"));
    assert_eq!(
        node.kind,
        FleetNodeKind::SshTmux {
            destination: "me@box".to_string(),
            session_name: "main".to_string(),
            pane_ids: vec![primary, p4, p5],
        }
    );

    // Drop %4; fleet pane_ids shrinks to [primary, p5] and preserves connected/last_dispatched_command.
    send(
        &tx,
        &["%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,5}"],
    );
    wm.tick_tmux_link(30);
    assert_eq!(wm.fleet().nodes.len(), 1);
    let node = wm.fleet().nodes.get("tmux-me@box-main").unwrap();
    assert!(node.connected);
    assert_eq!(node.last_dispatched_command.as_deref(), Some("hostname"));
    assert_eq!(
        node.kind,
        FleetNodeKind::SshTmux {
            destination: "me@box".to_string(),
            session_name: "main".to_string(),
            pane_ids: vec![primary, p5],
        }
    );
}

#[test]
fn remote_exec_reaches_mapped_panes() {
    let mut wm = WindowManager::new_headless(24, 80);
    let primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, mut cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
        ],
    );
    wm.tick_tmux_link(10);
    let p4 = pane_for_tmux(&wm, 4).unwrap();
    let _ = drain_cmds(&mut cmd_rx);

    let executed = wm.exec_fleet_command(&FleetSelector::Node("me@box".into()), "uptime");
    assert_eq!(executed, vec![primary, p4]);

    wm.tick_tmux_link(20);
    assert_eq!(
        drain_cmds(&mut cmd_rx),
        vec![
            "send-keys -t %3 -H 75 70 74 69 6d 65 0a\n".to_string(),
            "send-keys -t %4 -H 75 70 74 69 6d 65 0a\n".to_string(),
        ]
    );
}

#[test]
fn removed_pane_does_not_receive_late_output() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm);
    send(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
        ],
    );
    wm.tick_tmux_link(10);
    let old_p4 = pane_for_tmux(&wm, 4).unwrap();

    // Remove %4 via layout diff.
    send(&tx, &["%layout-change @1 b25d,160x48,0,0,3"]);
    wm.tick_tmux_link(20);
    assert!(wm.pane(old_p4).is_none());

    // Rediscovering %4 creates a brand new PaneId, never recycling old_p4.
    send(&tx, &["%output %4 resurrected"]);
    wm.tick_tmux_link(30);
    let new_p4 = pane_for_tmux(&wm, 4).expect("rediscovered %4");
    assert_ne!(new_p4, old_p4);
    assert!(wm.pane(old_p4).is_none());
    assert!(
        wm.pane(new_p4)
            .unwrap()
            .vt()
            .contents()
            .contains("resurrected")
    );
}

/// The hydrate batch: the client size of the linked pane, one state query for all panes, then
/// one capture per pane (sizes follow the pane, so only the command shapes are pinned here).
#[allow(dead_code)]
fn assert_hydrate_batch(cmds: Vec<String>, panes: &[usize]) {
    assert_eq!(cmds.len(), panes.len() + 2, "{cmds:?}");
    assert!(cmds[0].starts_with("refresh-client -C "), "{cmds:?}");
    assert!(cmds[1].starts_with("display-message -p \"bilterm-state #{W:#{P:[#{pane_id} "), "{cmds:?}");
    for (cmd, pane) in cmds[2..].iter().zip(panes) {
        assert_eq!(*cmd, format!("capture-pane -p -e -J -S -2000 -t %{pane}\n"));
    }
}
