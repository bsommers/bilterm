//! Phase 50.1-13: `find.*` RPC and the `bilterm_find` MCP tool (FND-R04, UNI-R01 API leg).
//! Every row carries `source` (D-U3) and every envelope a numeric `masked` (D-U2).

use std::time::Instant;

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::wm::SplitDirection;

fn plane_with_panes(extra_splits: usize) -> AutomationControlPlane {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    for _ in 0..extra_splits {
        plane.wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    plane.wm.update_agent_states(Instant::now());
    plane
}

fn params(json: &str) -> JsonValue {
    JsonValue::parse(json).expect("valid params json")
}

fn rows_of(v: &JsonValue) -> Vec<JsonValue> {
    v.get("rows").and_then(JsonValue::as_array).expect("rows array").to_vec()
}

fn mcp(plane: &mut AutomationControlPlane, line: &str) -> JsonValue {
    let resp = plane.handle_mcp_line(line).expect("mcp response");
    JsonValue::parse(&resp).expect("valid mcp json")
}

// ----- Task 1: RPC ---------------------------------------------------------------------------

#[test]
fn find_agents_rows_carry_live_source() {
    let mut plane = plane_with_panes(2);
    let v = plane.dispatch_method("find.agents", &params("{}")).unwrap();
    assert_eq!(v.get("kind").and_then(JsonValue::as_str), Some("agent"));
    assert_eq!(v.get("masked").and_then(JsonValue::as_i64), Some(0));
    assert_eq!(v.get("returned").and_then(JsonValue::as_i64), Some(3));
    let rows = rows_of(&v);
    assert_eq!(rows.len(), 3);
    for row in &rows {
        let src = row.get("source").expect("source on every row");
        assert_eq!(src.get("kind").and_then(JsonValue::as_str), Some("live"));
        assert!(!src.get("name").and_then(JsonValue::as_str).unwrap_or("").is_empty());
    }
}

#[test]
fn limit_is_clamped_and_validated() {
    let mut plane = plane_with_panes(2);
    let v = plane.dispatch_method("find.agents", &params(r#"{"limit":0}"#)).unwrap();
    assert_eq!(rows_of(&v).len(), 1, "limit 0 is clamped up to 1");
    let v = plane.dispatch_method("find.agents", &params(r#"{"limit":10000}"#)).unwrap();
    assert!(rows_of(&v).len() <= 200);
    let err = plane.dispatch_method("find.agents", &params(r#"{"limit":"x"}"#)).unwrap_err();
    assert_eq!(err, "invalid 'limit' parameter");
    let err = plane.dispatch_method("find.history", &params(r#"{"limit":-3}"#)).unwrap_err();
    assert_eq!(err, "invalid 'limit' parameter");
    let err = plane.dispatch_method("find.sessions", &params(r#"{"limit":1.5}"#)).unwrap_err();
    assert_eq!(err, "invalid 'limit' parameter");
}

#[test]
fn wm_command_find_returns_same_rows_under_data() {
    let mut plane = plane_with_panes(2);
    let direct = plane.dispatch_method("find.agents", &params("{}")).unwrap();
    let via = plane.dispatch_method("wm.command", &params(r#"{"command":":find agent"}"#)).unwrap();
    let data = via.get("data").expect("data under wm.command");
    assert_eq!(rows_of(data), rows_of(&direct));
    assert!(plane.wm.is_find_visible(), ":find opens the modal");
}

#[test]
fn find_does_not_open_modal_or_move_focus() {
    let mut plane = plane_with_panes(2);
    let focus = plane.wm.active_pane_id();
    for m in ["find.agents", "find.sessions", "find.history"] {
        plane.dispatch_method(m, &params(r#"{"query":"x"}"#)).unwrap();
        assert!(!plane.wm.is_find_visible(), "{m} must not open the modal");
        assert_eq!(plane.wm.active_pane_id(), focus, "{m} must not move focus");
    }
    // Command data from a previous `wm.command` is not consumed or produced by find.*.
    let via = plane.dispatch_method("wm.command", &params(r#"{"command":":theme 3278"}"#)).unwrap();
    assert!(via.get("data").is_none());
}

#[test]
fn find_methods_succeed_on_empty_params() {
    let mut plane = plane_with_panes(0);
    for m in ["find.agents", "find.sessions", "find.history"] {
        let v = plane.dispatch_method(m, &params("{}")).unwrap();
        assert!(v.get("masked").and_then(JsonValue::as_i64).is_some(), "{m} masked is a number");
    }
}

#[test]
fn query_is_capped() {
    let mut plane = plane_with_panes(0);
    let long = "q".repeat(5000);
    let v = plane
        .dispatch_method("find.agents", &params(&format!(r#"{{"query":"{long}"}}"#)))
        .unwrap();
    let q = v.get("query").and_then(JsonValue::as_str).unwrap();
    assert!(q.chars().count() <= 128);
}

// ----- Task 2: MCP and disk sources -------------------------------------------------------------

#[test]
fn mcp_find_equals_direct_dispatch() {
    let mut plane = plane_with_panes(2);
    let direct = plane.dispatch_method("find.agents", &params(r#"{"query":"","limit":5}"#)).unwrap();
    let r = mcp(
        &mut plane,
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_find","arguments":{"kind":"agent","query":"","limit":5}}}"#,
    );
    let result = r.get("result").expect("result");
    assert_eq!(result.get("isError").and_then(JsonValue::as_bool), Some(false));
    let text = result
        .get("content")
        .and_then(JsonValue::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("text"))
        .and_then(JsonValue::as_str)
        .expect("text payload");
    assert_eq!(JsonValue::parse(text).unwrap(), direct);
}

#[test]
fn mcp_find_requires_kind() {
    let mut plane = plane_with_panes(0);
    for args in [r#"{}"#, r#"{"kind":"nope"}"#, r#"{"kind":7}"#] {
        let r = mcp(
            &mut plane,
            &format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"bilterm_find","arguments":{args}}}}}"#
            ),
        );
        let err = r.get("error").unwrap_or_else(|| panic!("expected error for {args}"));
        assert_eq!(err.get("code").and_then(JsonValue::as_i64), Some(-32602));
        let msg = err.get("message").and_then(JsonValue::as_str).unwrap();
        assert!(msg.starts_with("Invalid params: missing 'kind'"), "{msg}");
    }
}

#[test]
fn mcp_find_accepts_session_and_history_kinds() {
    let mut plane = plane_with_panes(0);
    for (kind, want) in [("session", "session"), ("sessions", "session"), ("history", "history")] {
        let r = mcp(
            &mut plane,
            &format!(
                r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"bilterm_find","arguments":{{"kind":"{kind}"}}}}}}"#
            ),
        );
        let text = r
            .get("result")
            .and_then(|x| x.get("content"))
            .and_then(JsonValue::as_array)
            .and_then(|c| c.first())
            .and_then(|c| c.get("text"))
            .and_then(JsonValue::as_str)
            .unwrap_or_else(|| panic!("no payload for {kind}"));
        let v = JsonValue::parse(text).unwrap();
        assert_eq!(v.get("kind").and_then(JsonValue::as_str), Some(want));
    }
}

#[test]
fn tools_list_has_eleven_tools_with_find_last() {
    let mut plane = plane_with_panes(0);
    let r = mcp(&mut plane, r#"{"jsonrpc":"2.0","id":4,"method":"tools/list"}"#);
    let tools = r.get("result").and_then(|x| x.get("tools")).and_then(JsonValue::as_array).unwrap();
    // Phase 51-10 appended `bilterm_agents_list` and `bilterm_agents_demo` after `bilterm_find`.
    assert_eq!(tools.len(), 13);
    let last = &tools[10];
    assert_eq!(last.get("name").and_then(JsonValue::as_str), Some("bilterm_find"));
    let props = last.get("inputSchema").and_then(|s| s.get("properties")).unwrap();
    for p in ["kind", "query", "limit"] {
        assert!(props.get(p).is_some(), "schema property {p}");
    }
}

#[test]
fn rpc_sessions_include_saved_session_source_with_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut plane = plane_with_panes(0);
    plane.wm.config_mut().session_dir = Some(dir.path().to_str().unwrap().to_string());
    plane.wm.save_session("work").unwrap();
    let v = plane.dispatch_method("find.sessions", &params("{}")).unwrap();
    let rows = rows_of(&v);
    let saved = rows
        .iter()
        .position(|r| r.get("source").and_then(|s| s.get("kind")).and_then(JsonValue::as_str) == Some("saved-session"))
        .expect("a saved-session row");
    let src = rows[saved].get("source").unwrap();
    assert_eq!(src.get("name").and_then(JsonValue::as_str), Some("work"));
    assert!(src.get("path").and_then(JsonValue::as_str).unwrap().ends_with("work.toml"));
    let live = rows
        .iter()
        .position(|r| r.get("source").and_then(|s| s.get("kind")).and_then(JsonValue::as_str) == Some("live"))
        .expect("a live row");
    assert!(live < saved, "live rows are listed before saved ones");
}

#[test]
fn rpc_history_masks_secrets_from_history_log() {
    let dir = tempfile::tempdir().unwrap();
    let mut plane = plane_with_panes(0);
    plane.wm.config_mut().history_dir = Some(dir.path().to_str().unwrap().to_string());
    let session = plane.wm.config().history_session.clone();
    let log_dir = dir.path().join("sessions").join(bilterm::pane::history_store::sanitize_session_name(&session));
    std::fs::create_dir_all(&log_dir).unwrap();
    // Real `history.log` line layout: `[ts | session=S | run=R | pane=N] <command>`.
    let line = |cmd: &str| format!("[2026-01-01T00:00:00Z | session={session} | run=old-run | pane=1] {cmd}\n");
    let body = format!("{}{}", line("ls -la"), line("export API_TOKEN=abc123"));
    std::fs::write(log_dir.join("history.log"), body).unwrap();
    let resp = plane.dispatch_method("find.history", &params("{}")).unwrap();
    let text = resp.to_json_string();
    assert!(resp.get("masked").and_then(JsonValue::as_i64).unwrap() >= 1);
    assert!(!text.contains("abc123"), "secret must never appear: {text}");
    // The MCP wrapper must not leak it either.
    let r = plane
        .handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"bilterm_find","arguments":{"kind":"history"}}}"#,
        )
        .unwrap();
    assert!(!r.contains("abc123"));
}
