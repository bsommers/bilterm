//! Red-phase TDD regression suite for `:ssh-tmux` (fix/ssh-tmux-complete).
//!
//! Root causes covered:
//!  A. Live (non-headless) connect spawns a local `/bin/bash` PTY for the remote pane, so typed
//!     bytes are doubled / executed locally; the prompt stays visible during password entry.
//!  B. An idle remote tmux session emits no `%layout-change`/`%output`: primary never mapped,
//!     hydrate never starts, input dropped, and outbound commands lag one tick.
//!  C. `:ssh-tmux prefill` only works from the plain prompt state.
//!
//! All tests use the fake `LinkConnector`/`LinkDriver` seam (no real ssh). Non-headless tests
//! use `WindowManager::spawn_default` (one real local shell for pane 1 only; the link must never
//! add more local PTYs).
//!
//! Only existing APIs are used (`CompositePane::has_pty`, `spawn_default`), so this file compiles
//! against current code and fails at runtime.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};

use bilterm::action::Action;
use bilterm::input::handle_wm_key_event;
use bilterm::pane::PaneId;
use bilterm::tmux::TmuxControlSubprocess;
use bilterm::tmux::link::{
    DriverPoll, LinkConnector, LinkDriver, LinkState, LinkTarget, LinkTransport,
};
use bilterm::wm::link_cmd::RemoteLinkOp;
use bilterm::wm::{Modal, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

type LineTx = tokio::sync::mpsc::UnboundedSender<String>;
type CmdRx = tokio::sync::mpsc::UnboundedReceiver<String>;

#[derive(Default)]
struct DrvState {
    polls: VecDeque<DriverPoll>,
    written_raw: Vec<u8>,
}

#[derive(Clone, Default)]
struct DrvHandle(Arc<Mutex<DrvState>>);

impl DrvHandle {
    fn written_raw(&self) -> Vec<u8> {
        self.0.lock().unwrap().written_raw.clone()
    }
}

struct FakeDriver(DrvHandle);

impl LinkDriver for FakeDriver {
    fn pump(&mut self) -> DriverPoll {
        self.0.0.lock().unwrap().polls.pop_front().unwrap_or_default()
    }
    fn write_raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.0.0.lock().unwrap().written_raw.extend_from_slice(bytes);
        Ok(())
    }
    fn resize(&mut self, _rows: u16, _cols: u16) {}
    fn kill(&mut self) {}
}

struct Spawned {
    line_tx: LineTx,
    cmd_rx: CmdRx,
    driver: DrvHandle,
}

#[derive(Clone, Default)]
struct Conn(Arc<Mutex<Option<Spawned>>>);

impl Conn {
    fn take(&self) -> Spawned {
        self.0.lock().unwrap().take().expect("transport spawned")
    }
}

impl LinkConnector for Conn {
    fn connect(&mut self, target: &LinkTarget, _size: (u16, u16)) -> io::Result<LinkTransport> {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let driver = DrvHandle::default();
        let control = TmuxControlSubprocess::from_channels(&target.session, cmd_tx, line_rx);
        *self.0.lock().unwrap() = Some(Spawned { line_tx, cmd_rx, driver: driver.clone() });
        Ok(LinkTransport { control, driver: Some(Box::new(FakeDriver(driver))) })
    }
}

fn feed(tx: &LineTx, lines: &[&str]) {
    for l in lines {
        tx.send((*l).to_string()).unwrap();
    }
}

fn drain(rx: &mut CmdRx) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(c) = rx.try_recv() {
        out.push(c);
    }
    out
}

fn key(wm: &mut WindowManager, c: KeyCode) {
    handle_wm_key_event(wm, KeyEvent::new(c, KeyModifiers::NONE)).unwrap();
}

fn type_str(wm: &mut WindowManager, s: &str) {
    for ch in s.chars() {
        key(wm, KeyCode::Char(ch));
    }
}

/// Decodes all `send-keys -t %N -H xx ..` payload bytes found in `cmds`.
fn sent_bytes(cmds: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    for c in cmds {
        if c.starts_with("send-keys") {
            if let Some((_, hex)) = c.split_once(" -H") {
                for h in hex.split_whitespace() {
                    out.push(u8::from_str_radix(h, 16).unwrap());
                }
            }
        }
    }
    out
}

fn count(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}

fn tmux_id_of(wm: &WindowManager, pane: PaneId) -> Option<usize> {
    wm.tmux_link_summary().panes.into_iter().find_map(|(p, t)| (p == pane).then_some(t))
}

fn idle_greeting() -> [&'static str; 3] {
    ["%begin 1 1 0", "%end 1 1 0", "%session-changed $0 bilterm"]
}

// ---------------------------------------------------------------------------------------------
// C. prefill from every state
// ---------------------------------------------------------------------------------------------

#[test]
fn prefill_works_from_normal_raw_copy_linkraw_and_modal_states() {
    type Setup = fn(&mut WindowManager);
    let setups: Vec<(&str, Setup)> = vec![
        ("normal", |_| {}),
        ("raw passthrough", |wm| wm.active_pane_mut().prompt_mut().manual_raw_override = true),
        ("copy mode", |wm| wm.active_pane_mut().enter_copy_mode()),
        ("link_raw_input", |wm| wm.active_pane_mut().set_link_raw_input(true)),
        ("modal help", |wm| *wm.modal_mut() = Modal::Help { scroll: 0 }),
        ("modal palette", |wm| {
            *wm.modal_mut() = Modal::Palette { query: "x".into(), selected: 0 }
        }),
    ];
    for (name, setup) in setups {
        let mut wm = WindowManager::new_headless(24, 80);
        setup(&mut wm);
        wm.apply(Action::RemoteLink(RemoteLinkOp::Prefill)).unwrap();
        assert_eq!(*wm.modal(), Modal::None, "{name}: modal closed");
        let p = wm.active_pane();
        assert!(!p.copy_mode().active, "{name}: not in copy mode");
        assert!(!p.prompt().manual_raw_override, "{name}: manual_raw_override cleared");
        assert!(p.prompt().force_command_prompt, "{name}: force_command_prompt set");
        assert!(p.is_prompt_visible(), "{name}: prompt visible");
        assert_eq!(p.prompt().text(), ":ssh-tmux ", "{name}: prompt text");
        assert_eq!(p.prompt().cursor_char_idx(), ":ssh-tmux ".chars().count(), "{name}: cursor at end");
        assert_eq!(
            p.status_message(),
            Some("Type <user@host> [session] and press Enter"),
            "{name}: status"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// A. no local PTY for remote panes
// ---------------------------------------------------------------------------------------------

#[test]
fn connect_ssh_tmux_never_spawns_local_pty_even_when_not_headless() {
    let mut wm = WindowManager::spawn_default(24, 80).expect("local shell for pane 1");
    let conn = Conn::default();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "bilterm", vec![]).unwrap();
    assert!(!wm.pane(id).unwrap().has_pty(), "primary remote pane must not own a local PTY");
    for (pid, p) in wm.panes() {
        if *pid != 1 {
            assert!(!p.has_pty(), "pane {pid} (remote) has a local PTY");
        }
    }
}

#[test]
fn ssh_password_never_echoed_on_screen_even_when_force_command_prompt_was_true() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = Conn::default();
    wm.set_link_connector(Box::new(conn.clone()));
    // Prefill leaves force_command_prompt = true on the originating pane.
    wm.apply(Action::RemoteLink(RemoteLinkOp::Prefill)).unwrap();
    let id = wm.connect_ssh_tmux("me@box", "bilterm", vec![]).unwrap();
    wm.focus_and_raise(id);
    let mut sp = conn.take();
    sp.driver.0.lock().unwrap().polls.push_back(DriverPoll {
        raw_output: b"me@box's password: ".to_vec(),
        ..Default::default()
    });
    wm.tick_tmux_link(50);
    assert!(matches!(
        wm.tmux_link_state(),
        LinkState::Connecting { awaiting_control: true, .. }
    ));
    // Worst case: stale force_command_prompt AND link_raw_input both set.
    {
        let p = wm.active_pane_mut();
        p.prompt_mut().force_command_prompt = true;
        p.set_link_raw_input(true);
        p.prompt_mut().force_command_prompt = true;
        assert!(!p.is_prompt_visible(), "prompt must be hidden while the link awaits ssh auth");
    }
    type_str(&mut wm, "hunter2");
    key(&mut wm, KeyCode::Enter);
    wm.tick_tmux_link(100);
    assert!(count(&sp.driver.written_raw(), b"hunter2") == 1, "password reaches transport once");
    for (pid, p) in wm.panes() {
        assert!(!p.prompt().text().contains("hunter2"), "pane {pid} prompt echoed password");
        assert!(!p.vt().contents().contains("hunter2"), "pane {pid} screen echoed password");
        assert!(
            !p.prompt().history.iter().any(|h| h.contains("hunter2")),
            "pane {pid} history has password"
        );
    }
    assert!(
        drain(&mut sp.cmd_rx).iter().all(|c| !c.contains("68 75 6e")),
        "password must not be sent as tmux send-keys"
    );
}

// ---------------------------------------------------------------------------------------------
// A/B. exactly-once delivery
// ---------------------------------------------------------------------------------------------

#[test]
fn no_double_characters_on_keystrokes_or_prompt_submission() {
    let mut wm = WindowManager::spawn_default(24, 80).expect("local shell for pane 1");
    let conn = Conn::default();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    wm.focus_and_raise(id);
    let mut sp = conn.take();
    feed(
        &sp.line_tx,
        &["%begin 1 1 0", "%end 1 1 0", "%session-changed $1 main", "%layout-change @1 b25d,80x24,0,0,0"],
    );
    wm.tick_tmux_link(100);
    assert!(matches!(wm.tmux_link_state(), LinkState::Connected { .. }));
    let _ = drain(&mut sp.cmd_rx);

    // Raw keystrokes: each reaches the transport exactly once.
    wm.active_pane_mut().prompt_mut().manual_raw_override = true;
    type_str(&mut wm, "qzx");
    wm.tick_tmux_link(200);
    let raw = sent_bytes(&drain(&mut sp.cmd_rx));
    assert_eq!(raw, b"qzx".to_vec(), "each keystroke exactly once");

    // Prompt submission: the command is sent exactly once, never executed locally.
    wm.active_pane_mut().prompt_mut().manual_raw_override = false;
    type_str(&mut wm, "echo wvu");
    key(&mut wm, KeyCode::Enter);
    wm.tick_tmux_link(300);
    let sent = sent_bytes(&drain(&mut sp.cmd_rx));
    assert_eq!(count(&sent, b"echo wvu"), 1, "submitted command sent once: {sent:?}");
    assert!(!wm.pane(id).unwrap().has_pty(), "no local PTY to double-write into");
}

// ---------------------------------------------------------------------------------------------
// B. idle remote session
// ---------------------------------------------------------------------------------------------

#[test]
fn idle_remote_tmux_session_maps_pane_hydrates_and_delivers_commands_on_same_tick() {
    let mut wm = WindowManager::new_headless(24, 80);
    let conn = Conn::default();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "bilterm", vec![]).unwrap();
    wm.focus_and_raise(id);
    let mut sp = conn.take();
    feed(&sp.line_tx, &idle_greeting());
    wm.tick_tmux_link(100);
    assert!(matches!(wm.tmux_link_state(), LinkState::Connected { .. }));
    assert_eq!(tmux_id_of(&wm, id), Some(0), "primary mapped to %0 without layout/output");
    let cmds = drain(&mut sp.cmd_rx);
    assert!(
        cmds.iter().any(|c| c.starts_with("list-panes -s -F \"#{window_id} #{pane_id} #{window_name}\"")),
        "list-panes queued and written in the same tick: {cmds:?}"
    );

    // Reply, then hydrate must capture %0.
    feed(&sp.line_tx, &["%begin 2 2 1", "@0 %0 bash", "%end 2 2 1"]);
    let mut all = Vec::new();
    for i in 1..=12u64 {
        wm.tick_tmux_link(100 + i * 500);
        all.extend(drain(&mut sp.cmd_rx));
    }
    assert!(all.iter().any(|c| c.starts_with("capture-pane") && c.contains("%0")), "hydrate: {all:?}");

    // Typed command is delivered in the tick that processes it, not dropped.
    type_str(&mut wm, "ls");
    key(&mut wm, KeyCode::Enter);
    wm.tick_tmux_link(7000);
    let sent = sent_bytes(&drain(&mut sp.cmd_rx));
    assert_eq!(count(&sent, b"ls"), 1, "command delivered same tick: {sent:?}");
    assert_ne!(
        wm.pane(id).unwrap().status_message(),
        Some("Link down: input not delivered"),
        "input must not be dropped"
    );
}

#[test]
fn list_panes_reconciles_non_zero_first_pane_id_and_multi_pane_without_local_pty() {
    let mut wm = WindowManager::spawn_default(24, 80).expect("local shell for pane 1");
    let conn = Conn::default();
    wm.set_link_connector(Box::new(conn.clone()));
    let id = wm.connect_ssh_tmux("me@box", "bilterm", vec![]).unwrap();
    let mut sp = conn.take();
    feed(&sp.line_tx, &idle_greeting());
    wm.tick_tmux_link(100);
    let cmds = drain(&mut sp.cmd_rx);
    assert!(cmds.iter().any(|c| c.starts_with("list-panes")), "list-panes sent: {cmds:?}");
    // %0 absent: remote session's panes are %5 and %6.
    feed(&sp.line_tx, &["%begin 2 2 1", "@3 %5 bash", "@3 %6 vim", "%end 2 2 1"]);
    wm.tick_tmux_link(200);
    assert_eq!(tmux_id_of(&wm, id), Some(5), "primary remapped to first listed pane");
    let extra = wm
        .tmux_link_summary()
        .panes
        .into_iter()
        .find_map(|(p, t)| (t == 6).then_some(p))
        .expect("extra pane %6 mapped");
    assert_ne!(extra, id);
    assert!(!wm.pane(extra).unwrap().has_pty(), "extra remote pane has no local PTY");
    assert!(!wm.pane(id).unwrap().has_pty(), "primary has no local PTY");
}
