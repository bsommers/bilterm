//! MCP resources (`resources/list`, `resources/read`) and JSON-RPC error codes.

use bilterm::api::{AutomationControlPlane, JsonValue};

fn call(plane: &mut AutomationControlPlane, line: &str) -> JsonValue {
    JsonValue::parse(&plane.handle_mcp_line(line).expect("response")).expect("json")
}

fn err_code(v: &JsonValue) -> i64 {
    v.get("error")
        .and_then(|e| e.get("code"))
        .and_then(JsonValue::as_i64)
        .expect("error code")
}

fn read(plane: &mut AutomationControlPlane, uri: &str) -> JsonValue {
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{{"uri":"{uri}"}}}}"#
    );
    call(plane, &line)
}

fn text_of(resp: &JsonValue) -> JsonValue {
    let Some(JsonValue::Array(contents)) = resp.get("result").and_then(|r| r.get("contents"))
    else {
        panic!("no contents: {}", resp.to_json_string());
    };
    let t = contents[0].get("text").and_then(JsonValue::as_str).unwrap();
    JsonValue::parse(t).expect("body json")
}

#[test]
fn initialize_advertises_resources_and_list_has_all_uris() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let init = call(&mut plane, r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#);
    assert!(
        init.get("result")
            .and_then(|r| r.get("capabilities"))
            .and_then(|c| c.get("resources"))
            .is_some()
    );
    let list = call(&mut plane, r#"{"jsonrpc":"2.0","id":2,"method":"resources/list"}"#);
    let Some(JsonValue::Array(items)) = list.get("result").and_then(|r| r.get("resources"))
    else {
        panic!("no resources")
    };
    let uris: Vec<&str> = items
        .iter()
        .filter_map(|i| i.get("uri").and_then(JsonValue::as_str))
        .collect();
    for u in ["bilterm://state", "bilterm://config", "bilterm://commands", "bilterm://pane/1"] {
        assert!(uris.contains(&u), "missing {u} in {uris:?}");
    }
}

#[test]
fn resources_read_each_uri() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let state = text_of(&read(&mut plane, "bilterm://state"));
    assert!(state.get("workspace").and_then(|w| w.get("panes")).is_some());
    assert!(state.get("appearance_profile").is_some());
    assert!(state.get("display_mode").is_some());
    assert!(state.get("drawer").and_then(|d| d.get("open")).is_some());

    let cfg = text_of(&read(&mut plane, "bilterm://config"));
    assert!(cfg.get("theme").is_some());
    assert!(cfg.get("leader_key").is_some());

    let cmds = text_of(&read(&mut plane, "bilterm://commands"));
    assert!(cmds.get("count").and_then(JsonValue::as_i64).unwrap() > 10);
    let text = cmds.to_json_string();
    // Catalog fields come from the command registry: every entry has a summary,
    // aliases name their canonical command, and examples/shortcuts are exposed.
    assert!(!text.contains("\"summary\":\"\""), "a command has an empty summary");
    assert!(text.contains("\"alias_of\":\":remote\""), "`:fleet` should alias `:remote`");
    assert!(text.contains("\"examples\":[\":split h\""), "`:split` should list its palette examples");

    let pane = text_of(&read(&mut plane, "bilterm://pane/1"));
    assert!(pane.get("lines").is_some() || pane.get("pane_id").is_some());
}

#[test]
fn unknown_resource_and_tool_use_invalid_params_code() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    assert_eq!(err_code(&read(&mut plane, "bilterm://nope")), -32602);
    assert_eq!(err_code(&read(&mut plane, "bilterm://pane/999")), -32602);
    let bad_tool = call(
        &mut plane,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"nonexistent_tool"}}"#,
    );
    assert_eq!(err_code(&bad_tool), -32602);
    let bad_method = call(&mut plane, r#"{"jsonrpc":"2.0","id":4,"method":"bogus/method"}"#);
    assert_eq!(err_code(&bad_method), -32601);
}

#[test]
fn tool_level_failure_sets_is_error() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let r = call(
        &mut plane,
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"bilterm_inspect_pane","arguments":{"pane_id":999}}}"#,
    );
    assert_eq!(
        r.get("result").and_then(|x| x.get("isError")).and_then(JsonValue::as_bool),
        Some(true)
    );
}

#[test]
fn tools_list_only_gains_find() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let r = call(&mut plane, r#"{"jsonrpc":"2.0","id":6,"method":"tools/list"}"#);
    let Some(JsonValue::Array(tools)) = r.get("result").and_then(|x| x.get("tools")) else {
        panic!("no tools")
    };
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(JsonValue::as_str))
        .collect();
    assert_eq!(
        names,
        [
            "bilterm_workspace_status",
            "bilterm_exec_command",
            "bilterm_inspect_pane",
            "bilterm_send_keys",
            "bilterm_colon_command",
            "bilterm_run_playbook",
            "bilterm_remote_connect",
            "bilterm_fleet_exec",
            "bilterm_workspace_fontsize",
            "bilterm_canvas",
            "bilterm_find",
            "bilterm_agents_list",
            "bilterm_agents_demo",
            "bilterm_remote_link",
        ]
    );
}
