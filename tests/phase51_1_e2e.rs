//! Phase 51.1-15: End-to-end drop/reconnect/hydrate/hold scenarios and deterministic-LCG control
//! stream fuzz (PST-01, PST-02, PST-03, PST-04).
//!
//! Uses only `WindowManager::new_headless`, `TmuxControlSubprocess::from_channels`, an injected
//! `LinkConnector` and a fake `LinkDriver`. All time advances via `tick_tmux_link(ms)`.

use std::collections::VecDeque;
use std::fs;
use std::io;
use std::sync::{Arc, Mutex};

use bilterm::input::handle_wm_key_event;
use bilterm::pane::PaneId;
use bilterm::pty::PtyEvent;
use bilterm::tmux::TmuxControlSubprocess;
use bilterm::tmux::hydrate::HydratePhase;
use bilterm::tmux::link::{
    DriverExit, DriverPoll, LinkConnector, LinkDriver, LinkState, LinkTarget, LinkTransport,
    MAPPED_PANES_CAP, OUTBOUND_CAP,
};
use bilterm::wm::{SavedWorkspaceSession, SplitDirection, WindowManager};
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

    fn written_raw(&self) -> Vec<u8> {
        self.0.lock().unwrap().written_raw.clone()
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
    line_tx: LineTx,
    cmd_rx: CmdRx,
    driver: FakeDriverHandle,
}

#[derive(Default)]
struct TestConnectorInner {
    calls: usize,
    spawned: VecDeque<SpawnedHandle>,
    next_errors: VecDeque<String>,
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
    fn connect(&mut self, target: &LinkTarget, _size: (u16, u16)) -> io::Result<LinkTransport> {
        let mut g = self.0.lock().unwrap();
        g.calls += 1;
        if let Some(err) = g.next_errors.pop_front() {
            return Err(io::Error::other(err));
        }
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let driver = FakeDriverHandle::default();
        let control = TmuxControlSubprocess::from_channels(&target.session, cmd_tx, line_rx);
        g.spawned.push_back(SpawnedHandle {
            target: target.clone(),
            line_tx,
            cmd_rx,
            driver: driver.clone(),
        });
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

#[test]
fn drop_reconnect_hydrate_scenario() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));

    // 1. Connect -> ssh prompt text appears in the primary pane
    let p_primary = wm.connect_ssh_tmux("user@box", "main", vec![]).unwrap();
    wm.focus_and_raise(p_primary);
    assert_eq!(conn.calls(), 1);
    let mut h1 = conn.take_next();
    assert_eq!(h1.target.dest(), "user@box");
    assert_eq!(h1.target.session, "main");

    h1.driver.push_poll(DriverPoll {
        raw_output: b"user@box's password: ".to_vec(),
        diag: Vec::new(),
        control_started: false,
        exited: None,
    });
    assert!(wm.tick_tmux_link(50));
    assert!(
        wm.pane(p_primary)
            .unwrap()
            .vt()
            .contents()
            .contains("user@box's password:")
    );

    // 2. Typed password reaches only driver.write_raw (never prompt text, history or outbound queue)
    for ch in "hunter2".chars() {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    wm.tick_tmux_link(100);
    assert_eq!(h1.driver.written_raw(), b"hunter2\r");
    assert!(wm.pane(p_primary).unwrap().prompt().history.is_empty());
    assert_eq!(wm.pane(p_primary).unwrap().prompt().text(), "");
    assert!(drain_cmds(&mut h1.cmd_rx).is_empty());

    // 3. Control start at 400 ms, %layout-change with tmux panes 3 and 4
    feed(
        &h1.line_tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%session-changed $1 main",
            "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}",
        ],
    );
    assert!(wm.tick_tmux_link(400));
    assert_eq!(*wm.tmux_link_state(), LinkState::Connected { since_ms: 400 });
    let p3 = pane_for_tmux(&wm, 3).expect("pane for %3");
    let p4 = pane_for_tmux(&wm, 4).expect("pane for %4");
    assert_eq!(p3, p_primary, "first tmux pane %3 adopts the primary pane");
    assert_ne!(p4, p3);

    // 4. Hydrate after 300 ms settle (at 700 ms): refresh-client + two capture-pane commands
    wm.tick_tmux_link(700);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Sent);
    assert_hydrate_batch(drain_cmds(&mut h1.cmd_rx), &[3, 4]);
    feed(
        &h1.line_tx,
        &[
            "%begin 1 2 1",
            "%end 1 2 1",
            "%begin 1 3 1",
            "welcome_pane_three",
            "%end 1 3 1",
            "%begin 1 4 1",
            "welcome_pane_four",
            "%end 1 4 1",
        ],
    );
    wm.tick_tmux_link(710);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Done);
    assert!(
        wm.pane(p3)
            .unwrap()
            .vt()
            .contents()
            .contains("welcome_pane_three")
    );
    assert!(
        wm.pane(p4)
            .unwrap()
            .vt()
            .contents()
            .contains("welcome_pane_four")
    );

    // 5. `ls` typed in pane for %3 sent as `send-keys -t %3 -H 6c 73 0a`
    wm.pane_mut(p3).unwrap().submit_command("ls").unwrap();
    wm.tick_tmux_link(750);
    // Flow control is requested once the hydrate is done (it precedes the first keystrokes).
    assert_eq!(
        drain_cmds(&mut h1.cmd_rx),
        vec![
            "refresh-client -f pause-after=2\n".to_string(),
            "send-keys -t %3 -H 6c 73 0a\n".to_string(),
        ]
    );

    // 6. EOF at 10 s (10_000 ms) -> LINK DOWN banners on both panes
    drop(h1.line_tx);
    assert!(wm.tick_tmux_link(10_000));
    assert_eq!(
        *wm.tmux_link_state(),
        LinkState::Backoff {
            attempt: 1,
            retry_at_ms: 11_000
        }
    );
    assert!(
        wm.pane(p3)
            .unwrap()
            .link_banner()
            .unwrap()
            .text
            .starts_with("LINK DOWN")
    );
    assert!(
        wm.pane(p4)
            .unwrap()
            .link_banner()
            .unwrap()
            .text
            .starts_with("LINK DOWN")
    );

    // 7. Retry at 11 s (11_000 ms): new transport, same layout -> same two Bilterm panes reused
    assert!(wm.tick_tmux_link(11_000));
    assert_eq!(conn.calls(), 2);
    let mut h2 = conn.take_next();
    feed(
        &h2.line_tx,
        &[
            "%begin 2 1 0",
            "%end 2 1 0",
            "%session-changed $1 main",
            "%layout-change @1 b25d,160x48,0,0,3",
            "%layout-change @2 b25d,160x48,0,0,4",
        ],
    );
    assert!(wm.tick_tmux_link(11_100));
    assert_eq!(
        *wm.tmux_link_state(),
        LinkState::Connected { since_ms: 11_100 }
    );
    assert_eq!(pane_for_tmux(&wm, 3), Some(p3), "%3 reused same PaneId");
    assert_eq!(pane_for_tmux(&wm, 4), Some(p4), "%4 reused same PaneId");

    // 8. Hydrate again on generation 2
    wm.tick_tmux_link(11_400);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Sent);
    assert_hydrate_batch(drain_cmds(&mut h2.cmd_rx), &[3, 4]);
    feed(
        &h2.line_tx,
        &[
            "%begin 2 2 1",
            "%end 2 2 1",
            "%begin 2 3 1",
            "rehydrated_three",
            "%end 2 3 1",
            "%begin 2 4 1",
            "rehydrated_four",
            "%end 2 4 1",
        ],
    );
    wm.tick_tmux_link(11_410);
    assert_eq!(wm.link_hydrate_phase(), HydratePhase::Done);
    assert!(
        wm.pane(p3)
            .unwrap()
            .vt()
            .contents()
            .contains("rehydrated_three")
    );
    assert!(
        wm.pane(p4)
            .unwrap()
            .vt()
            .contents()
            .contains("rehydrated_four")
    );

    // 9. `%window-close @2` removes pane p4 (%4)
    let count_before_close = wm.pane_count();
    feed(&h2.line_tx, &["%window-close @2"]);
    wm.tick_tmux_link(11_500);
    assert_eq!(wm.pane_count(), count_before_close - 1);
    assert!(wm.pane(p4).is_none());
    assert!(wm.pane(p3).is_some());

    // 10. `%exit` with no reason -> REMOTE SESSION ENDED, no retry for 10 simulated minutes
    feed(&h2.line_tx, &["%exit"]);
    wm.tick_tmux_link(12_000);
    assert_eq!(*wm.tmux_link_state(), LinkState::Ended);
    let ended_banner = wm.pane(p3).unwrap().link_banner().expect("ended banner");
    assert!(ended_banner.text.starts_with("REMOTE SESSION ENDED"));
    assert!(ended_banner.interactive);
    for step in 1..=60 {
        wm.tick_tmux_link(12_000 + step * 10_000);
    }
    assert_eq!(
        conn.calls(),
        2,
        "no connector call during 10 simulated minutes in Ended"
    );

    // 11. `r` on the banner starts a fresh link (connector call #3), state Connecting
    wm.focus_and_raise(p3);
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
    )
    .unwrap();
    assert_eq!(conn.calls(), 3);
    assert!(matches!(wm.tmux_link_state(), LinkState::Connecting { .. }));
}

#[test]
fn exit_vs_retry_matrix() {
    #[derive(Clone, Debug)]
    enum ScenarioKind {
        ControlLine(&'static str),
        DriverExit {
            code: Option<i32>,
            tail: &'static str,
        },
        SpawnErr(&'static str),
    }

    let cases: &[(&str, ScenarioKind, &str, bool)] = &[
        (
            "server_exited_unexpectedly_retries",
            ScenarioKind::ControlLine("%exit server exited unexpectedly"),
            "failed", // after 10 min of failing retries (20 attempts exhausted)
            true,
        ),
        (
            "detached_ends_without_retry",
            ScenarioKind::ControlLine("%exit detached"),
            "ended",
            false,
        ),
        (
            "permission_denied_fails_immediately",
            ScenarioKind::DriverExit {
                code: Some(255),
                tail: "user@box: Permission denied (publickey).",
            },
            "failed",
            false,
        ),
        (
            "connection_refused_retries",
            ScenarioKind::DriverExit {
                code: Some(255),
                tail: "ssh: connect to host box port 22: Connection refused",
            },
            "failed", // retries up to max_attempts over 10 min
            true,
        ),
        (
            "spawn_error_fails_immediately",
            ScenarioKind::SpawnErr("No such file or directory"),
            "failed",
            false,
        ),
        (
            "exit_zero_ends_without_retry",
            ScenarioKind::DriverExit {
                code: Some(0),
                tail: "",
            },
            "ended",
            false,
        ),
    ];

    for (label, kind, expected_final_state, expects_retries) in cases {
        let mut wm = WindowManager::new_headless(24, 80);
        let conn = match kind {
            ScenarioKind::SpawnErr(msg) => TestConnector::failing(msg),
            _ => TestConnector::new(),
        };
        wm.set_link_connector(Box::new(conn.clone()));
        let _id = wm.connect_ssh_tmux("user@box", "main", vec![]).unwrap();

        match kind {
            ScenarioKind::ControlLine(line) => {
                let h = conn.take_next();
                feed(&h.line_tx, &["%output %3 hi", line]);
                wm.tick_tmux_link(100);
            }
            ScenarioKind::DriverExit { code, tail } => {
                let h = conn.take_next();
                h.driver.push_poll(DriverPoll {
                    raw_output: Vec::new(),
                    diag: if tail.is_empty() {
                        Vec::new()
                    } else {
                        vec![(*tail).to_string()]
                    },
                    control_started: false,
                    exited: Some(DriverExit { code: *code }),
                });
                wm.tick_tmux_link(100);
            }
            ScenarioKind::SpawnErr(_) => {}
        }

        if *expects_retries {
            assert_eq!(
                wm.tmux_link_summary().state,
                "reconnecting",
                "{label} should enter reconnecting first"
            );
        }

        // Advance clock across 10 simulated minutes (600_000 ms) in 1s steps, dropping any new transports.
        for t in (1000..=600_000).step_by(1000) {
            if matches!(wm.tmux_link_state(), LinkState::Connecting { .. }) {
                let h = conn.take_next();
                drop(h.line_tx);
            }
            wm.tick_tmux_link(t);
            if matches!(wm.tmux_link_state(), LinkState::Connecting { .. }) {
                let h = conn.take_next();
                drop(h.line_tx);
                wm.tick_tmux_link(t + 1);
            }
        }

        assert_eq!(
            wm.tmux_link_summary().state,
            *expected_final_state,
            "{label} final state"
        );
        if *expects_retries {
            assert_eq!(
                conn.calls(),
                21,
                "{label}: 1 initial + 20 retries across 10 minutes"
            );
        } else {
            assert_eq!(conn.calls(), 1, "{label}: must never retry");
        }
    }
}

#[test]
fn ssh_pane_exit_is_held_and_restore_is_honest() {
    let sessions = tempfile::tempdir().unwrap();
    let sess_dir = sessions.path().to_string_lossy().into_owned();

    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().session_dir = Some(sess_dir.clone());
    let ssh_id = wm.spawn_ssh_pane("user@box", None, vec![]).unwrap();
    wm.pane_mut(ssh_id)
        .unwrap()
        .process_pty_output(b"client_loop: send disconnect: Broken pipe\r\n");
    wm.pane_mut(ssh_id)
        .unwrap()
        .ingest_pty_events(vec![PtyEvent::Exited(Some(255))]);
    assert!(wm.poll_all_ptys());

    let held = wm
        .pane(ssh_id)
        .unwrap()
        .held_exited_shell()
        .expect("ssh pane held on exit");
    assert_eq!(held.exit_code, Some(255));
    assert!(held.banner_message.contains("255"), "{}", held.banner_message);
    assert!(
        held.banner_message.contains("Broken pipe"),
        "{}",
        held.banner_message
    );

    // Save session, rewrite pane 2 to a local shell `/bin/bash` with `remote_target`, then restore.
    let mut wm_save = WindowManager::new_headless(24, 80);
    wm_save.split_focused(SplitDirection::Vertical).unwrap();
    wm_save.config_mut().session_dir = Some(sess_dir.clone());
    let path = wm_save.save_session("e2e_honest").unwrap();

    let raw = fs::read_to_string(&path).unwrap();
    let mut saved: SavedWorkspaceSession = toml::from_str(&raw).unwrap();
    for p in &mut saved.panes {
        if p.id == 2 {
            p.shell = Some("/bin/bash".into());
            p.remote_target = Some("user@box/main".into());
            p.title = "box: main".into();
        }
    }
    fs::write(&path, toml::to_string(&saved).unwrap()).unwrap();

    let mut wm_restored = WindowManager::new_headless(24, 80);
    wm_restored.config_mut().session_dir = Some(sess_dir);
    wm_restored.restore_session("e2e_honest").unwrap();

    let p2 = wm_restored.pane(2).unwrap();
    assert_eq!(p2.remote_target(), None, "fake remote badge must be dropped");
    assert!(
        p2.title().ends_with(" (not connected)"),
        "title was: {}",
        p2.title()
    );
}

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    fn gen_range(&mut self, upper: u32) -> u32 {
        if upper == 0 {
            0
        } else {
            self.next_u32() % upper
        }
    }
}

#[test]
fn fuzz_control_lines_never_break_the_workspace() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));

    let local_pane_1_before = wm.pane(1).unwrap().vt().contents();
    let _primary = wm.connect_ssh_tmux("fuzz@box", "fuzz", vec![]).unwrap();
    let h = conn.take_next();

    let mut rng = Lcg::new(0x5110_15E2_E000_CAFE);
    for i in 0..5000usize {
        let kind = rng.gen_range(12);
        let id = match rng.gen_range(4) {
            0 => rng.gen_range(80) as usize,
            1 => rng.gen_range(10_000) as usize,
            _ => rng.next_u32() as usize,
        };
        let win = rng.gen_range(200) as usize;
        let line = match kind {
            0 => format!("%output %{id} hello_{i}\\015\\012"),
            1 => {
                let id2 = id.wrapping_add(1);
                format!("%layout-change @{win} b25d,160x48,0,0{{80x48,0,0,{id},79x48,81,0,{id2}}}")
            }
            2 => format!("%window-add @{win}"),
            3 => format!("%window-close @{win}"),
            4 => format!("%window-pane-changed @{win} %{id}"),
            5 => format!("%window-renamed @{win} win_{i}_\x1b[31mred\x07"),
            6 => format!("%session-changed ${win} sess_{i}"),
            7 => format!("%begin {i} {} 1", rng.gen_range(500)),
            8 => format!("%end {i} {} 1", rng.gen_range(500)),
            9 => format!("%error {i} {} 1", rng.gen_range(500)),
            10 => {
                // Truncated / octal junk / bare % / empty
                match rng.gen_range(5) {
                    0 => "%".to_string(),
                    1 => "".to_string(),
                    2 => format!("%output %{id} \\777\\999\\00"),
                    3 => "%layout-change @notanumber {{{{".to_string(),
                    _ => format!("garbage line {i} \x1b]0;evil\x07"),
                }
            }
            _ => {
                // Very long payload (up to 100 KB on selected iterations to stay fast and test bounds)
                let len = if i % 500 == 0 {
                    100_000
                } else {
                    (rng.gen_range(512) + 64) as usize
                };
                let body = "X".repeat(len);
                format!("%output %{id} {body}")
            }
        };

        h.line_tx.send(line).unwrap();
        if i % 25 == 0 {
            wm.tick_tmux_link((i as u64) * 10);
            assert!(
                wm.pane_count() <= MAPPED_PANES_CAP + 2,
                "pane_count {} exceeded bound at step {i}",
                wm.pane_count()
            );
            assert!(
                wm.tmux_outbound_queued() <= OUTBOUND_CAP,
                "outbound queue {} exceeded OUTBOUND_CAP at step {i}",
                wm.tmux_outbound_queued()
            );
            assert_eq!(
                wm.pane(1).unwrap().vt().contents(),
                local_pane_1_before,
                "local pane 1 (unmapped) was modified at step {i}"
            );
        }
    }

    // Close with a clean %exit so the link ends in a defined state.
    h.line_tx.send("%exit".to_string()).unwrap();
    wm.tick_tmux_link(60_000);
    assert!(
        wm.pane_count() <= MAPPED_PANES_CAP + 2,
        "final pane_count {}",
        wm.pane_count()
    );
    assert!(wm.tmux_outbound_queued() <= OUTBOUND_CAP);
    assert_eq!(wm.pane(1).unwrap().vt().contents(), local_pane_1_before);
    assert!(matches!(
        wm.tmux_link_state(),
        LinkState::Ended | LinkState::Connected { .. } | LinkState::Backoff { .. }
    ));
}

#[test]
fn fake_connector_is_the_only_transport() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = TestConnector::new();
    wm.set_link_connector(Box::new(conn.clone()));

    assert_eq!(conn.calls(), 0);
    let _id = wm.connect_ssh_tmux("user@box", "main", vec![]).unwrap();
    assert_eq!(conn.calls(), 1);
    let h = conn.take_next();
    drop(h.line_tx);
    wm.tick_tmux_link(100);
    wm.tick_tmux_link(1100);
    assert_eq!(conn.calls(), 2);
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
