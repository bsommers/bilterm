//! Phase 51.1-06: reply correlation (FIFO), attach hydrate planning and capture rendering.
//! Pure logic only; no tmux, no WindowManager, no clock.

use bilterm::tmux::hydrate::{
    BlockHeader, BlockKind, HydratePhase, HydratePlanner, Reply, ReplyCorrelator, ReplyKind,
    capture_pane_command, render_capture,
};
use bilterm::tmux::link::{GREETING_GRACE_MS, HYDRATE_REPLY_LINES_CAP, PENDING_REPLIES_CAP};

fn hdr(kind: BlockKind, number: u64) -> BlockHeader {
    BlockHeader {
        kind,
        time: 1_700_000_000,
        number,
        flags: 1,
    }
}

fn block(c: &mut ReplyCorrelator, number: u64, body: &[&str], ok: bool) -> Option<Reply> {
    c.on_begin(&hdr(BlockKind::Begin, number));
    for l in body {
        c.on_line(l);
    }
    let kind = if ok { BlockKind::End } else { BlockKind::Error };
    c.on_end(&hdr(kind, number), ok)
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

// ---- Task 1: ReplyCorrelator ----

#[test]
fn correlator_greeting_block_is_discarded() {
    let mut c = ReplyCorrelator::new();
    assert!(block(&mut c, 1, &["hello", "world"], true).is_none());
    assert_eq!(c.pending(), 0);
}

#[test]
fn correlator_fifo_matches_in_write_order() {
    let mut c = ReplyCorrelator::new();
    c.enqueue(ReplyKind::Ignore);
    c.enqueue(ReplyKind::CapturePane { tmux_pane: 3 });
    c.enqueue(ReplyKind::CapturePane { tmux_pane: 4 });
    assert_eq!(c.pending(), 3);

    let r1 = block(&mut c, 10, &[], true).expect("reply 1");
    assert_eq!(r1.kind, ReplyKind::Ignore);
    assert_eq!(c.pending(), 2);

    let r2 = block(&mut c, 11, &["three-a", "three-b"], true).expect("reply 2");
    assert_eq!(r2.kind, ReplyKind::CapturePane { tmux_pane: 3 });
    assert_eq!(r2.lines, s(&["three-a", "three-b"]));
    assert!(r2.ok && !r2.truncated);
    assert_eq!(c.pending(), 1);

    let r3 = block(&mut c, 12, &["four"], true).expect("reply 3");
    assert_eq!(r3.kind, ReplyKind::CapturePane { tmux_pane: 4 });
    assert_eq!(r3.lines, s(&["four"]));
    assert_eq!(c.pending(), 0);
}

#[test]
fn correlator_error_block_is_not_ok() {
    let mut c = ReplyCorrelator::new();
    c.enqueue(ReplyKind::CapturePane { tmux_pane: 9 });
    let r = block(&mut c, 2, &["can't find pane: %9"], false).expect("reply");
    assert!(!r.ok);
    assert_eq!(r.kind, ReplyKind::CapturePane { tmux_pane: 9 });
    assert_eq!(r.lines, s(&["can't find pane: %9"]));
}

#[test]
fn correlator_mismatched_end_number_is_ignored() {
    let mut c = ReplyCorrelator::new();
    c.enqueue(ReplyKind::Ignore);
    c.on_begin(&hdr(BlockKind::Begin, 5));
    c.on_line("body");
    assert!(c.on_end(&hdr(BlockKind::End, 6), true).is_none());
    // still open: the right end closes it with the collected body
    let r = c.on_end(&hdr(BlockKind::End, 5), true).expect("reply");
    assert_eq!(r.kind, ReplyKind::Ignore);
    assert_eq!(r.lines, s(&["body"]));
}

#[test]
fn correlator_second_begin_fails_the_open_block() {
    let mut c = ReplyCorrelator::new();
    c.enqueue(ReplyKind::CapturePane { tmux_pane: 1 });
    c.enqueue(ReplyKind::CapturePane { tmux_pane: 2 });
    c.on_begin(&hdr(BlockKind::Begin, 5));
    c.on_line("partial");
    c.on_begin(&hdr(BlockKind::Begin, 6));
    c.on_line("second");

    let failed = c.take_failed();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].kind, ReplyKind::CapturePane { tmux_pane: 1 });
    assert!(!failed[0].ok);
    assert_eq!(failed[0].lines, s(&["partial"]));
    assert!(c.take_failed().is_empty(), "drained");

    let r = c
        .on_end(&hdr(BlockKind::End, 6), true)
        .expect("second survives");
    assert_eq!(r.kind, ReplyKind::CapturePane { tmux_pane: 2 });
    assert_eq!(r.lines, s(&["second"]));
    assert!(r.ok);
}

#[test]
fn correlator_body_is_capped() {
    let mut c = ReplyCorrelator::new();
    c.enqueue(ReplyKind::CapturePane { tmux_pane: 1 });
    c.on_begin(&hdr(BlockKind::Begin, 1));
    for i in 0..(HYDRATE_REPLY_LINES_CAP + 50) {
        c.on_line(&format!("line {i}"));
    }
    let r = c.on_end(&hdr(BlockKind::End, 1), true).expect("reply");
    assert_eq!(r.lines.len(), HYDRATE_REPLY_LINES_CAP);
    assert_eq!(r.lines[0], "line 0");
    assert!(r.truncated);
}

#[test]
fn correlator_fifo_overflow_clears_and_flags() {
    let mut c = ReplyCorrelator::new();
    for _ in 0..PENDING_REPLIES_CAP {
        c.enqueue(ReplyKind::Ignore);
    }
    assert_eq!(c.pending(), PENDING_REPLIES_CAP);
    assert!(!c.overflowed());
    c.enqueue(ReplyKind::Ignore);
    assert_eq!(c.pending(), 0);
    assert!(c.overflowed());

    c.enqueue(ReplyKind::Ignore);
    c.on_begin(&hdr(BlockKind::Begin, 1));
    c.clear();
    assert_eq!(c.pending(), 0);
    assert!(!c.overflowed());
    // the open block is gone: a late end matches nothing
    assert!(c.on_end(&hdr(BlockKind::End, 1), true).is_none());
}

#[test]
fn correlator_lines_outside_a_block_are_ignored() {
    let mut c = ReplyCorrelator::new();
    c.enqueue(ReplyKind::Ignore);
    c.on_line("stray");
    let r = block(&mut c, 1, &["real"], true).expect("reply");
    assert_eq!(r.lines, s(&["real"]));
}

// ---- Task 2: HydratePlanner, capture_pane_command, render_capture ----

#[test]
fn planner_waits_for_greeting() {
    let mut p = HydratePlanner::new();
    assert_eq!(p.phase(), HydratePhase::AwaitingGreeting);
    assert!(p.plan(&[1], 80, 24).is_empty());
    p.note_line(100);
    p.tick(100 + GREETING_GRACE_MS - 1);
    assert_eq!(p.phase(), HydratePhase::AwaitingGreeting);
    p.tick(100 + GREETING_GRACE_MS);
    assert_eq!(p.phase(), HydratePhase::Ready);
}

#[test]
fn planner_tick_without_any_line_stays_waiting() {
    let mut p = HydratePlanner::new();
    p.tick(1_000_000);
    assert_eq!(p.phase(), HydratePhase::AwaitingGreeting);
}

#[test]
fn planner_block_end_makes_ready_early() {
    let mut p = HydratePlanner::new();
    p.note_line(0);
    p.note_block_end();
    assert_eq!(p.phase(), HydratePhase::Ready);
}

#[test]
fn planner_plan_lists_refresh_then_captures_in_order() {
    let mut p = HydratePlanner::new();
    p.note_block_end();
    let cmds = p.plan(&[5, 3], 120, 40);
    assert_eq!(cmds.len(), 3);
    assert_eq!(cmds[0].0, "refresh-client -C 120x40\n");
    assert_eq!(cmds[0].1, ReplyKind::Ignore);
    assert_eq!(cmds[1].0, "capture-pane -p -e -J -S -2000 -t %3\n");
    assert_eq!(cmds[1].1, ReplyKind::CapturePane { tmux_pane: 3 });
    assert_eq!(cmds[2].0, "capture-pane -p -e -J -S -2000 -t %5\n");
    assert_eq!(cmds[2].1, ReplyKind::CapturePane { tmux_pane: 5 });
    assert_eq!(p.phase(), HydratePhase::Ready, "Ready until mark_sent");
}

#[test]
fn planner_capture_done_completes() {
    let mut p = HydratePlanner::new();
    p.note_block_end();
    let _ = p.plan(&[3, 5], 80, 24);
    p.mark_sent();
    assert_eq!(p.phase(), HydratePhase::Sent);
    assert_eq!(p.remaining(), 2);
    assert!(!p.note_capture_done(3));
    assert_eq!(p.remaining(), 1);
    assert!(!p.note_capture_done(99), "unknown pane is harmless");
    assert!(!p.note_capture_done(3), "repeat is harmless");
    assert!(p.note_capture_done(5));
    assert_eq!(p.phase(), HydratePhase::Done);
    assert_eq!(p.remaining(), 0);
    assert!(!p.note_capture_done(5), "nothing left after Done");
}

#[test]
fn planner_reset_starts_over() {
    let mut p = HydratePlanner::new();
    p.note_line(10);
    p.note_block_end();
    let _ = p.plan(&[1], 80, 24);
    p.mark_sent();
    p.reset();
    assert_eq!(p.phase(), HydratePhase::AwaitingGreeting);
    assert_eq!(p.remaining(), 0);
    // first-line time was forgotten: a tick far in the future alone does not make it Ready
    p.tick(10_000_000);
    assert_eq!(p.phase(), HydratePhase::AwaitingGreeting);
    assert!(p.plan(&[1], 80, 24).is_empty());
}

#[test]
fn planner_empty_pane_list_plans_only_refresh() {
    let mut p = HydratePlanner::new();
    p.note_block_end();
    let cmds = p.plan(&[], 80, 24);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].1, ReplyKind::Ignore);
    // Nothing to wait for: mark_sent goes straight to Done (no capture can ever complete).
    p.mark_sent();
    assert_eq!(p.phase(), HydratePhase::Done);
    assert_eq!(p.remaining(), 0);
}

#[test]
fn planner_duplicate_panes_are_deduped() {
    let mut p = HydratePlanner::new();
    p.note_block_end();
    let cmds = p.plan(&[4, 4, 2], 80, 24);
    assert_eq!(cmds.len(), 3);
    p.mark_sent();
    assert_eq!(p.remaining(), 2);
}

#[test]
fn capture_command_is_exact() {
    assert_eq!(
        capture_pane_command(7),
        "capture-pane -p -e -J -S -2000 -t %7\n"
    );
}

#[test]
fn render_capture_cases() {
    assert_eq!(render_capture(&s(&["a", "b", "", ""])), b"a\r\nb".to_vec());
    assert!(render_capture(&s(&["", "", ""])).is_empty());
    assert!(render_capture(&[]).is_empty());
    assert_eq!(render_capture(&s(&["only"])), b"only".to_vec());
    // leading and interior blanks are kept
    assert_eq!(
        render_capture(&s(&["", "x", "", "y"])),
        b"\r\nx\r\n\r\ny".to_vec()
    );
    // escape sequences pass through untouched
    let esc = "\u{1b}[31mred\u{1b}[0m";
    assert_eq!(
        render_capture(&s(&[esc, "ok"])),
        format!("{esc}\r\nok").into_bytes()
    );
    // no cursor addressing is added
    let out = String::from_utf8(render_capture(&s(&["a", "b"]))).unwrap();
    assert!(!out.contains("\u{1b}["));
}
