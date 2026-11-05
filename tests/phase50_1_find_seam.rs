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

// ---- Task 2: registry contract, JSON shape ----------------------------------------------------

use bilterm::api::JsonValue;
use bilterm::colon_command::execute_find::find_results_json;
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::registry::{
    AgentRow, FindResults, FindRow, HistoryRow, Origin, PaneRow, Recency, ResultSource, RowData,
    SessionRow, Transport, *,
};

fn source(origin: Origin) -> ResultSource {
    ResultSource {
        origin,
        name: "dev".to_string(),
        path: (origin != Origin::Live).then(|| "/tmp/x/dev.toml".to_string()),
    }
}

fn row(kind: FindKind, data: RowData, origin: Origin) -> FindRow {
    let mut r = FindRow::empty(kind);
    r.id = format!("row:{}", data.row_name());
    r.label = "label".to_string();
    r.source = source(origin);
    r.session = "dev".to_string();
    r.data = data;
    r
}

#[test]
fn find_results_json_has_source_and_masked() {
    let rows = vec![
        row(
            FindKind::Agent,
            RowData::Agent(AgentRow {
                pane: 2,
                name: "claude".into(),
                state: AgentState::Working,
                since_ms: 5,
                profile: Some("claude".into()),
                title: "t".into(),
            }),
            Origin::Live,
        ),
        row(
            FindKind::Session,
            RowData::Session(SessionRow { name: "dev".into(), active: true, parent: None, pane_count: 3 }),
            Origin::SavedSession,
        ),
        row(
            FindKind::Session,
            RowData::Pane(PaneRow { pane: 1, title: "sh".into(), cwd: None, cwd_trusted: false, running_command: None }),
            Origin::Live,
        ),
        row(
            FindKind::History,
            RowData::History(HistoryRow {
                seq: 9,
                pane: Some(1),
                cwd: Some("/w".into()),
                command: "ls".into(),
                exit_code: Some(0),
                duration_ms: Some(12),
                at: Recency::Logged("2026-11-05T00:00:00Z".into()),
            }),
            Origin::HistoryLog,
        ),
    ];
    let mut with_also = rows[1].clone();
    with_also.also_in = vec![source(Origin::HistoryLog)];
    let mut all = rows;
    all.push(with_also);
    let results = FindResults {
        kind: FindKind::Session,
        query: "q".into(),
        total: 12,
        truncated: true,
        masked: 3,
        rows: all,
    };
    let text = find_results_json(&results).to_json_string();
    let json = JsonValue::parse(&text).expect("valid json");
    assert_eq!(json.get("masked").and_then(JsonValue::as_i64), Some(3));
    assert_eq!(json.get("total").and_then(JsonValue::as_i64), Some(12));
    assert_eq!(json.get("returned").and_then(JsonValue::as_i64), Some(5));
    assert_eq!(json.get("truncated").and_then(JsonValue::as_bool), Some(true));
    let rows = json.get("rows").and_then(JsonValue::as_array).unwrap();
    assert_eq!(rows.len(), 5);
    let mut kinds = Vec::new();
    for r in rows {
        let src = r.get("source").expect("every row has source");
        let k = src.get("kind").and_then(JsonValue::as_str).unwrap();
        assert!(["live", "saved-session", "history-log"].contains(&k), "{k}");
        assert!(src.get("name").and_then(JsonValue::as_str).is_some());
        assert!(src.get("path").is_some());
        assert!(r.get("also_in").and_then(JsonValue::as_array).is_some());
        kinds.push(r.get("row").and_then(JsonValue::as_str).unwrap().to_string());
    }
    assert_eq!(kinds, ["agent", "session", "pane", "history", "session"]);
    assert_eq!(rows[0].get("state").and_then(JsonValue::as_str), Some("working"));
    assert_eq!(rows[1].get("pane_count").and_then(JsonValue::as_i64), Some(3));
    assert_eq!(rows[2].get("cwd_trusted").and_then(JsonValue::as_bool), Some(false));
    assert_eq!(rows[3].get("command").and_then(JsonValue::as_str), Some("ls"));
    assert_eq!(rows[3].get("exit_code").and_then(JsonValue::as_i64), Some(0));
    let also = rows[4].get("also_in").and_then(JsonValue::as_array).unwrap();
    assert_eq!(also[0].get("kind").and_then(JsonValue::as_str), Some("history-log"));
    // Every Origin and Transport has a stable wire name and label.
    assert_eq!(source(Origin::Live).label(), "live");
    assert_eq!(source(Origin::SavedSession).label(), "saved:dev");
    assert_eq!(source(Origin::HistoryLog).label(), "log:dev");
    assert_eq!(Transport::SshTmux.as_str(), "ssh-tmux");
    assert!(Origin::Live.priority() < Origin::SavedSession.priority());
    assert!(Origin::SavedSession.priority() < Origin::HistoryLog.priority());
}

#[test]
fn caps_match_contract() {
    assert_eq!(CMD_RING_CAP, 2048);
    assert_eq!(CMD_CAP, 512);
    assert_eq!(NAME_CAP, 64);
    assert_eq!(FIND_RESULTS_CAP, 200);
    assert_eq!(FIND_DEFAULT_LIMIT, 50);
    assert_eq!(FIND_QUERY_CAP, 128);
    assert_eq!(FIND_HISTORY_READ_CAP, 20_000);
    assert_eq!(HISTORY_LOG_BYTES_CAP, 16 * 1024 * 1024);
    assert_eq!(SAVED_SESSIONS_CAP, 64);
    assert_eq!(SAVED_FILE_BYTES_CAP, 2 * 1024 * 1024);
    assert_eq!(PANES_PER_SESSION_CAP, 64);
    assert_eq!(PANE_OUTBOX_CAP, 64);
}

#[test]
fn open_with_empty_providers_reports_zero_results() {
    let mut wm = fixture(100, 30);
    run_colon(&mut wm, ":find session dev");
    let r = wm.find_results();
    assert_eq!((r.kind, r.query.as_str(), r.rows.len(), r.total, r.masked), (FindKind::Session, "dev", 0, 0, 0));
    let data = wm.take_command_data().expect("command data stashed");
    let json = JsonValue::parse(&data).unwrap();
    assert_eq!(json.get("kind").and_then(JsonValue::as_str), Some("session"));
    assert_eq!(json.get("returned").and_then(JsonValue::as_i64), Some(0));
    assert_eq!(json.get("masked").and_then(JsonValue::as_i64), Some(0));
    let status = wm.active_pane().status_message().unwrap_or("").to_string();
    assert_eq!(status, "Find session \"dev\": 0 results (0 live, 0 saved, 0 log)");
    run_colon(&mut wm, ":finder close");
    assert!(!wm.is_find_visible());
}

// ---- Task 3: key table, entry chord, hooks ----------------------------------------------------

use bilterm::input::handle_wm_key_event;
use bilterm::input::keymap::{
    DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS, FIND_MODAL_BINDINGS,
    default_global_action, default_leader_action, find_modal_key_action,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn press(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap();
}

fn ch(wm: &mut WindowManager, c: char) {
    press(wm, KeyCode::Char(c), KeyModifiers::NONE);
}

const CTRL_ALT: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);

#[test]
fn find_key_table_rows_are_unique_and_unshadowed() {
    for (i, (m, c, _)) in FIND_MODAL_BINDINGS.iter().enumerate() {
        for (m2, c2, _) in &FIND_MODAL_BINDINGS[i + 1..] {
            assert!(!(m == m2 && c == c2), "duplicate find row {m:?} {c:?}");
        }
        assert!(
            !DEFAULT_GLOBAL_BINDINGS.iter().any(|(gm, gc, _)| gm == m && gc == c),
            "find row {m:?} {c:?} shadows a global binding"
        );
        assert!(
            !DEFAULT_PANE_BINDINGS.iter().any(|(pm, pc, _)| pm == m && pc == c),
            "find row {m:?} {c:?} shadows a pane binding"
        );
        // Leader rows are post-prefix chords; the modal swallows the prefix while open, so a leader
        // chord and a modal row can never be live at once. Only plain navigation keys may coincide.
        if *m == KeyModifiers::NONE && DEFAULT_LEADER_BINDINGS.iter().any(|(lc, _)| lc == c) {
            assert!(
                matches!(
                    c,
                    KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Tab | KeyCode::BackTab
                ),
                "unexpected leader overlap {c:?}"
            );
        }
    }
    // The entry chord is read from the live global table, never hard-coded here.
    let entry: Vec<_> = DEFAULT_GLOBAL_BINDINGS
        .iter()
        .filter(|(_, _, a)| matches!(a, Action::Find(FindOp::Open { .. })))
        .collect();
    assert!(!entry.is_empty(), "no global Find entry chord");
    for (m, c, _) in &entry {
        assert!(!FIND_MODAL_BINDINGS.iter().any(|(fm, fc, _)| fm == m && fc == c));
    }
    // Every row lowers to Action::Find.
    assert!(FIND_MODAL_BINDINGS.iter().all(|(_, _, a)| matches!(a, Action::Find(_))));
}

#[test]
fn find_entry_chord_opens_modal_over_raw_passthrough() {
    let mut wm = fixture(120, 40);
    wm.active_pane_mut().toggle_manual_raw_override();
    assert_eq!(
        wm.active_pane().effective_routing_mode(),
        bilterm::pty::line_discipline::InputRoutingMode::RawPtyPassthrough,
        "fixture must be in raw passthrough"
    );
    press(&mut wm, KeyCode::Char('f'), CTRL_ALT);
    assert_eq!(wm.find_modal_state(), Some((FindKind::Agent, "", 0)), "Ctrl+Alt+F opens over a raw pane");
    press(&mut wm, KeyCode::Char('f'), CTRL_ALT);
    assert!(!wm.is_find_visible(), "and closes again");
    press(&mut wm, KeyCode::Char('F'), CTRL_ALT | KeyModifiers::SHIFT);
    assert!(wm.is_find_visible(), "shifted form also opens");
    let key = KeyEvent::new(KeyCode::Char('f'), CTRL_ALT);
    assert!(matches!(default_global_action(key, false), Some(Action::Find(FindOp::Open { kind: FindKind::Agent, .. }))));
}

#[test]
fn find_leader_question_mark_still_toggles_help() {
    let key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    assert_eq!(default_leader_action(key), Some(Action::HelpToggle));
    assert!(
        !DEFAULT_LEADER_BINDINGS.iter().any(|(_, a)| matches!(a, Action::Find(_))),
        "Find has no leader chord (P-06)"
    );
}

#[test]
fn find_modal_keys_edit_state() {
    let mut wm = fixture(100, 30);
    press(&mut wm, KeyCode::Char('f'), CTRL_ALT);
    ch(&mut wm, 'a');
    ch(&mut wm, 'b');
    assert_eq!(wm.find_modal_state(), Some((FindKind::Agent, "ab", 0)));
    press(&mut wm, KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(wm.find_modal_state(), Some((FindKind::Agent, "a", 0)));
    press(&mut wm, KeyCode::Char('B'), KeyModifiers::SHIFT);
    assert_eq!(wm.find_modal_state(), Some((FindKind::Agent, "aB", 0)));
    press(&mut wm, KeyCode::Down, KeyModifiers::NONE); // zero rows: selection stays 0
    assert_eq!(wm.find_modal_state().unwrap().2, 0);
    press(&mut wm, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(wm.find_modal_state().unwrap().0, FindKind::Session);
    press(&mut wm, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(wm.find_modal_state().unwrap().0, FindKind::Agent);
    press(&mut wm, KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_eq!(wm.find_modal_state(), Some((FindKind::Agent, "", 0)));
    // Keys with no mapping are swallowed, not leaked to the pane or the global table.
    press(&mut wm, KeyCode::F(5), KeyModifiers::NONE);
    press(&mut wm, KeyCode::Char('q'), KeyModifiers::ALT);
    assert!(wm.is_find_visible());
    assert!(find_modal_key_action(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT)).is_none());
    press(&mut wm, KeyCode::Char('y'), KeyModifiers::CONTROL);
    assert!(wm.is_find_visible(), "Ctrl+Y with no rows only reports");
    press(&mut wm, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!wm.is_find_visible());
}

#[test]
fn find_mouse_hook_is_installed_and_inert_in_the_skeleton() {
    use bilterm::ui::find_ui::{FindLayout, find_layout};
    let mut wm = fixture(100, 30);
    run_colon(&mut wm, ":find agent");
    let area = Rect::new(0, 0, 100, 30);
    let outer = wm.active_modal_rect(area).expect("find modal has geometry");
    let inner = bilterm::wm::inset_window_frame(outer);
    let layout: FindLayout = find_layout(inner, wm.config());
    assert!(layout.list.height > 0 && layout.footer.height == 1);
    let mouse = crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Moved,
        column: layout.list.x,
        row: layout.list.y,
        modifiers: KeyModifiers::NONE,
    };
    assert!(bilterm::input::find_modal::handle_find_modal_mouse(&mut wm, mouse, outer, area).is_none());
    assert!(bilterm::input::find_modal::lower_find_mouse(mouse, &layout, 0, 0, FindKind::Agent).is_none());
    assert!(wm.is_find_visible());
}
