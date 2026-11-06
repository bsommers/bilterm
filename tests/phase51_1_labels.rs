//! Phase 51.1-08: Honest restore labels, honest drawer badges, and honest registry metadata
//! (PST-03, FND-R03).

use std::fs;

use bilterm::tmux::TmuxControlSubprocess;
use bilterm::wm::layout_profile::keep_remote_badge;
use bilterm::wm::registry::remote::transport_of;
use bilterm::wm::registry::Transport;
use bilterm::wm::{DrawerRowKind, SavedWorkspaceSession, SplitDirection, WindowManager};

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

fn pane_for_tmux(wm: &WindowManager, tmux_pane: usize) -> usize {
    let marker = format!("%{tmux_pane}");
    *wm.panes()
        .iter()
        .find(|(_, p)| p.title().contains(&marker))
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no bilterm pane for %{tmux_pane}"))
}

// ---------------------------------------------------------------------------------------------
// Task 1: Honest restore and honest drawer badges
// ---------------------------------------------------------------------------------------------

#[test]
fn restore_drops_remote_badge_on_local_shell() {
    let sessions = tempfile::tempdir().unwrap();
    let sess_dir = sessions.path().to_string_lossy().into_owned();

    let mut wm1 = WindowManager::new_headless(24, 80);
    wm1.split_focused(SplitDirection::Vertical).unwrap();
    wm1.config_mut().session_dir = Some(sess_dir.clone());
    let path = wm1.save_session("work").unwrap();

    let raw = fs::read_to_string(&path).unwrap();
    let mut saved: SavedWorkspaceSession = toml::from_str(&raw).unwrap();
    for p in &mut saved.panes {
        if p.id == 2 {
            p.shell = Some("/bin/bash".into());
            p.remote_target = Some("user@mac/bilterm".into());
            p.title = "bigmac.local: gitlab".into();
        }
    }
    fs::write(&path, toml::to_string(&saved).unwrap()).unwrap();

    let mut wm2 = WindowManager::new_headless(24, 80);
    wm2.config_mut().session_dir = Some(sess_dir);
    wm2.restore_session("work").unwrap();

    let p2 = wm2.pane(2).unwrap();
    assert_eq!(p2.remote_target(), None);
    assert!(
        p2.title().ends_with(" (not connected)"),
        "expected title to end with (not connected), got: {}",
        p2.title()
    );
    let status = p2.status_message().expect("status message set");
    assert!(status.contains("user@mac/bilterm"), "status was: {status}");
    assert!(status.contains(":ssh-tmux"), "status was: {status}");
}

#[test]
fn restore_keeps_badge_for_ssh_program() {
    let sessions = tempfile::tempdir().unwrap();
    let sess_dir = sessions.path().to_string_lossy().into_owned();

    let mut wm1 = WindowManager::new_headless(24, 80);
    wm1.split_focused(SplitDirection::Vertical).unwrap();
    wm1.config_mut().session_dir = Some(sess_dir.clone());
    let path = wm1.save_session("work_ssh").unwrap();

    let raw = fs::read_to_string(&path).unwrap();
    let mut saved: SavedWorkspaceSession = toml::from_str(&raw).unwrap();
    for p in &mut saved.panes {
        if p.id == 2 {
            p.shell = Some("ssh".into());
            p.shell_args = vec!["-tt".into(), "--".into(), "user@mac".into()];
            p.remote_target = Some("user@mac".into());
            p.title = "user@mac shell".into();
        }
    }
    fs::write(&path, toml::to_string(&saved).unwrap()).unwrap();

    let mut wm2 = WindowManager::new_headless(24, 80);
    wm2.config_mut().session_dir = Some(sess_dir);
    wm2.restore_session("work_ssh").unwrap();

    let p2 = wm2.pane(2).unwrap();
    assert_eq!(p2.remote_target(), Some("user@mac"));
    assert_eq!(p2.title(), "user@mac shell");
}

#[test]
fn restore_without_remote_target_is_unchanged() {
    let sessions = tempfile::tempdir().unwrap();
    let sess_dir = sessions.path().to_string_lossy().into_owned();

    let mut wm1 = WindowManager::new_headless(24, 80);
    wm1.split_focused(SplitDirection::Vertical).unwrap();
    wm1.config_mut().session_dir = Some(sess_dir.clone());
    let path = wm1.save_session("work_plain").unwrap();

    let raw = fs::read_to_string(&path).unwrap();
    let mut saved: SavedWorkspaceSession = toml::from_str(&raw).unwrap();
    for p in &mut saved.panes {
        if p.id == 2 {
            p.shell = Some("/bin/bash".into());
            p.remote_target = None;
            p.title = "my local bash".into();
        }
    }
    fs::write(&path, toml::to_string(&saved).unwrap()).unwrap();

    let mut wm2 = WindowManager::new_headless(24, 80);
    wm2.config_mut().session_dir = Some(sess_dir);
    wm2.restore_session("work_plain").unwrap();

    let p2 = wm2.pane(2).unwrap();
    assert_eq!(p2.remote_target(), None);
    assert_eq!(p2.title(), "my local bash");
    assert!(!p2.title().contains("(not connected)"));
    assert!(
        p2.status_message().is_none_or(|s| !s.contains("Restored without a live remote link") && !s.contains(":ssh-tmux")),
        "unexpected remote status: {:?}",
        p2.status_message()
    );
}

#[test]
fn restore_ssh_fallback_to_default_shell_drops_badge() {
    assert!(keep_remote_badge("ssh", true));
    assert!(!keep_remote_badge("ssh", false));
    assert!(keep_remote_badge("/usr/bin/ssh", true));
    assert!(!keep_remote_badge("bash", true));
    assert!(!keep_remote_badge("", true));
}

#[test]
fn drawer_marks_only_mapped_panes_tmux() {
    let mut wm = WindowManager::new_headless(24, 80);
    let (line_tx, _cmd_rx) = attach(&mut wm, "test");
    feed(&line_tx, &["%output %3 x\r\n"]);
    wm.poll_all_ptys();

    let rows = wm.drawer_rows();
    let header = rows
        .iter()
        .find(|r| matches!(r.kind, DrawerRowKind::SessionHeader(_)))
        .expect("session header row");
    assert!(
        header.transport_badges.contains(&"🔗 TMUX".to_string()),
        "header badges: {:?}",
        header.transport_badges
    );

    let pane1_row = rows
        .iter()
        .find(|r| r.kind == DrawerRowKind::Pane(1))
        .expect("pane 1 row");
    assert!(
        !pane1_row.transport_badges.contains(&"🔗 TMUX".to_string()),
        "local pane 1 should not have TMUX badge, got: {:?}",
        pane1_row.transport_badges
    );

    let pane2_row = rows
        .iter()
        .find(|r| r.kind == DrawerRowKind::Pane(2))
        .expect("pane 2 row");
    assert!(
        pane2_row.transport_badges.contains(&"🔗 TMUX".to_string()),
        "mapped pane 2 should have TMUX badge, got: {:?}",
        pane2_row.transport_badges
    );
}

// ---------------------------------------------------------------------------------------------
// Task 2: Registry rows reflect a live link
// ---------------------------------------------------------------------------------------------

#[test]
fn unattached_ssh_tmux_pane_stays_metadata_only() {
    let mut wm = WindowManager::new_headless(24, 80);
    let id = wm.connect_ssh_tmux("me@box", "main", vec!["tag1".into()]).unwrap();
    let t = transport_of(&wm, id);
    assert_eq!(t.transport, Transport::SshTmux);
    assert_eq!(t.host.as_deref(), Some("me@box"));
    assert_eq!(t.tmux_session.as_deref(), Some("main"));
    assert!(t.metadata_only);
    assert!(!t.cwd_trusted);
}

#[test]
fn linked_ssh_tmux_pane_is_live() {
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
            "%output %3 hi\r\n",
        ],
    );
    assert!(wm.tick_tmux_link(100));

    let t = transport_of(&wm, id);
    assert_eq!(t.transport, Transport::SshTmux);
    assert_eq!(t.host.as_deref(), Some("me@box"));
    assert_eq!(t.tmux_session.as_deref(), Some("main"));
    assert_eq!(t.tmux_pane, Some(3));
    assert!(!t.metadata_only);
    assert!(!t.cwd_trusted);
}

#[test]
fn second_mapped_pane_is_linked_too() {
    let mut wm = WindowManager::new_headless(24, 80);
    let _id1 = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm, "main");
    feed(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%session-changed $1 main",
            "%layout-change @1 b25d,160x48,0,0,3",
            "%output %3 hi\r\n",
            "%output %4 second\r\n",
        ],
    );
    assert!(wm.tick_tmux_link(100));

    let id2 = pane_for_tmux(&wm, 4);
    let t2 = transport_of(&wm, id2);
    assert_eq!(t2.transport, Transport::SshTmux);
    assert_eq!(t2.host.as_deref(), Some("me@box"));
    assert_eq!(t2.tmux_session.as_deref(), Some("main"));
    assert_eq!(t2.tmux_pane, Some(4));
    assert!(!t2.metadata_only);
}

#[test]
fn link_down_makes_rows_metadata_only_again() {
    let mut wm = WindowManager::new_headless(24, 80);
    let id1 = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (tx, _cmd_rx) = attach(&mut wm, "main");
    feed(
        &tx,
        &[
            "%begin 1 1 0",
            "%end 1 1 0",
            "%session-changed $1 main",
            "%layout-change @1 b25d,160x48,0,0,3",
            "%output %3 hi\r\n",
            "%output %4 second\r\n",
        ],
    );
    assert!(wm.tick_tmux_link(100));

    let id2 = pane_for_tmux(&wm, 4);
    assert!(!transport_of(&wm, id1).metadata_only);
    assert!(!transport_of(&wm, id2).metadata_only);

    feed(&tx, &["%exit server exited unexpectedly"]);
    wm.tick_tmux_link(200);

    assert!(transport_of(&wm, id1).metadata_only);
    assert!(transport_of(&wm, id2).metadata_only);
}
