//! Phase 51-10: `agents.list` / `agents.demo` named RPC methods and the `bilterm_agents_list` /
//! `bilterm_agents_demo` MCP tools (UNI-R01 API leg). Every named request must equal the colon
//! path (`wm.command ":agents ..."`) and agree over direct dispatch, JSON-RPC TCP and HTTP.

use std::sync::{Arc, Once};
use std::time::Duration;

use bilterm::api::{
    parse_ctl_args, serve_http_standalone, serve_jsonrpc_tcp, AutomationControlPlane, JsonValue,
    RPC_METHODS,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

const TOKEN: &str = "phase51-token-0123456789";
static INIT: Once = Once::new();

fn temp_root() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bilterm-p51-api-{}", std::process::id()));
    INIT.call_once(|| {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY-equivalent: runs once before any plane is created by any test here.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
    dir
}

fn plane() -> AutomationControlPlane {
    let root = temp_root();
    let snap = root.join("snap");
    std::fs::create_dir_all(&snap).unwrap();
    let mut p = AutomationControlPlane::new_headless(24, 80);
    p.snapshot_dir = Some(snap.canonicalize().unwrap());
    p
}

fn j(raw: &str) -> JsonValue {
    JsonValue::parse(raw).unwrap_or_else(|| panic!("bad json {raw}"))
}

fn state(p: &AutomationControlPlane) -> String {
    format!("{:?} {:?}", p.wm.agents_list_summary(), p.wm.agents_demo_summary())
}

/// Comparable core of a `wm.command` / `agents.*` result (data may carry volatile text).
fn core(r: &Result<JsonValue, String>) -> String {
    match r {
        Ok(v) => ["status", "command", "pane_count"]
            .iter()
            .map(|k| v.get(k).map(JsonValue::to_json_string).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("|"),
        Err(e) => format!("ERR {e}"),
    }
}

const OPS: &[(&str, &str)] = &[
    ("open", ""),
    ("close", ""),
    ("toggle", ""),
    ("dismiss", ""),
    ("blur", ""),
    ("select", "down"),
    ("select", "first"),
    ("jump", ""),
    ("zoom", ""),
    ("pin", ""),
    ("unpin", ""),
    ("scale", "110"),
    ("scale", "+10"),
    ("filter", "agents"),
    ("filter", "all"),
    ("dock", ""),
    ("float", ""),
    ("move", "1 0"),
    ("resize", "1 0"),
    ("rect", "2 2 40 10"),
    ("size", "auto"),
    ("scroll", "-3"),
];

#[test]
fn agents_list_matches_colon_path() {
    for (op, args) in OPS {
        let mut named = plane();
        let mut colon = plane();
        let params = if args.is_empty() {
            format!(r#"{{"op":"{op}"}}"#)
        } else {
            format!(r#"{{"op":"{op}","args":"{args}"}}"#)
        };
        let a = named.dispatch_method("agents.list", &j(&params));
        let cmd = format!(":agents list {op} {args}");
        let b = colon.dispatch_method(
            "wm.command",
            &JsonValue::obj([("command", JsonValue::String(cmd.trim_end().to_string()))]),
        );
        assert_eq!(core(&a), core(&b), "op {op} {args}");
        assert_eq!(state(&named), state(&colon), "state after {op} {args}");
    }
}

#[test]
fn agents_list_default_is_data_read() {
    for params in ["{}", r#"{"op":"list"}"#] {
        let mut p = plane();
        let r = p.dispatch_method("agents.list", &j(params)).unwrap();
        assert_eq!(r.get("command").and_then(JsonValue::as_str), Some(":agents list"));
        let al = r.get("agents_list").expect("agents_list object");
        for k in ["active", "pinned", "focused", "docked", "filter", "selected", "row_count", "scale_pct"] {
            assert!(al.get(k).is_some(), "agents_list.{k}");
        }
        assert_eq!(al.get("filter").and_then(JsonValue::as_str), Some("all"));
        assert!(r.get("demo").is_none());
        assert!(p.wm.agents_list_summary().active, "reading the list also opens it (D-03)");
    }
}

#[test]
fn agents_list_rejects_bad_params() {
    let long = "a".repeat(33);
    let nine = format!(r#"{{"op":"scroll","args":{}}}"#, JsonValue::Array(vec![JsonValue::String("1".into()); 9]).to_json_string());
    let bad: Vec<String> = vec![
        r#"{"op":"quit"}"#.into(),
        r#"{"op":7}"#.into(),
        r#"{"op":true}"#.into(),
        r#"{"op":"select","args":["down up"]}"#.into(),
        r#"{"op":"select","args":["down\n:quit"]}"#.into(),
        r#"{"op":"select","args":"down\n:quit"}"#.into(),
        r#"{"op":"select","args":["a;b"]}"#.into(),
        r#"{"op":"select","args":[":quit"]}"#.into(),
        r#"{"op":"select","args":":quit"}"#.into(),
        format!(r#"{{"op":"select","args":["{long}"]}}"#),
        nine,
        r#"{"op":"select","args":[["down"]]}"#.into(),
        r#"{"op":"scale","args":[1.5]}"#.into(),
        r#"{"op":"select","args":[true]}"#.into(),
        r#"{"op":"select","args":[""]}"#.into(),
        r#"{"op":"select","args":{"a":1}}"#.into(),
        r#"{"op":"pin","args":["1","$(x)"]}"#.into(),
        r#"{"args":["1"]}"#.into(),
        r#"[1]"#.into(),
    ];
    for params in &bad {
        let mut p = plane();
        let before = state(&p);
        let panes = p.wm.pane_count();
        let err = p.dispatch_method("agents.list", &j(params)).expect_err(params);
        assert!(err.starts_with("invalid agents.list parameter"), "{params}: {err}");
        assert_eq!(state(&p), before, "no state change for {params}");
        assert_eq!(p.wm.pane_count(), panes);
    }
    // injection payloads never reach a second command.
    for payload in [r#"{"op":"select","args":["1",":quit"]}"#, r#"{"op":"select","args":"1 ; :quit"}"#] {
        let mut p = plane();
        assert!(p.dispatch_method("agents.list", &j(payload)).is_err());
    }
}

#[test]
fn agents_args_accept_string_or_array() {
    let (mut a, mut b, mut c) = (plane(), plane(), plane());
    let ra = a.dispatch_method("agents.list", &j(r#"{"op":"select","args":["down"]}"#));
    let rb = b.dispatch_method("agents.list", &j(r#"{"op":"select","args":"down"}"#));
    assert_eq!(core(&ra), core(&rb));
    assert_eq!(state(&a), state(&b));
    let rc = c.dispatch_method("agents.list", &j(r#"{"op":"scale","args":[110]}"#));
    assert_eq!(core(&rc), "\"handled\"|\":agents list scale 110\"|1");
    assert_eq!(c.wm.agents_list_summary().scale_pct, 110);
}

#[test]
fn agents_demo_start_stop() {
    let mut p = plane();
    p.wm.config_mut().api.allow_config_write = false;
    let panes_before = p.wm.pane_count();
    let r = p.dispatch_method("agents.demo", &j("{}")).unwrap();
    assert_eq!(r.get("command").and_then(JsonValue::as_str), Some(":agents demo"));
    let demo = r.get("demo").expect("demo object");
    assert_eq!(demo.get("running").and_then(JsonValue::as_bool), Some(true), "start reports a running demo");
    let endpoints = demo.get("endpoints").and_then(JsonValue::as_array).expect("endpoints");
    assert!(!endpoints.is_empty(), "the demo seeded endpoints");
    assert!(demo.get("spawned").and_then(JsonValue::as_array).is_some());
    let r = p.dispatch_method("agents.demo", &j(r#"{"stop":true}"#)).unwrap();
    assert_eq!(r.get("command").and_then(JsonValue::as_str), Some(":agents demo stop"));
    let demo = r.get("demo").expect("demo object after stop");
    assert_eq!(demo.get("running").and_then(JsonValue::as_bool), Some(false), "stop clears the demo");
    assert!(demo.get("endpoints").and_then(JsonValue::as_array).is_some_and(|e| e.is_empty()));
    assert!(demo.get("spawned").and_then(JsonValue::as_array).is_some_and(|e| e.is_empty()));
    assert_eq!(p.wm.pane_count(), panes_before, "stop closes exactly the panes the demo spawned");
    // A second stop is a no-op that still reports not running.
    let r = p.dispatch_method("agents.demo", &j(r#"{"stop":true}"#)).unwrap();
    assert_eq!(r.get("demo").and_then(|d| d.get("running")).and_then(JsonValue::as_bool), Some(false));
    let before = state(&p);
    let err = p.dispatch_method("agents.demo", &j(r#"{"stop":"yes"}"#)).unwrap_err();
    assert!(err.starts_with("invalid agents.demo parameter"), "{err}");
    assert_eq!(state(&p), before);
    // parity with the colon path for both forms.
    for (params, cmd) in [("{}", ":agents demo"), (r#"{"stop":true}"#, ":agents demo stop"), (r#"{"stop":false}"#, ":agents demo")] {
        let (mut a, mut b) = (plane(), plane());
        let ra = a.dispatch_method("agents.demo", &j(params));
        let rb = b.dispatch_method("wm.command", &JsonValue::obj([("command", JsonValue::String(cmd.into()))]));
        assert_eq!(core(&ra), core(&rb), "{params}");
        assert_eq!(state(&a), state(&b), "{params}");
    }
}

#[test]
fn rpc_methods_list_includes_new() {
    assert!(RPC_METHODS.contains(&"agents.list"));
    assert!(RPC_METHODS.contains(&"agents.demo"));
}

#[test]
fn ctl_parses_new_methods() {
    let args = vec!["agents.list".to_string(), r#"{"op":"pin"}"#.to_string()];
    let (ep, method, params) = parse_ctl_args(&args, "127.0.0.1:1");
    assert_eq!((ep.as_str(), method.as_str()), ("127.0.0.1:1", "agents.list"));
    assert_eq!(params.get("op").and_then(JsonValue::as_str), Some("pin"));
    let args = vec!["h:9".to_string(), "agents.demo".to_string(), r#"{"stop":true}"#.to_string()];
    let (ep, method, params) = parse_ctl_args(&args, "x:1");
    assert_eq!((ep.as_str(), method.as_str()), ("h:9", "agents.demo"));
    assert_eq!(params.get("stop").and_then(JsonValue::as_bool), Some(true));
}

// ---------------------------------------------------------------- Task 2: MCP, transports, ctl

fn mcp(p: &mut AutomationControlPlane, line: &str) -> JsonValue {
    j(&p.handle_mcp_line(line).expect("mcp response"))
}

fn tools(p: &mut AutomationControlPlane) -> Vec<JsonValue> {
    let r = mcp(p, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
    r.get("result").and_then(|x| x.get("tools")).and_then(JsonValue::as_array).unwrap().to_vec()
}

fn tool_call(p: &mut AutomationControlPlane, name: &str, args: &str) -> JsonValue {
    mcp(p, &format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{name}","arguments":{args}}}}}"#
    ))
}

fn tool_text(r: &JsonValue) -> String {
    r.get("result")
        .and_then(|x| x.get("content"))
        .and_then(JsonValue::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("text"))
        .and_then(JsonValue::as_str)
        .unwrap_or_else(|| panic!("no text: {}", r.to_json_string()))
        .to_string()
}

fn is_error(r: &JsonValue) -> Option<bool> {
    r.get("result").and_then(|x| x.get("isError")).and_then(JsonValue::as_bool)
}

#[test]
fn mcp_lists_agents_tools() {
    let mut p = plane();
    let t = tools(&mut p);
    assert_eq!(t.len(), 29);
    let names: Vec<&str> = t.iter().filter_map(|x| x.get("name").and_then(JsonValue::as_str)).collect();
    assert_eq!(&names[10..13], ["bilterm_find", "bilterm_agents_list", "bilterm_agents_demo"]);
    for (tool, props) in [("bilterm_agents_list", &["op", "args"][..]), ("bilterm_agents_demo", &["stop"][..])] {
        let spec = t.iter().find(|x| x.get("name").and_then(JsonValue::as_str) == Some(tool)).unwrap();
        let schema = spec.get("inputSchema").unwrap();
        assert_eq!(schema.get("type").and_then(JsonValue::as_str), Some("object"));
        for prop in props {
            assert!(schema.get("properties").and_then(|x| x.get(prop)).is_some(), "{tool}.{prop}");
        }
    }
}

#[test]
fn existing_tools_unchanged() {
    let mut p = plane();
    let names: Vec<String> = tools(&mut p)
        .iter()
        .filter_map(|x| x.get("name").and_then(JsonValue::as_str).map(str::to_string))
        .collect();
    assert_eq!(
        &names[..11],
        [
            "bilterm_workspace_status", "bilterm_exec_command", "bilterm_inspect_pane", "bilterm_send_keys",
            "bilterm_colon_command", "bilterm_run_playbook", "bilterm_remote_connect", "bilterm_fleet_exec",
            "bilterm_workspace_fontsize", "bilterm_canvas", "bilterm_find",
        ]
    );
    // the colon tool still reaches the new verbs.
    let r = tool_call(&mut p, "bilterm_colon_command", r#"{"command":":agents demo stop"}"#);
    assert_eq!(is_error(&r), Some(false));
}

#[test]
fn mcp_tools_call_matches_rpc() {
    for (tool, method, args) in [
        ("bilterm_agents_list", "agents.list", r#"{"op":"pin"}"#),
        ("bilterm_agents_list", "agents.list", r#"{"op":"scale","args":"120"}"#),
        ("bilterm_agents_list", "agents.list", r#"{"op":"filter","args":"agents"}"#),
        ("bilterm_agents_demo", "agents.demo", r#"{"stop":true}"#),
        ("bilterm_agents_demo", "agents.demo", r#"{}"#),
    ] {
        let (mut a, mut b) = (plane(), plane());
        let via_mcp = tool_call(&mut a, tool, args);
        assert_eq!(is_error(&via_mcp), Some(false), "{tool} {args}");
        let direct = b.dispatch_method(method, &j(args)).unwrap();
        assert_eq!(
            core(&Ok(j(&tool_text(&via_mcp)))),
            core(&Ok(direct.clone())),
            "{tool} {args}"
        );
        assert_eq!(state(&a), state(&b));
    }
    let mut p = plane();
    let r = tool_call(&mut p, "bilterm_agents_list", r#"{"op":"nope"}"#);
    assert_eq!(is_error(&r), Some(true));
    assert!(tool_text(&r).starts_with("invalid agents.list parameter"));
    let r = tool_call(&mut p, "bilterm_agents_demo", r#"{"stop":"x"}"#);
    assert_eq!(is_error(&r), Some(true));
    assert!(tool_text(&r).starts_with("invalid agents.demo parameter"));
    let r = tool_call(&mut p, "bilterm_agents_list", "[1]");
    assert_eq!(r.get("error").and_then(|e| e.get("code")).and_then(JsonValue::as_i64), Some(-32602));
    let r = tool_call(&mut p, "bilterm_agents_nope", "{}");
    assert_eq!(r.get("error").and_then(|e| e.get("code")).and_then(JsonValue::as_i64), Some(-32602));
}

#[test]
fn ctl_round_trip_for_new_methods() {
    for m in ["agents.list", "agents.demo"] {
        let (_, method, params) = parse_ctl_args(&[m.to_string()], "e:1");
        assert_eq!(method, m);
        let mut p = plane();
        let r = p.dispatch_method(&method, &params);
        assert!(!matches!(&r, Err(e) if e.starts_with("Method not found")), "{m}: {r:?}");
        assert!(r.is_ok(), "{m}: {r:?}");
    }
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Direct,
    Tcp,
    Http,
}

async fn send(kind: Kind, p: AutomationControlPlane, line: &str) -> String {
    match kind {
        Kind::Direct => {
            let mut p = p;
            p.handle_jsonrpc_line(line)
        }
        Kind::Tcp => {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = l.local_addr().unwrap();
            tokio::spawn(serve_jsonrpc_tcp(l, p, None));
            let mut s = TcpStream::connect(addr).await.unwrap();
            s.write_all(format!("{line}\n").as_bytes()).await.unwrap();
            let mut out = String::new();
            let _ = tokio::time::timeout(Duration::from_secs(5), BufReader::new(s).read_line(&mut out)).await;
            out.trim_end().to_string()
        }
        Kind::Http => {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = l.local_addr().unwrap();
            tokio::spawn(serve_http_standalone(l, p, Arc::from(TOKEN)));
            let mut s = TcpStream::connect(addr).await.unwrap();
            let req = format!(
                "POST /rpc HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Length: {}\r\n\r\n{line}",
                line.len()
            );
            s.write_all(req.as_bytes()).await.unwrap();
            let mut out = Vec::new();
            let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await;
            let text = String::from_utf8_lossy(&out).into_owned();
            let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
            if head.starts_with("HTTP/1.1 200") { body.to_string() } else { format!("HTTP:{head}") }
        }
    }
}

#[tokio::test]
async fn transports_agree() {
    let requests = [
        (r#"{"op":"pin"}"#, "agents.list"),
        (r#"{"op":"toggle"}"#, "agents.list"),
        (r#"{"op":"scale","args":[130]}"#, "agents.list"),
        (r#"{"stop":true}"#, "agents.demo"),
        (r#"{"op":"quit"}"#, "agents.list"),
        (r#"{"stop":1}"#, "agents.demo"),
    ];
    for (params, method) in requests {
        let line = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
        let mut seen: Vec<(Kind, String)> = Vec::new();
        for kind in [Kind::Direct, Kind::Tcp, Kind::Http] {
            let raw = send(kind, plane(), &line).await;
            let v = j(&raw);
            let body = match (v.get("result"), v.get("error")) {
                (Some(r), _) => r.to_json_string(),
                (None, Some(e)) => format!("error {}", e.to_json_string()),
                _ => panic!("{kind:?}: {raw}"),
            };
            seen.push((kind, body));
        }
        for (kind, body) in &seen[1..] {
            assert_eq!(body, &seen[0].1, "{method} {params}: {kind:?} differs from Direct");
        }
        if params.contains("quit") || params.contains(r#""stop":1"#) {
            assert!(seen[0].1.starts_with("error"), "{params}: {}", seen[0].1);
            assert!(seen[0].1.contains("invalid agents."), "{params}: {}", seen[0].1);
        }
    }
}
