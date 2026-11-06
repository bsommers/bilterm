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

// ---------------------------------------------------------------------------------------------
// Task 3: shared skeletons, pane fields, respawn dispatcher
// ---------------------------------------------------------------------------------------------

mod skeletons {
    use bilterm::pane::{HeldExitedShellState, LinkBanner};
    use bilterm::remote::RemoteHostProfile;
    use bilterm::tmux::TmuxControlBridge;
    use bilterm::tmux::hydrate::{
        BlockKind, HydratePhase, HydratePlanner, Reply, ReplyCorrelator, ReplyKind,
        capture_pane_command, parse_block_header, render_capture,
    };
    use bilterm::tmux::link::{
        DisconnectCause, ExitClass, LinkConnector, LinkDriver, LinkEvent, LinkMachine, LinkState,
        LinkTarget, ReconnectPolicy, TmuxLinkState, MAPPED_PANES_CAP,
    };
    use bilterm::tmux::reconnect::{backoff_delay_ms, classify_exit, disconnect_text, link_banner_text};
    use bilterm::tmux::splitter::{ControlLineSplitter, SplitOut, diag_lines};
    use bilterm::tmux::transport::{PtyLinkConnector, PtyLinkDriver};
    use bilterm::wm::WindowManager;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    fn target() -> LinkTarget {
        LinkTarget {
            profile: RemoteHostProfile::new("me@box", "me@box"),
            session: "main".into(),
            tags: vec![],
        }
    }

    #[test]
    fn parse_block_header_cases() {
        let b = parse_block_header("%begin 1700000000 4 1").unwrap();
        assert_eq!((b.kind, b.time, b.number, b.flags), (BlockKind::Begin, 1_700_000_000, 4, 1));
        assert_eq!(parse_block_header("%end 1700000000 4 1").unwrap().kind, BlockKind::End);
        assert_eq!(parse_block_header("%error 1700000000 4 1\r\n").unwrap().kind, BlockKind::Error);
        for bad in [
            "", "%begin", "%begin 1 2", "%begin x 2 1", "%begin 1 2 1 9", "%output %1 x",
            "begin 1 2 1", "%endx 1 2 1", "%begin -1 2 1",
        ] {
            assert!(parse_block_header(bad).is_none(), "`{bad}` must not parse");
        }
    }

    #[test]
    fn skeleton_signatures_compile_and_behave_neutrally() {
        // splitter: everything before control mode is raw
        let mut sp = ControlLineSplitter::new();
        assert_eq!(sp.push(b"hello"), vec![SplitOut::Raw(b"hello".to_vec())]);
        assert!(sp.push(b"").is_empty());
        assert!(!sp.in_control());
        assert!(sp.flush_partial().is_empty());
        assert!(diag_lines(b"x\ny").is_empty());

        // transport: no process, no error from the driver, unsupported connector
        let mut d = PtyLinkDriver::default();
        let poll = d.pump();
        assert!(poll.raw_output.is_empty() && poll.exited.is_none() && !poll.control_started);
        d.write_raw(b"x").unwrap();
        d.resize(24, 80);
        d.kill();
        let mut c = PtyLinkConnector::default();
        assert!(c.connect(&target(), (24, 80)).is_err());

        // reconnect helpers
        let p = ReconnectPolicy::default();
        assert_eq!((p.base_ms, p.cap_ms, p.factor, p.max_attempts, p.stable_after_ms), (1000, 30_000, 2, 20, 30_000));
        assert_eq!(backoff_delay_ms(&p, 3), 4000);
        assert_eq!(classify_exit(&DisconnectCause::Eof, None, None, &[]), ExitClass::Transient);
        let mut m = LinkMachine::new(p.clone());
        assert_eq!(*m.state(), LinkState::Idle);
        assert!(m.on_event(LinkEvent::UserDisconnect, 0).is_empty());
        assert!(m.tick(10).is_empty());
        assert!(m.next_deadline_ms().is_none() && m.retry_in_ms(5).is_none());
        let t = target();
        assert!(link_banner_text(&LinkState::Idle, &t, None, &p, 0).is_none());
        let down = bilterm::tmux::link::TmuxDisconnect {
            cause: DisconnectCause::Eof,
            reason: None,
            code: None,
            tail: vec![],
            at_ms: 0,
            class: ExitClass::Transient,
        };
        assert_eq!(disconnect_text(&down), "connection closed");

        // hydrate
        let mut hp = HydratePlanner::new();
        assert_eq!(hp.phase(), HydratePhase::AwaitingGreeting);
        assert!(hp.plan(&[0, 1], 80, 24).is_empty());
        assert!(!hp.note_capture_done(0));
        assert_eq!(hp.remaining(), 0);
        let mut rc = ReplyCorrelator::new();
        rc.enqueue(ReplyKind::Ignore);
        assert_eq!(rc.pending(), 0);
        assert!(!rc.overflowed() && rc.take_failed().is_empty());
        assert_eq!(capture_pane_command(1), "");
        assert!(render_capture(&["a".to_string()]).is_empty());
        let _ = Reply { kind: ReplyKind::Ignore, lines: vec![], ok: true, truncated: false };

        // link record defaults
        let rec = TmuxLinkState::default();
        assert!(rec.target.is_none() && rec.driver.is_none() && rec.connector.is_none());
        assert_eq!(*rec.machine.state(), LinkState::Idle);
        assert_eq!(MAPPED_PANES_CAP, 64);
        assert_eq!(LinkState::Backoff { attempt: 1, retry_at_ms: 1 }.as_str(), "reconnecting");
        assert_eq!(t.badge(), "me@box/main");
        assert_eq!(t.dest(), "me@box");
    }

    #[test]
    fn bridge_facing_signatures_work_over_the_existing_maps() {
        let mut wm = WindowManager::new_headless(24, 80);
        let mut b = TmuxControlBridge::default();
        assert!(b.mapped_panes().is_empty() && b.stale_panes().is_empty());
        assert!(b.exit_reason().is_none() && b.take_removed_panes().is_empty());
        assert_eq!(b.blocks_closed(), 0);
        b.handle_control_line(&mut wm, "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}");
        assert_eq!(b.mapped_panes().len(), 2);
        assert_eq!(b.window_panes(1), vec![3, 4]);
        b.handle_control_line(&mut wm, "%begin 1 1 0");
        b.handle_control_line(&mut wm, "%end 1 1 0");
        b.handle_control_line(&mut wm, "%begin 1 2 0");
        b.handle_control_line(&mut wm, "%error 1 2 0");
        assert_eq!(b.blocks_closed(), 2);
        // output is dropped while held
        let (t3, p3) = b.mapped_panes()[0];
        b.hold_output(t3);
        b.handle_control_line(&mut wm, &format!("%output %{t3} dropped"));
        assert!(!wm.pane(p3).unwrap().vt().contents().contains("dropped"));
        b.release_output(t3);
        b.handle_control_line(&mut wm, &format!("%output %{t3} shown"));
        assert!(wm.pane(p3).unwrap().vt().contents().contains("shown"));
        // queue_command appends the newline
        b.queue_command("list-windows".into(), ReplyKind::Ignore);
        assert_eq!(b.take_outbound_commands(), vec!["list-windows\n".to_string()]);
        assert!(b.take_replies().is_empty());
        // reset moves mappings to stale; nothing re-adopted => all orphans
        b.handle_control_line(&mut wm, "%exit server exited unexpectedly");
        assert_eq!(b.exit_reason(), Some("server exited unexpectedly"));
        b.reset_for_attach();
        assert!(b.exit_reason().is_none() && b.mapped_panes().is_empty());
        assert_eq!(b.stale_panes().len(), 2);
        assert_eq!(b.finish_reconcile().len(), 2);
        assert!(b.stale_panes().is_empty());
    }

    fn held_wm() -> WindowManager {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.active_pane_mut().hold_exited_shell(Some(1));
        assert!(wm.active_pane().held_exited_shell().is_some());
        wm
    }

    #[test]
    fn respawn_active_pane_legacy_branch_is_unchanged() {
        let mut a = held_wm();
        let mut b = held_wm();
        a.respawn_active_pane();
        let _ = b.active_pane_mut().respawn_shell_in_place();
        assert!(a.active_pane().held_exited_shell().is_none());
        assert_eq!(a.active_pane().status_message(), b.active_pane().status_message());
        assert_eq!(a.active_pane().vt().contents(), b.active_pane().vt().contents());
        assert_eq!(a.active_pane().status_message(), Some("Shell respawned cleanly"));

        // The colon form and Enter on a held pane reach the same dispatcher.
        let mut c = held_wm();
        let cmd = bilterm::colon_command::parse_colon_command(":respawn-pane").unwrap().unwrap();
        c.apply(cmd).unwrap();
        assert!(c.active_pane().held_exited_shell().is_none());
        let mut d = held_wm();
        let enter = crossterm::event::KeyEvent::new(crossterm::event::KeyCode::Enter, KeyModifiers::NONE);
        bilterm::input::handle_wm_key_event(&mut d, enter).unwrap();
        assert!(d.active_pane().held_exited_shell().is_none());
    }

    #[test]
    fn interactive_link_banner_routes_respawn_to_link_reconnect() {
        let mut wm = held_wm();
        wm.active_pane_mut().set_link_banner(Some(LinkBanner {
            text: "LINK DOWN".into(),
            interactive: true,
        }));
        wm.respawn_active_pane();
        // Reconnect was requested (no link exists, so only the status line says so); no respawn.
        assert!(wm.active_pane().held_exited_shell().is_some());
        assert_eq!(
            wm.active_pane().status_message(),
            Some("No tmux link to reconnect (use :ssh-tmux <dest> [session])")
        );
        // A non-interactive banner (connecting) never hijacks the respawn.
        let mut wm = held_wm();
        wm.active_pane_mut().set_link_banner(Some(LinkBanner {
            text: "LINK CONNECTING".into(),
            interactive: false,
        }));
        wm.respawn_active_pane();
        assert!(wm.active_pane().held_exited_shell().is_none());
    }

    #[test]
    fn pane_banner_and_remote_hold_api() {
        let mut wm = WindowManager::new_headless(24, 80);
        let pane = wm.active_pane_mut();
        assert!(pane.link_banner().is_none());
        pane.set_link_banner(Some(LinkBanner { text: "x".into(), interactive: true }));
        assert_eq!(pane.link_banner().unwrap().text, "x");
        pane.process_pty_output(b"first\r\nlast line  \r\n\r\n");
        assert_eq!(pane.last_output_line().as_deref(), Some("last line"));
        pane.hold_remote_exit(Some(255), Some("Connection closed".into()));
        let held = pane.held_exited_shell().unwrap();
        assert_eq!(held.exit_code, Some(255));
        assert_eq!(
            held.banner_message,
            "REMOTE SESSION ENDED (code 255): Connection closed | Enter/r: reconnect | d: detach | :quit!: exit"
        );
        assert_eq!(*held, HeldExitedShellState::remote(Some(255), Some("Connection closed".into())));
    }

    #[test]
    fn link_banner_click_hook_does_not_claim_events_yet() {
        let mut wm = WindowManager::new_headless(24, 80);
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 3,
            row: 3,
            modifiers: KeyModifiers::NONE,
        };
        assert!(bilterm::input::link_mouse::handle_link_banner_click(&mut wm, ev, Rect::new(0, 0, 80, 24)).is_none());
    }
}
