//! Phase 51.1-11: the `remote.link` RPC method and the `bilterm_remote_link` MCP tool (UNI-R01 API
//! leg), plus honest `remote.connect_*` results. Every named request must equal the colon path
//! (`wm.command ":ssh-tmux ..."`) and agree over direct dispatch, JSON-RPC TCP and HTTP. No ssh or
//! tmux is ever started: the transport is `TmuxControlSubprocess::from_channels`.

use std::sync::{Arc, Once};
use std::time::Duration;

use bilterm::api::{
    parse_ctl_args, serve_http_standalone, serve_jsonrpc_tcp, AutomationControlPlane, JsonValue,
    RPC_METHODS,
};
use bilterm::tmux::TmuxControlSubprocess;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

const TOKEN: &str = "phase511-token-0123456789";
static INIT: Once = Once::new();

fn temp_root() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bilterm-p511-api-{}", std::process::id()));
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

fn link_state(p: &AutomationControlPlane) -> String {
    format!("{:?} panes={}", p.wm.tmux_link_summary(), p.wm.pane_count())
}

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

fn wm_command(p: &mut AutomationControlPlane, cmd: &str) -> Result<JsonValue, String> {
    p.dispatch_method("wm.command", &JsonValue::obj([("command", JsonValue::String(cmd.into()))]))
}

fn s(v: &JsonValue, k: &str) -> Option<String> {
    v.get(k).and_then(JsonValue::as_str).map(str::to_string)
}

// ---------------------------------------------------------------- Task 1: remote.link, connect results

#[test]
fn remote_link_matches_colon_path() {
    for (op, colon) in [
        ("status", ":ssh-tmux info"),
        ("reconnect", ":ssh-tmux reconnect"),
        ("disconnect", ":ssh-tmux disconnect"),
    ] {
        let mut named = plane();
        let mut by_colon = plane();
        for p in [&mut named, &mut by_colon] {
            p.dispatch_method("remote.connect_tmux", &j(r#"{"target":"me@box","session":"main"}"#))
                .unwrap();
        }
        let a = named.dispatch_method("remote.link", &j(&format!(r#"{{"op":"{op}"}}"#)));
        let b = wm_command(&mut by_colon, colon);
        assert_eq!(core(&a), core(&b), "op {op}");
        let link = a.unwrap();
        assert_eq!(
            s(link.get("link").unwrap(), "state").unwrap(),
            named.wm.tmux_link_summary().state,
            "op {op}"
        );
        assert_eq!(link_state(&named), link_state(&by_colon), "state after {op}");
    }
}

#[test]
fn remote_link_default_is_status() {
    for params in ["{}", r#"{"op":"status"}"#] {
        let mut p = plane();
        let r = p.dispatch_method("remote.link", &j(params)).unwrap();
        assert_eq!(s(&r, "command").as_deref(), Some(":ssh-tmux info"));
        assert_eq!(s(r.get("link").unwrap(), "state").as_deref(), Some("idle"));
        assert!(r.get("data").is_some(), "data from the info verb");
    }
    let mut p = plane();
    let r = p.dispatch_method("remote.link", &JsonValue::Null).unwrap();
    assert_eq!(s(&r, "command").as_deref(), Some(":ssh-tmux info"));
}

#[test]
fn remote_link_rejects_bad_params() {
    let long = "a".repeat(40);
    let bad: Vec<String> = vec![
        r#"{"op":"quit"}"#.into(),
        r#"{"op":7}"#.into(),
        r#"{"op":true}"#.into(),
        r#"{"op":["status"]}"#.into(),
        r#"{"op":{"a":1}}"#.into(),
        r#"{"op":null}"#.into(),
        r#"{"op":"info;:quit"}"#.into(),
        r#"{"op":"status;:quit"}"#.into(),
        r#"{"op":"status\n:quit"}"#.into(),
        r#"{"op":":status"}"#.into(),
        r#"{"op":""}"#.into(),
        r#"{"op":"Status"}"#.into(),
        r#"{"op":"status "}"#.into(),
        format!(r#"{{"op":"{long}"}}"#),
        r#"[1]"#.into(),
        r#""status""#.into(),
    ];
    for params in &bad {
        let mut p = plane();
        let before = link_state(&p);
        let err = p.dispatch_method("remote.link", &j(params)).expect_err(params);
        assert!(err.starts_with("invalid remote.link parameter"), "{params}: {err}");
        assert_eq!(link_state(&p), before, "no state change for {params}");
    }
    // An extra top-level string param never reaches a command: the colon string comes from `op`.
    let mut p = plane();
    let r = p.dispatch_method("remote.link", &j(r#"{"op":"status","extra":":quit"}"#)).unwrap();
    assert_eq!(s(&r, "status").as_deref(), Some("handled"));
    assert_eq!(s(&r, "command").as_deref(), Some(":ssh-tmux info"));
}

#[test]
fn connect_tmux_is_honest() {
    let mut p = plane();
    let r = p
        .dispatch_method("remote.connect_tmux", &j(r#"{"target":"me@box","session":"main"}"#))
        .unwrap();
    assert_eq!(s(&r, "status").as_deref(), Some("connecting"));
    assert_eq!(s(&r, "state").as_deref(), Some("connecting"));
    assert!(r.get("pane_id").and_then(JsonValue::as_usize).is_some());
    assert_eq!(s(&r, "target").as_deref(), Some("me@box"));
    assert_eq!(s(&r, "session").as_deref(), Some("main"));
    assert_eq!(s(r.get("link").unwrap(), "dest").as_deref(), Some("me@box"));
    assert!(!r.to_json_string().contains("connected"), "{}", r.to_json_string());
    // The ssh+tmux_session form reports the same honest shape.
    let mut p = plane();
    let r = p
        .dispatch_method("remote.connect_ssh", &j(r#"{"target":"me@box","tmux_session":"main"}"#))
        .unwrap();
    assert_eq!(s(&r, "status").as_deref(), Some("connecting"));
    assert_eq!(s(&r, "state").as_deref(), Some("connecting"));
    assert_eq!(s(&r, "session").as_deref(), Some("main"));
    assert!(!r.to_json_string().contains("connected"), "{}", r.to_json_string());
}

#[test]
fn second_connect_is_an_error() {
    let mut p = plane();
    p.dispatch_method("remote.connect_tmux", &j(r#"{"target":"me@box","session":"main"}"#)).unwrap();
    let panes = p.wm.pane_count();
    let err = p
        .dispatch_method("remote.connect_tmux", &j(r#"{"target":"you@other","session":"x"}"#))
        .unwrap_err();
    assert!(err.contains("already active"), "{err}");
    assert_eq!(p.wm.pane_count(), panes);
    let err = p
        .dispatch_method("remote.connect_ssh", &j(r#"{"target":"you@other","tmux_session":"x"}"#))
        .unwrap_err();
    assert!(err.contains("already active"), "{err}");
    assert_eq!(p.wm.pane_count(), panes);
}

#[test]
fn link_json_keys_are_stable() {
    let mut p = plane();
    let r = p
        .dispatch_method("remote.connect_tmux", &j(r#"{"target":"me@box","session":"main"}"#))
        .unwrap();
    let primary = r.get("pane_id").and_then(JsonValue::as_usize).unwrap();
    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    p.wm.attach_tmux_control(TmuxControlSubprocess::from_channels("main", cmd_tx, line_rx));
    for l in [
        "%begin 1 1 0",
        "%end 1 1 0",
        "%session-changed $1 main",
        "%layout-change @1 b25d,160x48,0,0,3",
        "%output %3 x\\015\\012",
    ] {
        line_tx.send(l.to_string()).unwrap();
    }
    assert!(p.wm.tick_tmux_link(100));
    let r = p.dispatch_method("remote.link", &j("{}")).unwrap();
    let link = r.get("link").unwrap();
    let JsonValue::Object(map) = link else { panic!("link is not an object") };
    let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "attempt", "dest", "last_exit", "max_attempts", "panes", "primary_pane", "retry_in_ms",
            "session", "stale_panes", "state"
        ]
    );
    assert_eq!(s(link, "state").as_deref(), Some("connected"));
    assert_eq!(s(link, "dest").as_deref(), Some("me@box"));
    assert_eq!(s(link, "session").as_deref(), Some("main"));
    assert!(link.get("attempt").and_then(JsonValue::as_i64).is_some());
    assert!(link.get("max_attempts").and_then(JsonValue::as_i64).is_some());
    assert_eq!(link.get("primary_pane").and_then(JsonValue::as_usize), Some(primary));
    assert!(matches!(link.get("retry_in_ms"), Some(JsonValue::Null)));
    assert!(matches!(link.get("last_exit"), Some(JsonValue::Null)));
    assert!(link.get("stale_panes").and_then(JsonValue::as_array).is_some());
    let panes = link.get("panes").and_then(JsonValue::as_array).expect("panes array");
    assert!(!panes.is_empty());
    for pane in panes {
        assert!(pane.get("pane").and_then(JsonValue::as_usize).is_some());
        assert!(pane.get("tmux_pane").and_then(JsonValue::as_usize).is_some());
    }
    let text = r.to_json_string();
    assert!(!text.contains("argv"), "{text}");
}

#[test]
fn rpc_methods_list_includes_remote_link() {
    assert!(RPC_METHODS.contains(&"remote.link"));
    assert_eq!(RPC_METHODS.iter().filter(|m| **m == "remote.link").count(), 1);
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
    mcp(
        p,
        &format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{name}","arguments":{args}}}}}"#
        ),
    )
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
fn mcp_lists_remote_link_tool() {
    let mut p = plane();
    let t = tools(&mut p);
    assert_eq!(t.len(), 14, "13 before Phase 51.1 plus bilterm_remote_link");
    let last = t.last().unwrap();
    assert_eq!(s(last, "name").as_deref(), Some("bilterm_remote_link"));
    let schema = last.get("inputSchema").unwrap();
    assert_eq!(s(schema, "type").as_deref(), Some("object"));
    let op = schema.get("properties").and_then(|x| x.get("op")).expect("op property");
    assert_eq!(s(op, "type").as_deref(), Some("string"));
    let desc = s(op, "description").unwrap();
    for word in ["status", "reconnect", "disconnect"] {
        assert!(desc.contains(word), "{desc}");
    }
}

#[test]
fn mcp_tools_call_matches_rpc() {
    for args in [r#"{"op":"status"}"#, r#"{"op":"reconnect"}"#, r#"{"op":"disconnect"}"#, r#"{}"#] {
        let (mut a, mut b) = (plane(), plane());
        let via_mcp = tool_call(&mut a, "bilterm_remote_link", args);
        assert_eq!(is_error(&via_mcp), Some(false), "{args}");
        let direct = b.dispatch_method("remote.link", &j(args)).unwrap();
        assert_eq!(tool_text(&via_mcp), direct.to_json_string(), "{args}");
    }
    let mut p = plane();
    let r = tool_call(&mut p, "bilterm_remote_link", r#"{"op":"nope"}"#);
    assert_eq!(is_error(&r), Some(true));
    assert!(tool_text(&r).starts_with("invalid remote.link parameter"));
    let r = tool_call(&mut p, "bilterm_remote_link", "[1]");
    assert_eq!(r.get("error").and_then(|e| e.get("code")).and_then(JsonValue::as_i64), Some(-32602));
    let r = tool_call(&mut p, "bilterm_remote_nope", "{}");
    assert_eq!(r.get("error").and_then(|e| e.get("code")).and_then(JsonValue::as_i64), Some(-32602));
}

#[test]
fn ctl_round_trip() {
    let (_, method, params) =
        parse_ctl_args(&["remote.link".to_string(), r#"{"op":"status"}"#.to_string()], "e:1");
    assert_eq!(method, "remote.link");
    assert_eq!(params.get("op").and_then(JsonValue::as_str), Some("status"));
    let mut p = plane();
    let r = p.dispatch_method(&method, &params);
    assert!(!matches!(&r, Err(e) if e.starts_with("Method not found")), "{r:?}");
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn existing_tools_unchanged() {
    let mut p = plane();
    let names: Vec<String> =
        tools(&mut p).iter().filter_map(|x| s(x, "name")).collect();
    assert_eq!(
        &names[..13],
        [
            "bilterm_workspace_status", "bilterm_exec_command", "bilterm_inspect_pane", "bilterm_send_keys",
            "bilterm_colon_command", "bilterm_run_playbook", "bilterm_remote_connect", "bilterm_fleet_exec",
            "bilterm_workspace_fontsize", "bilterm_canvas", "bilterm_find", "bilterm_agents_list",
            "bilterm_agents_demo",
        ]
    );
    // bilterm_remote_connect still maps to remote.connect_ssh (missing target -> -32602).
    let r = tool_call(&mut p, "bilterm_remote_connect", "{}");
    assert_eq!(r.get("error").and_then(|e| e.get("code")).and_then(JsonValue::as_i64), Some(-32602));
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
    for params in [r#"{}"#, r#"{"op":"status"}"#, r#"{"op":"reconnect"}"#, r#"{"op":"disconnect"}"#, r#"{"op":"quit"}"#, r#"{"op":5}"#]
    {
        let line = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"remote.link","params":{params}}}"#);
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
            assert_eq!(body, &seen[0].1, "{params}: {kind:?} differs from Direct");
        }
        if params.contains("quit") || params.contains(r#""op":5"#) {
            assert!(seen[0].1.starts_with("error"), "{params}: {}", seen[0].1);
            assert!(seen[0].1.contains("invalid remote.link parameter"), "{params}: {}", seen[0].1);
        }
    }
}
