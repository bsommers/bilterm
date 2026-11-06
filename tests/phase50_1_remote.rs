//! Phase 50.1-06: SSH, ssh-tmux and `tmux -CC` bridge panes classify into `TransportInfo`
//! (FND-R03). No live ssh or tmux is used: SSH panes are headless, tmux uses `from_channels`.

use bilterm::tmux::{TmuxControlSubprocess, TmuxControlEvent, parse_tmux_control_line};
use bilterm::wm::WindowManager;
use bilterm::wm::registry::remote::transport_of;
use bilterm::wm::registry::{Transport, TransportInfo};

fn tmux_wm(session: &str) -> (WindowManager, tokio::sync::mpsc::UnboundedSender<String>) {
    let mut wm = WindowManager::new_headless(20, 80);
    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    wm.attach_tmux_control(TmuxControlSubprocess::from_channels(session, cmd_tx, line_rx));
    (wm, line_tx)
}

/// Bilterm pane whose title marks it as the mirror of tmux pane `%n`.
fn bilterm_pane_for(wm: &WindowManager, tmux_pane: usize) -> usize {
    let want = format!("TMUX %{tmux_pane}");
    *wm.panes()
        .iter()
        .find(|(_, p)| p.title().starts_with(&want))
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no bilterm pane for %{tmux_pane}"))
}

fn send(tx: &tokio::sync::mpsc::UnboundedSender<String>, lines: &[&str]) {
    for l in lines {
        tx.send((*l).to_string()).unwrap();
    }
}

#[test]
fn local_pane_is_local_and_cwd_trusted() {
    let wm = WindowManager::new_headless(20, 80);
    let t = transport_of(&wm, wm.active_pane_id());
    assert_eq!(t, TransportInfo::local());
    assert_eq!(t.transport, Transport::Local);
    assert!(t.cwd_trusted);
    assert!(!t.metadata_only);
}

#[test]
fn ssh_pane_is_ssh_with_host_and_tags() {
    let mut wm = WindowManager::new_headless(24, 80);
    let id = wm.spawn_ssh_pane("me@box", None, vec!["prod".into()]).unwrap();
    let t = transport_of(&wm, id);
    assert_eq!(t.transport, Transport::Ssh);
    assert_eq!(t.host.as_deref(), Some("me@box"));
    assert!(t.tags.iter().any(|x| x == "prod"), "tags: {:?}", t.tags);
    assert!(!t.metadata_only);
    assert_eq!(t.tmux_session, None);
}

#[test]
fn ssh_tmux_pane_is_metadata_only() {
    let mut wm = WindowManager::new_headless(24, 80);
    let id = wm.connect_ssh_tmux("me@box", "main", vec!["db".into()]).unwrap();
    let t = transport_of(&wm, id);
    assert_eq!(t.transport, Transport::SshTmux);
    assert_eq!(t.host.as_deref(), Some("me@box"));
    assert_eq!(t.tmux_session.as_deref(), Some("main"));
    assert!(t.metadata_only, "connect_ssh_tmux starts no tmux (gap G2)");
    assert!(t.tags.iter().any(|x| x == "db"), "tags: {:?}", t.tags);
}

#[test]
fn ssh_cwd_untrusted_until_osc7() {
    let mut wm = WindowManager::new_headless(24, 80);
    let id = wm.spawn_ssh_pane("me@box", None, vec![]).unwrap();
    assert!(!transport_of(&wm, id).cwd_trusted);
    wm.pane_mut(id).unwrap().process_pty_output(b"\x1b]7;file://box/srv\x07");
    assert!(transport_of(&wm, id).cwd_trusted);
}

#[test]
fn fresh_bridge_does_not_mislabel_pane_one() {
    // `TmuxControlBridge::new()` maps %0 -> pane 1, but nothing is attached.
    let wm = WindowManager::new_headless(20, 80);
    let t = transport_of(&wm, 1);
    assert_eq!(t.transport, Transport::Local);
    assert_eq!(t.tmux_pane, None);
}

#[test]
fn parse_window_renamed() {
    assert_eq!(
        parse_tmux_control_line("%window-renamed @3 build logs"),
        TmuxControlEvent::WindowRenamed { window_id: 3, name: "build logs".into() }
    );
    assert!(matches!(
        parse_tmux_control_line("%window-renamed 3 oops"),
        TmuxControlEvent::Other(_)
    ));
    assert!(matches!(
        parse_tmux_control_line("%window-renamed @x oops"),
        TmuxControlEvent::Other(_)
    ));
}

#[test]
fn attached_tmux_pane_is_tmux_transport_with_window_and_session() {
    let (mut wm, tx) = tmux_wm("boot");
    send(
        &tx,
        &[
            "%session-changed $1 work",
            "%layout-change @2 b25d,160x48,0,0{80x48,0,0,4,79x48,81,0,5}",
            "%window-renamed @2 editor",
            "%output %4 hi",
        ],
    );
    wm.poll_all_ptys();
    let id = bilterm_pane_for(&wm, 4);
    let t = transport_of(&wm, id);
    assert_eq!(t.transport, Transport::Tmux);
    assert_eq!(t.tmux_session.as_deref(), Some("work"));
    assert_eq!(t.tmux_window.as_deref(), Some("@2:editor"));
    assert_eq!(t.tmux_pane, Some(4));
    assert!(!t.cwd_trusted);
    assert!(!t.metadata_only);
    // the sibling pane in the same window reports the same window
    let t5 = transport_of(&wm, bilterm_pane_for(&wm, 5));
    assert_eq!(t5.tmux_window.as_deref(), Some("@2:editor"));
    assert_eq!(t5.tmux_pane, Some(5));
}

#[test]
fn window_without_rename_renders_bare_id() {
    let (mut wm, tx) = tmux_wm("s");
    send(&tx, &["%layout-change @2 b25d,80x24,0,0,4", "%output %4 x"]);
    wm.poll_all_ptys();
    let t = transport_of(&wm, bilterm_pane_for(&wm, 4));
    assert_eq!(t.tmux_window.as_deref(), Some("@2"));
}

#[test]
fn window_close_drops_window_data() {
    let (mut wm, tx) = tmux_wm("s");
    send(
        &tx,
        &["%layout-change @2 b25d,80x24,0,0,4", "%window-renamed @2 gone", "%output %4 x"],
    );
    wm.poll_all_ptys();
    let id = bilterm_pane_for(&wm, 4);
    assert_eq!(transport_of(&wm, id).tmux_window.as_deref(), Some("@2:gone"));
    send(&tx, &["%window-close @2"]);
    wm.poll_all_ptys();
    assert_eq!(transport_of(&wm, id).tmux_window, None);
}

#[test]
fn window_pane_changed_updates_the_map() {
    let (mut wm, tx) = tmux_wm("s");
    send(
        &tx,
        &["%layout-change @2 b25d,80x24,0,0,4", "%window-pane-changed @3 %4", "%window-renamed @3 moved"],
    );
    wm.poll_all_ptys();
    let t = transport_of(&wm, bilterm_pane_for(&wm, 4));
    assert_eq!(t.tmux_window.as_deref(), Some("@3:moved"));
}

#[test]
fn tmux_names_are_sanitised() {
    let (mut wm, tx) = tmux_wm("sess\u{1b}[31m\u{202e}x");
    send(
        &tx,
        &[
            "%layout-change @2 b25d,80x24,0,0,4",
            "%window-renamed @2 ed\u{1b}[31mit\u{202e}or",
            "%output %4 x",
        ],
    );
    wm.poll_all_ptys();
    let t = transport_of(&wm, bilterm_pane_for(&wm, 4));
    for s in [t.tmux_window.as_deref().unwrap(), t.tmux_session.as_deref().unwrap()] {
        assert!(
            !s.chars().any(|c| c.is_control() || ('\u{202a}'..='\u{202e}').contains(&c)),
            "unsanitised: {s:?}"
        );
    }
    assert!(t.tmux_window.as_deref().unwrap().starts_with("@2:ed"));
}
