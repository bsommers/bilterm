//! Phase 51.1-15: Tracing contract verification (`51.1-CONTRACT.md` section 11, PST-04).
//!
//! Captures structured `tracing` events via a thread-local subscriber (`with_default`) across the
//! persistent link and remote pane lifecycle scenarios, verifying all documented event names,
//! required fields, target (`bilterm::link`), log levels, and zero leakage of typed passwords,
//! identity-file paths, pane screen text, or the legacy string `Attached`.

use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

use bilterm::input::handle_wm_key_event;
use bilterm::pty::PtyEvent;
use bilterm::remote::{RemoteHostProfile, redact_ssh_argv};
use bilterm::tmux::TmuxControlSubprocess;
use bilterm::tmux::link::{
    DriverPoll, LinkConnector, LinkDriver, LinkTarget, LinkTransport, ReconnectPolicy,
};
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

type LineTx = tokio::sync::mpsc::UnboundedSender<String>;

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

fn capture_with_level<F: FnOnce()>(level: tracing::Level, f: F) -> String {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(LogBuffer(Arc::clone(&buf)))
        .with_max_level(level)
        .with_ansi(false)
        .with_target(true)
        .finish();
    tracing::subscriber::with_default(subscriber, f);
    String::from_utf8(buf.lock().unwrap().clone()).unwrap()
}

#[derive(Clone, Default)]
struct FakeDriverState {
    polls: VecDeque<DriverPoll>,
    written_raw: Vec<u8>,
}

#[derive(Clone, Default)]
struct FakeDriverHandle(Arc<Mutex<FakeDriverState>>);

struct FakeDriver(FakeDriverHandle);

impl LinkDriver for FakeDriver {
    fn pump(&mut self) -> DriverPoll {
        self.0
            .0
            .lock()
            .unwrap()
            .polls
            .pop_front()
            .unwrap_or_default()
    }
    fn write_raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.0
            .0
            .lock()
            .unwrap()
            .written_raw
            .extend_from_slice(bytes);
        Ok(())
    }
    fn resize(&mut self, _rows: u16, _cols: u16) {}
    fn kill(&mut self) {}
}

struct Spawned {
    line_tx: LineTx,
    driver: FakeDriverHandle,
}

#[derive(Default)]
struct TraceConnectorInner {
    spawned: VecDeque<Spawned>,
    next_errors: VecDeque<String>,
    close_immediately: bool,
}

#[derive(Clone, Default)]
struct TraceConnector(Arc<Mutex<TraceConnectorInner>>);

impl TraceConnector {
    fn take_next(&self) -> Spawned {
        self.0
            .lock()
            .unwrap()
            .spawned
            .pop_front()
            .expect("spawned transport")
    }
}

impl LinkConnector for TraceConnector {
    fn connect(&mut self, target: &LinkTarget, _size: (u16, u16)) -> io::Result<LinkTransport> {
        let mut g = self.0.lock().unwrap();
        if let Some(err) = g.next_errors.pop_front() {
            return Err(io::Error::other(err));
        }
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let driver = FakeDriverHandle::default();
        let control = TmuxControlSubprocess::from_channels(&target.session, cmd_tx, line_rx);
        if g.close_immediately {
            drop(line_tx);
        } else {
            g.spawned.push_back(Spawned {
                line_tx,
                driver: driver.clone(),
            });
        }
        Ok(LinkTransport {
            control,
            driver: Some(Box::new(FakeDriver(driver))),
        })
    }
}

fn run_all_scenarios() {
    const SECRET_PASSWORD: &str = "SuperSecretPass99!";
    const SECRET_KEY_PATH: &str = "/home/u/.ssh/id_secret";
    const SECRET_SCREEN_TEXT: &str = "CONFIDENTIAL_PANE_SCREEN_PAYLOAD_42";

    // 1. Connect with profile containing identity_file `/home/u/.ssh/id_secret`, type password,
    //    control up, hydrate, then %exit server exited unexpectedly (retry), then give up.
    let mut wm = WindowManager::new_headless(24, 80);
    let mut profile = RemoteHostProfile::new("prodbox", "ops@box.example");
    profile.identity_file = Some(SECRET_KEY_PATH.to_string());
    wm.config_mut()
        .remotes
        .insert("prodbox".to_string(), profile);
    wm.set_link_policy(ReconnectPolicy {
        max_attempts: 2,
        ..ReconnectPolicy::default()
    });
    let conn = TraceConnector::default();
    wm.set_link_connector(Box::new(conn.clone()));

    let p_primary = wm.connect_ssh_tmux("prodbox", "main", vec![]).unwrap();
    wm.focus_and_raise(p_primary);
    let h1 = conn.take_next();
    h1.driver.0.lock().unwrap().polls.push_back(DriverPoll {
        raw_output: b"Password: ".to_vec(),
        diag: Vec::new(),
        control_started: false,
        exited: None,
    });
    wm.tick_tmux_link(10);

    for ch in SECRET_PASSWORD.chars() {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    wm.tick_tmux_link(20);

    // Control starts + layout + hydrate capture containing SECRET_SCREEN_TEXT
    for line in [
        "%begin 1 1 0",
        "%end 1 1 0",
        "%layout-change @1 b25d,160x48,0,0,3",
    ] {
        h1.line_tx.send(line.to_string()).unwrap();
    }
    wm.tick_tmux_link(100);
    wm.tick_tmux_link(400);
    for line in [
        "%begin 1 2 1",
        "%end 1 2 1",
        "%begin 1 3 1",
        SECRET_SCREEN_TEXT,
        "%end 1 3 1",
    ] {
        h1.line_tx.send(line.to_string()).unwrap();
    }
    wm.tick_tmux_link(410);

    // %exit transient -> emits link.control_exit, link.down, link.retry
    h1.line_tx
        .send("%exit server exited unexpectedly".to_string())
        .unwrap();
    wm.tick_tmux_link(500);

    // Exhaust retries to emit link.give_up
    conn.0.lock().unwrap().close_immediately = true;
    wm.tick_tmux_link(1500);
    wm.tick_tmux_link(1501);
    wm.tick_tmux_link(3501);
    wm.tick_tmux_link(3502);

    // 2. Spawn failure -> emits link.spawn_failed
    let mut wm_fail = WindowManager::new_headless(24, 80);
    let conn_fail = TraceConnector::default();
    conn_fail
        .0
        .lock()
        .unwrap()
        .next_errors
        .push_back("ssh binary missing".to_string());
    wm_fail.set_link_connector(Box::new(conn_fail));
    let _ = wm_fail.connect_ssh_tmux("ops@box.example", "main", vec![]);

    // 3. Exited remote `:ssh` pane -> emits pane.exit
    let mut wm_ssh = WindowManager::new_headless(24, 80);
    let ssh_id = wm_ssh
        .spawn_ssh_pane("ops@box.example", None, vec![])
        .unwrap();
    wm_ssh
        .pane_mut(ssh_id)
        .unwrap()
        .process_pty_output(format!("{SECRET_SCREEN_TEXT}\r\nConnection reset\r\n").as_bytes());
    wm_ssh
        .pane_mut(ssh_id)
        .unwrap()
        .ingest_pty_events(vec![PtyEvent::Exited(Some(255))]);
    wm_ssh.poll_all_ptys();
}

#[test]
fn logs_contain_events_and_no_secrets() {
    let log = capture_with_level(tracing::Level::DEBUG, run_all_scenarios);

    let required_events: &[(&str, &[&str])] = &[
        ("link.connect", &["dest=", "session=", "attempt=", "argv="]),
        ("link.up", &["dest=", "session=", "after_ms="]),
        (
            "link.down",
            &[
                "dest=",
                "session=",
                "cause=",
                "class=",
                "code=",
                "reason=",
                "tail_lines=",
            ],
        ),
        ("link.retry", &["attempt=", "max=", "delay_ms="]),
        ("link.give_up", &["attempts="]),
        ("link.control_exit", &["reason="]),
        ("link.spawn_failed", &["error="]),
        ("link.hydrate", &["panes=", "lines="]),
        ("pane.exit", &["pane=", "code=", "reason=", "remote_target="]),
    ];

    for (ev, fields) in required_events {
        let matching_lines: Vec<&str> = log.lines().filter(|l| l.contains(ev)).collect();
        assert!(
            !matching_lines.is_empty(),
            "missing event `{ev}` in captured log:\n{log}"
        );
        for field in *fields {
            assert!(
                matching_lines.iter().any(|l| l.contains(field)),
                "event `{ev}` missing field `{field}` in lines:\n{}",
                matching_lines.join("\n")
            );
        }
    }

    // Secrets and forbidden strings must never appear in the log.
    assert!(
        !log.contains("SuperSecretPass99!"),
        "typed password leaked into log:\n{log}"
    );
    assert!(
        !log.contains("id_secret"),
        "identity_file path leaked into log:\n{log}"
    );
    assert!(
        !log.contains("CONFIDENTIAL_PANE_SCREEN_PAYLOAD_42"),
        "pane screen text leaked into log:\n{log}"
    );
    assert!(
        !log.contains("Attached"),
        "legacy 'Attached' string appeared in log:\n{log}"
    );
}

#[test]
fn argv_is_redacted_when_logged() {
    let mut profile = RemoteHostProfile::new("prodbox", "ops@box.example");
    profile.identity_file = Some("/home/u/.ssh/id_secret".to_string());
    let args = profile
        .build_ssh_tmux_cc_args("main", Some(Path::new("/tmp/secret-ctl-dir")))
        .unwrap();
    let redacted = redact_ssh_argv(&args);
    assert!(!redacted.contains("id_secret"), "{redacted}");
    assert!(redacted.contains("-i <key>"), "{redacted}");

    let log = capture_with_level(tracing::Level::DEBUG, run_all_scenarios);
    let connect_line = log
        .lines()
        .find(|l| l.contains("link.connect"))
        .expect("link.connect line");
    assert!(
        connect_line.contains("-i <key>"),
        "expected redacted `-i <key>` in link.connect line: {connect_line}"
    );
    assert!(
        !connect_line.contains("id_secret"),
        "secret path must not appear in link.connect line: {connect_line}"
    );
}

#[test]
fn info_level_is_enough_for_the_main_events() {
    let log = capture_with_level(tracing::Level::INFO, run_all_scenarios);

    for ev in [
        "link.connect",
        "link.up",
        "link.down",
        "link.retry",
        "link.give_up",
        "link.control_exit",
        "link.spawn_failed",
        "pane.exit",
    ] {
        assert!(
            log.contains(ev),
            "INFO log must include `{ev}`, got:\n{log}"
        );
    }
    assert!(
        !log.contains("link.hydrate"),
        "link.hydrate is DEBUG-only and must not appear at INFO:\n{log}"
    );
}

#[test]
fn events_use_the_documented_target() {
    let log = capture_with_level(tracing::Level::DEBUG, run_all_scenarios);
    let non_empty: Vec<&str> = log.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(!non_empty.is_empty());
    for line in non_empty {
        assert!(
            line.contains("bilterm::link"),
            "every captured link event must use target `bilterm::link`, got line: {line}"
        );
    }
}
