//! Phase 51.1-01: honest `:ssh-tmux`, the headless tracer through every layer, the
//! `Action::RemoteLink` family and the shared seam skeletons. No ssh or tmux is ever started:
//! the transport is `TmuxControlSubprocess::from_channels`.

use bilterm::tmux::TmuxControlSubprocess;
use bilterm::tmux::link::LinkState;
use bilterm::wm::WindowManager;

type LineTx = tokio::sync::mpsc::UnboundedSender<String>;
type CmdRx = tokio::sync::mpsc::UnboundedReceiver<String>;

fn attach(wm: &mut WindowManager, session: &str) -> (LineTx, CmdRx) {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    wm.attach_tmux_control(TmuxControlSubprocess::from_channels(session, cmd_tx, line_rx));
    (line_tx, cmd_rx)
}

fn feed(tx: &LineTx, lines: &[&str]) {
    for l in lines {
        tx.send((*l).to_string()).unwrap();
    }
}

fn all_text(wm: &WindowManager) -> Vec<String> {
    let mut out = Vec::new();
    for p in wm.panes().values() {
        out.push(p.title().to_string());
        out.push(p.status_message().unwrap_or("").to_string());
    }
    out.push(wm.active_pane().status_message().unwrap_or("").to_string());
    out
}

#[test]
fn ssh_tmux_status_is_honest_before_transport() {
    let mut wm = WindowManager::new_headless(24, 80);
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let status = wm.pane(id).unwrap().status_message().unwrap().to_string();
    assert!(status.contains("Connecting"), "{status}");
    assert_eq!(status, "Connecting to me@box (tmux session 'main')...");
    for t in all_text(&wm) {
        assert!(!t.contains("Attached"), "claimed an attach: {t}");
    }
    assert!(matches!(
        wm.tmux_link_state(),
        LinkState::Connecting { awaiting_control: true, .. }
    ));
    assert!(wm.fleet().nodes.values().any(|n| matches!(
        n.kind,
        bilterm::remote::FleetNodeKind::SshTmux { .. }
    )));
}

#[test]
fn tracer_headless_link_adopts_primary_pane_and_reports_connected() {
    let mut wm = WindowManager::new_headless(24, 80);
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm, "main");
    feed(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%session-changed $1 main",
            "%layout-change @1 b25d,160x48,0,0,3",
            "%output %3 hello\\015\\012",
        ],
    );
    // Not connected until the tick consumes the first control line.
    assert!(matches!(wm.tmux_link_state(), LinkState::Connecting { .. }));
    assert!(wm.tick_tmux_link(100));
    assert_eq!(wm.pane_count(), 2, "primary adopted, no extra pane");
    let primary = wm.pane(id).unwrap();
    assert!(primary.vt().contents().contains("hello"), "{}", primary.vt().contents());
    assert!(primary.title().contains("%3"), "{}", primary.title());
    assert!(matches!(wm.tmux_link_state(), LinkState::Connected { .. }));
    assert_eq!(
        primary.status_message(),
        Some("Connected: tmux 'main' on me@box")
    );
    assert!(wm.pane_is_linked(id));
}

#[test]
fn second_link_is_refused_with_clear_error() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let before = wm.pane_count();
    let err = wm.connect_ssh_tmux("you@other", "x", vec![]).unwrap_err().to_string();
    assert!(err.contains("already active"), "{err}");
    assert!(err.contains(":ssh-tmux disconnect"), "{err}");
    assert_eq!(wm.pane_count(), before);
}

#[test]
fn bad_destination_creates_no_pane_and_no_link() {
    let mut wm = WindowManager::new_headless(24, 80);
    let before = wm.pane_count();
    assert!(wm.connect_ssh_tmux("-oProxyCommand=evil", "main", vec![]).is_err());
    assert_eq!(wm.pane_count(), before);
    assert_eq!(*wm.tmux_link_state(), LinkState::Idle);
}

#[test]
fn legacy_poll_without_link_is_unchanged() {
    let mut wm = WindowManager::new_headless(24, 80);
    let (tx, _cmd_rx) = attach(&mut wm, "main");
    feed(&tx, &["%output %5 x"]);
    wm.poll_all_ptys();
    assert_eq!(wm.pane_count(), 2);
    assert!(wm.panes().values().any(|p| p.title().starts_with("TMUX %5")));
    assert_eq!(*wm.tmux_link_state(), LinkState::Idle);
}
