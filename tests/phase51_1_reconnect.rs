//! Phase 51.1-04: the pure reconnect brain. Fake clock only: every call passes `now_ms`, nothing
//! sleeps and no process is started.

use bilterm::pane::LinkBanner;
use bilterm::remote::RemoteHostProfile;
use bilterm::tmux::link::{
    DisconnectCause, ExitClass, LinkCmd, LinkEvent, LinkMachine, LinkState, LinkTarget,
    ReconnectPolicy, TmuxDisconnect,
};
use bilterm::tmux::reconnect::{
    backoff_delay_ms, classify_exit, disconnect_text, link_banner_text,
};

fn down(class: ExitClass, cause: DisconnectCause, at_ms: u64) -> TmuxDisconnect {
    TmuxDisconnect {
        cause,
        reason: None,
        code: None,
        tail: Vec::new(),
        at_ms,
        class,
    }
}

fn transient(at_ms: u64) -> LinkEvent {
    LinkEvent::Down(down(ExitClass::Transient, DisconnectCause::Eof, at_ms))
}

fn target() -> LinkTarget {
    LinkTarget {
        profile: RemoteHostProfile::new("box", "me@box"),
        session: "main".to_string(),
        tags: Vec::new(),
    }
}

fn connected_machine(policy: ReconnectPolicy, started_at: u64) -> LinkMachine {
    let mut m = LinkMachine::new(policy);
    m.on_event(LinkEvent::Connect, 0);
    m.on_event(LinkEvent::ControlStarted, started_at);
    m
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_string()).collect()
}

#[test]
fn backoff_table() {
    let p = ReconnectPolicy::default();
    assert_eq!(backoff_delay_ms(&p, 0), 0);
    let got: Vec<u64> = (1..=8).map(|n| backoff_delay_ms(&p, n)).collect();
    assert_eq!(
        got,
        vec![1000, 2000, 4000, 8000, 16000, 30000, 30000, 30000]
    );
    let c = ReconnectPolicy {
        base_ms: 500,
        cap_ms: 4000,
        factor: 3,
        ..ReconnectPolicy::default()
    };
    let got: Vec<u64> = (1..=3).map(|n| backoff_delay_ms(&c, n)).collect();
    assert_eq!(got, vec![500, 1500, 4000]);
    assert_eq!(backoff_delay_ms(&p, 200), 30000);
    assert_eq!(backoff_delay_ms(&p, u32::MAX), 30000);
    let huge = ReconnectPolicy {
        base_ms: u64::MAX,
        cap_ms: u64::MAX,
        factor: u32::MAX,
        ..ReconnectPolicy::default()
    };
    assert_eq!(backoff_delay_ms(&huge, u32::MAX), u64::MAX);
}

#[test]
fn connect_then_control_started() {
    let mut m = LinkMachine::new(ReconnectPolicy::default());
    assert_eq!(m.on_event(LinkEvent::Connect, 0), vec![LinkCmd::Spawn]);
    assert_eq!(
        *m.state(),
        LinkState::Connecting {
            since_ms: 0,
            awaiting_control: true
        }
    );
    assert_eq!(
        m.on_event(LinkEvent::ControlStarted, 400),
        vec![LinkCmd::MarkConnected]
    );
    assert_eq!(*m.state(), LinkState::Connected { since_ms: 400 });
}

#[test]
fn transient_drop_backs_off_then_spawns() {
    let mut m = connected_machine(ReconnectPolicy::default(), 400);
    assert_eq!(
        m.on_event(transient(1000), 1000),
        vec![LinkCmd::MarkDown { retry_in_ms: 1000 }]
    );
    assert_eq!(
        *m.state(),
        LinkState::Backoff {
            attempt: 1,
            retry_at_ms: 2000
        }
    );
    assert!(m.tick(1999).is_empty());
    assert_eq!(m.tick(2000), vec![LinkCmd::Spawn]);
    assert!(matches!(m.state(), LinkState::Connecting { .. }));
    assert_eq!(
        m.on_event(transient(2500), 2500),
        vec![LinkCmd::MarkDown { retry_in_ms: 2000 }]
    );
    assert_eq!(
        *m.state(),
        LinkState::Backoff {
            attempt: 2,
            retry_at_ms: 4500
        }
    );
    assert_eq!(m.retry_in_ms(3000), Some(1500));
    assert_eq!(m.next_deadline_ms(), Some(4500));
    assert_eq!(m.retry_in_ms(9000), Some(0));
}

#[test]
fn attempt_cap_fails() {
    let policy = ReconnectPolicy {
        max_attempts: 3,
        ..ReconnectPolicy::default()
    };
    let mut m = connected_machine(policy, 0);
    let mut now = 100;
    for n in 1..=3u32 {
        let cmds = m.on_event(transient(now), now);
        assert!(matches!(cmds.as_slice(), [LinkCmd::MarkDown { .. }]), "{n}");
        assert!(matches!(m.state(), LinkState::Backoff { attempt, .. } if *attempt == n));
        now += 100_000;
        assert_eq!(m.tick(now), vec![LinkCmd::Spawn]);
    }
    assert_eq!(m.on_event(transient(now), now), vec![LinkCmd::MarkFailed]);
    assert_eq!(*m.state(), LinkState::Failed { attempts: 3 });
    for t in [now + 1, now + 1_000_000, u64::MAX] {
        assert!(m.tick(t).is_empty());
    }
}

#[test]
fn stable_connection_resets_counter() {
    // Connected for 31000 ms: the count restarts at 1.
    let mut m = connected_machine(ReconnectPolicy::default(), 0);
    m.on_event(transient(31_000), 31_000);
    assert!(matches!(m.state(), LinkState::Backoff { attempt: 1, .. }));

    // Two failed connects, then a short connection: the count keeps climbing.
    let mut m = LinkMachine::new(ReconnectPolicy::default());
    m.on_event(LinkEvent::Connect, 0);
    m.on_event(transient(10), 10);
    m.tick(1_010);
    m.on_event(transient(1_020), 1_020);
    assert!(matches!(m.state(), LinkState::Backoff { attempt: 2, .. }));
    m.tick(3_020);
    m.on_event(LinkEvent::ControlStarted, 3_100);
    m.on_event(transient(13_100), 13_100);
    assert!(matches!(m.state(), LinkState::Backoff { attempt: 3, .. }));

    // The same history but a stable connection: back to 1.
    let mut m = LinkMachine::new(ReconnectPolicy::default());
    m.on_event(LinkEvent::Connect, 0);
    m.on_event(transient(10), 10);
    m.tick(1_010);
    m.on_event(LinkEvent::ControlStarted, 1_100);
    m.on_event(transient(31_100), 31_100);
    assert!(matches!(m.state(), LinkState::Backoff { attempt: 1, .. }));
    assert_eq!(m.attempt(), 1);
}

#[test]
fn fatal_ended_user_never_retry() {
    let cases = [
        (
            ExitClass::Fatal,
            LinkCmd::MarkFailed,
            LinkState::Failed { attempts: 0 },
        ),
        (ExitClass::Ended, LinkCmd::MarkEnded, LinkState::Ended),
        (ExitClass::User, LinkCmd::MarkDetached, LinkState::Detached),
    ];
    for (class, cmd, state) in cases {
        let mut m = connected_machine(ReconnectPolicy::default(), 10);
        let ev = LinkEvent::Down(down(class, DisconnectCause::ControlExit, 50));
        assert_eq!(m.on_event(ev, 50), vec![cmd], "{class:?}");
        assert_eq!(*m.state(), state, "{class:?}");
        assert!(m.tick(10_000_000).is_empty(), "{class:?}");
        assert_eq!(m.next_deadline_ms(), None);
    }
}

#[test]
fn user_disconnect_wins() {
    let mk: [fn() -> LinkMachine; 3] = [
        || connected_machine(ReconnectPolicy::default(), 5),
        || {
            let mut m = LinkMachine::new(ReconnectPolicy::default());
            m.on_event(LinkEvent::Connect, 0);
            m
        },
        || {
            let mut m = connected_machine(ReconnectPolicy::default(), 5);
            m.on_event(transient(10), 10);
            m
        },
    ];
    for make in mk {
        let mut m = make();
        assert_eq!(
            m.on_event(LinkEvent::UserDisconnect, 20),
            vec![LinkCmd::KillTransport, LinkCmd::MarkDetached]
        );
        assert_eq!(*m.state(), LinkState::Detached);
        assert!(m.on_event(transient(21), 21).is_empty());
        assert_eq!(*m.state(), LinkState::Detached);
        assert!(m.tick(1_000_000).is_empty());
    }
}

#[test]
fn user_reconnect_from_every_state() {
    let fatal = || LinkEvent::Down(down(ExitClass::Fatal, DisconnectCause::Eof, 5));
    let ended = || LinkEvent::Down(down(ExitClass::Ended, DisconnectCause::Eof, 5));
    let mut cases: Vec<(&str, LinkMachine, Vec<LinkCmd>)> = Vec::new();

    cases.push((
        "idle",
        LinkMachine::new(ReconnectPolicy::default()),
        vec![LinkCmd::Spawn],
    ));

    let m = connected_machine(ReconnectPolicy::default(), 1);
    cases.push(("connected", m, vec![LinkCmd::KillTransport, LinkCmd::Spawn]));

    let mut m = LinkMachine::new(ReconnectPolicy::default());
    m.on_event(LinkEvent::Connect, 0);
    cases.push((
        "connecting",
        m,
        vec![LinkCmd::KillTransport, LinkCmd::Spawn],
    ));

    let mut m = connected_machine(ReconnectPolicy::default(), 1);
    m.on_event(transient(5), 5);
    cases.push(("backoff", m, vec![LinkCmd::Spawn]));

    let mut m = connected_machine(ReconnectPolicy::default(), 1);
    m.on_event(LinkEvent::UserDisconnect, 5);
    cases.push(("detached", m, vec![LinkCmd::Spawn]));

    let mut m = connected_machine(ReconnectPolicy::default(), 1);
    m.on_event(ended(), 5);
    cases.push(("ended", m, vec![LinkCmd::Spawn]));

    let mut m = connected_machine(ReconnectPolicy::default(), 1);
    m.on_event(fatal(), 5);
    cases.push(("failed", m, vec![LinkCmd::Spawn]));

    for (name, mut m, want) in cases {
        assert_eq!(m.on_event(LinkEvent::UserReconnect, 100), want, "{name}");
        assert_eq!(m.attempt(), 0, "{name}");
        assert!(
            matches!(
                m.state(),
                LinkState::Connecting {
                    since_ms: 100,
                    awaiting_control: true
                }
            ),
            "{name}: {:?}",
            m.state()
        );
    }
}

#[test]
fn user_reconnect_resets_attempt_counter() {
    let mut m = connected_machine(ReconnectPolicy::default(), 0);
    m.on_event(transient(10), 10);
    m.tick(1_010);
    m.on_event(transient(1_020), 1_020);
    assert_eq!(m.attempt(), 2);
    m.on_event(LinkEvent::UserReconnect, 1_030);
    assert_eq!(m.attempt(), 0);
    m.on_event(transient(1_040), 1_040);
    assert!(matches!(m.state(), LinkState::Backoff { attempt: 1, .. }));
}

#[test]
fn stale_down_is_ignored() {
    let policy = ReconnectPolicy::default();
    let mut idle = LinkMachine::new(policy.clone());
    assert!(idle.on_event(transient(1), 1).is_empty());
    assert_eq!(*idle.state(), LinkState::Idle);

    let mut backoff = connected_machine(policy.clone(), 0);
    backoff.on_event(transient(10), 10);
    let before = backoff.clone();
    assert!(backoff.on_event(transient(11), 11).is_empty());
    assert_eq!(backoff, before);

    for class in [ExitClass::Fatal, ExitClass::Ended, ExitClass::User] {
        let mut m = connected_machine(policy.clone(), 0);
        m.on_event(LinkEvent::Down(down(class, DisconnectCause::Eof, 5)), 5);
        let before = m.clone();
        assert!(m.on_event(transient(6), 6).is_empty(), "{class:?}");
        assert_eq!(m, before, "{class:?}");
    }
}

#[test]
fn connect_is_ignored_while_active() {
    let mut connecting = LinkMachine::new(ReconnectPolicy::default());
    connecting.on_event(LinkEvent::Connect, 0);
    let before = connecting.clone();
    assert!(connecting.on_event(LinkEvent::Connect, 5).is_empty());
    assert_eq!(connecting, before);

    let mut connected = connected_machine(ReconnectPolicy::default(), 1);
    assert!(connected.on_event(LinkEvent::Connect, 9).is_empty());
    assert_eq!(*connected.state(), LinkState::Connected { since_ms: 1 });

    let mut backoff = connected_machine(ReconnectPolicy::default(), 1);
    backoff.on_event(transient(5), 5);
    let before = backoff.clone();
    assert!(backoff.on_event(LinkEvent::Connect, 6).is_empty());
    assert_eq!(backoff, before);
}

#[test]
fn connect_is_accepted_after_a_link_ended() {
    let ends = [
        LinkEvent::Down(down(ExitClass::Fatal, DisconnectCause::Eof, 5)),
        LinkEvent::Down(down(ExitClass::Ended, DisconnectCause::Eof, 5)),
        LinkEvent::UserDisconnect,
    ];
    for end in ends {
        let mut m = connected_machine(ReconnectPolicy::default(), 1);
        m.on_event(transient(2), 2);
        m.tick(1_002);
        m.on_event(end, 1_003);
        assert_eq!(m.on_event(LinkEvent::Connect, 2_000), vec![LinkCmd::Spawn]);
        assert_eq!(m.attempt(), 0);
        assert!(matches!(
            m.state(),
            LinkState::Connecting { since_ms: 2000, .. }
        ));
    }
}

#[test]
fn control_started_only_counts_while_connecting() {
    let mut m = LinkMachine::new(ReconnectPolicy::default());
    assert!(m.on_event(LinkEvent::ControlStarted, 1).is_empty());
    assert_eq!(*m.state(), LinkState::Idle);
    let mut m = connected_machine(ReconnectPolicy::default(), 1);
    assert!(m.on_event(LinkEvent::ControlStarted, 50).is_empty());
    assert_eq!(*m.state(), LinkState::Connected { since_ms: 1 });
}

#[test]
fn classify_table() {
    use DisconnectCause::{ControlExit, Eof, ProcessExit, SpawnFailed, UserDetach};
    use ExitClass::{Ended, Fatal, Transient, User};
    let none: &[String] = &[];
    let fatal_tails = [
        "me@box: Permission denied (publickey,password).",
        "Host key verification failed.",
        "No matching host key type found.",
        "zsh:1: command not found: tmux",
        "bash: tmux: No such file or directory",
    ];
    for t in fatal_tails {
        assert_eq!(
            classify_exit(&ProcessExit(Some(255)), None, Some(255), &s(&[t])),
            Fatal,
            "{t}"
        );
        assert_eq!(classify_exit(&Eof, None, None, &s(&[t])), Fatal, "{t}");
    }
    assert_eq!(
        classify_exit(&ProcessExit(Some(127)), None, Some(127), none),
        Fatal
    );
    let transient_tails = [
        "ssh: connect to host box port 22: Connection refused",
        "ssh: connect to host box port 22: Operation timed out",
        "client_loop: send disconnect: Broken pipe",
        "Connection reset by peer",
        "Connection to box closed by remote host.",
        "ssh: Could not resolve hostname box: Name or service not known",
        "connect: Network is unreachable",
    ];
    for t in transient_tails {
        assert_eq!(classify_exit(&Eof, None, None, &s(&[t])), Transient, "{t}");
        assert_eq!(
            classify_exit(&ProcessExit(Some(1)), None, Some(1), &s(&[t])),
            Transient,
            "{t}"
        );
    }
    assert_eq!(
        classify_exit(&ProcessExit(Some(255)), None, Some(255), none),
        Transient
    );
    assert_eq!(classify_exit(&Eof, None, None, none), Transient);
    assert_eq!(
        classify_exit(&ProcessExit(None), None, None, none),
        Transient
    );
    assert_eq!(
        classify_exit(&ProcessExit(Some(0)), None, Some(0), none),
        Ended
    );
    for r in [None, Some("detached"), Some("exited"), Some("")] {
        assert_eq!(classify_exit(&ControlExit, r, None, none), Ended, "{r:?}");
    }
    for r in [
        "server exited unexpectedly",
        "lost tty",
        "too far behind",
        "connection lost",
        "Connection closed",
    ] {
        assert_eq!(
            classify_exit(&ControlExit, Some(r), None, none),
            Transient,
            "{r}"
        );
    }
    assert_eq!(classify_exit(&UserDetach, None, None, none), User);
    assert_eq!(
        classify_exit(&SpawnFailed("boom".into()), None, None, none),
        Fatal
    );
    // Case-insensitive, and every tail line counts (not just the first or last).
    let tail = s(&[
        "welcome",
        "PERMISSION DENIED (publickey)",
        "Connection closed",
    ]);
    assert_eq!(classify_exit(&Eof, None, None, &tail), Fatal);
}

#[test]
fn disconnect_text_variants() {
    let mk = |cause: DisconnectCause, reason: Option<&str>, tail: &[&str]| TmuxDisconnect {
        cause,
        reason: reason.map(str::to_string),
        code: None,
        tail: s(tail),
        at_ms: 0,
        class: ExitClass::Transient,
    };
    assert_eq!(
        disconnect_text(&mk(DisconnectCause::ControlExit, Some("detached"), &[])),
        "tmux exited: detached"
    );
    assert_eq!(
        disconnect_text(&mk(DisconnectCause::ControlExit, None, &[])),
        "tmux exited"
    );
    assert_eq!(
        disconnect_text(&mk(DisconnectCause::Eof, None, &[])),
        "connection closed"
    );
    assert_eq!(
        disconnect_text(&mk(
            DisconnectCause::ProcessExit(Some(255)),
            None,
            &["client_loop: send disconnect: Broken pipe"]
        )),
        "ssh exited 255: client_loop: send disconnect: Broken pipe"
    );
    assert_eq!(
        disconnect_text(&mk(DisconnectCause::ProcessExit(None), None, &[])),
        "ssh exited"
    );
    assert_eq!(
        disconnect_text(&mk(
            DisconnectCause::SpawnFailed("No such file".into()),
            None,
            &[]
        )),
        "spawn failed: No such file"
    );
    assert_eq!(
        disconnect_text(&mk(DisconnectCause::UserDetach, None, &[])),
        "stopped by user"
    );
}

fn banner(state: LinkState, exit: Option<&TmuxDisconnect>, now: u64) -> Option<LinkBanner> {
    link_banner_text(&state, &target(), exit, &ReconnectPolicy::default(), now)
}

fn eof_exit() -> TmuxDisconnect {
    down(ExitClass::Transient, DisconnectCause::Eof, 0)
}

#[test]
fn banner_texts_match_contract() {
    let exit = eof_exit();
    let b = banner(
        LinkState::Backoff {
            attempt: 3,
            retry_at_ms: 18_000,
        },
        Some(&exit),
        10_000,
    )
    .unwrap();
    assert_eq!(
        b.text,
        "LINK DOWN (connection closed) | retry 3/20 in 8s | Enter/r: now | x: stop | d: detach"
    );
    assert!(b.interactive);

    let fatal = TmuxDisconnect {
        tail: s(&["Permission denied (publickey)."]),
        ..down(ExitClass::Fatal, DisconnectCause::ProcessExit(Some(255)), 0)
    };
    let b = banner(LinkState::Failed { attempts: 0 }, Some(&fatal), 0).unwrap();
    assert_eq!(
        b.text,
        "LINK FAILED (ssh exited 255: Permission denied (publickey).) | Enter/r: retry | d: detach"
    );
    assert!(b.interactive);

    let b = banner(LinkState::Detached, None, 0).unwrap();
    assert_eq!(b.text, "LINK STOPPED | Enter/r: reconnect | d: detach");
    assert!(b.interactive);

    let b = banner(LinkState::Ended, None, 0).unwrap();
    assert_eq!(
        b.text,
        "REMOTE SESSION ENDED | Enter/r: start a new one | d: detach"
    );
    assert!(b.interactive);

    let b = banner(
        LinkState::Connecting {
            since_ms: 0,
            awaiting_control: true,
        },
        None,
        5,
    )
    .unwrap();
    assert_eq!(
        b.text,
        "LINK CONNECTING me@box/main | type here for ssh prompts | Ctrl+Alt+R: restart"
    );
    assert!(!b.interactive);

    assert!(banner(LinkState::Idle, None, 0).is_none());
    assert!(banner(LinkState::Connected { since_ms: 0 }, None, 0).is_none());
}

#[test]
fn backoff_and_failed_banners_without_a_recorded_exit_still_render() {
    let b = banner(
        LinkState::Backoff {
            attempt: 1,
            retry_at_ms: 1000,
        },
        None,
        0,
    )
    .unwrap();
    assert!(b.text.starts_with("LINK DOWN ("), "{}", b.text);
    assert!(b.text.contains("retry 1/20 in 1s"), "{}", b.text);
    let b = banner(LinkState::Failed { attempts: 20 }, None, 0).unwrap();
    assert!(b.text.starts_with("LINK FAILED ("), "{}", b.text);
}

#[test]
fn retry_seconds_round_up() {
    let exit = eof_exit();
    let at = |retry_at, now| {
        banner(
            LinkState::Backoff {
                attempt: 1,
                retry_at_ms: retry_at,
            },
            Some(&exit),
            now,
        )
        .unwrap()
        .text
    };
    assert!(at(1001, 1000).contains("in 1s"), "{}", at(1001, 1000));
    assert!(at(1000, 1000).contains("in 0s"), "{}", at(1000, 1000));
    assert!(at(500, 1000).contains("in 0s"), "past deadline");
    assert!(at(3001, 1000).contains("in 3s"));
    assert!(at(3000, 1000).contains("in 2s"));
}

#[test]
fn banner_text_is_sanitised_and_capped() {
    let evil = format!(
        "x\x1b[31m\x1b]0;pwn\x07line1\nline2\u{202e}rtl\u{2066}{}",
        "A".repeat(400)
    );
    let exit = TmuxDisconnect {
        reason: Some(evil.clone()),
        tail: vec![evil.clone()],
        ..down(ExitClass::Transient, DisconnectCause::ControlExit, 0)
    };
    let eof_tail = TmuxDisconnect {
        tail: vec![evil.clone()],
        ..down(
            ExitClass::Transient,
            DisconnectCause::ProcessExit(Some(255)),
            0,
        )
    };
    let states = [
        LinkState::Backoff {
            attempt: 20,
            retry_at_ms: 30_000,
        },
        LinkState::Failed { attempts: 20 },
    ];
    for e in [&exit, &eof_tail] {
        for st in states {
            let b = banner(st, Some(e), 0).unwrap();
            assert!(
                b.text.chars().count() <= 160,
                "{} chars",
                b.text.chars().count()
            );
            assert!(!b.text.contains('\n'));
            assert!(
                !b.text.chars().any(|c| c.is_control()
                    || ('\u{202a}'..='\u{202e}').contains(&c)
                    || ('\u{2066}'..='\u{2069}').contains(&c)),
                "{:?}",
                b.text
            );
            // The fixed hints survive a hostile cause.
            assert!(b.text.contains("| Enter/r:"), "{}", b.text);
        }
        let d = disconnect_text(e);
        assert!(
            d.chars().count() <= 160 && !d.chars().any(char::is_control),
            "{d:?}"
        );
    }
    let long_target = LinkTarget {
        profile: RemoteHostProfile::new("box", format!("user@{}", "h".repeat(300))),
        session: "s".repeat(300),
        tags: Vec::new(),
    };
    let b = link_banner_text(
        &LinkState::Connecting {
            since_ms: 0,
            awaiting_control: true,
        },
        &long_target,
        None,
        &ReconnectPolicy::default(),
        0,
    )
    .unwrap();
    assert!(b.text.chars().count() <= 160);
    assert!(b.text.ends_with("Ctrl+Alt+R: restart"), "{}", b.text);
}

#[test]
fn status_never_says_attached_or_connected_early() {
    let exit = eof_exit();
    let fatal = down(ExitClass::Fatal, DisconnectCause::Eof, 0);
    let all = [
        banner(
            LinkState::Connecting {
                since_ms: 0,
                awaiting_control: true,
            },
            None,
            0,
        ),
        banner(
            LinkState::Backoff {
                attempt: 1,
                retry_at_ms: 10,
            },
            Some(&exit),
            0,
        ),
        banner(LinkState::Failed { attempts: 1 }, Some(&fatal), 0),
        banner(LinkState::Detached, None, 0),
        banner(LinkState::Ended, None, 0),
    ];
    for b in all.into_iter().flatten() {
        let low = b.text.to_lowercase();
        assert!(!low.contains("attached"), "{}", b.text);
        assert!(!low.contains("connected"), "{}", b.text);
    }
}
