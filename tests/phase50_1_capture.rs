//! Phase 50.1-10: command capture (pane outbox -> tick -> ring -> History rows), redaction,
//! bounds, host/cwd rules and exit-code join. Headless only.

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::pane::history_store::HistoryStore;
use bilterm::wm::find_cmd::FindKind;
use bilterm::wm::registry::{Origin, RowData};
use bilterm::wm::WindowManager;

/// A headless wm whose pane 1 records into a tempdir history.log.
fn wm_with_history() -> (WindowManager, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = WindowManager::new_headless(24, 80);
    enable_history(&mut wm, 1, tmp.path());
    (wm, tmp)
}

fn enable_history(wm: &mut WindowManager, pane: usize, dir: &std::path::Path) {
    let store: &mut HistoryStore = wm.pane_mut(pane).unwrap().history_store_mut();
    store.base_dir = dir.to_path_buf();
    store.enabled = true;
    store.session_file_path = dir.join("sessions/default/history.log");
    store.run_id = "run-cap".to_string();
}

fn log_text(dir: &std::path::Path) -> String {
    std::fs::read_to_string(dir.join("sessions/default/history.log")).unwrap_or_default()
}

fn history_rows(wm: &mut WindowManager, query: &str) -> Vec<bilterm::wm::registry::FindRow> {
    wm.run_find(FindKind::History, query, 200)
        .rows
        .into_iter()
        .filter(|r| r.source.origin == Origin::Live)
        .collect()
}

#[test]
fn typed_command_reaches_history_candidates() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.pane_mut(1).unwrap().submit_command("echo hi").unwrap();
    wm.poll_all_ptys();
    let r = wm.run_find(FindKind::History, "echo", 50);
    assert_eq!(r.rows.len(), 1);
    let row = &r.rows[0];
    assert_eq!(row.kind, FindKind::History);
    assert_eq!(row.source.origin, Origin::Live);
    assert_eq!(row.source.name, wm.config().history_session);
    assert_eq!(row.label, "echo hi");
    assert_eq!(row.pane, Some(1));
    assert!(row.id.starts_with(&format!("history:{}/", wm.config().history_session)));
    let RowData::History(h) = &row.data else { panic!("not a history row") };
    assert_eq!(h.command, "echo hi");
    assert_eq!(h.exit_code, None);
    assert_eq!(wm.registry().history_len(), 1);
}

fn password_prompt_pane(wm: &mut WindowManager) {
    wm.pane_mut(1).unwrap().line_discipline_mut().echo_enabled = false;
    assert!(wm.pane(1).unwrap().is_password_prompt());
}

#[test]
fn pane_exec_with_allow_echo_lock_leaves_no_trace() {
    let (wm, tmp) = wm_with_history();
    let mut plane = AutomationControlPlane::new(wm, ratatui::layout::Rect::new(0, 0, 80, 24));
    password_prompt_pane(&mut plane.wm);
    let blocks_before = plane.wm.pane(1).unwrap().vt().command_blocks().len();

    let out = plane.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":1,"method":"pane.exec","params":{"command":"hunter2","allow_echo_lock":true}}"#,
    );
    let v = JsonValue::parse(&out).unwrap();
    assert!(v.get("result").is_some(), "exec must still succeed: {out}");
    // Still written to the PTY path.
    assert!(plane.wm.pane(1).unwrap().last_pty_write().starts_with(b"hunter2"));

    let pane = plane.wm.pane_mut(1).unwrap();
    assert!(pane.prompt().history.iter().all(|h| !h.contains("hunter2")));
    assert_eq!(pane.vt().command_blocks().len(), blocks_before);
    assert!(pane.vt().command_blocks().iter().all(|b| !b.command.contains("hunter2")));
    assert!(pane.prompt().running_command.as_deref().is_none_or(|c| !c.contains("hunter2")));
    // Drained by the tick below: nothing reaches the ring, nothing is counted.
    assert!(!log_text(tmp.path()).contains("hunter2"));

    plane.wm.poll_all_ptys();
    assert_eq!(plane.wm.registry().history_len(), 0);
    assert_eq!(plane.wm.registry().masked_live(), 0);
    assert!(!format!("{:?}", plane.wm.registry()).contains("hunter2"));
}
