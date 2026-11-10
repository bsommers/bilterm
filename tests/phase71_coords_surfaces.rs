//! Phase 71 (UNI-R01): `bilterm_coords_*` MCP tools and `bilterm coords` CLI lower to `coords.*` RPC.

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::cli::{CliEnv, RpcCall, is_cli_command, parse, usage};

fn plane() -> AutomationControlPlane {
    AutomationControlPlane::new_headless(24, 80)
}

fn rpc(p: &mut AutomationControlPlane, m: &str, params: &str) -> Result<JsonValue, String> {
    p.dispatch_method(m, &JsonValue::parse(params).unwrap())
}

fn mcp_text(p: &mut AutomationControlPlane, name: &str, args: &str) -> String {
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{name}","arguments":{args}}}}}"#
    );
    let resp = JsonValue::parse(&p.handle_mcp_line(&line).unwrap()).unwrap();
    resp.get("result").and_then(|r| r.get("content")).and_then(JsonValue::as_array).unwrap()[0]
        .get("text")
        .and_then(JsonValue::as_str)
        .unwrap()
        .to_string()
}

fn argv(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

fn cli(s: &str) -> Result<RpcCall, String> {
    parse(&argv(s), &CliEnv::default())
}

#[test]
fn mcp_tools_listed_and_match_rpc() {
    let mut p = plane();
    let list = p.handle_mcp_line(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
    assert!(list.contains("bilterm_coords_resolve") && list.contains("bilterm_coords_format"));
    let coord = rpc(&mut p, "coords.format", "{}").unwrap();
    let path = coord.get("path").and_then(JsonValue::as_str).unwrap().to_string();
    let args = format!(r#"{{"coord":"{path}"}}"#);
    let text = mcp_text(&mut p, "bilterm_coords_resolve", &args);
    assert_eq!(text, rpc(&mut p, "coords.resolve", &args).unwrap().to_json_string());
    let args = r#"{"style":"verbose"}"#;
    assert_eq!(
        mcp_text(&mut p, "bilterm_coords_format", args),
        rpc(&mut p, "coords.format", args).unwrap().to_json_string()
    );
}

#[test]
fn mcp_bad_coord_is_error() {
    let mut p = plane();
    let text = mcp_text(&mut p, "bilterm_coords_resolve", r#"{"coord":"not a coord!!"}"#);
    assert!(!text.is_empty() && !text.starts_with('{'), "{text}");
    let line = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"bilterm_coords_resolve","arguments":{}}}"#;
    assert!(p.handle_mcp_line(line).unwrap().contains("missing"));
}

#[test]
fn cli_parses_resolve_and_format() {
    assert!(is_cli_command(Some("coords")));
    let c = cli("coords resolve d0:w1:main:y2").unwrap();
    assert_eq!(c.method, "coords.resolve");
    assert_eq!(c.params.get("coord").and_then(JsonValue::as_str), Some("d0:w1:main:y2"));
    let c = cli("coords format").unwrap();
    assert_eq!(c.method, "coords.format");
    assert!(c.params.get("pane_id").is_none() && c.params.get("style").is_none());
    let c = cli("coords format --pane 3 --style verbose").unwrap();
    assert_eq!(c.params.get("pane_id").and_then(JsonValue::as_usize), Some(3));
    assert_eq!(c.params.get("style").and_then(JsonValue::as_str), Some("verbose"));
    assert!(usage().contains("bilterm coords resolve") && usage().contains("bilterm coords format"));
}

#[test]
fn cli_errors() {
    assert!(cli("coords").is_err());
    assert!(cli("coords resolve").is_err());
    assert!(cli("coords resolve not-a-coord!!").unwrap_err().contains("invalid coord"));
    assert!(cli("coords format --pane x").is_err());
    assert!(cli("coords format --style bogus").unwrap_err().contains("unknown style"));
    assert!(cli("coords nope").is_err());
}
