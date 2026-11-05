//! Phase 50.1-01 seam: `:find agent|session|history` and `:finder ...` lower to `Action::Find`,
//! open a rendered `Modal::Find`; legacy `:find <text>` / `:search` parsing is unchanged (D-U1).

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::ui::{WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::wm::find_cmd::{FindKind, FindOp};
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

fn fixture(cols: u16, rows: u16) -> WindowManager {
    let mut wm = WindowManager::new_headless(rows, cols);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(Rect::new(0, 0, cols, rows)).unwrap();
    wm
}

fn parse(cmd: &str) -> Action {
    parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"))
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    wm.apply(parse(cmd)).unwrap();
}

fn render(wm: &WindowManager, cols: u16, rows: u16) -> String {
    let area = Rect::new(0, 0, cols, rows);
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buffer_to_snapshot_string(&buf)
}

#[test]
fn find_tracer_opens_modal_and_renders() {
    let mut wm = fixture(100, 30);
    assert!(!wm.is_find_visible());
    run_colon(&mut wm, ":find agent foo");
    assert_eq!(wm.find_modal_state(), Some((FindKind::Agent, "foo", 0)));
    let frame = render(&wm, 100, 30);
    assert!(frame.contains("FIND"), "frame:\n{frame}");
    assert!(frame.contains("Find agent > foo_"), "frame:\n{frame}");
    run_colon(&mut wm, ":finder close");
    assert!(!wm.is_find_visible());
}

#[test]
fn find_reserved_words_parse_and_others_do_not() {
    let open = |kind, q: &str| Action::Find(FindOp::Open { kind, query: q.to_string() });
    assert_eq!(parse(":find agent"), open(FindKind::Agent, ""));
    assert_eq!(parse(":find agents  a   b"), open(FindKind::Agent, "a b"));
    assert_eq!(parse(":find session x"), open(FindKind::Session, "x"));
    assert_eq!(parse(":find sessions"), open(FindKind::Session, ""));
    assert_eq!(parse(":find history git status"), open(FindKind::History, "git status"));
    // D-U1: every other first word keeps its scrollback-search meaning, byte-identical.
    assert_eq!(parse(":find foo"), Action::SearchQuery("foo".to_string()));
    assert_eq!(parse(":find agentx"), Action::SearchQuery("agentx".to_string()));
    assert_eq!(parse(":find foo agent"), Action::SearchQuery("foo agent".to_string()));
    assert_eq!(parse(":find"), Action::SearchOff);
    assert_eq!(parse(":find off"), Action::SearchOff);
    assert_eq!(parse(":find clear"), Action::SearchOff);
    assert_eq!(parse(":find prefill"), Action::SearchPromptPrefill);
    assert_eq!(parse(":find next"), Action::SearchNext);
    assert_eq!(parse(":find n"), Action::SearchNext);
    assert_eq!(parse(":find prev"), Action::SearchPrev);
    assert_eq!(parse(":search foo"), Action::SearchQuery("foo".to_string()));
    assert_eq!(parse(":search agent"), Action::SearchQuery("agent".to_string()));
    assert_eq!(parse(":search"), Action::SearchOff);
}

#[test]
fn finder_grammar_round_trips() {
    let f = |op| Action::Find(op);
    assert_eq!(parse(":finder kind agent"), f(FindOp::SetKind(FindKind::Agent)));
    assert_eq!(parse(":finder kind session"), f(FindOp::SetKind(FindKind::Session)));
    assert_eq!(parse(":finder kind history"), f(FindOp::SetKind(FindKind::History)));
    assert_eq!(parse(":finder kind next"), f(FindOp::CycleKind(true)));
    assert_eq!(parse(":finder kind prev"), f(FindOp::CycleKind(false)));
    assert_eq!(parse(":finder query hello world"), f(FindOp::SetQuery("hello world".to_string())));
    assert_eq!(parse(":finder query"), f(FindOp::ClearQuery));
    assert_eq!(parse(":finder move +3"), f(FindOp::Move(3)));
    assert_eq!(parse(":finder move -2"), f(FindOp::Move(-2)));
    assert_eq!(parse(":finder move top"), f(FindOp::Move(i16::MIN)));
    assert_eq!(parse(":finder move bottom"), f(FindOp::Move(i16::MAX)));
    assert_eq!(parse(":finder select 4"), f(FindOp::Select(4)));
    assert_eq!(parse(":finder enter"), f(FindOp::Activate));
    assert_eq!(parse(":finder prefill"), f(FindOp::Prefill));
    assert_eq!(parse(":finder close"), f(FindOp::Close));
    for bad in [":finder", ":finder kind", ":finder kind nope", ":finder move", ":finder move x", ":finder select -1", ":finder bogus", ":finder enter now"] {
        let err = parse_colon_command(bad).unwrap().unwrap_err();
        assert!(err.starts_with("Usage: :finder"), "{bad}: {err}");
    }
}

#[test]
fn finder_edit_ops_change_modal_state_and_cap_the_query() {
    let mut wm = fixture(100, 30);
    // No-ops while closed.
    run_colon(&mut wm, ":finder query abc");
    assert!(!wm.is_find_visible());
    run_colon(&mut wm, ":find history");
    run_colon(&mut wm, ":finder query abc");
    assert_eq!(wm.find_modal_state(), Some((FindKind::History, "abc", 0)));
    run_colon(&mut wm, ":finder kind next");
    assert_eq!(wm.find_modal_state().unwrap().0, FindKind::Agent);
    run_colon(&mut wm, ":finder query");
    assert_eq!(wm.find_modal_state().unwrap().1, "");
    // T-50.1-01: query length is bounded at parse and at every edit op.
    let long = "x".repeat(10_000);
    run_colon(&mut wm, &format!(":finder query {long}"));
    assert_eq!(wm.find_modal_state().unwrap().1.chars().count(), 128);
    for _ in 0..200 {
        wm.apply(Action::Find(FindOp::Push('y'))).unwrap();
    }
    assert_eq!(wm.find_modal_state().unwrap().1.chars().count(), 128);
    run_colon(&mut wm, &format!(":find session {long}"));
    assert_eq!(wm.find_modal_state().unwrap().1.chars().count(), 128);
    // Selection clamps to zero rows.
    run_colon(&mut wm, ":finder move +5");
    run_colon(&mut wm, ":finder select 9");
    assert_eq!(wm.find_modal_state().unwrap().2, 0);
}
