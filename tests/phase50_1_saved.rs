//! Phase 50.1-11: disk half of the registry (saved sessions, history.log) in tempdirs.
//!
//! `config.session_dir` / `config.history_dir` point at tempdirs, so no test reads or writes the
//! real state directories (and none depends on `XDG_STATE_HOME`).

use std::fs;
use std::path::PathBuf;

use bilterm::api::JsonValue;
use bilterm::colon_command::execute_find::find_results_json;
use bilterm::pane::history_store::{HistoryMeta, HistoryStore, current_run_id};
use bilterm::wm::find_cmd::FindKind;
use bilterm::wm::registry::{
    DiskCache, FIND_HISTORY_READ_CAP, FindRow, Origin, RowData, SAVED_SESSIONS_CAP, saved,
};
use bilterm::wm::{SplitDirection, WindowManager};
use tempfile::TempDir;

struct Fix {
    wm: WindowManager,
    sessions: TempDir,
    history: TempDir,
}

fn fixture() -> Fix {
    let sessions = tempfile::tempdir().unwrap();
    let history = tempfile::tempdir().unwrap();
    let mut wm = WindowManager::new_headless(24, 80);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.config_mut().session_dir = Some(sessions.path().to_string_lossy().into_owned());
    wm.config_mut().history_dir = Some(history.path().to_string_lossy().into_owned());
    Fix { wm, sessions, history }
}

impl Fix {
    fn active(&self) -> String {
        self.wm.config().history_session.clone()
    }

    fn log_path(&self, session: &str) -> PathBuf {
        self.history.path().join("sessions").join(session).join("history.log")
    }

    fn write_log(&self, session: &str, text: &str) -> PathBuf {
        let p = self.log_path(session);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, text).unwrap();
        p
    }

    fn store(&self, run: &str, pane: usize) -> HistoryStore {
        HistoryStore::new(pane, &self.active(), run, self.history.path().to_path_buf(), true, 25)
    }

    fn history_rows(&mut self, query: &str) -> bilterm::wm::registry::FindResults {
        self.wm.run_find(FindKind::History, query, 200)
    }
}

fn json_text(rows: &bilterm::wm::registry::FindResults) -> String {
    format!("{:?}", find_results_json(rows))
}

fn session_rows(rows: &[FindRow]) -> Vec<&FindRow> {
    rows.iter().filter(|r| matches!(r.data, RowData::Session(_))).collect()
}

fn commands(rows: &[FindRow]) -> Vec<String> {
    rows.iter()
        .filter_map(|r| match &r.data {
            RowData::History(h) => Some(h.command.clone()),
            _ => None,
        })
        .collect()
}

fn line(session: &str, run: &str, pane: usize, cmd: &str) -> String {
    format!("[2026-01-01T00:00:00Z | session={session} | run={run} | pane={pane}] {cmd}")
}

#[test]
fn saved_session_row_has_file_source() {
    let mut fx = fixture();
    let path = fx.wm.save_session("work").unwrap();
    assert!(path.ends_with("work.toml"));
    let r = fx.wm.run_find(FindKind::Session, "work", 50);
    let rows = session_rows(&r.rows);
    assert_eq!(rows.len(), 1, "{:?}", r.rows.iter().map(|x| &x.id).collect::<Vec<_>>());
    let row = rows[0];
    assert_eq!(row.source.origin, Origin::SavedSession);
    assert_eq!(row.source.name, "work");
    assert!(row.source.path.as_deref().unwrap().ends_with("work.toml"));
    assert!(json_text(&r).contains("work.toml"));
    let v = find_results_json(&r);
    let JsonValue::Array(jr) = v.get("rows").unwrap() else { panic!("rows") };
    let src = jr[0].get("source").unwrap();
    assert_eq!(src.get("kind").and_then(JsonValue::as_str), Some("saved-session"));
    // Its panes come along as Pane rows tagged with the same source.
    let all = fx.wm.run_find(FindKind::Session, "", 200);
    assert!(all.rows.iter().any(|r| matches!(r.data, RowData::Pane(_)) && r.source.origin == Origin::SavedSession));
}

#[test]
fn history_log_rows_have_log_source_and_meta() {
    let mut fx = fixture();
    let mut st = fx.store("run-old", 2);
    st.record_command_meta("ssh-ok ls -la", &HistoryMeta { host: Some("box1".into()), transport: Some("ssh".into()), cwd: Some("/srv/app dir".into()) })
        .unwrap();
    st.record_command_meta("make test", &HistoryMeta::default()).unwrap();
    st.record_exit("make test", 2).unwrap();
    let r = fx.history_rows("");
    let log = fx.log_path(&fx.active());
    assert_eq!(r.rows.len(), 2, "{:?}", commands(&r.rows));
    for row in &r.rows {
        assert_eq!(row.source.origin, Origin::HistoryLog);
        assert_eq!(row.source.name, fx.active());
        assert_eq!(row.source.path.as_deref(), Some(log.to_string_lossy().as_ref()));
    }
    let by = |c: &str| {
        r.rows
            .iter()
            .find(|x| matches!(&x.data, RowData::History(h) if h.command == c))
            .unwrap_or_else(|| panic!("missing {c}"))
    };
    let RowData::History(a) = &by("ssh-ok ls -la").data else { unreachable!() };
    assert_eq!(a.cwd.as_deref(), Some("/srv/app dir"));
    assert_eq!(by("ssh-ok ls -la").host.as_deref(), Some("box1"));
    let RowData::History(b) = &by("make test").data else { unreachable!() };
    assert_eq!(b.exit_code, Some(2));
    assert!(by("make test").detail.contains("exit 2"), "{}", by("make test").detail);
}

#[test]
fn history_log_secret_lines_are_masked_and_counted() {
    let mut fx = fixture();
    let a = fx.active();
    let text = [
        line(&a, "r1", 1, "echo one"),
        line(&a, "r1", 1, "export API_TOKEN=abc123"),
        line(&a, "r1", 1, "git status"),
        line(&a, "r1", 1, "mysql -u r -phunter2"),
        line(&a, "r1", 1, "cargo build"),
    ]
    .join("\n");
    fx.write_log(&a, &text);
    let r = fx.history_rows("");
    assert_eq!(r.rows.len(), 3, "{:?}", commands(&r.rows));
    assert_eq!(r.masked, 2);
    let js = json_text(&r);
    let rows_dbg = format!("{:?}", r.rows);
    for secret in ["abc123", "hunter2"] {
        assert!(!js.contains(secret), "json leaked {secret}");
        assert!(!rows_dbg.contains(secret), "rows leaked {secret}");
    }
    let mut cache = DiskCache::default();
    let rows = saved::candidates(&fx.wm, FindKind::History, &mut cache);
    assert_eq!(rows.len(), 3);
    assert_eq!(cache.masked(), 2);
    let cache_dbg = format!("{cache:?}");
    for secret in ["abc123", "hunter2"] {
        assert!(!cache_dbg.contains(secret), "cache Debug leaked {secret}");
    }
}

#[test]
fn colon_and_blank_lines_are_skipped_silently() {
    let mut fx = fixture();
    let a = fx.active();
    let text = [
        line(&a, "r1", 1, "echo keep"),
        line(&a, "r1", 1, ":find agent"),
        line(&a, "r1", 1, "   "),
        line(&a, "r1", 1, "  :session list"),
        "garbage line without header".to_string(),
    ]
    .join("\n");
    fx.write_log(&a, &text);
    let r = fx.history_rows("");
    assert_eq!(commands(&r.rows), vec!["echo keep".to_string()]);
    assert_eq!(r.masked, 0, "colon/blank lines are not counted as masked");
}

#[test]
fn current_run_duplicates_of_live_ring_are_skipped() {
    // The ring is filled by plan 50.1-10; the multiset matching itself is unit-tested in
    // `registry::saved::tests::live_set_matches_as_a_multiset`. Here: with nothing in the ring,
    // lines of the current run are all kept (no spurious dedupe), as are lines of other runs.
    let mut fx = fixture();
    let a = fx.active();
    let run = current_run_id();
    let text = [line(&a, &run, 1, "echo live"), line(&a, "other-run", 1, "echo live")].join("\n");
    fx.write_log(&a, &text);
    let r = fx.history_rows("");
    assert_eq!(commands(&r.rows), vec!["echo live".to_string(), "echo live".to_string()]);
    // Pane ids are only trusted for lines of this run.
    let mut panes: Vec<_> = r.rows.iter().map(|x| x.pane).collect();
    panes.sort();
    assert_eq!(panes, vec![None, Some(1)]);
}

#[test]
fn saved_session_pane_history_is_searchable_and_masked() {
    let mut fx = fixture();
    let mut snap = fx.wm.snapshot_session("old");
    snap.panes[0].history = vec![
        "git log --oneline".to_string(),
        "curl -H 'Authorization: Bearer s3cr3tvalue' https://x".to_string(),
        "ls".to_string(),
    ];
    fs::write(fx.sessions.path().join("old.toml"), toml::to_string_pretty(&snap).unwrap()).unwrap();
    let r = fx.history_rows("");
    assert_eq!(r.masked, 1);
    let mut cmds = commands(&r.rows);
    cmds.sort();
    assert_eq!(cmds, vec!["git log --oneline".to_string(), "ls".to_string()]);
    for row in &r.rows {
        assert_eq!(row.source.origin, Origin::SavedSession);
        assert_eq!(row.source.name, "old");
        assert!(row.source.path.as_deref().unwrap().ends_with("old.toml"));
    }
    assert!(!json_text(&r).contains("s3cr3tvalue"));
}

#[test]
fn read_is_bounded() {
    let fx = fixture();
    let a = fx.active();
    let mut text = String::new();
    for i in 0..60_000 {
        text.push_str(&line(&a, "r1", 1, &format!("cmd{i}")));
        text.push('\n');
    }
    fx.write_log(&a, &text);
    let mut cache = DiskCache::default();
    let rows = saved::candidates(&fx.wm, FindKind::History, &mut cache);
    assert_eq!(rows.len(), FIND_HISTORY_READ_CAP);
    let cmds = commands(&rows);
    assert!(cmds.contains(&"cmd59999".to_string()));
    assert!(!cmds.contains(&"cmd0".to_string()));
    assert!(!cmds.contains(&"cmd39999".to_string()), "only the newest {FIND_HISTORY_READ_CAP} lines");
}

#[test]
fn history_only_session_is_listed_with_log_source() {
    let mut fx = fixture();
    let log = fx.write_log("oldwork", &line("oldwork", "r1", 1, "echo hi"));
    let r = fx.wm.run_find(FindKind::Session, "oldwork", 50);
    let rows = session_rows(&r.rows);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source.origin, Origin::HistoryLog);
    assert_eq!(rows[0].source.name, "oldwork");
    assert_eq!(rows[0].source.path.as_deref(), Some(log.to_string_lossy().as_ref()));
    assert_eq!(rows[0].source.label(), "log:oldwork");
}

#[test]
fn session_in_both_places_prefers_saved() {
    let mut fx = fixture();
    fx.wm.save_session("work").unwrap();
    let log = fx.write_log("work", &line("work", "r1", 1, "echo hi"));
    let mut cache = DiskCache::default();
    let raw = saved::candidates(&fx.wm, FindKind::Session, &mut cache);
    let origins: Vec<Origin> = session_rows(&raw).iter().filter(|r| r.session == "work").map(|r| r.source.origin).collect();
    assert!(origins.contains(&Origin::SavedSession) && origins.contains(&Origin::HistoryLog), "{origins:?}");
    let r = fx.wm.run_find(FindKind::Session, "work", 50);
    let rows = session_rows(&r.rows);
    assert_eq!(rows.len(), 1, "merged by session name");
    assert_eq!(rows[0].source.origin, Origin::SavedSession);
    assert_eq!(rows[0].also_in.len(), 1);
    assert_eq!(rows[0].also_in[0].origin, Origin::HistoryLog);
    assert_eq!(rows[0].also_in[0].path.as_deref(), Some(log.to_string_lossy().as_ref()));
    assert!(json_text(&r).contains("history-log"));
}

#[cfg(unix)]
#[test]
fn symlinked_and_oversized_files_are_skipped() {
    let mut fx = fixture();
    std::os::unix::fs::symlink("/etc/passwd", fx.sessions.path().join("evil.toml")).unwrap();
    let big = format!("# {}\n", "x".repeat(3 * 1024 * 1024));
    fs::write(fx.sessions.path().join("big.toml"), big).unwrap();
    fx.wm.save_session("fine").unwrap();
    // A symlinked history directory is not followed either.
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("history.log"), line("x", "r", 1, "echo no")).unwrap();
    fs::create_dir_all(fx.history.path().join("sessions")).unwrap();
    std::os::unix::fs::symlink(outside.path(), fx.history.path().join("sessions").join("linked")).unwrap();
    let r = fx.wm.run_find(FindKind::Session, "", 200);
    let names: Vec<&str> = session_rows(&r.rows).iter().map(|x| x.session.as_str()).collect();
    assert!(names.contains(&"fine"), "{names:?}");
    for bad in ["evil", "big", "linked"] {
        assert!(!names.contains(&bad), "{bad} must be skipped: {names:?}");
    }
}

#[test]
fn unsafe_names_are_rejected() {
    let mut fx = fixture();
    for bad in ["..%2fx", "a b", "semi;colon"] {
        fx.write_log(bad, &line(bad, "r", 1, "echo x"));
        fs::write(fx.sessions.path().join(format!("{bad}.toml")), "name = \"x\"").unwrap();
    }
    fx.write_log("goodname", &line("goodname", "r", 1, "echo x"));
    let r = fx.wm.run_find(FindKind::Session, "", 200);
    let names: Vec<&str> = session_rows(&r.rows).iter().map(|x| x.session.as_str()).collect();
    assert_eq!(names.iter().filter(|n| **n != fx.active()).copied().collect::<Vec<_>>(), vec!["goodname"], "{names:?}");
}

#[test]
fn more_than_64_sessions_are_capped() {
    let mut fx = fixture();
    let snap = fx.wm.snapshot_session("s");
    let text = toml::to_string_pretty(&snap).unwrap();
    for i in 0..100 {
        fs::write(fx.sessions.path().join(format!("s{i:03}.toml")), &text).unwrap();
    }
    let mut cache = DiskCache::default();
    let rows = saved::candidates(&fx.wm, FindKind::Session, &mut cache);
    assert_eq!(session_rows(&rows).len(), SAVED_SESSIONS_CAP);
    let r = fx.wm.run_find(FindKind::Session, "", 200);
    assert!(session_rows(&r.rows).len() <= SAVED_SESSIONS_CAP + 1, "cap plus the live session");
}

#[test]
fn malformed_toml_is_skipped() {
    let mut fx = fixture();
    fs::write(fx.sessions.path().join("broken.toml"), "this is = = not toml [[[").unwrap();
    fs::write(fx.sessions.path().join("wrongshape.toml"), "name = \"x\"\n").unwrap();
    fx.wm.save_session("fine").unwrap();
    let r = fx.wm.run_find(FindKind::Session, "", 200);
    let names: Vec<&str> = session_rows(&r.rows).iter().map(|x| x.session.as_str()).collect();
    assert!(names.contains(&"fine"));
    assert!(!names.contains(&"broken") && !names.contains(&"wrongshape"), "{names:?}");
}

#[test]
fn second_call_uses_cache_without_io() {
    let fx = fixture();
    fx.wm.save_session("work").unwrap();
    let a = fx.active();
    fx.write_log(&a, &format!("{}\n{}", line(&a, "r1", 1, "echo one"), line(&a, "r1", 1, "export TOKEN=zzz")));
    let mut cache = DiskCache::default();
    let s1 = saved::candidates(&fx.wm, FindKind::Session, &mut cache);
    let h1 = saved::candidates(&fx.wm, FindKind::History, &mut cache);
    assert!(!s1.is_empty() && h1.len() == 1 && cache.masked() == 1);
    let Fix { wm, sessions, history } = fx;
    drop(sessions);
    drop(history); // directories are gone now
    let s2 = saved::candidates(&wm, FindKind::Session, &mut cache);
    let h2 = saved::candidates(&wm, FindKind::History, &mut cache);
    assert_eq!(s2, s1);
    assert_eq!(h2, h1);
    assert_eq!(cache.masked(), 1, "masked is not double counted on a cached read");
    cache.invalidate();
    assert!(saved::candidates(&wm, FindKind::History, &mut cache).is_empty());
}

#[test]
fn saved_remote_pane_row_hides_untrusted_cwd() {
    let mut fx = fixture();
    let mut snap = fx.wm.snapshot_session("far");
    snap.panes[0].remote_target = Some("me@box".to_string());
    snap.panes[0].cwd = "/local/not/remote".to_string();
    snap.panes[0].title = "remote\u{1b}[31m".to_string();
    fs::write(fx.sessions.path().join("far.toml"), toml::to_string_pretty(&snap).unwrap()).unwrap();
    let r = fx.wm.run_find(FindKind::Session, "", 200);
    let pane = r
        .rows
        .iter()
        .find(|x| x.session == "far" && matches!(x.data, RowData::Pane(_)))
        .expect("saved pane row");
    let RowData::Pane(p) = &pane.data else { unreachable!() };
    assert_eq!(p.cwd, None);
    assert!(!p.cwd_trusted);
    assert_eq!(pane.host.as_deref(), Some("me@box"));
    assert_eq!(pane.pane, None, "a saved pane id is never a live jump target");
    assert!(!pane.label.chars().any(|c| c.is_control()));
    assert!(!json_text(&r).contains("/local/not/remote"));
}

