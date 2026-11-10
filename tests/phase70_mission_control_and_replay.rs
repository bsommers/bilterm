//! Phase 70 integration tests: HUD-01 mission control, HUD-02 replay, HUD-03 watchdog.

use bilterm::wm::agent_state::AgentState;
use bilterm::wm::agent_watchdog::*;
use bilterm::wm::mission_control_hud::*;
use bilterm::wm::turn_replay::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn card(id: usize, st: AgentState) -> AgentCardTelemetry {
    AgentCardTelemetry::new(id, &format!("agent{id}"), st)
}

fn hud_with(n: usize) -> MissionControlHud {
    let mut h = MissionControlHud::new();
    for i in 0..n {
        assert!(h.upsert_card(card(i, AgentState::Idle)));
    }
    h
}

fn row(buf: &Buffer, y: u16, w: u16) -> String {
    (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

// ---------- HUD-01 ----------

#[test]
fn grid_layout_columns_rows() {
    let h = hud_with(4);
    let area = Rect::new(0, 0, 68, 14);
    let l = h.layout(area);
    assert_eq!(l.len(), 4);
    assert_eq!(l[0].1, Rect::new(0, 0, 34, CARD_H));
    assert_eq!(l[1].1, Rect::new(34, 0, 34, CARD_H));
    assert_eq!(l[2].1, Rect::new(0, CARD_H, 34, CARD_H));
}

#[test]
fn layout_empty_when_area_too_small() {
    let h = hud_with(3);
    assert!(h.layout(Rect::new(0, 0, 5, 20)).is_empty());
    assert!(h.layout(Rect::new(0, 0, 80, 3)).is_empty());
    assert_eq!(MissionControlHud::page_size_for(Rect::new(0, 0, 5, 3)), 0);
}

#[test]
fn paging_with_tile_cap() {
    let mut h = hud_with(30);
    let area = Rect::new(0, 0, 200, 70);
    let size = MissionControlHud::page_size_for(area);
    assert_eq!(size, MAX_TILES_PER_PAGE);
    assert_eq!(h.layout(area).len(), MAX_TILES_PER_PAGE);
    assert_eq!(h.page_count(size), 3);
    h.next_page(size);
    assert_eq!(h.layout(area)[0].0, MAX_TILES_PER_PAGE);
    h.next_page(size);
    h.next_page(size);
    assert_eq!(h.page(), 2);
    assert_eq!(h.layout(area).len(), 6);
    h.prev_page();
    h.prev_page();
    h.prev_page();
    assert_eq!(h.page(), 0);
}

#[test]
fn card_cap_enforced() {
    let mut h = MissionControlHud::new();
    for i in 0..MAX_CARDS {
        assert!(h.upsert_card(card(i, AgentState::Idle)));
    }
    assert!(!h.upsert_card(card(MAX_CARDS + 1, AgentState::Idle)));
    assert!(h.upsert_card(card(0, AgentState::Working)));
    assert_eq!(h.len(), MAX_CARDS);
}

#[test]
fn selection_wraps_and_follows_page() {
    let mut h = hud_with(15);
    assert_eq!(h.selected(), Some(0));
    h.select_prev(12);
    assert_eq!(h.selected(), Some(14));
    assert_eq!(h.page(), 1);
    h.select_next(12);
    assert_eq!(h.selected(), Some(0));
    assert_eq!(h.page(), 0);
    assert!(h.select(5));
    assert!(!h.select(99));
    h.remove_card(5);
    assert_eq!(h.selected(), Some(0));
}

#[test]
fn sparkline_ring_wraparound() {
    let mut r = SparkRing::default();
    for i in 0..(SPARK_CAP as u32 + 10) {
        r.push(i);
    }
    assert_eq!(r.len(), SPARK_CAP);
    assert_eq!(r.samples().next(), Some(10));
    assert_eq!(r.samples().last(), Some(SPARK_CAP as u32 + 9));
}

#[test]
fn sparkline_glyph_scaling() {
    let mut r = SparkRing::default();
    assert_eq!(r.glyphs(8), "");
    for v in [0, 50, 100] {
        r.push(v);
    }
    let g: Vec<char> = r.glyphs(8).chars().collect();
    assert_eq!(g, vec!['▁', '▄', '█']);
    assert_eq!(r.glyphs(2).chars().count(), 2);
    let mut z = SparkRing::default();
    z.push(0);
    z.push(0);
    assert_eq!(z.glyphs(4), "▁▁");
}

#[test]
fn render_card_contents_into_buffer() {
    let mut h = MissionControlHud::new();
    let mut c = card(1, AgentState::Blocked);
    c.queue_depth = 3;
    c.set_badge(Some("wt-a"));
    c.set_tool(Some("edit"), Some("src/x.rs"));
    c.preview = vec!["hello world".into()];
    for v in [1, 5, 9, 2] {
        c.push_rate(v);
    }
    h.upsert_card(c);
    let area = Rect::new(0, 0, 34, 7);
    let mut buf = Buffer::empty(area);
    h.render(area, &mut buf);
    assert_eq!(buf[(0, 0)].symbol(), "┌");
    assert_eq!(buf[(33, 6)].symbol(), "┘");
    let title = row(&buf, 1, 34);
    assert!(title.contains("■ agent1"), "{title}");
    assert!(title.contains("<wt-a>"));
    let spark = row(&buf, 2, 34);
    assert!(spark.contains('█') && spark.contains("q:3"), "{spark}");
    assert!(row(&buf, 3, 34).contains("edit src/x.rs"));
    assert!(row(&buf, 4, 34).contains("hello world"));
    let btns = row(&buf, 5, 34);
    assert!(
        btns.contains("[Approve]")
            && btns.contains("[Prompt]")
            && btns.contains("[Diff]")
            && btns.contains("[Zoom]")
    );
}

#[test]
fn render_is_clipped_to_smaller_buffer() {
    let h = hud_with(4);
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
    h.render(Rect::new(0, 0, 200, 100), &mut buf);
    assert_eq!(buf[(0, 0)].symbol(), "┌");
}

#[test]
fn masked_tile_hides_preview() {
    let mut h = MissionControlHud::new();
    let mut c = card(2, AgentState::Blocked);
    c.preview = vec!["hunter2-secret".into()];
    c.masked = true;
    h.upsert_card(c);
    let area = Rect::new(0, 0, 34, 7);
    let mut buf = Buffer::empty(area);
    h.render(area, &mut buf);
    let all: String = (0..7).map(|y| row(&buf, y, 34)).collect();
    assert!(!all.contains("hunter2"));
    assert!(row(&buf, 4, 34).contains("masked"));
}

#[test]
fn traffic_arc_cells() {
    let mut h = hud_with(4);
    let area = Rect::new(0, 0, 68, 14);
    h.set_arcs(&[(0, 1)]);
    let cells = h.arc_cells(area);
    assert!(!cells.is_empty());
    assert_eq!(cells.first().map(|c| c.ch), Some('»'));
    let end = cells.last().copied();
    assert_eq!(end.map(|c| c.ch), Some('◉'));
    assert_eq!(end.map(|c| (c.x, c.y)), Some((51, 0)));
    // vertical arc 0 -> 2 goes down and skips card interiors
    h.set_arcs(&[(0, 2)]);
    let v = h.arc_cells(area);
    assert!(v.iter().all(|c| !(c.x == 17 && c.y > 0 && c.y < 6)));
    assert_eq!(v.last().map(|c| (c.x, c.y)), Some((17, 7)));
    // reverse direction glyph
    h.set_arcs(&[(1, 0)]);
    assert_eq!(h.arc_cells(area).first().map(|c| c.ch), Some('«'));
    // arcs to off-page / self panes are ignored
    h.set_arcs(&[(0, 99), (3, 3)]);
    assert!(h.arc_cells(area).is_empty());
}

#[test]
fn arcs_capped_and_drawn() {
    let mut h = hud_with(2);
    let many: Vec<(usize, usize)> = (0..100).map(|_| (0, 1)).collect();
    h.set_arcs(&many);
    let area = Rect::new(0, 0, 68, 7);
    assert!(h.arc_cells(area).len() <= MAX_ARC_CELLS);
    h.set_arcs(&[(0, 1)]);
    let mut buf = Buffer::empty(area);
    h.render(area, &mut buf);
    assert_eq!(buf[(51, 0)].symbol(), "◉");
}

#[test]
fn button_hit_testing_each_button() {
    let mut h = MissionControlHud::new();
    let mut c = card(7, AgentState::Blocked);
    c.generation = 42;
    h.upsert_card(c);
    let area = Rect::new(0, 0, 34, 7);
    let rects = button_rects(Rect::new(0, 0, 34, 7));
    assert_eq!(rects.len(), 4);
    for (kind, r) in rects {
        let hit = h.hit_test(area, r.x, r.y);
        let token = (kind == HudButtonKind::Approve).then_some(42);
        assert_eq!(
            hit,
            Some(HudHit::Button(HudButton {
                pane: 7,
                kind,
                generation: token
            }))
        );
        // last cell of the button too
        assert!(matches!(
            h.hit_test(area, r.x + r.width - 1, r.y),
            Some(HudHit::Button(_))
        ));
        // gap/after is not this button
        assert_ne!(
            h.hit_test(area, r.x + r.width, r.y),
            Some(HudHit::Button(HudButton {
                pane: 7,
                kind,
                generation: token
            }))
        );
    }
}

#[test]
fn hit_card_and_outside() {
    let h = hud_with(1);
    let area = Rect::new(0, 0, 34, 7);
    assert_eq!(h.hit_test(area, 5, 2), Some(HudHit::Card(0)));
    assert_eq!(h.hit_test(area, 0, 0), Some(HudHit::Card(0)));
    assert_eq!(h.hit_test(area, 50, 2), None);
    assert_eq!(h.hit_test(area, 5, 30), None);
    assert_eq!(MissionControlHud::new().hit_test(area, 1, 1), None);
}

#[test]
fn approve_disabled_unless_blocked() {
    for st in [
        AgentState::Working,
        AgentState::Idle,
        AgentState::Done,
        AgentState::Errored,
    ] {
        let mut h = MissionControlHud::new();
        h.upsert_card(card(1, st));
        let r = button_rects(Rect::new(0, 0, 34, 7))[0].1;
        assert_eq!(
            h.hit_test(Rect::new(0, 0, 34, 7), r.x, r.y),
            Some(HudHit::Disabled {
                pane: 1,
                kind: HudButtonKind::Approve
            })
        );
    }
}

#[test]
fn approve_generation_token_goes_stale() {
    let mut h = MissionControlHud::new();
    let mut c = card(1, AgentState::Blocked);
    c.generation = 5;
    h.upsert_card(c);
    let area = Rect::new(0, 0, 34, 7);
    let r = button_rects(area)[0].1;
    let Some(HudHit::Button(b)) = h.hit_test(area, r.x, r.y) else {
        panic!("no button")
    };
    assert!(h.approve_token_valid(&b));
    h.card_mut(1).unwrap().generation = 6;
    assert!(!h.approve_token_valid(&b));
    h.card_mut(1).unwrap().generation = 5;
    h.set_state(1, AgentState::Working);
    assert!(!h.approve_token_valid(&b));
    let other = HudButton {
        pane: 1,
        kind: HudButtonKind::Zoom,
        generation: None,
    };
    assert!(!h.approve_token_valid(&other));
}

#[test]
fn narrow_card_omits_buttons_that_do_not_fit() {
    assert!(button_rects(Rect::new(0, 0, 10, 7)).is_empty());
    assert_eq!(button_rects(Rect::new(0, 0, 22, 7)).len(), 2);
    assert!(button_rects(Rect::new(0, 0, 34, 3)).is_empty());
}

#[test]
fn refresh_cap_and_dirty() {
    let mut h = MissionControlHud::new();
    assert!(h.is_dirty());
    assert!(h.take_refresh(1000));
    assert!(!h.is_dirty());
    assert!(!h.take_refresh(1001));
    h.mark_dirty();
    assert!(!h.should_refresh(1000 + MIN_REFRESH_MS - 1));
    assert!(h.is_dirty());
    assert!(h.take_refresh(1000 + MIN_REFRESH_MS));
    assert!(!h.take_refresh(5000));
    // clock going backwards does not underflow
    h.mark_dirty();
    assert!(!h.should_refresh(10));
}

#[test]
fn updates_set_dirty() {
    let mut h = hud_with(1);
    h.take_refresh(0);
    h.record_rate(0, 10);
    assert!(h.is_dirty());
    h.take_refresh(1000);
    h.set_state(0, AgentState::Idle);
    assert!(!h.is_dirty());
    h.set_state(0, AgentState::Done);
    assert!(h.is_dirty());
    h.take_refresh(2000);
    h.record_rate(77, 1);
    assert!(!h.is_dirty());
}

#[test]
fn state_glyphs_distinct() {
    let g: Vec<char> = [
        AgentState::Working,
        AgentState::Blocked,
        AgentState::Idle,
        AgentState::Done,
        AgentState::Errored,
    ]
    .iter()
    .map(|s| card(0, *s).glyph())
    .collect();
    let mut d = g.clone();
    d.sort();
    d.dedup();
    assert_eq!(d.len(), 5);
}

// ---------- HUD-02 ----------

fn log_with(n: usize) -> TurnReplayLog {
    let mut l = TurnReplayLog::new();
    for i in 0..n {
        l.begin_turn(&format!("p{i}"), i as u64 * 10);
        l.end_turn(i as u64 * 10 + 5, vec![format!("out{i}")]);
    }
    l
}

#[test]
fn replay_records_a_turn() {
    let mut l = TurnReplayLog::new();
    assert!(l.current().is_none());
    let id = l.begin_turn("fix bug", 100);
    assert!(l.in_turn());
    l.mark_working(110);
    l.mark_working(120);
    l.record_output("line a");
    l.record_output("line b");
    assert_eq!(l.end_turn(150, vec![]), Some(id));
    let t = l.current().unwrap();
    assert_eq!(
        (
            t.turn_id,
            t.prompt.as_str(),
            t.start_ts,
            t.end_ts,
            t.working_ts
        ),
        (id, "fix bug", 100, Some(150), Some(110))
    );
    assert_eq!(t.snapshot, vec!["line a", "line b"]);
    assert_eq!(l.end_turn(1, vec![]), None);
}

#[test]
fn replay_snapshot_overrides_running_output() {
    let mut l = TurnReplayLog::new();
    l.begin_turn("p", 0);
    l.record_output("x");
    l.end_turn(1, vec!["screen1".into(), "screen2".into()]);
    assert_eq!(l.current().unwrap().snapshot, vec!["screen1", "screen2"]);
}

#[test]
fn replay_files_dedup_and_cap() {
    let mut l = TurnReplayLog::new();
    l.begin_turn("p", 0);
    l.touch_file("a.rs");
    l.touch_file("a.rs");
    l.touch_file("b.rs");
    for i in 0..(MAX_FILES_PER_TURN + 10) {
        l.touch_file(&format!("f{i}"));
    }
    l.end_turn(1, vec![]);
    let t = l.current().unwrap();
    assert_eq!(t.files.iter().filter(|f| *f == "a.rs").count(), 1);
    assert_eq!(t.files.len(), MAX_FILES_PER_TURN);
}

#[test]
fn replay_events_outside_turn_are_ignored() {
    let mut l = TurnReplayLog::new();
    l.record_output("x");
    l.touch_file("f");
    l.mark_working(1);
    assert!(l.is_empty() && !l.in_turn());
}

#[test]
fn replay_cursor_navigation_and_bounds() {
    let mut l = log_with(3);
    assert_eq!(l.current().unwrap().prompt, "p2");
    assert!(!l.next());
    assert!(l.prev());
    assert_eq!(l.current().unwrap().prompt, "p1");
    assert!(l.prev());
    assert!(!l.prev());
    assert_eq!(l.current().unwrap().prompt, "p0");
    assert!(l.last());
    assert_eq!(l.current().unwrap().prompt, "p2");
    assert!(l.first());
    assert!(l.goto(1));
    assert_eq!(l.current().unwrap().prompt, "p1");
    assert!(!l.goto(999));
    assert_eq!(l.current().unwrap().prompt, "p1");
    let mut e = TurnReplayLog::new();
    assert!(!e.prev() && !e.next() && !e.first() && !e.last() && !e.goto(0));
}

#[test]
fn replay_new_turn_keeps_cursor_when_scrubbing() {
    let mut l = log_with(3);
    l.first();
    l.begin_turn("later", 500);
    l.end_turn(510, vec![]);
    assert_eq!(l.current().unwrap().prompt, "p0");
    l.last();
    l.begin_turn("newest", 600);
    l.end_turn(610, vec![]);
    assert_eq!(l.current().unwrap().prompt, "newest");
}

#[test]
fn replay_begin_closes_open_turn() {
    let mut l = TurnReplayLog::new();
    l.begin_turn("a", 0);
    l.begin_turn("b", 5);
    assert_eq!(l.len(), 1);
    assert_eq!(l.turns().next().unwrap().end_ts, Some(5));
}

#[test]
fn replay_scroll_clamps() {
    let mut l = TurnReplayLog::new();
    l.begin_turn("p", 0);
    l.end_turn(1, (0..10).map(|i| format!("l{i}")).collect());
    l.scroll_up(5);
    assert_eq!(l.scroll(), 0);
    l.scroll_down(3, 4);
    assert_eq!(l.scroll(), 3);
    l.scroll_down(100, 4);
    assert_eq!(l.scroll(), 6);
    assert_eq!(l.visible_lines(4), &["l6", "l7", "l8", "l9"]);
    l.scroll_up(2);
    assert_eq!(l.visible_lines(4)[0], "l4");
    l.scroll_up(100);
    assert_eq!(l.scroll(), 0);
    assert_eq!(l.visible_lines(100).len(), 10);
    // short snapshot never scrolls
    let mut s = log_with(1);
    s.scroll_down(5, 10);
    assert_eq!(s.scroll(), 0);
}

#[test]
fn replay_navigation_resets_scroll() {
    let mut l = TurnReplayLog::new();
    for i in 0..2 {
        l.begin_turn("p", i);
        l.end_turn(i, (0..20).map(|n| n.to_string()).collect());
    }
    l.scroll_down(5, 4);
    assert_eq!(l.scroll(), 5);
    l.prev();
    assert_eq!(l.scroll(), 0);
}

#[test]
fn replay_eviction_cap() {
    let l = log_with(MAX_TURNS + 10);
    assert_eq!(l.len(), MAX_TURNS);
    assert_eq!(l.turns().next().unwrap().prompt, "p10");
    let mut l = log_with(MAX_TURNS);
    l.first();
    l.begin_turn("new", 9999);
    l.end_turn(10000, vec![]);
    assert_eq!(l.len(), MAX_TURNS);
    assert!(
        l.current().is_some(),
        "cursor must stay valid after eviction"
    );
    assert_eq!(l.current().unwrap().prompt, "p1");
}

#[test]
fn replay_snapshot_and_text_caps() {
    let mut l = TurnReplayLog::new();
    l.begin_turn(&"x".repeat(MAX_PROMPT_CHARS + 50), 0);
    for i in 0..(MAX_SNAPSHOT_LINES + 20) {
        l.record_output(&format!("{i}{}", "y".repeat(MAX_LINE_CHARS * 2)));
    }
    l.end_turn(1, vec![]);
    let t = l.current().unwrap();
    assert_eq!(t.prompt.chars().count(), MAX_PROMPT_CHARS);
    assert_eq!(t.snapshot.len(), MAX_SNAPSHOT_LINES);
    assert!(t.snapshot[0].starts_with("20"));
    assert!(
        t.snapshot
            .iter()
            .all(|s| s.chars().count() <= MAX_LINE_CHARS)
    );
    let mut l2 = TurnReplayLog::new();
    l2.begin_turn("p", 0);
    l2.end_turn(
        1,
        (0..MAX_SNAPSHOT_LINES + 5).map(|i| i.to_string()).collect(),
    );
    assert_eq!(l2.current().unwrap().snapshot.len(), MAX_SNAPSHOT_LINES);
}

// ---------- HUD-03 ----------

fn wd(f: impl FnOnce(&mut WatchdogConfig)) -> AgentWatchdog {
    let mut c = WatchdogConfig::default();
    f(&mut c);
    AgentWatchdog::new(c)
}

#[test]
fn config_defaults_and_validation() {
    let d = WatchdogConfig::default();
    assert_eq!((d.stall_secs, d.loop_window, d.loop_repeat), (120, 20, 4));
    assert!(d.validate().is_ok());
    let bad: [fn(&mut WatchdogConfig); 8] = [
        |c| c.stall_secs = 0,
        |c| c.stall_secs = MAX_SECS + 1,
        |c| c.loop_window = 1,
        |c| c.loop_window = MAX_LOOP_WINDOW + 1,
        |c| c.loop_repeat = 1,
        |c| c.loop_repeat = 99,
        |c| c.cooldown_secs = MAX_SECS + 1,
        |c| {
            c.loop_window = 3;
            c.loop_repeat = 4;
        },
    ];
    for f in bad {
        let mut c = WatchdogConfig::default();
        f(&mut c);
        assert!(c.validate().is_err(), "{c:?}");
    }
}

#[test]
fn stall_triggers_after_threshold_only() {
    let mut w = wd(|_| {});
    w.set_working(1, true, 1000);
    w.observe_line(1, "thinking", 1010);
    assert!(
        w.tick(1010 + 120).is_empty(),
        "exactly stall_secs is not a stall"
    );
    let a = w.tick(1010 + 121);
    assert_eq!(
        a,
        vec![WatchdogAlert {
            pane: 1,
            kind: AlertKind::Stall,
            target: None
        }]
    );
}

#[test]
fn stall_not_when_not_working_or_output_resets() {
    let mut w = wd(|_| {});
    w.observe_line(1, "x", 0);
    assert!(w.tick(10_000).is_empty());
    w.set_working(1, true, 10_000);
    assert!(w.tick(10_100).is_empty());
    w.observe_line(1, "alive", 10_100);
    assert!(w.tick(10_200).is_empty());
    w.set_working(1, false, 10_201);
    assert!(w.tick(20_000).is_empty());
}

#[test]
fn working_resume_resets_stall_clock() {
    let mut w = wd(|_| {});
    w.observe_line(1, "old", 0);
    w.set_working(1, true, 5000);
    assert!(w.tick(5100).is_empty());
    assert_eq!(w.tick(5121).len(), 1);
}

#[test]
fn loop_detected_with_digit_normalized_lines() {
    let mut w = wd(|_| {});
    for i in 0..4 {
        w.observe_line(
            2,
            &format!("retry attempt {i} failed at 0x{:08x}", 0xdead0000u32 + i),
            100 + i as u64,
        );
    }
    let a = w.tick(110);
    assert_eq!(a.len(), 1);
    match &a[0].kind {
        AlertKind::Loop { signature, count } => {
            assert_eq!(*count, 4);
            assert_eq!(signature, "retry attempt # failed at <hex>");
        }
        k => panic!("{k:?}"),
    }
}

#[test]
fn loop_below_repeat_or_outside_window_not_triggered() {
    let mut w = wd(|_| {});
    for _ in 0..3 {
        w.observe_line(2, "same", 1);
    }
    assert!(w.tick(2).is_empty());
    let mut w = wd(|c| c.loop_window = 5);
    for _ in 0..3 {
        w.observe_line(2, "same", 1);
    }
    for i in 0..5 {
        w.observe_line(
            2,
            &format!("different line {}", ["a", "b", "c", "d", "e"][i]),
            1,
        );
    }
    w.observe_line(2, "same", 1);
    assert!(w.tick(2).is_empty());
}

#[test]
fn blank_lines_ignored_for_loops() {
    let mut w = wd(|_| {});
    for _ in 0..10 {
        w.observe_line(1, "   ", 1);
    }
    assert!(w.tick(2).is_empty());
}

#[test]
fn repeated_command_loop() {
    let mut w = wd(|_| {});
    for i in 0..4 {
        w.observe_command(3, &format!("cargo test  --run {i}"), i);
    }
    let a = w.tick(10);
    assert_eq!(a.len(), 1);
    match &a[0].kind {
        AlertKind::Loop { signature, count } => {
            assert!(
                signature.starts_with("cmd: cargo test --run #"),
                "{signature}"
            );
            assert_eq!(*count, 4);
        }
        k => panic!("{k:?}"),
    }
}

#[test]
fn cooldown_dedupes_per_pane_and_kind() {
    let mut w = wd(|c| c.cooldown_secs = 100);
    w.set_working(1, true, 0);
    assert_eq!(w.tick(200).len(), 1);
    assert!(w.tick(250).is_empty(), "within cooldown");
    assert!(w.tick(299).is_empty());
    assert_eq!(w.tick(300).len(), 1, "cooldown elapsed");
    // different pane unaffected
    w.set_working(2, true, 300);
    assert_eq!(w.tick(430).iter().filter(|a| a.pane == 2).count(), 1);
    // different kind on the same pane is not suppressed by the stall cooldown
    for _ in 0..4 {
        w.observe_line(1, "spin", 431);
    }
    let a = w.tick(431);
    assert!(
        a.iter()
            .any(|x| x.pane == 1 && matches!(x.kind, AlertKind::Loop { .. }))
    );
}

#[test]
fn loop_window_cleared_after_alert() {
    let mut w = wd(|c| c.cooldown_secs = 0);
    for _ in 0..4 {
        w.observe_line(1, "spin", 1);
    }
    assert_eq!(w.tick(2).len(), 1);
    assert!(w.tick(3).is_empty(), "stale window must not re-alert");
}

#[test]
fn disabled_config_emits_nothing() {
    let mut w = wd(|c| c.enabled = false);
    w.set_working(1, true, 0);
    for _ in 0..10 {
        w.observe_line(1, "spin", 0);
        w.observe_command(1, "spin", 0);
    }
    assert!(w.tick(100_000).is_empty());
}

#[test]
fn alert_carries_orchestrator_target() {
    let mut w = wd(|_| {});
    w.set_orchestrator(Some(9));
    w.set_working(1, true, 0);
    let a = w.tick(500);
    assert_eq!(a[0].target, Some(9));
}

#[test]
fn orchestrator_parsing() {
    assert_eq!(resolve_orchestrator(Some("7")), Some(7));
    assert_eq!(resolve_orchestrator(Some(" 12\n")), Some(12));
    assert_eq!(resolve_orchestrator(None), None);
    assert_eq!(resolve_orchestrator(Some("")), None);
    assert_eq!(resolve_orchestrator(Some("abc")), None);
    assert_eq!(resolve_orchestrator(Some("-1")), None);
    assert_eq!(resolve_orchestrator(Some("99999999999999999999999")), None);
    let _ = orchestrator_from_env();
}

#[test]
fn normalization_rules() {
    assert_eq!(normalize_signature("  a   b\tc "), "a b c");
    assert_eq!(
        normalize_signature("took 123ms, 45 items"),
        "took #ms, # items"
    );
    assert_eq!(normalize_signature("id deadbeef01 ok"), "id <hex> ok");
    assert_eq!(normalize_signature("addr 0x7ffd1234"), "addr <hex>");
    assert_eq!(normalize_signature("face"), "face");
    assert!(normalize_signature(&"z".repeat(1000)).chars().count() <= MAX_SIGNATURE);
}

#[test]
fn pane_cap_and_forget() {
    let mut w = wd(|_| {});
    for p in 0..(MAX_PANES + 20) {
        w.observe_line(p, "x", 0);
    }
    assert_eq!(w.watched(), MAX_PANES);
    w.forget(0);
    assert_eq!(w.watched(), MAX_PANES - 1);
}
