//! Phase 51.1-05: the tmux -CC bridge hardened for a remote session (PST-02). Headless: the
//! transport is `TmuxControlSubprocess::from_channels`, no ssh or tmux is ever started.

use bilterm::tmux::hydrate::ReplyKind;
use bilterm::tmux::link::{LinkState, MAPPED_PANES_CAP};
use bilterm::tmux::{TmuxControlBridge, TmuxControlEvent, TmuxControlSubprocess};
use bilterm::wm::WindowManager;

type LineTx = tokio::sync::mpsc::UnboundedSender<String>;

fn wm() -> WindowManager {
    WindowManager::new_headless(24, 80)
}

fn screen(wm: &WindowManager, id: usize) -> String {
    wm.pane(id).unwrap().vt().contents()
}

fn attach(wm: &mut WindowManager) -> LineTx {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::mem::forget(cmd_rx); // keep the command channel open for the whole test
    wm.attach_tmux_control(TmuxControlSubprocess::from_channels("main", cmd_tx, line_rx));
    line_tx
}

/// `(tmux pane, bilterm pane)` pairs the bridge holds, read through the public link summary.
/// The summary needs a link record to list panes, so these tests connect one first.
fn mapped(wm: &WindowManager) -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = wm.tmux_link_summary().panes.iter().map(|&(p, t)| (t, p)).collect();
    v.sort();
    v
}

fn send(tx: &LineTx, lines: &[&str]) {
    for l in lines {
        tx.send((*l).to_string()).unwrap();
    }
}

#[test]
fn fresh_bridge_maps_nothing() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    assert!(b.mapped_panes().is_empty());
    b.handle_control_line(&mut wm, "%output %0 hi");
    assert_eq!(b.mapped_panes(), vec![(0, 2)]);
    assert!(screen(&wm, 2).contains("hi"));
    assert!(!screen(&wm, 1).contains("hi"), "local pane 1 stays untouched");
}

#[test]
fn primary_is_adopted_by_first_tmux_pane() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    b.set_primary(1);
    b.set_badge(Some("me@box/main".into()));
    wm.pane_mut(1).unwrap().process_pty_output(b"ssh banner text");
    b.handle_control_line(&mut wm, "%layout-change @1 b25d,160x48,0,0,3");
    assert_eq!(b.mapped_panes(), vec![(3, 1)]);
    assert_eq!(wm.pane_count(), 1, "adopted, no new pane");
    assert!(!screen(&wm, 1).contains("ssh banner"), "adopted pane is reset");
    assert!(wm.pane(1).unwrap().title().contains("%3"));
    assert_eq!(b.primary(), None);
    b.handle_control_line(&mut wm, "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}");
    let other = b.tmux_to_bilterm[&4];
    assert_ne!(other, 1);
    // New panes keep the plain `TMUX %n` title and no `remote_target` (registry label-only rule, plan 08).
    assert_eq!(wm.pane(other).unwrap().title(), "TMUX %4");
}

fn attach_3_and_4(wm: &mut WindowManager, b: &mut TmuxControlBridge) -> (usize, usize) {
    b.set_primary(1);
    b.handle_control_line(wm, "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,4}");
    (b.tmux_to_bilterm[&3], b.tmux_to_bilterm[&4])
}

#[test]
fn reattach_reuses_stale_panes_by_tmux_id() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    let (p3, p4) = attach_3_and_4(&mut wm, &mut b);
    let count = wm.pane_count();
    b.reset_for_attach();
    assert_eq!(b.stale_panes(), vec![p3, p4]);
    b.handle_control_line(&mut wm, "%layout-change @1 b25d,160x48,0,0{80x48,0,0,3,79x48,81,0,5}");
    assert_eq!(b.tmux_to_bilterm[&3], p3, "same tmux id keeps its pane");
    assert_eq!(b.tmux_to_bilterm[&5], p4, "new id adopts the stale pane");
    assert_eq!(wm.pane_count(), count);
    assert!(b.finish_reconcile().is_empty());
}

#[test]
fn reattach_with_fewer_panes_reports_orphans() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    let (p3, p4) = attach_3_and_4(&mut wm, &mut b);
    b.reset_for_attach();
    b.handle_control_line(&mut wm, "%layout-change @1 b25d,160x48,0,0,3");
    assert_eq!(b.tmux_to_bilterm[&3], p3);
    assert_eq!(b.finish_reconcile(), vec![p4]);
    assert!(wm.pane(p4).is_some(), "orphans are reported, never removed");
    assert!(b.stale_panes().is_empty());
}

#[test]
fn window_close_queues_removal() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    let (p3, p4) = attach_3_and_4(&mut wm, &mut b);
    let count = wm.pane_count();
    b.handle_control_line(&mut wm, "%window-close @1");
    assert_eq!(b.take_removed_panes(), vec![p3, p4]);
    assert!(b.take_removed_panes().is_empty());
    assert!(b.mapped_panes().is_empty());
    assert_eq!(wm.pane_count(), count, "the bridge removes nothing itself");

    let mut b2 = TmuxControlBridge::new();
    b2.handle_control_line(&mut wm, "%layout-change @2 b25d,160x48,0,0,8");
    let p8 = b2.tmux_to_bilterm[&8];
    b2.handle_control_line(&mut wm, "%unlinked-window-close @2");
    assert_eq!(b2.take_removed_panes(), vec![p8]);
}

#[test]
fn layout_diff_queues_closed_pane() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    let (_p3, p4) = attach_3_and_4(&mut wm, &mut b);
    b.handle_control_line(&mut wm, "%layout-change @1 b25d,160x48,0,0,3");
    assert_eq!(b.take_removed_panes(), vec![p4]);
    assert!(!b.mapped_panes().iter().any(|(t, _)| *t == 4));
    // An unparsable layout never mass-removes.
    b.handle_control_line(&mut wm, "%layout-change @1 garbage");
    assert!(b.take_removed_panes().is_empty());
    assert_eq!(b.mapped_panes().len(), 1);
}

#[test]
fn exit_reason_is_stored() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    let ev = b.handle_control_line(&mut wm, "%exit server exited unexpectedly");
    assert!(matches!(ev, TmuxControlEvent::Exit { .. }));
    assert!(b.exited);
    assert_eq!(b.exit_reason(), Some("server exited unexpectedly"));

    let mut b = TmuxControlBridge::new();
    let long = "x".repeat(300);
    b.handle_control_line(&mut wm, &format!("%exit \x1b[31mred {long}"));
    let r = b.exit_reason().unwrap();
    assert!(!r.contains('\x1b'));
    assert!(r.chars().count() <= 160, "{}", r.chars().count());

    let mut b = TmuxControlBridge::new();
    b.handle_control_line(&mut wm, "%exit");
    assert!(b.exited && b.exit_reason().is_none());
}

#[test]
fn block_body_lines_are_not_events() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    b.handle_control_line(&mut wm, "%begin 10 5 1");
    for body in ["%exit", "%output %9 evil", "%window-close @1", "%end 10 99 1"] {
        assert!(matches!(
            b.handle_control_line(&mut wm, body),
            TmuxControlEvent::Other(_)
        ));
    }
    assert!(!b.exited);
    assert!(b.mapped_panes().is_empty());
    assert!(b.take_removed_panes().is_empty());
    assert_eq!(b.blocks_closed(), 0);
    assert_eq!(b.handle_control_line(&mut wm, "%end 10 5 1"), TmuxControlEvent::EndBlock);
    assert_eq!(b.blocks_closed(), 1);
    b.handle_control_line(&mut wm, "%output %9 ok");
    assert_eq!(b.mapped_panes().len(), 1);
    assert!(screen(&wm, b.tmux_to_bilterm[&9]).contains("ok"));

    // %error closes a block too.
    b.handle_control_line(&mut wm, "%begin 10 6 1");
    b.handle_control_line(&mut wm, "%exit");
    assert_eq!(b.handle_control_line(&mut wm, "%error 10 6 1"), TmuxControlEvent::ErrorBlock);
    assert!(!b.exited);
    assert_eq!(b.blocks_closed(), 2);

    // A second %begin abandons the unterminated block and opens the new one.
    b.handle_control_line(&mut wm, "%begin 10 7 1");
    b.handle_control_line(&mut wm, "%begin 10 8 1");
    assert_eq!(b.blocks_closed(), 3);
    b.handle_control_line(&mut wm, "%exit");
    assert!(!b.exited, "still inside the new block");
    b.handle_control_line(&mut wm, "%end 10 8 1");
    assert_eq!(b.blocks_closed(), 4);
    b.handle_control_line(&mut wm, "%exit");
    assert!(b.exited);
}

#[test]
fn mapped_panes_are_capped() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    for n in 0..200 {
        b.handle_control_line(&mut wm, &format!("%output %{n} x"));
    }
    assert_eq!(b.mapped_panes().len(), MAPPED_PANES_CAP);
    assert_eq!(wm.pane_count(), 1 + MAPPED_PANES_CAP);
    assert_eq!(b.take_diag().len(), 1, "one diag line, not one per dropped event");
    // Already mapped ids keep working.
    b.handle_control_line(&mut wm, "%output %0 STILL_WORKS");
    assert!(screen(&wm, b.tmux_to_bilterm[&0]).contains("STILL_WORKS"));
    assert_eq!(b.mapped_panes().len(), MAPPED_PANES_CAP);
}

#[test]
fn queued_commands_keep_order() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    b.handle_control_line(&mut wm, "%output %3 x");
    let p = b.tmux_to_bilterm[&3];
    b.route_prompt_submission(p, "ls").unwrap();
    b.queue_split(p, true).unwrap();
    b.queue_resize_client(100, 30);
    b.queue_command("capture-pane -p -t %3".into(), ReplyKind::CapturePane { tmux_pane: 3 });
    let cmds = b.take_outbound_commands();
    assert_eq!(cmds.len(), 4);
    assert!(cmds[0].starts_with("send-keys -t %3"));
    assert!(cmds[1].starts_with("split-window"));
    assert!(cmds[2].starts_with("refresh-client"));
    assert_eq!(cmds[3], "capture-pane -p -t %3\n");
    assert!(b.take_outbound_commands().is_empty());
}

#[test]
fn outbound_queue_is_bounded_and_keeps_captures() {
    let mut b = TmuxControlBridge::new();
    b.queue_command("capture-pane -p -t %1".into(), ReplyKind::CapturePane { tmux_pane: 1 });
    for i in 0..400 {
        b.queue_command(format!("cmd{i}"), ReplyKind::Ignore);
    }
    let cmds = b.take_outbound_commands();
    assert_eq!(cmds.len(), bilterm::tmux::link::OUTBOUND_CAP);
    assert_eq!(cmds[0], "capture-pane -p -t %1\n", "oldest plain commands are dropped first");
}

#[test]
fn hold_output_drops_until_release() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    b.handle_control_line(&mut wm, "%output %3 first");
    let p = b.tmux_to_bilterm[&3];
    b.hold_output(3);
    b.handle_control_line(&mut wm, "%output %3 lost");
    b.release_output(3);
    b.handle_control_line(&mut wm, "%output %3 kept");
    let s = screen(&wm, p);
    assert!(s.contains("kept") && !s.contains("lost"), "{s}");
}

#[test]
fn replies_are_collected_without_events() {
    let mut wm = wm();
    let mut b = TmuxControlBridge::new();
    b.queue_command("list-windows".into(), ReplyKind::Ignore);
    b.take_outbound_commands();
    b.handle_control_line(&mut wm, "%begin 1 1 0");
    b.handle_control_line(&mut wm, "0: bash*");
    b.handle_control_line(&mut wm, "%end 1 1 0");
    // Plan 06 fills the correlator; the bridge only forwards and never panics or mis-routes.
    let _ = b.take_replies();
    assert_eq!(b.blocks_closed(), 1);
}

// ---- Task 2: poll_tmux_control and attach ----

#[test]
fn percent_exit_drops_control_and_reports_nothing_more() {
    let mut wm = wm();
    wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let tx = attach(&mut wm);
    send(&tx, &["%output %3 hi", "%exit server exited unexpectedly"]);
    wm.poll_all_ptys();
    assert!(!wm.is_tmux_control_attached());
    // Second poll with nothing attached is a no-op.
    assert_eq!(wm.poll_tmux_control(), 0);
}

#[test]
fn exit_in_the_middle_of_a_batch_stops_processing() {
    let mut wm = wm();
    let tx = attach(&mut wm);
    send(&tx, &["%output %3 hi", "%exit bye", "%output %7 late"]);
    wm.poll_all_ptys();
    assert!(!wm.is_tmux_control_attached());
    let before = wm.pane_count();
    assert_eq!(before, 2, "pane for %3 only; %7 never applied");
}

#[test]
fn first_line_moves_connecting_to_connected_even_for_output() {
    let mut wm = wm();
    wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    assert!(matches!(wm.tmux_link_state(), LinkState::Connecting { .. }));
    let tx = attach(&mut wm);
    send(&tx, &["%output %3 hi"]);
    wm.poll_all_ptys();
    assert!(matches!(wm.tmux_link_state(), LinkState::Connected { .. }));
}

#[test]
fn legacy_exit_without_link_is_silent_drop() {
    let mut wm = wm();
    let tx = attach(&mut wm);
    send(&tx, &["%exit"]);
    wm.poll_all_ptys();
    assert!(!wm.is_tmux_control_attached());
    assert!(matches!(wm.tmux_link_state(), LinkState::Idle));
}

#[test]
fn attach_resets_maps_for_second_attach() {
    let mut wm = wm();
    wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let tx = attach(&mut wm);
    send(&tx, &["%output %3 one"]);
    wm.poll_all_ptys();
    let first = mapped(&wm);
    assert_eq!(first.len(), 1);
    send(&tx, &["%exit"]);
    wm.poll_all_ptys();
    let tx2 = attach(&mut wm);
    assert!(mapped(&wm).is_empty());
    send(&tx2, &["%output %3 two"]);
    wm.poll_all_ptys();
    assert_eq!(mapped(&wm), first, "same id reuses its pane, no duplicate");
    assert_eq!(wm.pane_count(), 2);
}

#[test]
fn attach_makes_old_panes_stale_and_adopts_by_lowest_id() {
    let mut wm = wm();
    wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let tx = attach(&mut wm);
    send(&tx, &["%output %3 a", "%output %4 b"]);
    wm.poll_all_ptys();
    let tx2 = attach(&mut wm);
    send(&tx2, &["%output %9 c"]);
    wm.poll_all_ptys();
    let m = mapped(&wm);
    assert_eq!(m, vec![(9, 2)], "lowest stale pane adopted");
    assert_eq!(wm.pane_count(), 3, "no extra pane created");
}
