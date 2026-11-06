//! Phase 50.1-10: command capture (pane outbox -> tick -> ring -> History rows), redaction,
//! bounds, host/cwd rules and exit-code join. Headless only.

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::pane::history_store::{HistoryStore, parse_history_line};
use bilterm::wm::find_cmd::FindKind;
use bilterm::wm::registry::{CMD_RING_CAP, Origin, PANE_OUTBOX_CAP, RowData};
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

#[test]
fn secret_looking_command_is_not_stored_but_counted() {
    let (mut wm, tmp) = wm_with_history();
    wm.pane_mut(1).unwrap().submit_command("export API_TOKEN=abc123").unwrap();
    // Harmony (P-04): still in the in-memory recall history and block list.
    let pane = wm.pane(1).unwrap();
    assert!(pane.prompt().history.iter().any(|h| h.contains("abc123")));
    assert!(pane.vt().command_blocks().iter().any(|b| b.command.contains("abc123")));
    assert!(!log_text(tmp.path()).contains("abc123"));
    wm.poll_all_ptys();
    let r = wm.run_find(FindKind::History, "", 50);
    assert_eq!(r.masked, 1);
    assert!(r.rows.iter().all(|row| !row.label.contains("abc123")));
    assert_eq!(wm.registry().history_len(), 0);
    assert!(!format!("{:?}", wm.registry()).contains("abc123"));
    assert!(!format!("{r:?}").contains("abc123"));
}

#[test]
fn colon_commands_keep_existing_recording_but_are_not_captured() {
    let (mut wm, tmp) = wm_with_history();
    wm.pane_mut(1).unwrap().submit_command(":version on").unwrap();
    let pane = wm.pane(1).unwrap();
    assert!(pane.prompt().history.iter().any(|h| h == ":version on"));
    assert!(pane.vt().command_blocks().iter().any(|b| b.command == ":version on"));
    assert!(log_text(tmp.path()).contains("] :version on"));
    wm.poll_all_ptys();
    assert_eq!(wm.registry().history_len(), 0);
}

#[test]
fn normal_command_is_unchanged_for_legacy_readers() {
    let (mut wm, tmp) = wm_with_history();
    wm.pane_mut(1).unwrap().set_tracked_cwd(Some("/tmp/work dir".into()));
    wm.pane_mut(1).unwrap().submit_command("ls -la").unwrap();
    let log = log_text(tmp.path());
    let line = log.lines().next().expect("one line");
    let entry = parse_history_line(line).expect("legacy parse");
    assert_eq!(entry.command, "ls -la");
    assert_eq!(entry.session, "default");
    assert_eq!(entry.run_id, "run-cap");
    assert_eq!(entry.pane_id, Some(1));
    assert!(line.contains("cwd=/tmp/work%20dir"), "{line}");
    assert_eq!(
        wm.pane(1).unwrap().history_store().load_recent_commands(),
        vec!["ls -la".to_string()]
    );
}

#[test]
fn ring_is_bounded() {
    let mut wm = WindowManager::new_headless(24, 80);
    let total = CMD_RING_CAP + 100;
    for i in 0..total {
        wm.pane_mut(1).unwrap().submit_command(&format!("echo n{i}")).unwrap();
        if i % 50 == 49 {
            wm.poll_all_ptys();
        }
    }
    wm.poll_all_ptys();
    assert_eq!(wm.registry().history_len(), CMD_RING_CAP);
    let all = bilterm::wm::registry::capture::candidates(&wm);
    assert_eq!(all.len(), CMD_RING_CAP);
    let seqs: Vec<u64> = all.iter().map(|r| r.order).collect();
    assert!(seqs.windows(2).all(|w| w[0] < w[1]), "seq strictly increasing");
    assert_eq!(seqs[0], 100, "oldest evicted");
    assert_eq!(*seqs.last().unwrap(), total as u64 - 1);
    assert!(all.iter().any(|r| r.label == format!("echo n{}", total - 1)));
    assert!(!all.iter().any(|r| r.label == "echo n0"));
}

#[test]
fn outbox_is_bounded_between_ticks() {
    let mut wm = WindowManager::new_headless(24, 80);
    for i in 0..200 {
        wm.pane_mut(1).unwrap().submit_command(&format!("echo b{i}")).unwrap();
    }
    wm.poll_all_ptys();
    assert_eq!(wm.registry().history_len(), PANE_OUTBOX_CAP);
    let rows = history_rows(&mut wm, "echo b199");
    assert_eq!(rows.len(), 1);
}

#[test]
fn command_text_is_sanitised_and_capped() {
    let mut wm = WindowManager::new_headless(24, 80);
    let nasty = format!("echo \u{1b}[31m{}\u{202e}end", "x".repeat(2000));
    wm.pane_mut(1).unwrap().submit_command(&nasty).unwrap();
    wm.poll_all_ptys();
    let rows = history_rows(&mut wm, "");
    assert_eq!(rows.len(), 1);
    let RowData::History(h) = &rows[0].data else { panic!() };
    for text in [&h.command, &rows[0].label] {
        assert!(text.chars().count() <= 512);
        assert!(!text.chars().any(|c| c.is_control() || c == '\u{202e}'), "{text:?}");
    }
}

#[test]
fn ssh_pane_records_host_and_never_local_cwd() {
    let (mut wm, tmp) = wm_with_history();
    {
        let pane = wm.pane_mut(1).unwrap();
        pane.set_remote_target(Some("me@box".to_string()));
        pane.set_tracked_cwd(Some("/home/local-user".into()));
        pane.submit_command("uptime").unwrap();
    }
    let first = log_text(tmp.path()).lines().next().unwrap().to_string();
    assert!(first.contains("host=me@box"), "{first}");
    assert!(first.contains("transport=ssh"), "{first}");
    assert!(!first.contains("cwd="), "local cwd leaked: {first}");
    {
        let pane = wm.pane_mut(1).unwrap();
        pane.process_pty_output(b"\x1b]7;file://box/srv/app\x07");
        pane.submit_command("ls").unwrap();
    }
    let log = log_text(tmp.path());
    let second = log.lines().nth(1).unwrap();
    assert!(second.contains("cwd=/srv/app"), "{second}");
    assert!(!log.contains("local-user"));
    wm.poll_all_ptys();
    let rows = history_rows(&mut wm, "");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.host.as_deref() == Some("me@box")));
    let RowData::History(h0) = &rows.iter().find(|r| r.label == "uptime").unwrap().data else {
        panic!()
    };
    assert_eq!(h0.cwd, None);
}

#[test]
fn local_pane_records_tracked_cwd() {
    let (mut wm, tmp) = wm_with_history();
    wm.pane_mut(1).unwrap().set_tracked_cwd(Some("/tmp/work".into()));
    wm.pane_mut(1).unwrap().submit_command("make").unwrap();
    let log = log_text(tmp.path());
    assert!(log.contains("cwd=/tmp/work"), "{log}");
    assert!(!log.contains("host="));
    wm.poll_all_ptys();
    let rows = history_rows(&mut wm, "make");
    let RowData::History(h) = &rows[0].data else { panic!() };
    assert_eq!(h.cwd.as_deref(), Some("/tmp/work"));
    assert_eq!(rows[0].host, None);
}

#[test]
fn exit_code_is_joined_and_annotated_once() {
    let (mut wm, tmp) = wm_with_history();
    wm.pane_mut(1).unwrap().submit_command("false").unwrap();
    wm.pane_mut(1).unwrap().process_pty_output(b"\x1b]133;D;1\x07");
    wm.poll_all_ptys();
    wm.poll_all_ptys();
    let rows = history_rows(&mut wm, "false");
    assert_eq!(rows.len(), 1);
    let RowData::History(h) = &rows[0].data else { panic!() };
    assert_eq!(h.exit_code, Some(1));
    assert!(h.duration_ms.is_some());
    assert!(rows[0].detail.contains("exit 1"), "{}", rows[0].detail);
    let log = log_text(tmp.path());
    assert_eq!(log.lines().filter(|l| l.starts_with("~exit")).count(), 1, "{log}");
    wm.poll_all_ptys();
    assert_eq!(log_text(tmp.path()).lines().filter(|l| l.starts_with("~exit")).count(), 1);
    // The annotated record round-trips for readers.
    let recs = wm.pane(1).unwrap().history_store().load_entries_annotated().unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].exit_code, Some(1));
}

#[test]
fn evicted_block_leaves_exit_unknown_without_panic() {
    let (mut wm, tmp) = wm_with_history();
    wm.pane_mut(1).unwrap().submit_command("old").unwrap();
    {
        let pane = wm.pane_mut(1).unwrap();
        pane.process_pty_output("line\r\n".repeat(200).as_bytes());
        pane.vt_mut().set_scrollback_capacity(5);
        assert!(pane.vt().command_blocks().is_empty(), "block should be evicted");
        pane.process_pty_output(b"\x1b]133;D;3\x07");
    }
    wm.poll_all_ptys();
    wm.poll_all_ptys();
    let rows = history_rows(&mut wm, "old");
    assert_eq!(rows.len(), 1);
    let RowData::History(h) = &rows[0].data else { panic!() };
    assert_eq!(h.exit_code, None);
    assert!(!log_text(tmp.path()).contains("~exit"));
}
