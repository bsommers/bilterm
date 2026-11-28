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
            "bilterm_agent_mail",
            "bilterm_window_manage",
            "bilterm_session_map",
            "bilterm_daemon",
            "bilterm_dos",
            "bilterm_editor",
            "bilterm_web_surface",
            "bilterm_drawer_preview",
            "bilterm_clipboard",
            "bilterm_traffic",
            "bilterm_voice",
            "bilterm_speech",
            "bilterm_browser",
            "bilterm_video",
            "bilterm_keys",
            "bilterm_agent_control",
            "bilterm_repaint",
            "bilterm_coords_resolve",
            "bilterm_coords_format",
            "bilterm_session_find",
        ]
    );
}

const MAX: usize = bilterm::api::mcp_resources::MAX_MCP_ARTIFACT_BYTES;
const PAGE: usize = bilterm::api::mcp_resources::MCP_LIST_PAGE_SIZE;

fn list_page(plane: &mut AutomationControlPlane, cursor: Option<&str>) -> JsonValue {
    let params = cursor.map_or(String::new(), |c| format!(r#","params":{{"cursor":"{c}"}}"#));
    let line = format!(r#"{{"jsonrpc":"2.0","id":20,"method":"resources/list"{params}}}"#);
    call(plane, &line)
}

fn page_items(resp: &JsonValue) -> Vec<JsonValue> {
    match resp.get("result").and_then(|r| r.get("resources")) {
        Some(JsonValue::Array(a)) => a.clone(),
        _ => panic!("no resources: {}", resp.to_json_string()),
    }
}

fn uris_of(items: &[JsonValue]) -> Vec<String> {
    items.iter().filter_map(|i| i.get("uri").and_then(JsonValue::as_str).map(String::from)).collect()
}

fn next_cursor(resp: &JsonValue) -> Option<String> {
    resp.get("result").and_then(|r| r.get("nextCursor")).and_then(JsonValue::as_str).map(String::from)
}

#[test]
fn context_list_exposes_artifact_prefix_only() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    plane.wm.context_bus().set("artifact:plan", "# Plan", None).unwrap();
    plane.wm.context_bus().set("secret:token", "do-not-expose", None).unwrap();
    let uris = uris_of(&page_items(&list_page(&mut plane, None)));
    assert!(uris.contains(&"bilterm://context/artifact:plan".to_string()), "{uris:?}");
    assert!(!uris.iter().any(|u| u.contains("secret")), "{uris:?}");
}

#[test]
fn context_read_returns_markdown_labeled_untrusted() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    plane.wm.context_bus().set("artifact:plan", "# Plan\n- [ ] Task 1", None).unwrap();
    let resp = read(&mut plane, "bilterm://context/artifact:plan");
    let Some(JsonValue::Array(contents)) = resp.get("result").and_then(|r| r.get("contents")) else {
        panic!("no contents: {}", resp.to_json_string());
    };
    assert_eq!(contents[0].get("mimeType").and_then(JsonValue::as_str), Some("text/markdown"));
    assert_eq!(contents[0].get("text").and_then(JsonValue::as_str), Some("# Plan\n- [ ] Task 1"));
    assert_eq!(contents[0].get("_meta").and_then(|m| m.get("untrusted")).and_then(JsonValue::as_bool), Some(true));
    let items = page_items(&list_page(&mut plane, None));
    let desc = items
        .iter()
        .find(|i| i.get("uri").and_then(JsonValue::as_str) == Some("bilterm://context/artifact:plan"))
        .and_then(|i| i.get("description").and_then(JsonValue::as_str))
        .unwrap()
        .to_string();
    assert!(desc.contains("Untrusted"), "{desc}");
}

#[test]
fn context_read_refuses_non_artifact_and_lookalike_keys() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    for k in ["secret:token", "artifactx:foo", "Artifact:foo", "artifact", ""] {
        plane.wm.context_bus().set(if k.is_empty() { "other" } else { k }, "v", None).unwrap();
        assert_eq!(err_code(&read(&mut plane, &format!("bilterm://context/{k}"))), -32602, "{k}");
    }
    // Empty artifact name and a non-percent-encoded bare prefix are refused.
    assert_eq!(err_code(&read(&mut plane, "bilterm://context/artifact:")), -32602);
    assert_eq!(err_code(&read(&mut plane, "bilterm://context/")), -32602);
}

#[test]
fn context_read_unknown_and_expired_is_not_found() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    assert_eq!(err_code(&read(&mut plane, "bilterm://context/artifact:missing")), -32002);
    plane.wm.context_bus().set_with_ttl("artifact:soon", "x", None, 1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(err_code(&read(&mut plane, "bilterm://context/artifact:soon")), -32002);
}

#[test]
fn context_cap_boundary_exact_and_over() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let exact = "x".repeat(MAX);
    plane.wm.context_bus().set("artifact:exact", &exact, None).unwrap();
    let resp = read(&mut plane, "bilterm://context/artifact:exact");
    let Some(JsonValue::Array(c)) = resp.get("result").and_then(|r| r.get("contents")) else {
        panic!("exact-size artifact must be readable");
    };
    assert_eq!(c[0].get("text").and_then(JsonValue::as_str).map(str::len), Some(MAX));
    // The bus refuses anything larger, so no listed artifact can exceed the read cap.
    assert!(plane.wm.context_bus().set("artifact:over", &"x".repeat(MAX + 1), None).is_err());
    assert_eq!(MAX, bilterm::wm::context_bus::MAX_VALUE_BYTES);
}

#[test]
fn context_list_is_paginated_without_duplicates() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let total = PAGE * 2 + 7;
    let big = "y".repeat(MAX);
    for i in 0..total {
        plane.wm.context_bus().set(&format!("artifact:n{i:04}"), &big, None).unwrap();
    }
    let mut seen = Vec::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let resp = list_page(&mut plane, cursor.as_deref());
        let items = page_items(&resp);
        seen.extend(uris_of(&items).into_iter().filter(|u| u.starts_with("bilterm://context/artifact:")));
        pages += 1;
        // Listing never embeds artifact bodies.
        assert!(resp.to_json_string().len() < 64 * 1024, "page too large");
        match next_cursor(&resp) {
            Some(c) => cursor = Some(c),
            None => break,
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(seen.len(), total);
    let mut dedup = seen.clone();
    dedup.sort();
    dedup.dedup();
    assert_eq!(dedup.len(), total, "duplicates across pages");
    // Static resources appear on the first page only.
    let first_cursor = next_cursor(&list_page(&mut plane, None));
    let second = page_items(&list_page(&mut plane, first_cursor.as_deref()));
    assert!(!uris_of(&second).contains(&"bilterm://state".to_string()));
    assert_eq!(err_code(&list_page(&mut plane, Some("bogus"))), -32602);
}

#[test]
fn context_every_listed_uri_reads_back_including_odd_keys() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let odd = "artifact:a b?c#d%e\u{e9}";
    plane.wm.context_bus().set(odd, "odd body", None).unwrap();
    let uris: Vec<String> = uris_of(&page_items(&list_page(&mut plane, None)))
        .into_iter()
        .filter(|u| u.starts_with("bilterm://context/"))
        .collect();
    assert_eq!(uris.len(), 1);
    assert!(!uris[0].contains([' ', '?', '#']), "{}", uris[0]);
    let resp = read(&mut plane, &uris[0]);
    let Some(JsonValue::Array(c)) = resp.get("result").and_then(|r| r.get("contents")) else {
        panic!("not readable: {}", resp.to_json_string());
    };
    assert_eq!(c[0].get("text").and_then(JsonValue::as_str), Some("odd body"));
    // Malformed escapes are rejected strictly.
    for bad in ["artifact:a%", "artifact:a%2", "artifact:a%zz", "artifact:a%ff"] {
        assert_eq!(err_code(&read(&mut plane, &format!("bilterm://context/{bad}"))), -32602, "{bad}");
    }
}

#[test]
fn context_name_label_is_sanitised() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    plane.wm.context_bus().set("artifact:ok\u{202e}evil\u{2066}x", "v", None).unwrap();
    let items = page_items(&list_page(&mut plane, None));
    let name = items
        .iter()
        .filter(|i| i.get("uri").and_then(JsonValue::as_str).is_some_and(|u| u.starts_with("bilterm://context/")))
        .find_map(|i| i.get("name").and_then(JsonValue::as_str))
        .unwrap();
    assert_eq!(name, "Shared Artifact 'okevilx'");
}
