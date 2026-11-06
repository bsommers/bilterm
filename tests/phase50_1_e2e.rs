//! Phase 50.1-15: end-to-end proof of D-U2 (secrets), D-U3 (sources), FND-R03 (ssh/tmux), the
//! bounds, and that Find never executes text. Everything runs in tempdirs (`config.session_dir`,
//! `config.history_dir`); no live ssh or tmux is used.
//!
//! Known gap, not tested here (documented in chapter 12): `:ssh-tmux` starts no tmux, so the
//! local shell of a label-only `:ssh-tmux` pane is labelled with the remote host and transport
//! `ssh-tmux` for every command typed in it (`capture_context` keys on `remote_target`).

use std::fs;

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::pane::history_store::{HistoryStore, current_run_id};
use bilterm::tmux::TmuxControlSubprocess;
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::find_cmd::FindKind;
use bilterm::wm::registry::{
    CMD_RING_CAP, DiskCache, FIND_HISTORY_READ_CAP, FIND_RESULTS_CAP, Origin, RowData, saved,
};
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tempfile::TempDir;

const SCREEN: Rect = Rect { x: 0, y: 0, width: 140, height: 44 };

struct Env {
    plane: AutomationControlPlane,
    _sessions: TempDir,
    history: TempDir,
}

fn env() -> Env {
    let sessions = tempfile::tempdir().unwrap();
    let history = tempfile::tempdir().unwrap();
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    plane.wm.split_focused(SplitDirection::Vertical).unwrap();
    plane.wm.resize_all(SCREEN).unwrap();
    plane.wm.config_mut().session_dir = Some(sessions.path().to_string_lossy().into_owned());
    plane.wm.config_mut().history_dir = Some(history.path().to_string_lossy().into_owned());
    Env { plane, _sessions: sessions, history }
}

impl Env {
    fn log_path(&self, session: &str) -> std::path::PathBuf {
        self.history.path().join("sessions").join(session).join("history.log")
    }

    fn write_log(&self, session: &str, text: &str) {
        let p = self.log_path(session);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    /// Makes `pane` record into the tempdir history.log of the active session, in this run.
    fn enable_log(&mut self, pane: usize) {
        let session = self.plane.wm.config().history_session.clone();
        let base = self.history.path().to_path_buf();
        let store: &mut HistoryStore = self.plane.wm.pane_mut(pane).unwrap().history_store_mut();
        store.base_dir = base.clone();
        store.enabled = true;
        store.run_id = current_run_id();
        store.session_file_path = base.join("sessions").join(session).join("history.log");
    }

    fn submit(&mut self, pane: usize, cmd: &str) {
        self.plane.wm.pane_mut(pane).unwrap().submit_command(cmd).unwrap();
    }

    fn rpc(&mut self, method: &str, params: &str) -> JsonValue {
        self.plane.dispatch_method(method, &JsonValue::parse(params).unwrap()).unwrap()
    }

    fn mcp(&mut self, kind: &str, query: &str) -> String {
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"bilterm_find","arguments":{{"kind":"{kind}","query":"{query}"}}}}}}"#
        );
        self.plane.handle_mcp_line(&line).expect("mcp response")
    }

    fn colon(&mut self, cmd: &str) {
        let a: Action = parse_colon_command(cmd).unwrap().unwrap();
        self.plane.wm.apply(a).unwrap();
    }
}

fn frame(wm: &WindowManager) -> String {
    let mut buf = Buffer::empty(SCREEN);
    WorkspaceWidget::new(wm).render(SCREEN, &mut buf);
    (0..SCREEN.height)
        .map(|y| (0..SCREEN.width).map(|x| buf.cell((x, y)).map_or(" ", |c| c.symbol())).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn status(wm: &WindowManager) -> String {
    wm.active_pane().status_message().unwrap_or_default().to_string()
}

fn old_line(session: &str, pane: usize, cmd: &str) -> String {
    format!("[2026-01-01T00:00:00Z | session={session} | run=old-run | pane={pane}] {cmd}\n")
}

fn rows(v: &JsonValue) -> Vec<JsonValue> {
    v.get("rows").and_then(JsonValue::as_array).expect("rows").to_vec()
}

fn str_of<'a>(v: &'a JsonValue, key: &str) -> &'a str {
    v.get(key).and_then(JsonValue::as_str).unwrap_or_default()
}

/// The `(kind, name, path)` triplet of a JSON row's `source`.
fn triplet(row: &JsonValue) -> (String, String, String) {
    let s = row.get("source").expect("source on every row");
    let path = s.get("path").and_then(JsonValue::as_str).unwrap_or("-").to_string();
    (str_of(s, "kind").to_string(), str_of(s, "name").to_string(), path)
}

fn mcp_payload(resp: &str) -> JsonValue {
    let v = JsonValue::parse(resp).unwrap();
    let text = v
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(JsonValue::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("text"))
        .and_then(JsonValue::as_str)
        .unwrap_or_else(|| panic!("no mcp payload: {resp}"));
    JsonValue::parse(text).unwrap()
}

#[test]
fn secrets_never_surface_anywhere() {
    const SECRETS: [&str; 4] = ["s3cr3tvalue", "Hunt3r2", "P4ssw0rd", "Pa55w0rdTyped"];
    let mut e = env();
    e.enable_log(1);
    e.enable_log(2);
    // On disk before the run: a history.log line and a saved-session pane history.
    e.write_log("default", &old_line("default", 1, "sshpass -p P4ssw0rd ssh h"));
    e.plane.wm.pane_mut(2).unwrap().prompt_mut().history.push("sshpass -p P4ssw0rd ssh h".into());
    e.plane.wm.pane_mut(2).unwrap().prompt_mut().history.push("echo saved-ok".into());
    let saved_path = e.plane.wm.save_session("work").unwrap();
    assert!(fs::read_to_string(&saved_path).unwrap().contains("P4ssw0rd"), "fixture: saved file holds the secret");
    // Typed in this run: two secret-looking commands and one harmless one.
    e.submit(1, "export API_TOKEN=s3cr3tvalue");
    e.submit(1, "mysql -u r -pHunt3r2");
    e.submit(1, "echo visible-cmd");
    // A password typed at a `!ECHO` prompt through pane.exec.
    e.plane.wm.pane_mut(1).unwrap().line_discipline_mut().echo_enabled = false;
    assert!(e.plane.wm.pane(1).unwrap().is_password_prompt());
    e.rpc("pane.exec", r#"{"command":"Pa55w0rdTyped","pane_id":1,"allow_echo_lock":true}"#);
    e.plane.wm.poll_all_ptys();

    let mut surfaces: Vec<(String, String)> = Vec::new();
    let mut masked_seen = 0;
    for kind in ["history", "session", "agent"] {
        e.colon(&format!(":find {kind}"));
        surfaces.push((format!("UI cells {kind}"), frame(&e.plane.wm)));
        surfaces.push((format!("status {kind}"), status(&e.plane.wm)));
        surfaces.push((format!("command data {kind}"), e.plane.wm.take_command_data().unwrap_or_default()));
        surfaces.push((format!("results Debug {kind}"), format!("{:?}", e.plane.wm.find_results())));
        if kind == "history" {
            masked_seen = e.plane.wm.find_results().masked;
            assert!(masked_seen >= 4, "export, mysql, log line, saved history: masked={masked_seen}");
            let ui = frame(&e.plane.wm);
            assert!(ui.contains(&format!("{masked_seen} masked")), "UI footer shows the count:\n{ui}");
            assert!(status(&e.plane.wm).contains(&format!(", {masked_seen} masked")), "{}", status(&e.plane.wm));
            let rpc = e.rpc("find.history", "{}");
            assert_eq!(rpc.get("masked").and_then(JsonValue::as_i64), Some(masked_seen as i64), "JSON masked");
            let mcp = e.mcp("history", "");
            assert_eq!(mcp_payload(&mcp).get("masked").and_then(JsonValue::as_i64), Some(masked_seen as i64), "MCP masked");
            // The harmless command is still findable (the filter is not a blanket drop).
            assert!(rows(&rpc).iter().any(|r| str_of(r, "command") == "echo visible-cmd"));
        }
        e.colon(":finder close");
        for method in ["find.agents", "find.sessions", "find.history"] {
            surfaces.push((format!("RPC {method}"), e.rpc(method, r#"{"query":""}"#).to_json_string()));
            surfaces.push((format!("RPC {method} ssh"), e.rpc(method, r#"{"query":"ssh"}"#).to_json_string()));
        }
        surfaces.push((format!("MCP {kind}"), e.mcp(kind, "")));
        surfaces.push((format!("MCP {kind} mysql"), e.mcp(kind, "mysql")));
    }
    surfaces.push(("registry Debug".into(), format!("{:?}", e.plane.wm.registry())));
    // history.log: the run must not have written any secret (the seeded line predates the run and stays on disk).
    let log = fs::read_to_string(e.log_path("default")).unwrap_or_default();
    surfaces.push(("history.log written by this run".into(), log.lines().filter(|l| !l.contains("run=old-run")).collect::<Vec<_>>().join("\n")));
    for (name, text) in &surfaces {
        for secret in SECRETS {
            assert!(!text.contains(secret), "`{secret}` surfaced on {name}");
        }
    }
    assert!(surfaces.iter().any(|(_, t)| t.contains("echo visible-cmd")));
    assert!(log.contains("echo visible-cmd"), "the harmless command is logged");
    assert!(masked_seen >= 4);
}

#[test]
fn sources_ordered_live_saved_log_and_identical_across_surfaces() {
    let mut e = env();
    e.plane.wm.config_mut().history_session = "main".to_string();
    e.plane.wm.save_session("work").unwrap();
    e.write_log("oldwork", &old_line("oldwork", 1, "make old"));
    e.colon(":find session");
    let results = e.plane.wm.find_results().clone();
    let order: Vec<Origin> = results.rows.iter().map(|r| r.source.origin).collect();
    assert!(order.windows(2).all(|w| w[0].priority() <= w[1].priority()), "live, then saved, then log: {order:?}");
    for o in [Origin::Live, Origin::SavedSession, Origin::HistoryLog] {
        assert!(order.contains(&o), "missing {o:?} rows: {order:?}");
    }
    let want: Vec<(String, String, String)> = results
        .rows
        .iter()
        .map(|r| {
            (
                r.source.origin.as_str().to_string(),
                r.source.name.clone(),
                r.source.path.clone().unwrap_or_else(|| "-".to_string()),
            )
        })
        .collect();
    assert!(want.contains(&("live".into(), "main".into(), "-".into())));
    assert!(want.iter().any(|t| t.0 == "saved-session" && t.1 == "work" && t.2.ends_with("work.toml")));
    assert!(want.iter().any(|t| t.0 == "history-log" && t.1 == "oldwork" && t.2.ends_with("history.log")));

    // `:find` data, status counts, RPC and MCP carry the same triplets in the same order.
    let data = JsonValue::parse(&e.plane.wm.take_command_data().unwrap()).unwrap();
    let rpc = e.rpc("find.sessions", r#"{"limit":50}"#);
    let mcp = mcp_payload(&e.mcp("session", ""));
    for (surface, json) in [("`:find` data", &data), ("RPC", &rpc), ("MCP", &mcp)] {
        let got: Vec<_> = rows(json).iter().map(triplet).collect();
        assert_eq!(got, want, "{surface} source triplets differ");
    }
    let count = |k: &str| want.iter().filter(|t| t.0 == k).count();
    let counts = format!("({} live, {} saved, {} log)", count("live"), count("saved-session"), count("history-log"));
    assert!(status(&e.plane.wm).contains(&counts), "{} lacks {counts}", status(&e.plane.wm));

    // UI: every row's tag in the list and its source line in the footer.
    for (i, r) in results.rows.iter().enumerate() {
        e.colon(&format!(":finder select {i}"));
        let ui = frame(&e.plane.wm);
        assert!(ui.contains(&format!("[{}]", r.source.label())), "row {i} tag [{}]:\n{ui}", r.source.label());
        let (kind, name, path) = &want[i];
        let line = if path == "-" { format!("source: {kind} {name}") } else { format!("source: {kind} {name} {path}") };
        assert!(ui.contains(&line), "row {i} source line `{line}`:\n{ui}");
    }
}

#[test]
fn ssh_tmux_end_to_end() {
    let mut e = env();
    let ssh = e.plane.wm.spawn_ssh_pane("me@box", None, vec![]).unwrap();
    let label_only = e.plane.wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    e.plane.wm.attach_tmux_control(TmuxControlSubprocess::from_channels("boot", cmd_tx, line_rx));
    for l in [
        "%session-changed $1 work",
        "%layout-change @2 b25d,160x48,0,0{80x48,0,0,4,79x48,81,0,5}",
        "%window-renamed @2 editor",
        "%output %4 hi",
    ] {
        line_tx.send(l.to_string()).unwrap();
    }
    e.plane.wm.poll_all_ptys();
    let tmux_pane = *e.plane.wm.panes().iter().find(|(_, p)| p.title().starts_with("TMUX %4")).map(|(id, _)| id).unwrap();
    e.submit(ssh, "uptime");
    e.plane.wm.poll_all_ptys();

    let sessions = |e: &mut Env, q: &str| rows(&e.rpc("find.sessions", &format!(r#"{{"query":"{q}"}}"#)));
    let pane_ids = |rs: &[JsonValue]| -> Vec<i64> { rs.iter().filter_map(|r| r.get("pane").and_then(JsonValue::as_i64)).collect() };

    let box_rows = sessions(&mut e, "box");
    let ids = pane_ids(&box_rows);
    assert!(ids.contains(&(ssh as i64)) && ids.contains(&(label_only as i64)), "by host: {ids:?}");
    let label = box_rows.iter().find(|r| r.get("pane").and_then(JsonValue::as_i64) == Some(label_only as i64)).unwrap();
    assert_eq!(label.get("metadata_only").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(str_of(label, "transport"), "ssh-tmux");
    assert_eq!(str_of(label, "tmux_session"), "main");
    let ssh_row = box_rows.iter().find(|r| r.get("pane").and_then(JsonValue::as_i64) == Some(ssh as i64)).unwrap();
    assert_eq!(ssh_row.get("metadata_only").and_then(JsonValue::as_bool), Some(false));
    assert_eq!(str_of(ssh_row, "host"), "me@box");

    for q in ["editor", "work"] {
        let hit = sessions(&mut e, q);
        let t = hit
            .iter()
            .find(|r| r.get("pane").and_then(JsonValue::as_i64) == Some(tmux_pane as i64))
            .unwrap_or_else(|| panic!("tmux pane not found by `{q}`"));
        assert_eq!(str_of(t, "transport"), "tmux");
        assert_eq!(t.get("tmux_pane").and_then(JsonValue::as_i64), Some(4), "by tmux pane id");
        assert_eq!(str_of(t, "tmux_session"), "work");
        assert!(str_of(t, "tmux_window").contains("editor"), "{}", str_of(t, "tmux_window"));
    }

    let hist = rows(&e.rpc("find.history", r#"{"query":"uptime"}"#));
    let up = hist.iter().find(|r| str_of(r, "command") == "uptime").expect("command typed in the SSH pane");
    assert_eq!(str_of(up, "host"), "me@box");
    assert_eq!(up.get("pane").and_then(JsonValue::as_i64), Some(ssh as i64));
    assert_eq!(up.get("source").map(|s| str_of(s, "kind")), Some("live"));

    let agents = rows(&e.rpc("find.agents", "{}"));
    let listed = pane_ids(&agents);
    for id in e.plane.wm.panes().keys() {
        assert!(listed.contains(&(*id as i64)), "`:find agent` lists pane {id}: {listed:?}");
    }
    // By pane id (the agent label carries `#<id>`).
    let by_id = rows(&e.rpc("find.agents", &format!("{{\"query\":\"#{ssh}\"}}")));
    assert_eq!(by_id.first().and_then(|r| r.get("pane")).and_then(JsonValue::as_i64), Some(ssh as i64));
}

#[test]
fn bounds_hold() {
    let mut e = env();
    for i in 0..5000 {
        e.submit(1, &format!("echo n{i}"));
        if i % 50 == 49 {
            e.plane.wm.poll_all_ptys();
        }
    }
    e.plane.wm.poll_all_ptys();
    assert_eq!(e.plane.wm.registry().history_len(), CMD_RING_CAP);
    for kind in [FindKind::Agent, FindKind::Session, FindKind::History] {
        let r = e.plane.wm.run_find(kind, "", 100_000);
        assert!(r.rows.len() <= FIND_RESULTS_CAP, "{kind:?}: {} rows", r.rows.len());
        let method = match kind {
            FindKind::Agent => "find.agents",
            FindKind::Session => "find.sessions",
            FindKind::History => "find.history",
        };
        let j = e.rpc(method, r#"{"limit":100000}"#);
        assert!(rows(&j).len() <= FIND_RESULTS_CAP);
    }
    let total = e.plane.wm.run_find(FindKind::History, "echo", 200);
    assert_eq!(total.rows.len(), FIND_RESULTS_CAP);
    assert!(total.truncated);

    // 100_000 log lines: only the newest FIND_HISTORY_READ_CAP are read.
    let e = env();
    let body: String = (0..100_000).map(|i| old_line("default", 1, &format!("echo l{i}"))).collect();
    e.write_log("default", &body);
    let mut cache = DiskCache::default();
    let disk = saved::candidates(&e.plane.wm, FindKind::History, &mut cache);
    let history: Vec<&str> = disk
        .iter()
        .filter_map(|r| match &r.data {
            RowData::History(h) => Some(h.command.as_str()),
            _ => None,
        })
        .collect();
    assert!(history.len() <= FIND_HISTORY_READ_CAP, "{} rows", history.len());
    assert!(history.contains(&"echo l99999"));
    assert!(!history.contains(&"echo l0"));
    assert!(!history.contains(&format!("echo l{}", 100_000 - FIND_HISTORY_READ_CAP - 1).as_str()));
}

#[test]
fn find_never_executes_text() {
    let mut e = env();
    e.submit(1, "echo prefill-me");
    e.plane.wm.poll_all_ptys();
    let write1 = e.plane.wm.pane(1).unwrap().last_pty_write().to_vec();
    let write2 = e.plane.wm.pane(2).unwrap().last_pty_write().to_vec();
    let history1 = e.plane.wm.pane(1).unwrap().prompt().history.clone();
    let history2 = e.plane.wm.pane(2).unwrap().prompt().history.clone();
    let untouched = |e: &Env| {
        e.plane.wm.pane(1).unwrap().last_pty_write() == write1.as_slice()
            && e.plane.wm.pane(2).unwrap().last_pty_write() == write2.as_slice()
            && e.plane.wm.pane(1).unwrap().prompt().history == history1
            && e.plane.wm.pane(2).unwrap().prompt().history == history2
    };

    // Ctrl+Y: the command lands in the focused pane's prompt, nothing is sent or submitted.
    e.plane.wm.focus_and_raise(2);
    e.colon(":find history prefill");
    assert_eq!(e.plane.wm.find_results().rows.len(), 1);
    e.colon(":finder prefill");
    assert_eq!(e.plane.wm.active_pane_id(), 2);
    assert_eq!(e.plane.wm.active_pane().prompt().text(), "echo prefill-me");
    assert!(untouched(&e), "prefill must not write to a PTY or touch prompt.history");

    // Enter on a live history row jumps to its pane; it does not run the text either.
    e.plane.wm.active_pane_mut().prompt_mut().set_text("");
    e.colon(":find history prefill");
    e.colon(":finder enter");
    assert_eq!(e.plane.wm.active_pane_id(), 1, "jump to the pane the command was typed in");
    assert!(untouched(&e), "Enter must not write to a PTY or touch prompt.history");
}

/// Carry-over: the live ring suppresses the very `history.log` line the same pane just wrote
/// (otherwise every command typed this run would be listed twice, once `live` and once `log`).
#[test]
fn live_ring_suppresses_its_own_history_log_line() {
    let mut e = env();
    e.write_log("default", &old_line("default", 1, "echo old-one"));
    e.enable_log(1);
    e.submit(1, "echo dup-me");
    e.plane.wm.poll_all_ptys();
    let on_disk = fs::read_to_string(e.log_path("default")).unwrap();
    assert_eq!(on_disk.matches("echo dup-me").count(), 1, "the run wrote the line: {on_disk}");

    e.colon(":find history echo");
    let results = e.plane.wm.find_results().clone();
    let labels = |origin: Origin| -> Vec<String> {
        results.rows.iter().filter(|r| r.source.origin == origin).map(|r| r.label.clone()).collect()
    };
    assert_eq!(labels(Origin::Live), vec!["echo dup-me".to_string()], "live ring row");
    assert_eq!(labels(Origin::HistoryLog), vec!["echo old-one".to_string()], "log row of another run only");
    for json in [e.rpc("find.history", r#"{"query":"echo"}"#), mcp_payload(&e.mcp("history", "echo"))] {
        let dup = rows(&json).iter().filter(|r| str_of(r, "command") == "echo dup-me").count();
        assert_eq!(dup, 1, "listed once over RPC/MCP");
    }
}

/// Carry-over: Enter on a row whose source is not `live` never jumps to a pane (a pane id from a
/// log line or a saved layout can name an unrelated live pane of this run).
#[test]
fn activate_on_non_live_rows_never_jumps_to_a_pane() {
    let mut e = env();
    e.plane.wm.config_mut().history_session = "main".to_string();
    e.plane.wm.focus_and_raise(1);
    // A current-run log line naming live pane 2, absent from the ring (so it is a `log` row).
    let run = current_run_id();
    e.write_log(
        "main",
        &format!("[2026-01-01T00:00:00Z | session=main | run={run} | pane=2] echo from-log\n"),
    );
    e.write_log("oldwork", &old_line("oldwork", 2, "make old"));
    e.colon(":find history from-log");
    let row = e.plane.wm.find_results().rows.first().cloned().expect("log row");
    assert_eq!(row.source.origin, Origin::HistoryLog);
    e.colon(":finder enter");
    assert_eq!(e.plane.wm.active_pane_id(), 1, "a log row must not focus pane 2");
    assert_eq!(e.plane.wm.active_pane().prompt().text(), "echo from-log", "text goes to the prompt");
    assert!(e.plane.wm.active_pane().prompt().history.iter().all(|h| h != "echo from-log"), "and is not run");
    assert!(!e.plane.wm.is_find_visible());

    // A history-only session has no layout: Enter keeps the window open and moves nothing.
    e.colon(":find session oldwork");
    let rows_now = e.plane.wm.find_results().rows.clone();
    let idx = rows_now.iter().position(|r| r.source.origin == Origin::HistoryLog).expect("history-only session row");
    e.colon(&format!(":finder select {idx}"));
    e.colon(":finder enter");
    assert!(e.plane.wm.is_find_visible(), "stays open");
    assert!(status(&e.plane.wm).contains("history only"), "{}", status(&e.plane.wm));
    assert_eq!(e.plane.wm.active_pane_id(), 1);
    e.colon(":finder close");

    // Saved rows carry no live pane id; a saved Pane row switches session instead of focusing a pane.
    e.plane.wm.save_session("work").unwrap();
    e.colon(":find session work");
    let saved_panes: Vec<_> = e
        .plane
        .wm
        .find_results()
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.source.origin == Origin::SavedSession && r.source.name == "work" && matches!(r.data, RowData::Pane(_)))
        .map(|(i, r)| (i, r.pane))
        .collect();
    assert!(!saved_panes.is_empty(), "saved session lists its panes");
    assert!(saved_panes.iter().all(|(_, p)| p.is_none()), "saved pane rows hold no live pane id");
    e.colon(&format!(":finder select {}", saved_panes[0].0));
    e.colon(":finder enter");
    assert!(
        status(&e.plane.wm).contains("Switched to session work"),
        "saved pane row switches session: {}",
        status(&e.plane.wm)
    );
}
