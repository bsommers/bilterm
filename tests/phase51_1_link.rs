//! Phase 51.1-12: persistent SSH + tmux link lifecycle, reconnect supervisor, banners and summary.
//! Fake clock only: every test uses `WindowManager::new_headless`, `TmuxControlSubprocess::from_channels`,
//! an injected `LinkConnector` and a fake `LinkDriver`. No ssh or tmux child process is ever started.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::handle_wm_key_event;
use bilterm::pane::{LinkBanner, PaneId};
use bilterm::tmux::TmuxControlSubprocess;
use bilterm::tmux::link::{
    DriverExit, DriverPoll, LinkConnector, LinkDriver, LinkState, LinkTarget, LinkTransport,
    ReconnectPolicy,
};
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

type LineTx = tokio::sync::mpsc::UnboundedSender<String>;
type CmdRx = tokio::sync::mpsc::UnboundedReceiver<String>;

#[derive(Clone, Default)]
struct FakeDriverState {
    polls: VecDeque<DriverPoll>,
    written_raw: Vec<u8>,
    resizes: Vec<(u16, u16)>,
    killed: bool,
}

#[derive(Clone, Default)]
struct FakeDriverHandle(Arc<Mutex<FakeDriverState>>);

impl FakeDriverHandle {
    fn push_poll(&self, poll: DriverPoll) {
        self.0.lock().unwrap().polls.push_back(poll);
    }

    fn killed(&self) -> bool {
        self.0.lock().unwrap().killed
    }
}

struct FakeDriver(FakeDriverHandle);

impl LinkDriver for FakeDriver {
    fn pump(&mut self) -> DriverPoll {
        let mut g = self.0.0.lock().unwrap();
        if g.killed {
            return DriverPoll::default();
        }
        g.polls.pop_front().unwrap_or_default()
    }

    fn write_raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut g = self.0.0.lock().unwrap();
        if !g.killed {
            g.written_raw.extend_from_slice(bytes);
        }
        Ok(())
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        let mut g = self.0.0.lock().unwrap();
        if !g.killed {
            g.resizes.push((rows, cols));
        }
    }

    fn kill(&mut self) {
        self.0.0.lock().unwrap().killed = true;
    }
}

struct SpawnedHandle {
    target: LinkTarget,
    size: (u16, u16),
    line_tx: LineTx,
    _cmd_rx: CmdRx,
    driver: FakeDriverHandle,
}

#[derive(Default)]
struct TestConnectorInner {
    calls: usize,
    spawned: VecDeque<SpawnedHandle>,
    next_errors: VecDeque<String>,
    close_immediately: bool,
}

#[derive(Clone, Default)]
struct TestConnector(Arc<Mutex<TestConnectorInner>>);

impl TestConnector {
    fn new() -> Self {
        Self::default()
    }

    fn failing(msg: &str) -> Self {
        let c = Self::new();
        c.0.lock().unwrap().next_errors.push_back(msg.to_string());
        c
    }

    fn dying_immediately() -> Self {
        let c = Self::new();
        c.0.lock().unwrap().close_immediately = true;
        c
    }

    fn calls(&self) -> usize {
        self.0.lock().unwrap().calls
    }

    fn take_next(&self) -> SpawnedHandle {
        self.0
            .lock()
            .unwrap()
            .spawned
            .pop_front()
            .expect("expected a spawned transport")
    }
}

impl LinkConnector for TestConnector {
    fn connect(&mut self, target: &LinkTarget, size: (u16, u16)) -> io::Result<LinkTransport> {
        let mut g = self.0.lock().unwrap();
        g.calls += 1;
        if let Some(err) = g.next_errors.pop_front() {
            return Err(io::Error::other(err));
        }
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let driver = FakeDriverHandle::default();
        let control = TmuxControlSubprocess::from_channels(&target.session, cmd_tx, line_rx);
        if g.close_immediately {
            drop(line_tx);
        } else {
            g.spawned.push_back(SpawnedHandle {
                target: target.clone(),
                size,
                line_tx,
                _cmd_rx: cmd_rx,
                driver: driver.clone(),
            });
        }
        Ok(LinkTransport {
            control,
            driver: Some(Box::new(FakeDriver(driver))),
        })
    }
}

fn feed(tx: &LineTx, lines: &[&str]) {
    for l in lines {
        tx.send((*l).to_string()).unwrap();
    }
}

fn banner_of(wm: &WindowManager, id: PaneId) -> Option<LinkBanner> {
    wm.pane(id).unwrap().link_banner().cloned()
}

fn status_of(wm: &WindowManager, id: PaneId) -> Option<String> {
    wm.pane(id).unwrap().status_message().map(str::to_string)
}

// -----------------------------------------------------------------------------
// Task 1: connect, spawn through the connector, awaiting-control behaviour
// -----------------------------------------------------------------------------

#[test]
fn connect_with_connector_spawns_and_shows_connecting() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));

    let id = wm.connect_ssh_tmux("me@box", "main", vec!["prod".into()]).unwrap();
    assert_eq!(conn.calls(), 1);
    let h = conn.take_next();
    assert_eq!(h.target.dest(), "me@box");
    assert_eq!(h.target.session, "main");
    assert_eq!(h.size, (24, 80));
    assert!(wm.is_tmux_control_attached());
    assert!(matches!(
        wm.tmux_link_state(),
        LinkState::Connecting {
            awaiting_control: true,
            ..
        }
    ));

    let pane = wm.pane(id).unwrap();
    assert_eq!(
        pane.status_message(),
        Some("Connecting to me@box (tmux session 'main')...")
    );
    let b = pane.link_banner().expect("connecting banner present");
    assert_eq!(
        b.text,
        "LINK CONNECTING me@box/main | type here for ssh prompts | Ctrl+Alt+R: restart"
    );
    assert!(!b.interactive);
    assert!(pane.remote_input_tap());
    assert!(!pane.is_prompt_visible(), "raw input hides the prompt");
}

#[test]
fn typed_password_never_reaches_history() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();

    for c in "secret".chars() {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)).unwrap();
    }
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();

    let pane = wm.pane(id).unwrap();
    assert!(pane.prompt().history.is_empty());
    assert_eq!(pane.prompt().text(), "");
    assert_eq!(pane.last_pty_write(), b"\r");
}

#[test]
fn control_start_connects_and_clears_banner() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h = conn.take_next();

    feed(&h.line_tx, &["%begin 1 1 0", "%end 1 1 0", "%output %3 x"]);
    assert!(wm.tick_tmux_link(300));
    assert_eq!(*wm.tmux_link_state(), LinkState::Connected { since_ms: 300 });

    let pane = wm.pane(id).unwrap();
    assert!(pane.link_banner().is_none());
    assert!(pane.is_prompt_visible(), "raw input turned off");
    assert!(pane.remote_input_tap(), "tap stays on");
    assert_eq!(
        pane.status_message(),
        Some("Connected: tmux 'main' on me@box")
    );
    let node = wm.fleet().nodes.get("tmux-me@box-main").unwrap();
    assert!(node.connected);
}

#[test]
fn driver_control_started_and_raw_output_work() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h = conn.take_next();

    h.driver.push_poll(DriverPoll {
        raw_output: b"me@box's password: ".to_vec(),
        diag: vec![],
        control_started: false,
        exited: None,
    });
    assert!(wm.tick_tmux_link(50));
    assert!(wm.pane(id).unwrap().vt().contents().contains("password:"));
    assert!(matches!(wm.tmux_link_state(), LinkState::Connecting { .. }));

    h.driver.push_poll(DriverPoll {
        raw_output: Vec::new(),
        diag: vec![],
        control_started: true,
        exited: None,
    });
    assert!(wm.tick_tmux_link(250));
    assert_eq!(*wm.tmux_link_state(), LinkState::Connected { since_ms: 250 });
    assert!(wm.pane(id).unwrap().link_banner().is_none());
}

#[test]
fn spawn_failure_is_fatal_and_honest() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::failing("No such file or directory");
    wm.set_link_connector(Box::new(conn.clone()));

    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    assert_eq!(conn.calls(), 1);
    assert!(matches!(wm.tmux_link_state(), LinkState::Failed { .. }));

    let sum = wm.tmux_link_summary();
    let exit = sum.last_exit.expect("last_exit recorded");
    assert_eq!(exit.cause, "spawn failed: No such file or directory");
    assert_eq!(exit.class, "fatal");

    let pane = wm.pane(id).unwrap();
    let b = pane.link_banner().expect("failed banner");
    assert_eq!(
        b.text,
        "LINK FAILED (spawn failed: No such file or directory) | Enter/r: retry | d: detach"
    );
    assert!(b.interactive);
    assert_eq!(
        pane.status_message(),
        Some("Link to me@box failed: spawn failed: No such file or directory")
    );
    let node = wm.fleet().nodes.get("tmux-me@box-main").unwrap();
    assert!(!node.connected);

    assert!(!wm.tick_tmux_link(60_000));
    assert_eq!(conn.calls(), 1);
}

#[test]
fn headless_without_connector_has_no_transport() {
    let mut wm = WindowManager::new_headless(24, 80);
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    assert!(!wm.is_tmux_control_attached());
    assert!(matches!(wm.tmux_link_state(), LinkState::Connecting { .. }));
    assert_eq!(
        status_of(&wm, id).as_deref(),
        Some("Connecting to me@box (tmux session 'main')...")
    );
    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    wm.attach_tmux_control(TmuxControlSubprocess::from_channels("main", cmd_tx, line_rx));
    line_tx.send("%output %3 hello".into()).unwrap();
    assert!(wm.tick_tmux_link(100));
    assert!(matches!(wm.tmux_link_state(), LinkState::Connected { .. }));
}

#[test]
fn second_connect_refused_and_same_target_reuses_primary() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::failing("boom");
    wm.set_link_connector(Box::new(conn.clone()));

    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    assert!(matches!(wm.tmux_link_state(), LinkState::Failed { .. }));
    let panes_before = wm.pane_count();

    // Another target while Failed (before forget) is refused:
    let err = wm
        .connect_ssh_tmux("other@box", "main", vec![])
        .unwrap_err()
        .to_string();
    assert!(err.contains("already"), "{err}");
    assert_eq!(wm.pane_count(), panes_before);

    // Same target + session reuses the primary pane and spawns again:
    let reused = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    assert_eq!(reused, id);
    assert_eq!(wm.pane_count(), panes_before);
    assert_eq!(conn.calls(), 2);
    assert!(matches!(wm.tmux_link_state(), LinkState::Connecting { .. }));

    // While Connecting, a second connect (even to the same target) is refused:
    assert!(wm.connect_ssh_tmux("me@box", "main", vec![]).is_err());
}

// -----------------------------------------------------------------------------
// Task 2: drop handling, classification, backoff supervisor, banners
// -----------------------------------------------------------------------------

#[test]
fn drop_backoff_reconnect_cycle() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));

    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h1 = conn.take_next();
    feed(
        &h1.line_tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%layout-change @1 b25d,160x48,0,0,3",
            "%output %3 hi",
        ],
    );
    assert!(wm.tick_tmux_link(100));
    assert_eq!(*wm.tmux_link_state(), LinkState::Connected { since_ms: 100 });
    let panes_connected = wm.pane_count();

    // Drop the sender (EOF) at 1000 ms:
    drop(h1.line_tx);
    assert!(wm.tick_tmux_link(1000));
    assert_eq!(
        *wm.tmux_link_state(),
        LinkState::Backoff {
            attempt: 1,
            retry_at_ms: 2000
        }
    );
    let b = banner_of(&wm, id).expect("backoff banner");
    assert_eq!(
        b.text,
        "LINK DOWN (connection closed) | retry 1/20 in 1s | Enter/r: now | x: stop | d: detach"
    );
    assert!(b.interactive);
    assert_eq!(
        status_of(&wm, id).as_deref(),
        Some("Link to me@box lost (connection closed); retry 1/20 in 1s")
    );

    // At 1500 ms: still 1s (ceil(500/1000) == 1), nothing spawned and banner unchanged:
    assert!(!wm.tick_tmux_link(1500));
    assert_eq!(conn.calls(), 1);
    assert!(banner_of(&wm, id).unwrap().text.contains("in 1s"));

    // At 2000 ms: retry deadline reached -> Spawn #2 -> Connecting:
    assert!(wm.tick_tmux_link(2000));
    assert_eq!(conn.calls(), 2);
    assert!(matches!(wm.tmux_link_state(), LinkState::Connecting { .. }));
    let h2 = conn.take_next();
    feed(
        &h2.line_tx,
        &[
            "%begin 2 1 0",
            "%end 2 1 0",
            "%layout-change @1 b25d,160x48,0,0,3",
            "%output %3 again",
        ],
    );
    assert!(wm.tick_tmux_link(2100));
    assert_eq!(*wm.tmux_link_state(), LinkState::Connected { since_ms: 2100 });
    assert!(banner_of(&wm, id).is_none());
    assert_eq!(wm.pane_count(), panes_connected);
    assert!(wm.pane(id).unwrap().vt().contents().contains("again"));
}

#[test]
fn countdown_banner_redraws_once_per_second() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.set_link_policy(ReconnectPolicy {
        base_ms: 3000,
        ..ReconnectPolicy::default()
    });
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h1 = conn.take_next();
    drop(h1.line_tx);

    // Drop at 0 -> retry_at_ms = 3000 (3s)
    assert!(wm.tick_tmux_link(0));
    assert!(banner_of(&wm, id).unwrap().text.contains("in 3s"));
    // At 500 ms -> ceil(2500/1000) = 3s -> no change
    assert!(!wm.tick_tmux_link(500));
    // At 1000 ms -> ceil(2000/1000) = 2s -> changed!
    assert!(wm.tick_tmux_link(1000));
    assert!(banner_of(&wm, id).unwrap().text.contains("in 2s"));
}

#[test]
fn percent_exit_transient_vs_ended() {
    // 1. Transient %exit reason -> Backoff
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h = conn.take_next();
    feed(&h.line_tx, &["%output %3 hi", "%exit server exited unexpectedly"]);
    assert!(wm.tick_tmux_link(100));
    assert!(matches!(wm.tmux_link_state(), LinkState::Backoff { .. }));
    assert!(
        banner_of(&wm, id)
            .unwrap()
            .text
            .contains("tmux exited: server exited unexpectedly")
    );

    // 2. %exit with no reason and %exit detached -> Ended, never retries
    for line in ["%exit", "%exit detached"] {
        let mut wm = WindowManager::new_headless(24, 80);
        let conn = TestConnector::new();
        wm.set_link_connector(Box::new(conn.clone()));
        let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        let h = conn.take_next();
        feed(&h.line_tx, &["%output %3 hi", line]);
        assert!(wm.tick_tmux_link(100));
        assert_eq!(*wm.tmux_link_state(), LinkState::Ended, "{line}");
        assert_eq!(
            banner_of(&wm, id).unwrap().text,
            "REMOTE SESSION ENDED | Enter/r: start a new one | d: detach"
        );
        assert_eq!(
            status_of(&wm, id).as_deref(),
            Some("Remote tmux session 'main' on me@box ended")
        );
        assert!(!wm.tick_tmux_link(10_000_000));
        assert_eq!(conn.calls(), 1);
    }
}

#[test]
fn fatal_tail_never_retries() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h = conn.take_next();

    h.driver.push_poll(DriverPoll {
        raw_output: Vec::new(),
        diag: vec!["me@box: Permission denied (publickey).".to_string()],
        control_started: false,
        exited: Some(DriverExit { code: Some(255) }),
    });
    assert!(wm.tick_tmux_link(200));
    assert_eq!(*wm.tmux_link_state(), LinkState::Failed { attempts: 0 });
    let status = status_of(&wm, id).unwrap();
    assert_eq!(
        status,
        "Link to me@box failed: ssh exited 255: me@box: Permission denied (publickey)."
    );
    assert!(!wm.tick_tmux_link(1_000_000));
    assert_eq!(conn.calls(), 1);
}

#[test]
fn reports_once_per_generation() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let _id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h = conn.take_next();

    // Both %exit on the control stream AND driver exited: Some(0) in the same tick:
    // poll_tmux_control reports ControlExit; driver exit must not overwrite or double-report.
    feed(&h.line_tx, &["%output %3 hi", "%exit server exited unexpectedly"]);
    h.driver.push_poll(DriverPoll {
        raw_output: Vec::new(),
        diag: vec![],
        control_started: false,
        exited: Some(DriverExit { code: Some(0) }),
    });
    wm.tick_tmux_link(100);
    let sum = wm.tmux_link_summary();
    assert_eq!(sum.attempt, 1, "counted once");
    assert_eq!(
        sum.last_exit.unwrap().cause,
        "tmux exited: server exited unexpectedly"
    );
}

#[test]
fn max_attempts_gives_up() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.set_link_policy(ReconnectPolicy {
        max_attempts: 3,
        ..ReconnectPolicy::default()
    });
    let conn = TestConnector::dying_immediately();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();

    // Attempt 1 drop at 0 -> retry in 1000
    wm.tick_tmux_link(0);
    assert_eq!(
        *wm.tmux_link_state(),
        LinkState::Backoff {
            attempt: 1,
            retry_at_ms: 1000
        }
    );
    // Spawn #2 at 1000, drops in next tick -> attempt 2, retry in 2000 (at 3000)
    wm.tick_tmux_link(1000);
    wm.tick_tmux_link(1001);
    assert_eq!(
        *wm.tmux_link_state(),
        LinkState::Backoff {
            attempt: 2,
            retry_at_ms: 3001
        }
    );
    // Spawn #3 at 3001, drops -> attempt 3, retry in 4000 (at 7002)
    wm.tick_tmux_link(3001);
    wm.tick_tmux_link(3002);
    assert_eq!(
        *wm.tmux_link_state(),
        LinkState::Backoff {
            attempt: 3,
            retry_at_ms: 7002
        }
    );
    // Spawn #4 at 7002, drops -> exceeds max_attempts (3) -> Failed{attempts: 3}
    wm.tick_tmux_link(7002);
    wm.tick_tmux_link(7003);
    assert_eq!(*wm.tmux_link_state(), LinkState::Failed { attempts: 3 });
    assert_eq!(
        banner_of(&wm, id).unwrap().text,
        "LINK FAILED (connection closed) | Enter/r: retry | d: detach"
    );
    let calls = conn.calls();
    wm.tick_tmux_link(1_000_000);
    assert_eq!(conn.calls(), calls);
}

#[test]
fn stable_connection_resets_attempts() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let _id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();

    // First drop before 30s -> attempt 1
    let h1 = conn.take_next();
    feed(&h1.line_tx, &["%output %3 a"]);
    wm.tick_tmux_link(100);
    drop(h1.line_tx);
    wm.tick_tmux_link(200);
    assert_eq!(wm.tmux_link_summary().attempt, 1);

    // Reconnect at 1200, connected at 1300, stays up for 31s -> drop at 32_500 resets to attempt 1
    wm.tick_tmux_link(1200);
    let h2 = conn.take_next();
    feed(&h2.line_tx, &["%output %3 b"]);
    wm.tick_tmux_link(1300);
    assert!(matches!(wm.tmux_link_state(), LinkState::Connected { since_ms: 1300 }));
    drop(h2.line_tx);
    wm.tick_tmux_link(32_500);
    assert_eq!(
        *wm.tmux_link_state(),
        LinkState::Backoff {
            attempt: 1,
            retry_at_ms: 33_500
        }
    );
}

#[test]
fn banner_applies_to_primary_mapped_and_stale_panes() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let p_primary = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h1 = conn.take_next();
    feed(
        &h1.line_tx,
        &[
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
            "%output %3 one",
            "%output %4 two",
        ],
    );
    wm.tick_tmux_link(100);
    let p_second = wm
        .tmux_link_summary()
        .panes
        .iter()
        .find(|&&(_, t)| t == 4)
        .unwrap()
        .0;

    // Drop -> both primary (%3) and second (%4) get the LINK DOWN banner
    drop(h1.line_tx);
    wm.tick_tmux_link(500);
    assert!(banner_of(&wm, p_primary).unwrap().text.starts_with("LINK DOWN"));
    assert!(banner_of(&wm, p_second).unwrap().text.starts_with("LINK DOWN"));

    // Reconnect at 1500 -> both are now stale (and primary) and show LINK CONNECTING
    wm.tick_tmux_link(1500);
    assert!(banner_of(&wm, p_primary).unwrap().text.starts_with("LINK CONNECTING"));
    assert!(banner_of(&wm, p_second).unwrap().text.starts_with("LINK CONNECTING"));

    let h2 = conn.take_next();
    feed(
        &h2.line_tx,
        &[
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
            "%output %3 ok",
        ],
    );
    wm.tick_tmux_link(1600);
    assert!(banner_of(&wm, p_primary).is_none());
    assert!(banner_of(&wm, p_second).is_none());
}

// -----------------------------------------------------------------------------
// Task 3: user verbs, summary, fleet integration, resize and forget
// -----------------------------------------------------------------------------

#[test]
fn reconnect_now_skips_backoff() {
    let mut wm = WindowManager::new_headless(24, 80);
    assert_eq!(
        wm.link_reconnect(),
        "No tmux link to reconnect (use :ssh-tmux <dest> [session])"
    );

    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h1 = conn.take_next();
    drop(h1.line_tx);
    wm.tick_tmux_link(100);
    assert!(matches!(wm.tmux_link_state(), LinkState::Backoff { attempt: 1, .. }));

    // Reconnect immediately from Backoff:
    let msg = wm.link_reconnect();
    assert_eq!(msg, "Connecting to me@box (tmux session 'main')...");
    assert_eq!(status_of(&wm, id).as_deref(), Some(msg.as_str()));
    assert_eq!(conn.calls(), 2);
    assert_eq!(wm.tmux_link_summary().attempt, 0);
    assert!(matches!(wm.tmux_link_state(), LinkState::Connecting { .. }));

    // Move to Connected, then link_reconnect kills the old driver and spawns #3:
    let h2 = conn.take_next();
    feed(&h2.line_tx, &["%output %3 hi"]);
    wm.tick_tmux_link(200);
    assert!(matches!(wm.tmux_link_state(), LinkState::Connected { .. }));
    wm.link_reconnect();
    assert!(h2.driver.killed());
    assert_eq!(conn.calls(), 3);
    assert!(matches!(wm.tmux_link_state(), LinkState::Connecting { .. }));
}

#[test]
fn user_disconnect_stops_and_keeps_panes() {
    let mut wm = WindowManager::new_headless(24, 80);
    assert_eq!(wm.link_disconnect(), "No tmux link to disconnect");

    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h1 = conn.take_next();
    feed(&h1.line_tx, &["%output %3 keepme"]);
    wm.tick_tmux_link(100);
    let panes_before = wm.pane_count();

    let msg = wm.link_disconnect();
    assert_eq!(
        msg,
        "Disconnected from me@box (tmux session 'main' kept on the remote)"
    );
    assert_eq!(*wm.tmux_link_state(), LinkState::Detached);
    assert!(h1.driver.killed());
    assert!(!wm.is_tmux_control_attached());
    assert_eq!(wm.pane_count(), panes_before);
    assert_eq!(
        banner_of(&wm, id).unwrap().text,
        "LINK STOPPED | Enter/r: reconnect | d: detach"
    );
    assert!(!wm.fleet().nodes.get("tmux-me@box-main").unwrap().connected);

    // Never retries on ticks:
    wm.tick_tmux_link(500_000);
    assert_eq!(conn.calls(), 1);

    // Reconnect spawns again:
    wm.link_reconnect();
    assert_eq!(conn.calls(), 2);
}

#[test]
fn summary_reports_state_and_timers() {
    let mut wm = WindowManager::new_headless(24, 80);
    let idle = wm.tmux_link_summary();
    assert_eq!(idle.state, "idle");
    assert!(idle.dest.is_none() && idle.session.is_none() && idle.panes.is_empty());

    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h1 = conn.take_next();
    feed(&h1.line_tx, &["%output %3 hi", "%exit server exited unexpectedly"]);
    wm.tick_tmux_link(100);

    let s = wm.tmux_link_summary();
    assert_eq!(s.state, "reconnecting");
    assert_eq!(s.dest.as_deref(), Some("me@box"));
    assert_eq!(s.session.as_deref(), Some("main"));
    assert_eq!(s.attempt, 1);
    assert_eq!(s.max_attempts, 20);
    assert!(s.retry_in_ms.is_some());
    assert_eq!(s.primary_pane, Some(id));
    let exit = s.last_exit.expect("last_exit");
    assert_eq!(exit.class, "transient");
    assert_eq!(exit.reason.as_deref(), Some("server exited unexpectedly"));
}

#[test]
fn forget_clears_link_without_touching_panes() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    let _id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h1 = conn.take_next();

    assert!(wm.disconnect_fleet_node("tmux-me@box-main"));
    assert_eq!(*wm.tmux_link_state(), LinkState::Idle);
    assert!(wm.tmux_link_summary().dest.is_none());
    assert_eq!(wm.pane_count(), 1, "node panes removed by disconnect_fleet_node");
    wm.tick_tmux_link(150);
    assert!(h1.driver.killed());

    // Connecting to a new host now succeeds:
    let _id2 = wm.connect_ssh_tmux("other@host", "dev", vec![]).unwrap();
    assert_eq!(conn.calls(), 2);
}

#[test]
fn info_verb_json_matches_summary() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));
    wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let h = conn.take_next();

    let check = |wm: &mut WindowManager| {
        let act: Action = parse_colon_command(":ssh-tmux info").unwrap().unwrap();
        wm.apply(act).unwrap();
        let raw = wm.take_command_data().expect("info data");
        let sum = wm.tmux_link_summary();
        assert!(raw.contains(&format!("\"state\":\"{}\"", sum.state)), "{raw}");
        assert!(raw.contains(&format!("\"attempt\":{}", sum.attempt)), "{raw}");
    };

    check(&mut wm);
    feed(&h.line_tx, &["%output %3 x"]);
    wm.tick_tmux_link(100);
    check(&mut wm);
    drop(h.line_tx);
    wm.tick_tmux_link(200);
    check(&mut wm);
}
