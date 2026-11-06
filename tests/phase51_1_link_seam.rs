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

// ---------------------------------------------------------------------------------------------
// Task 2: Action::RemoteLink, grammar, executor, key rows
// ---------------------------------------------------------------------------------------------

mod verbs {
    use bilterm::action::Action;
    use bilterm::colon_command::parse_colon_command;
    use bilterm::input::keymap::{DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS};
    use bilterm::input::{AppAction, handle_wm_key_event};
    use bilterm::wm::WindowManager;
    use bilterm::wm::link_cmd::RemoteLinkOp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn parse(s: &str) -> Result<Action, String> {
        parse_colon_command(s).expect("recognised")
    }

    #[test]
    fn grammar_maps_every_verb() {
        for (word, op) in [
            ("reconnect", RemoteLinkOp::Reconnect),
            ("disconnect", RemoteLinkOp::Disconnect),
            ("info", RemoteLinkOp::Info),
            ("prefill", RemoteLinkOp::Prefill),
        ] {
            assert_eq!(parse(&format!(":ssh-tmux {word}")).unwrap(), Action::RemoteLink(op));
            assert_eq!(parse(&format!(":sshtmux {word}")).unwrap(), Action::RemoteLink(op));
            assert_eq!(parse(&format!(":ssh-tmux {}", word.to_uppercase())).unwrap(), Action::RemoteLink(op));
            assert_eq!(parse(op.colon_form()).unwrap(), Action::RemoteLink(op), "colon_form round-trips");
        }
        assert_eq!(
            parse(":ssh-tmux me@box main").unwrap(),
            Action::SshTmuxConnect { target: "me@box".into(), session: "main".into() }
        );
        for list in [":ssh-tmux", ":ssh-tmux list", ":ssh-tmux ls", ":ssh-tmux status"] {
            assert_eq!(parse(list).unwrap(), Action::RemoteList, "{list}");
        }
        for bad in [":ssh-tmux reconnect now", ":ssh-tmux info x", ":sshtmux prefill a b"] {
            let err = parse(bad).unwrap_err();
            assert!(err.starts_with("Usage: :ssh-tmux reconnect | disconnect | info | prefill"), "{bad}: {err}");
        }
    }

    #[test]
    fn verbs_without_a_link_set_a_status_and_never_error() {
        let mut wm = WindowManager::new_headless(24, 80);
        for (op, want) in [
            (RemoteLinkOp::Reconnect, "No tmux link to reconnect (use :ssh-tmux <dest> [session])"),
            (RemoteLinkOp::Disconnect, "No tmux link to disconnect"),
            (RemoteLinkOp::Info, "No tmux link (use :ssh-tmux <dest> [session])"),
        ] {
            wm.apply(Action::RemoteLink(op)).expect("never an error without a link");
            assert_eq!(wm.active_pane().status_message(), Some(want));
        }
        // `Info` stored structured data.
        wm.apply(Action::RemoteLink(RemoteLinkOp::Info)).unwrap();
        let data = wm.take_command_data().expect("info stores data");
        assert!(data.contains("\"state\":\"idle\""), "{data}");
        assert!(wm.take_command_data().is_none(), "data is taken once");
    }

    #[test]
    fn prefill_sets_the_prompt() {
        let mut wm = WindowManager::new_headless(24, 80);
        assert!(wm.active_pane().is_prompt_visible());
        wm.apply(Action::RemoteLink(RemoteLinkOp::Prefill)).unwrap();
        assert_eq!(wm.active_pane().prompt().text(), ":ssh-tmux ");
    }

    fn leader_row(code: char) -> Option<&'static Action> {
        DEFAULT_LEADER_BINDINGS.iter().find(|(c, _)| *c == KeyCode::Char(code)).map(|(_, a)| a)
    }

    fn global_rows(code: char) -> Vec<&'static Action> {
        let ca = KeyModifiers::CONTROL | KeyModifiers::ALT;
        DEFAULT_GLOBAL_BINDINGS
            .iter()
            .filter(|(m, c, _)| *m == ca && *c == KeyCode::Char(code))
            .map(|(_, _, a)| a)
            .collect()
    }

    #[test]
    fn keymap_rows_lower_to_the_actions() {
        assert_eq!(leader_row('u'), Some(&Action::RemoteLink(RemoteLinkOp::Prefill)));
        assert_eq!(leader_row('U'), Some(&Action::RemoteLink(RemoteLinkOp::Reconnect)));
        assert_eq!(leader_row('Q'), Some(&Action::RemoteLink(RemoteLinkOp::Disconnect)));
        for c in ['r', 'R'] {
            assert_eq!(global_rows(c), vec![&Action::RemoteLink(RemoteLinkOp::Reconnect)]);
        }
        // Each link chord is unique across the tables (read live, nothing hard-coded).
        let link = |a: &Action| matches!(a, Action::RemoteLink(_));
        assert_eq!(DEFAULT_LEADER_BINDINGS.iter().filter(|(_, a)| link(a)).count(), 3);
        assert_eq!(DEFAULT_GLOBAL_BINDINGS.iter().filter(|(_, _, a)| link(a)).count(), 2);
        for c in ['u', 'U', 'Q'] {
            assert_eq!(DEFAULT_LEADER_BINDINGS.iter().filter(|(k, _)| *k == KeyCode::Char(c)).count(), 1, "leader {c}");
        }
        let ca = KeyModifiers::CONTROL | KeyModifiers::ALT;
        for c in ['r', 'R'] {
            let n = DEFAULT_GLOBAL_BINDINGS.iter().filter(|(m, k, _)| *m == ca && *k == KeyCode::Char(c)).count();
            assert_eq!(n, 1, "Ctrl+Alt+{c}");
        }
    }

    #[test]
    fn ctrl_alt_r_reaches_the_wm_in_raw_passthrough_and_leader_keys_run_verbs() {
        let mut wm = WindowManager::new_headless(24, 80);
        // Alternate screen forces raw passthrough, where only reserved global chords reach the WM.
        wm.active_pane_mut().process_pty_output(b"\x1b[?1049h");
        assert!(wm.active_pane().vt().is_alternate_screen());
        let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL | KeyModifiers::ALT);
        let r = handle_wm_key_event(&mut wm, key).unwrap();
        assert!(matches!(r, AppAction::Handled), "{r:?}");
        assert_eq!(
            wm.active_pane().status_message(),
            Some("No tmux link to reconnect (use :ssh-tmux <dest> [session])")
        );

        let mut wm = WindowManager::new_headless(24, 80);
        let leader = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        handle_wm_key_event(&mut wm, leader).unwrap();
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::SHIFT)).unwrap();
        assert_eq!(wm.active_pane().status_message(), Some("No tmux link to disconnect"));
    }
}
