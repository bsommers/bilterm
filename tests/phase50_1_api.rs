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

