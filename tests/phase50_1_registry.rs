//! Phase 50.1-03: live half of the registry (agents, panes, active session). Headless only.

use std::sync::Once;
use std::time::Instant;

use bilterm::api::JsonValue;
use bilterm::colon_command::execute_find::find_results_json;
use bilterm::pane::CompositePane;
use bilterm::wm::registry::{FindRow, Origin, PANES_PER_SESSION_CAP, RowData};
use bilterm::wm::find_cmd::FindKind;
use bilterm::wm::{SplitDirection, WindowManager};

static INIT: Once = Once::new();

fn init_env() {
    INIT.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-p50-1-reg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
}

fn wm_new(rows: u16, cols: u16) -> WindowManager {
    init_env();
    WindowManager::new_headless(rows, cols)
}

fn three_panes() -> WindowManager {
    let mut wm = wm_new(24, 80);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(wm.panes().len(), 3);
    wm.update_agent_states(Instant::now());
    wm
}

fn pane_rows(rows: &[FindRow]) -> Vec<&FindRow> {
    rows.iter().filter(|r| matches!(r.data, RowData::Pane(_))).collect()
}

#[test]
fn agent_rows_have_stable_ids_and_live_source() {
    let mut wm = three_panes();
    let session = wm.config().history_session.clone();
    let r = wm.run_find(FindKind::Agent, "", 50);
    assert_eq!(r.rows.len(), 3);
    for (row, pane) in r.rows.iter().zip(1usize..) {
        assert_eq!(row.id, format!("agent:{session}/{pane}"));
        assert_eq!(row.source.origin, Origin::Live);
        assert_eq!(row.source.name, session);
        assert_eq!(row.source.path, None);
        assert_eq!(row.pane, Some(pane));
        assert!(matches!(row.data, RowData::Agent(_)));
    }
    let v = find_results_json(&r);
    let JsonValue::Array(rows) = v.get("rows").unwrap() else { panic!("rows not array") };
    let src = rows[0].get("source").unwrap();
    assert_eq!(src.get("kind").and_then(JsonValue::as_str), Some("live"));
    assert_eq!(src.get("name").and_then(JsonValue::as_str), Some(session.as_str()));
}

#[test]
fn titles_are_sanitised_and_capped() {
    let mut wm = three_panes();
    let hostile = format!("\u{1b}[31mevil\u{202e}{}", "x".repeat(300));
    wm.pane_mut(1).unwrap().set_title(hostile);
    let r = wm.run_find(FindKind::Agent, "", 50);
    for row in &r.rows {
        for text in [&row.label, &row.detail] {
            assert!(!text.chars().any(|c| c.is_control() || c == '\u{202e}'), "{text:?}");
        }
        assert!(row.label.chars().count() <= 64, "{}", row.label);
    }
    let RowData::Agent(a) = &r.rows[0].data else { panic!("not agent") };
    assert!(!a.title.chars().any(|c| c.is_control() || c == '\u{202e}'));
    assert_eq!(a.title.chars().count(), 64);
    assert!(a.title.ends_with('…'));
    let sess = wm.run_find(FindKind::Session, "", 50);
    let pane = pane_rows(&sess.rows)[0];
    assert!(pane.label.chars().count() <= 64);
    assert!(pane.label.ends_with('…'));
    assert!(!pane.label.chars().any(|c| c.is_control() || c == '\u{202e}'));
}

#[test]
fn snapshot_has_no_disk_io() {
    let mut wm = three_panes();
    let missing = std::env::temp_dir().join(format!("bilterm-reg-missing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&missing);
    wm.config_mut().session_dir = Some(missing.to_string_lossy().to_string());
    let snap = wm.registry_snapshot();
    assert_eq!(snap.agents.len(), 3);
    assert_eq!(snap.sessions.len(), 1);
    assert_eq!(snap.panes.len(), 3);
    assert!(snap.history.is_empty());
    assert!(!missing.exists(), "snapshot must not create the session dir");
}

#[test]
fn agent_order_is_pane_id_ascending_for_empty_query() {
    let mut wm = three_panes();
    let r = wm.run_find(FindKind::Agent, "", 50);
    let panes: Vec<usize> = r.rows.iter().filter_map(|r| r.pane).collect();
    assert_eq!(panes, vec![1, 2, 3]);
    let orders: Vec<u64> = r.rows.iter().map(|r| r.order).collect();
    assert_eq!(orders, vec![u64::MAX - 1, u64::MAX - 2, u64::MAX - 3]);
}

#[test]
fn agent_label_and_detail_follow_contract() {
    let mut wm = three_panes();
    let r = wm.run_find(FindKind::Agent, "", 50);
    let row = &r.rows[0];
    assert!(row.label.ends_with(" #1"), "{}", row.label);
    assert!(row.detail.contains("local"), "{}", row.detail);
    let RowData::Agent(a) = &row.data else { panic!("not agent") };
    assert!(row.detail.starts_with(a.state.as_str()), "{}", row.detail);
}

#[test]
fn session_kind_lists_active_session_and_its_panes() {
    let mut wm = three_panes();
    let session = wm.config().history_session.clone();
    let r = wm.run_find(FindKind::Session, "", 50);
    let sessions: Vec<&FindRow> = r.rows.iter().filter(|r| matches!(r.data, RowData::Session(_))).collect();
    assert_eq!(sessions.len(), 1);
    let RowData::Session(s) = &sessions[0].data else { unreachable!() };
    assert!(s.active);
    assert_eq!(s.pane_count, 3);
    assert_eq!(s.name, session);
    assert_eq!(sessions[0].id, format!("session:{session}"));
    let panes = pane_rows(&r.rows);
    assert_eq!(panes.len(), 3);
    for (row, pane) in panes.iter().zip(1usize..) {
        assert_eq!(row.id, format!("pane:{session}/{pane}"));
        assert_eq!(row.kind, FindKind::Session);
    }
    assert!(r.rows.iter().all(|r| r.source.origin == Origin::Live && r.source.path.is_none()));
}

#[test]
fn ssh_pane_cwd_is_untrusted_without_osc7() {
    let mut wm = three_panes();
    {
        let p = wm.pane_mut(2).unwrap();
        p.set_remote_target(Some("me@box".into()));
        p.set_tracked_cwd(Some("/home/local/secret-project".into()));
    }
    let find_pane = |wm: &mut WindowManager| -> FindRow {
        let r = wm.run_find(FindKind::Session, "", 50);
        pane_rows(&r.rows).into_iter().find(|r| r.pane == Some(2)).unwrap().clone()
    };
    let row = find_pane(&mut wm);
    let RowData::Pane(p) = &row.data else { unreachable!() };
    assert_eq!(p.cwd, None);
    assert!(!p.cwd_trusted);
    assert!(!row.detail.contains("secret-project"), "{}", row.detail);

    wm.pane_mut(2).unwrap().process_pty_output(b"\x1b]7;file://box/srv/app\x07");
    let row = find_pane(&mut wm);
    let RowData::Pane(p) = &row.data else { unreachable!() };
    assert_eq!(p.cwd.as_deref(), Some("/srv/app"));
    assert!(p.cwd_trusted);
}

#[test]
fn local_pane_cwd_is_trusted() {
    let mut wm = three_panes();
    wm.pane_mut(1).unwrap().set_tracked_cwd(Some("/tmp/work".into()));
    let r = wm.run_find(FindKind::Session, "", 50);
    let row = pane_rows(&r.rows).into_iter().find(|r| r.pane == Some(1)).unwrap().clone();
    let RowData::Pane(p) = &row.data else { unreachable!() };
    assert_eq!(p.cwd.as_deref(), Some("/tmp/work"));
    assert!(p.cwd_trusted);
    assert!(row.detail.contains("/tmp/work"), "{}", row.detail);
}

#[test]
fn pane_rows_capped_per_session() {
    let mut wm = wm_new(24, 80);
    for id in 2..=100usize {
        wm.panes_mut().insert(id, CompositePane::new_headless(id, 24, 80));
    }
    assert_eq!(wm.panes().len(), 100);
    let r = wm.run_find(FindKind::Session, "", 200);
    assert_eq!(pane_rows(&r.rows).len(), PANES_PER_SESSION_CAP);
    let s = r.rows.iter().find_map(|r| match &r.data {
        RowData::Session(s) => Some(s.clone()),
        _ => None,
    });
    assert_eq!(s.unwrap().pane_count, 100);
}

#[test]
fn session_haystack_contains_pane_titles_and_hosts() {
    let mut wm = three_panes();
    wm.pane_mut(3).unwrap().set_remote_target(Some("me@boxhost".into()));
    wm.pane_mut(3).unwrap().set_title("deploy-watch");
    let reg = wm.registry_snapshot();
    let session = &reg.sessions[0];
    assert!(session.haystack.contains("deploy-watch"), "{}", session.haystack);
    assert!(session.haystack.starts_with(&session.label));
}
