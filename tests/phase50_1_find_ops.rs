//! Phase 50.1-12: result-aware `FindOp` execution (open, re-rank, kind switching, jump/prefill).
//! Headless only; saved and log rows are injected with `set_find_results` because the disk
//! providers belong to plan 11.

use std::time::Instant;

use bilterm::action::Action;
use bilterm::api::JsonValue;
use bilterm::colon_command::parse_colon_command;
use bilterm::wm::find_cmd::{FindKind, FindOp};
use bilterm::wm::registry::{
    FindResults, FindRow, HistoryRow, Origin, PaneRow, Recency, ResultSource, RowData, SessionRow,
};
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::layout::Rect;

fn fixture(panes: usize) -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    for _ in 1..panes {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(Rect::new(0, 0, 80, 24)).unwrap();
    wm.update_agent_states(Instant::now());
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

fn status(wm: &WindowManager) -> String {
    wm.pane(wm.active_pane_id()).unwrap().status_message().unwrap_or_default().to_string()
}

#[test]
fn find_tracer_agent_enter_focuses_pane() {
    let mut wm = fixture(3);
    assert!(wm.focus_and_raise(1) || wm.active_pane_id() == 1);
    assert_eq!(wm.active_pane_id(), 1);
    run_colon(&mut wm, ":find agent");
    let data = wm.take_command_data().expect("find stashes command data");
    let json = JsonValue::parse(&data).expect("valid json");
    let JsonValue::Array(rows) = json.get("rows").unwrap() else { panic!("rows") };
    assert_eq!(rows.len(), 3);
    let src = rows[0].get("source").unwrap();
    assert_eq!(src.get("kind").and_then(JsonValue::as_str), Some("live"));
    assert!(wm.is_find_visible());

    run_colon(&mut wm, ":finder select 2");
    let target = wm.find_results().rows[2].pane.expect("agent row has a pane");
    assert_ne!(target, 1);
    run_colon(&mut wm, ":finder enter");
    assert_eq!(wm.active_pane_id(), target);
    assert!(!wm.is_find_visible());
    assert!(status(&wm).contains("live"), "status: {}", status(&wm));
}

// ---- Task 2: query re-ranking, kind switching, cached reads, clamping -------------------------

fn find_json(wm: &mut WindowManager) -> JsonValue {
    JsonValue::parse(&wm.take_command_data().expect("command data")).expect("json")
}

fn ids(wm: &WindowManager) -> Vec<String> {
    wm.find_results().rows.iter().map(|r| r.id.clone()).collect()
}

#[test]
fn typing_reranks_without_disk_io() {
    let dir = tempfile::tempdir().unwrap();
    let mut wm = fixture(2);
    wm.config_mut().session_dir = Some(dir.path().to_str().unwrap().to_string());
    wm.save_and_activate_session(Some("work")).unwrap();
    run_colon(&mut wm, ":find session");
    let before = ids(&wm);
    assert!(!before.is_empty());
    std::fs::remove_dir_all(dir.path()).unwrap();
    wm.apply(Action::Find(FindOp::Push('z'))).unwrap();
    wm.apply(Action::Find(FindOp::Push('q'))).unwrap();
    assert!(ids(&wm).len() <= before.len());
    wm.apply(Action::Find(FindOp::Pop)).unwrap();
    wm.apply(Action::Find(FindOp::Pop)).unwrap();
    assert_eq!(ids(&wm), before, "cached candidates re-ranked without a new gather");
    assert_eq!(wm.find_modal_state().map(|(_, q, _)| q.to_string()), Some(String::new()));
}

#[test]
fn cycle_kind_loads_once_per_open() {
    let mut wm = fixture(3);
    run_colon(&mut wm, ":find agent");
    assert_eq!(wm.find_results().rows.len(), 3);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.update_agent_states(Instant::now());
    // Session kind is gathered lazily, after the split: sees four panes.
    wm.apply(Action::Find(FindOp::CycleKind(true))).unwrap();
    assert_eq!(wm.find_results().kind, FindKind::Session);
    let pane_rows = |wm: &WindowManager| {
        wm.find_results().rows.iter().filter(|r| matches!(r.data, RowData::Pane(_))).count()
    };
    assert_eq!(pane_rows(&wm), 4);
    wm.apply(Action::Find(FindOp::CycleKind(true))).unwrap();
    assert_eq!(wm.find_results().kind, FindKind::History);
    wm.apply(Action::Find(FindOp::CycleKind(true))).unwrap();
    assert_eq!(wm.find_results().kind, FindKind::Agent);
    assert_eq!(wm.find_results().rows.len(), 3, "agent candidates came from the cache, not a reload");
    // A new open reloads.
    run_colon(&mut wm, ":find agent");
    assert_eq!(wm.find_results().rows.len(), 4);
}

#[test]
fn selection_is_clamped_and_resets_on_query_change() {
    let mut wm = fixture(3);
    run_colon(&mut wm, ":find agent");
    let sel = |wm: &WindowManager| wm.find_modal_state().unwrap().2;
    wm.apply(Action::Find(FindOp::Move(100))).unwrap();
    assert_eq!(sel(&wm), 2);
    wm.apply(Action::Find(FindOp::Select(99))).unwrap();
    assert_eq!(sel(&wm), 2);
    wm.apply(Action::Find(FindOp::Move(-100))).unwrap();
    assert_eq!(sel(&wm), 0);
    wm.apply(Action::Find(FindOp::Move(i16::MAX))).unwrap();
    assert_eq!(sel(&wm), 2);
    wm.apply(Action::Find(FindOp::Push('1'))).unwrap();
    assert_eq!(sel(&wm), 0);
    wm.apply(Action::Find(FindOp::Select(2))).unwrap();
    wm.apply(Action::Find(FindOp::CycleKind(false))).unwrap();
    assert_eq!(sel(&wm), 0);
}

#[test]
fn status_message_has_counts_and_masked() {
    use bilterm::colon_command::execute_find::find_status_message;
    let mut wm = fixture(2);
    run_colon(&mut wm, ":find agent");
    let s = status(&wm);
    assert!(s.starts_with("Find agent \"\": 2 results (2 live, 0 saved, 0 log)"), "{s}");
    assert!(!s.contains("masked"), "{s}");
    let json = find_json(&mut wm);
    assert_eq!(json.get("masked").and_then(JsonValue::as_i64), Some(0));

    let rows = vec![
        row(FindKind::History, hist("a"), Origin::Live, "p", None),
        row(FindKind::History, hist("b"), Origin::SavedSession, "p", None),
        row(FindKind::History, hist("c"), Origin::HistoryLog, "p", None),
        row(FindKind::History, hist("d"), Origin::HistoryLog, "p", None),
    ];
    let r = FindResults { kind: FindKind::History, query: "x".into(), rows, total: 4, truncated: false, masked: 2 };
    assert_eq!(
        find_status_message(&r),
        "Find history \"x\": 4 results, 2 masked (1 live, 1 saved, 2 log)"
    );
}

#[test]
fn empty_results_activate_is_safe() {
    let mut wm = fixture(2);
    run_colon(&mut wm, ":find agent zzzzqqqq");
    assert!(wm.find_results().rows.is_empty());
    for op in [FindOp::Activate, FindOp::Prefill] {
        wm.apply(Action::Find(op)).unwrap();
        assert_eq!(status(&wm), "Find: no result selected");
        assert!(wm.is_find_visible());
    }
}

#[test]
fn query_cap_applies_to_set_query_and_push() {
    let mut wm = fixture(1);
    run_colon(&mut wm, ":find agent");
    wm.apply(Action::Find(FindOp::SetQuery("q".repeat(10_000)))).unwrap();
    assert_eq!(wm.find_modal_state().unwrap().1.chars().count(), 128);
    wm.apply(Action::Find(FindOp::Push('x'))).unwrap();
    assert_eq!(wm.find_modal_state().unwrap().1.chars().count(), 128);
    assert_eq!(wm.find_results().query.chars().count(), 128);
}

// ---- Task 3: jump semantics for sessions, history and label-only rows -------------------------

fn row(kind: FindKind, data: RowData, origin: Origin, session: &str, pane: Option<usize>) -> FindRow {
    let mut r = FindRow::empty(kind);
    r.data = data;
    r.source = ResultSource { origin, name: session.to_string(), path: None };
    r.session = session.to_string();
    r.pane = pane;
    r.label = "row".to_string();
    r
}

fn hist(command: &str) -> RowData {
    RowData::History(HistoryRow {
        seq: 1,
        pane: None,
        cwd: None,
        command: command.to_string(),
        exit_code: None,
        duration_ms: None,
        at: Recency::Saved,
    })
}

fn hist_pane(command: &str, pane: usize) -> RowData {
    let RowData::History(mut h) = hist(command) else { unreachable!() };
    h.pane = Some(pane);
    RowData::History(h)
}

fn session_data(name: &str) -> RowData {
    RowData::Session(SessionRow { name: name.to_string(), active: false, parent: None, pane_count: 2 })
}

fn pane_data(id: usize) -> RowData {
    RowData::Pane(PaneRow { pane: id, title: "t".into(), cwd: None, cwd_trusted: false, running_command: None })
}

/// Opens the window on `kind` and replaces the results with `rows` (selection 0).
fn inject(wm: &mut WindowManager, kind: FindKind, rows: Vec<FindRow>) {
    let word = kind.as_str();
    run_colon(wm, &format!(":find {word}"));
    let total = rows.len();
    wm.set_find_results(FindResults { kind, query: String::new(), rows, total, truncated: false, masked: 0 });
}

fn saved_work_fixture() -> (tempfile::TempDir, WindowManager) {
    let dir = tempfile::tempdir().unwrap();
    let mut wm = fixture(2);
    wm.config_mut().session_dir = Some(dir.path().to_str().unwrap().to_string());
    wm.save_and_activate_session(Some("work")).unwrap();
    wm.config_mut().history_session = "main".to_string();
    wm.config_mut().active_layout_profile = "main".to_string();
    (dir, wm)
}

#[test]
fn saved_session_row_switches_session() {
    let (_dir, mut wm) = saved_work_fixture();
    let r = row(FindKind::Session, session_data("work"), Origin::SavedSession, "work", None);
    inject(&mut wm, FindKind::Session, vec![r]);
    wm.apply(Action::Find(FindOp::Activate)).unwrap();
    assert_eq!(wm.config().history_session, "work");
    assert!(!wm.is_find_visible());
    let s = status(&wm);
    assert!(s.contains("autosave"), "{s}");
    assert!(s.contains("saved:work"), "{s}");
}

#[test]
fn saved_pane_row_focuses_that_pane_after_switch() {
    let (_dir, mut wm) = saved_work_fixture();
    let r = row(FindKind::Session, pane_data(2), Origin::SavedSession, "work", Some(2));
    inject(&mut wm, FindKind::Session, vec![r]);
    wm.apply(Action::Find(FindOp::Activate)).unwrap();
    assert_eq!(wm.config().history_session, "work");
    assert!(wm.pane(2).is_some());
    assert_eq!(wm.active_pane_id(), 2);
}

#[test]
fn history_only_session_row_reports_no_layout_and_stays_open() {
    let mut wm = fixture(1);
    let r = row(FindKind::Session, session_data("ghost"), Origin::HistoryLog, "ghost", None);
    inject(&mut wm, FindKind::Session, vec![r]);
    wm.apply(Action::Find(FindOp::Activate)).unwrap();
    assert_eq!(status(&wm), "session ghost has history only (no saved layout)");
    assert!(wm.is_find_visible());
}

#[test]
fn live_active_session_row_just_closes() {
    let mut wm = fixture(1);
    let name = wm.config().history_session.clone();
    let r = row(FindKind::Session, session_data(&name), Origin::Live, &name, None);
    inject(&mut wm, FindKind::Session, vec![r]);
    wm.apply(Action::Find(FindOp::Activate)).unwrap();
    assert!(!wm.is_find_visible());
    assert_eq!(wm.config().history_session, name);
}

#[test]
fn vanished_pane_row_reports_gone_and_stays_open() {
    let mut wm = fixture(2);
    let data = FindRow::empty(FindKind::Agent).data;
    let r = row(FindKind::Agent, data, Origin::Live, "s", Some(99));
    inject(&mut wm, FindKind::Agent, vec![r]);
    wm.apply(Action::Find(FindOp::Activate)).unwrap();
    assert_eq!(status(&wm), "pane #99 is gone");
    assert!(wm.is_find_visible());
}

#[test]
fn history_row_of_live_pane_scrolls_to_block() {
    let mut wm = fixture(2);
    assert!(wm.pane(1).is_some());
    wm.pane_mut(1).unwrap().vt_mut().record_command_submission("echo hi");
    wm.pane_mut(1).unwrap().process_pty_output("\r\nline".repeat(120).as_bytes());
    assert_eq!(wm.pane(1).unwrap().vt().scrollback_offset(), 0);
    wm.focus_and_raise(2);
    assert_eq!(wm.active_pane_id(), 2);
    let r = row(FindKind::History, hist_pane("echo hi", 1), Origin::Live, "s", Some(1));
    inject(&mut wm, FindKind::History, vec![r]);
    wm.apply(Action::Find(FindOp::Activate)).unwrap();
    assert_eq!(wm.active_pane_id(), 1);
    assert!(!wm.is_find_visible());
    assert!(wm.pane(1).unwrap().vt().scrollback_offset() > 0);
}

#[test]
fn history_row_prefills_prompt_without_running() {
    for op in [FindOp::Activate, FindOp::Prefill] {
        let mut wm = fixture(2);
        let before_write = wm.active_pane().last_pty_write().to_vec();
        let before_hist = wm.active_pane().prompt().history.clone();
        let r = row(FindKind::History, hist("git status"), Origin::HistoryLog, "proj", None);
        inject(&mut wm, FindKind::History, vec![r]);
        wm.apply(Action::Find(op)).unwrap();
        let pane = wm.active_pane();
        assert_eq!(pane.prompt().text(), "git status");
        assert_eq!(pane.last_pty_write(), before_write.as_slice());
        assert_eq!(pane.prompt().history, before_hist);
        assert!(!wm.is_find_visible());
        assert!(status(&wm).contains("log:proj"), "{}", status(&wm));
    }
}

#[test]
fn prefill_with_hidden_prompt_does_nothing_and_says_so() {
    let mut wm = fixture(1);
    wm.active_pane_mut().process_pty_output(b"\x1b[?1049h");
    assert!(!wm.active_pane().is_prompt_visible());
    let r = row(FindKind::History, hist("ls -la"), Origin::HistoryLog, "proj", None);
    inject(&mut wm, FindKind::History, vec![r]);
    wm.apply(Action::Find(FindOp::Prefill)).unwrap();
    assert!(status(&wm).contains("hidden"), "{}", status(&wm));
    assert_eq!(wm.active_pane().prompt().text(), "");
    assert!(wm.is_find_visible());
    assert!(wm.active_pane().last_pty_write().is_empty());
}

#[test]
fn metadata_only_row_focuses_pane_and_explains() {
    let mut wm = fixture(2);
    wm.focus_and_raise(2);
    let mut r = row(FindKind::Session, pane_data(1), Origin::Live, "s", Some(1));
    r.metadata_only = true;
    inject(&mut wm, FindKind::Session, vec![r]);
    wm.apply(Action::Find(FindOp::Activate)).unwrap();
    assert_eq!(wm.active_pane_id(), 1);
    assert!(status(&wm).contains("no remote tmux is attached"), "{}", status(&wm));
    assert!(!wm.is_find_visible());
}

#[test]
fn prefill_on_agent_row_says_history_only() {
    let mut wm = fixture(2);
    run_colon(&mut wm, ":find agent");
    wm.apply(Action::Find(FindOp::Prefill)).unwrap();
    assert_eq!(status(&wm), "Only history rows can be placed in the prompt");
    assert!(wm.is_find_visible());
}
