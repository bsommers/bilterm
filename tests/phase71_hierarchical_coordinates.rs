//! Phase 71 COORD: hierarchical 5-tuple coordinates `D:(W,S,Y,P)`.
//!
//! Covers parsing, formatting, live `coord_of` / `resolve_coord`, `:coords` dispatch
//! (colon -> `Action::Coords` -> `WindowManager::apply`), and title-bar badge rendering.

use bilterm::action::Action;
use bilterm::colon_command::parse::parse_colon_command;
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::buffer_to_snapshot_string;
use bilterm::wm::WindowManager;
use bilterm::wm::coordinates::{CoordDisplayStyle, CoordsOp, TargetCoord};
use bilterm::wm::split_tree::SplitDirection;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

const ROWS: u16 = 30;
const COLS: u16 = 120;
const AREA: Rect = Rect { x: 0, y: 0, width: COLS, height: ROWS };

fn full(pane: Option<&str>) -> TargetCoord {
    TargetCoord {
        daemon: Some("d0".into()),
        workspace: Some("w1".into()),
        session: Some("main".into()),
        window: Some("y2".into()),
        pane: pane.map(str::to_string),
    }
}

fn parse_ok(s: &str) -> TargetCoord {
    TargetCoord::parse(s).unwrap_or_else(|e| panic!("parse({s:?}) failed: {e}"))
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_daemon_tuple_with_pane() {
    let c = parse_ok("(d0:(w1,s1,y2,p1))");
    assert_eq!(c.daemon.as_deref(), Some("d0"));
    assert_eq!(c.workspace.as_deref(), Some("w1"));
    assert_eq!(c.session.as_deref(), Some("s1"));
    assert_eq!(c.window.as_deref(), Some("y2"));
    assert_eq!(c.pane.as_deref(), Some("p1"));
}

#[test]
fn parse_daemon_tuple_without_pane() {
    let c = parse_ok("(d0:(w1,s1,y2))");
    assert_eq!(c.daemon.as_deref(), Some("d0"));
    assert_eq!(c.window.as_deref(), Some("y2"));
    assert_eq!(c.pane, None);
}

#[test]
fn parse_compact_tuple_three_and_four_components() {
    let c3 = parse_ok("(w1,s1,y2)");
    assert_eq!(c3.daemon, None);
    assert_eq!(c3.workspace.as_deref(), Some("w1"));
    assert_eq!(c3.session.as_deref(), Some("s1"));
    assert_eq!(c3.window.as_deref(), Some("y2"));
    assert_eq!(c3.pane, None);

    let c4 = parse_ok("(w1,s1,y2,%3)");
    assert_eq!(c4.window.as_deref(), Some("y2"));
    assert_eq!(c4.pane.as_deref(), Some("%3"));
}

#[test]
fn parse_verbose_labels_any_order_and_case() {
    let c = parse_ok("(Wksp: w1, Sess: main, Win: y2, Pane: %3)");
    assert_eq!(c.workspace.as_deref(), Some("w1"));
    assert_eq!(c.session.as_deref(), Some("main"));
    assert_eq!(c.window.as_deref(), Some("y2"));
    assert_eq!(c.pane.as_deref(), Some("%3"));

    let r = parse_ok("(Pane: p6, Win: y2, Sess: main, Wksp: w1)");
    assert_eq!(r.workspace.as_deref(), Some("w1"));
    assert_eq!(r.session.as_deref(), Some("main"));
    assert_eq!(r.window.as_deref(), Some("y2"));
    assert_eq!(r.pane.as_deref(), Some("p6"));

    let lower = parse_ok("(wksp: w1, sess: main, win: y2)");
    assert_eq!(lower.workspace.as_deref(), Some("w1"));
    assert_eq!(lower.window.as_deref(), Some("y2"));
}

#[test]
fn parse_verbose_accepts_daemon_label() {
    let c = parse_ok("(Daemon: d0, Wksp: w1, Sess: main, Win: y2)");
    assert_eq!(c.daemon.as_deref(), Some("d0"));
    assert_eq!(c.workspace.as_deref(), Some("w1"));
}

#[test]
fn parse_colon_path_five_segments() {
    let c = parse_ok("d0:w1:s1:y2:p1");
    assert_eq!(c.daemon.as_deref(), Some("d0"));
    assert_eq!(c.workspace.as_deref(), Some("w1"));
    assert_eq!(c.session.as_deref(), Some("s1"));
    assert_eq!(c.window.as_deref(), Some("y2"));
    assert_eq!(c.pane.as_deref(), Some("p1"));
}

#[test]
fn parse_colon_path_four_segments() {
    let c = parse_ok("w1:s1:y2:p1");
    assert_eq!(c.daemon, None);
    assert_eq!(c.workspace.as_deref(), Some("w1"));
    assert_eq!(c.session.as_deref(), Some("s1"));
    assert_eq!(c.window.as_deref(), Some("y2"));
    assert_eq!(c.pane.as_deref(), Some("p1"));
}

#[test]
fn parse_colon_path_three_segments() {
    let c = parse_ok("w1:s1:y2");
    assert_eq!(c.workspace.as_deref(), Some("w1"));
    assert_eq!(c.session.as_deref(), Some("s1"));
    assert_eq!(c.window.as_deref(), Some("y2"));
    assert_eq!(c.pane, None);
}

#[test]
fn parse_colon_path_two_segments_session_window() {
    let c = parse_ok("s1:y2");
    assert_eq!(c.workspace, None);
    assert_eq!(c.session.as_deref(), Some("s1"));
    assert_eq!(c.window.as_deref(), Some("y2"));
}

#[test]
fn parse_colon_path_one_segment_is_window() {
    let c = parse_ok("y2");
    assert_eq!(c.window.as_deref(), Some("y2"));
    assert_eq!(c.workspace, None);
    assert_eq!(c.pane, None);
}

#[test]
fn parse_slash_separator_is_equivalent_to_colon() {
    assert_eq!(parse_ok("d0/w1/s1/y2/p1"), parse_ok("d0:w1:s1:y2:p1"));
    assert_eq!(parse_ok("w1/s1/y2"), parse_ok("w1:s1:y2"));
}

#[test]
fn parse_herdr_workspace_pane_shorthand() {
    let c = parse_ok("w12:p6");
    assert_eq!(c.workspace.as_deref(), Some("w12"));
    assert_eq!(c.pane.as_deref(), Some("p6"));
    assert_eq!(c.window, None);
}

#[test]
fn parse_window_aliases_normalize_to_y_form() {
    assert_eq!(parse_ok("#2").window.as_deref(), Some("y2"));
    assert_eq!(parse_ok("y2").window.as_deref(), Some("y2"));
    assert_eq!(parse_ok("Y2").window.as_deref(), Some("y2"));
    assert_eq!(parse_ok("2").window.as_deref(), Some("y2"));
}

#[test]
fn parse_percent_pane_id() {
    let c = parse_ok("%3");
    assert_eq!(c.pane.as_deref(), Some("%3"));
    assert_eq!(c.window, None);
}

#[test]
fn parse_is_whitespace_tolerant() {
    assert_eq!(parse_ok("  d0:w1:s1:y2  "), parse_ok("d0:w1:s1:y2"));
    assert_eq!(parse_ok(" ( w1 , s1 , y2 ) "), parse_ok("(w1,s1,y2)"));
}

#[test]
fn parse_rejects_malformed_input() {
    for bad in [
        "",
        "   ",
        "(w1,s1",            // unbalanced parens
        "w1,s1,y2)",         // unbalanced parens
        "d0:w1:s1:y2:p1:p2", // more than five components
        "w1:w2:y2",          // duplicate level
        "y2:s1",             // out of hierarchy order
        "(w1,s1,y2,p1,p2,p3)",
    ] {
        assert!(TargetCoord::parse(bad).is_err(), "expected parse error for {bad:?}");
    }
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

#[test]
fn format_verbose_with_and_without_pane() {
    assert_eq!(
        full(None).format(CoordDisplayStyle::Verbose).as_deref(),
        Some("(Wksp: w1, Sess: main, Win: y2)")
    );
    assert_eq!(
        full(Some("%3")).format(CoordDisplayStyle::Verbose).as_deref(),
        Some("(Wksp: w1, Sess: main, Win: y2, Pane: %3)")
    );
}

#[test]
fn format_compact_with_and_without_pane() {
    assert_eq!(full(None).format(CoordDisplayStyle::Compact).as_deref(), Some("(w1,main,y2)"));
    assert_eq!(
        full(Some("%3")).format(CoordDisplayStyle::Compact).as_deref(),
        Some("(w1,main,y2,%3)")
    );
}

#[test]
fn format_daemon_tuple_with_and_without_pane() {
    assert_eq!(
        full(None).format(CoordDisplayStyle::DaemonTuple).as_deref(),
        Some("(d0:(w1,main,y2))")
    );
    assert_eq!(
        full(Some("%3")).format(CoordDisplayStyle::DaemonTuple).as_deref(),
        Some("(d0:(w1,main,y2,%3))")
    );
}

#[test]
fn format_off_is_none() {
    assert_eq!(full(None).format(CoordDisplayStyle::Off), None);
    assert_eq!(full(Some("%3")).format(CoordDisplayStyle::Off), None);
}

#[test]
fn format_unset_level_renders_as_star_in_tuple_styles() {
    let c = TargetCoord { workspace: Some("w1".into()), window: Some("y2".into()), ..Default::default() };
    assert_eq!(c.format(CoordDisplayStyle::Compact).as_deref(), Some("(w1,*,y2)"));
}

#[test]
fn to_path_is_canonical_colon_form() {
    assert_eq!(full(None).to_path(), "d0:w1:main:y2");
    assert_eq!(full(Some("p1")).to_path(), "d0:w1:main:y2:p1");
    let partial = TargetCoord { window: Some("y2".into()), ..Default::default() };
    assert_eq!(partial.to_path(), "y2");
}

/// Only the DaemonTuple form carries the daemon level; Verbose and Compact drop it, so for those
/// styles only workspace/session/window/pane must survive the trip.
fn assert_round_trip(c: &TargetCoord, style: CoordDisplayStyle) {
    let text = c.format(style).expect("non-Off style formats");
    let back = parse_ok(&text);
    if style == CoordDisplayStyle::DaemonTuple {
        assert_eq!(&back, c, "round trip through {style:?} ({text})");
    } else {
        assert_eq!(
            (&back.workspace, &back.session, &back.window, &back.pane),
            (&c.workspace, &c.session, &c.window, &c.pane),
            "round trip through {style:?} ({text})"
        );
    }
}

#[test]
fn round_trip_all_styles_with_pane() {
    let c = full(Some("%3"));
    for style in [CoordDisplayStyle::Verbose, CoordDisplayStyle::Compact, CoordDisplayStyle::DaemonTuple] {
        assert_round_trip(&c, style);
    }
}

#[test]
fn round_trip_all_styles_without_pane() {
    let c = full(None);
    for style in [CoordDisplayStyle::Verbose, CoordDisplayStyle::Compact, CoordDisplayStyle::DaemonTuple] {
        assert_round_trip(&c, style);
    }
}

#[test]
fn round_trip_colon_path() {
    let c = full(Some("p1"));
    assert_eq!(parse_ok(&c.to_path()), c);
}

#[test]
fn style_names_round_trip() {
    assert_eq!(CoordDisplayStyle::from_name("verbose"), Some(CoordDisplayStyle::Verbose));
    assert_eq!(CoordDisplayStyle::from_name("COMPACT"), Some(CoordDisplayStyle::Compact));
    assert_eq!(CoordDisplayStyle::from_name("daemon"), Some(CoordDisplayStyle::DaemonTuple));
    assert_eq!(CoordDisplayStyle::from_name("daemon-tuple"), Some(CoordDisplayStyle::DaemonTuple));
    assert_eq!(CoordDisplayStyle::from_name("off"), Some(CoordDisplayStyle::Off));
    assert_eq!(CoordDisplayStyle::from_name("bogus"), None);
    assert_eq!(CoordDisplayStyle::DaemonTuple.name(), "daemon");
    assert_eq!(CoordDisplayStyle::Off.name(), "off");
}

// ---------------------------------------------------------------------------
// Live coordinates on a headless WindowManager
// ---------------------------------------------------------------------------

#[test]
fn coord_of_headless_window_is_fully_populated() {
    let wm = WindowManager::new_headless(ROWS, COLS);
    let c = wm.coord_of(1).expect("pane 1 exists");
    assert_eq!(c.daemon.as_deref(), Some("d0"));
    assert_eq!(c.workspace.as_deref(), Some("w1"));
    assert_eq!(c.window.as_deref(), Some("y1"));
    assert_eq!(c.pane, None, "normal window has no remote tmux pane");
    let session = c.session.clone().expect("session is always populated");
    assert!(!session.is_empty());
    assert_eq!(session, wm.config().history_session);
    assert_eq!(wm.coord_of(999), None);
}

#[test]
fn coord_of_tracks_split_panes() {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.resize_all(AREA).unwrap();
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();
    assert_eq!(wm.coord_of(p2).unwrap().window, Some(format!("y{p2}")));
    assert_eq!(wm.coord_of(1).unwrap().window.as_deref(), Some("y1"));
}

#[test]
fn resolve_wildcard_matches_all_windows_ascending() {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.resize_all(AREA).unwrap();
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();
    let res = wm.resolve_coord(&TargetCoord::default());
    let mut expected = vec![1, p2];
    expected.sort_unstable();
    assert_eq!(res.panes, expected);
    assert_eq!(res.coords.len(), res.panes.len());
}

#[test]
fn resolve_by_window_matches_only_that_window() {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.resize_all(AREA).unwrap();
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();
    let target = TargetCoord { window: Some(format!("y{p2}")), ..Default::default() };
    let res = wm.resolve_coord(&target);
    assert_eq!(res.panes, vec![p2]);
    assert_eq!(res.coords.len(), 1);
    assert_eq!(res.coords[0].window, Some(format!("y{p2}")));
}

#[test]
fn resolve_full_live_coord_round_trips_to_its_window() {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.resize_all(AREA).unwrap();
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();
    let live = wm.coord_of(p2).unwrap();
    let res = wm.resolve_coord(&live);
    assert_eq!(res.panes, vec![p2]);
    let by_path = wm.resolve_coord(&parse_ok(&live.to_path()));
    assert_eq!(by_path.panes, vec![p2]);
}

#[test]
fn resolve_mismatched_daemon_is_empty() {
    let wm = WindowManager::new_headless(ROWS, COLS);
    let target = TargetCoord { daemon: Some("d9".into()), ..Default::default() };
    let res = wm.resolve_coord(&target);
    assert!(res.panes.is_empty());
    assert!(res.coords.is_empty());
}

#[test]
fn resolve_mismatched_workspace_is_empty() {
    let wm = WindowManager::new_headless(ROWS, COLS);
    let target = TargetCoord { workspace: Some("w9".into()), ..Default::default() };
    assert!(wm.resolve_coord(&target).panes.is_empty());
}

#[test]
fn resolve_matches_session_only_when_it_is_active_session() {
    let wm = WindowManager::new_headless(ROWS, COLS);
    let session = wm.coord_of(1).unwrap().session.unwrap();
    let hit = TargetCoord { session: Some(session), ..Default::default() };
    assert_eq!(wm.resolve_coord(&hit).panes, vec![1]);
    let miss = TargetCoord { session: Some("no-such-session-zz".into()), ..Default::default() };
    assert!(wm.resolve_coord(&miss).panes.is_empty());
}

#[test]
fn resolve_herdr_workspace_pane_shorthand_maps_to_window() {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.resize_all(AREA).unwrap();
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();
    let res = wm.resolve_coord(&parse_ok(&format!("w1:p{p2}")));
    assert_eq!(res.panes, vec![p2]);
}

// ---------------------------------------------------------------------------
// :coords colon parsing
// ---------------------------------------------------------------------------

fn colon(s: &str) -> Option<Result<Action, String>> {
    parse_colon_command(s)
}

#[test]
fn colon_coords_style_words_lower_to_style_action() {
    assert_eq!(
        colon(":coords compact"),
        Some(Ok(Action::Coords(CoordsOp::Style(CoordDisplayStyle::Compact))))
    );
    assert_eq!(
        colon(":coords verbose"),
        Some(Ok(Action::Coords(CoordsOp::Style(CoordDisplayStyle::Verbose))))
    );
    assert_eq!(
        colon(":coords daemon"),
        Some(Ok(Action::Coords(CoordsOp::Style(CoordDisplayStyle::DaemonTuple))))
    );
    assert_eq!(
        colon(":coords off"),
        Some(Ok(Action::Coords(CoordsOp::Style(CoordDisplayStyle::Off))))
    );
}

#[test]
fn colon_coords_show_copy_and_locate() {
    assert_eq!(colon(":coords"), Some(Ok(Action::Coords(CoordsOp::Show))));
    assert_eq!(colon(":coords copy"), Some(Ok(Action::Coords(CoordsOp::Copy))));
    assert_eq!(
        colon(":coords locate d0:w1:main:y2"),
        Some(Ok(Action::Coords(CoordsOp::Locate("d0:w1:main:y2".into()))))
    );
}

#[test]
fn colon_coords_bad_argument_is_parse_error() {
    match colon(":coords bogus") {
        Some(Err(msg)) => assert!(!msg.is_empty()),
        other => panic!("expected parse error, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// :coords dispatch through WindowManager::apply
// ---------------------------------------------------------------------------

#[test]
fn apply_style_action_sets_coord_style() {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    assert_eq!(wm.coord_style(), CoordDisplayStyle::Off, "default is Off");
    wm.apply(Action::Coords(CoordsOp::Style(CoordDisplayStyle::Compact))).unwrap();
    assert_eq!(wm.coord_style(), CoordDisplayStyle::Compact);
    wm.apply(Action::Coords(CoordsOp::Style(CoordDisplayStyle::Verbose))).unwrap();
    assert_eq!(wm.coord_style(), CoordDisplayStyle::Verbose);
    wm.apply(Action::Coords(CoordsOp::Style(CoordDisplayStyle::DaemonTuple))).unwrap();
    assert_eq!(wm.coord_style(), CoordDisplayStyle::DaemonTuple);
    wm.apply(Action::Coords(CoordsOp::Style(CoordDisplayStyle::Off))).unwrap();
    assert_eq!(wm.coord_style(), CoordDisplayStyle::Off);
}

#[test]
fn colon_style_text_applied_changes_coord_style() {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    let action = colon(":coords compact").unwrap().unwrap();
    wm.apply(action).unwrap();
    assert_eq!(wm.coord_style(), CoordDisplayStyle::Compact);
}

#[test]
fn apply_locate_focuses_the_matching_pane() {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.resize_all(AREA).unwrap();
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();

    wm.focus_and_raise(1);
    assert_eq!(wm.active_pane_id(), 1);
    let path = wm.coord_of(p2).unwrap().to_path();
    wm.apply(Action::Coords(CoordsOp::Locate(path))).unwrap();
    assert_eq!(wm.active_pane_id(), p2, "Locate by canonical path focuses window {p2}");

    wm.focus_and_raise(p2);
    wm.apply(Action::Coords(CoordsOp::Locate("y1".into()))).unwrap();
    assert_eq!(wm.active_pane_id(), 1, "Locate by window alone focuses window 1");
}

#[test]
fn apply_show_sets_command_data_with_path() {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    let expected_path = wm.coord_of(wm.active_pane_id()).unwrap().to_path();
    wm.apply(Action::Coords(CoordsOp::Show)).unwrap();
    let data = wm.take_command_data().expect(":coords Show sets command data");
    assert!(data.contains("\"path\""), "payload is JSON with a path key: {data}");
    assert!(data.contains(&expected_path), "payload carries {expected_path}: {data}");
    assert_eq!(wm.take_command_data(), None, "take_command_data consumes the payload");
}

// ---------------------------------------------------------------------------
// Title-bar badge rendering
// ---------------------------------------------------------------------------

fn render_text(wm: &WindowManager) -> String {
    let mut buf = Buffer::empty(AREA);
    WorkspaceWidget::new(wm).render(AREA, &mut buf);
    buffer_to_snapshot_string(&buf)
}

/// Two full-width stacked windows: a lone window has no title-bar frame, so the badge needs a split.
fn wm_with_style(style: CoordDisplayStyle) -> WindowManager {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.resize_all(AREA).unwrap();
    wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.resize_all(AREA).unwrap();
    wm.set_coord_style(style);
    wm
}

#[test]
fn badge_off_draws_no_coordinate() {
    let text = render_text(&wm_with_style(CoordDisplayStyle::Off));
    assert!(!text.contains("(w1"), "Off must not draw a badge:\n{text}");
}

#[test]
fn badge_compact_draws_compact_tuple() {
    let text = render_text(&wm_with_style(CoordDisplayStyle::Compact));
    assert!(text.contains("(w1,"), "Compact badge missing:\n{text}");
}

#[test]
fn badge_verbose_draws_labels() {
    let text = render_text(&wm_with_style(CoordDisplayStyle::Verbose));
    assert!(text.contains("Wksp:"), "Verbose badge missing:\n{text}");
}

#[test]
fn badge_daemon_tuple_draws_daemon_prefix() {
    let text = render_text(&wm_with_style(CoordDisplayStyle::DaemonTuple));
    assert!(text.contains("(d0:("), "DaemonTuple badge missing:\n{text}");
}

#[test]
fn badge_follows_style_changed_via_colon_action() {
    let mut wm = wm_with_style(CoordDisplayStyle::Off);
    wm.apply(colon(":coords compact").unwrap().unwrap()).unwrap();
    assert!(render_text(&wm).contains("(w1,"));
    wm.apply(colon(":coords off").unwrap().unwrap()).unwrap();
    assert!(!render_text(&wm).contains("(w1"));
}
