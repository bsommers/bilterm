//! Phase 49-07: table-driven API contract suite. The SAME requests are driven through every
//! in-process transport (direct dispatch, JSON-RPC over TCP, HTTP `/rpc`, MCP `tools/call`) and
//! the results must be equivalent. Differences between transports are asserted as they really
//! are (see `KNOWN DIFFERENCES` in the tests) so they stay visible.

use std::sync::{Arc, Once};
use std::time::Duration;

use bilterm::api::{
    serve_http_standalone, serve_jsonrpc_tcp, AutomationControlPlane, JsonValue,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

const TOKEN: &str = "conformance-token-0123456789";
const DENIED: &str =
    "config writes over the API are disabled; set [api] allow_config_write = true in config.toml";

static INIT: Once = Once::new();

/// Isolates every XDG dir in a temp dir once per process (nothing real is ever touched).
fn temp_root() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bilterm-conformance-{}", std::process::id()));
    INIT.call_once(|| {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY-equivalent: runs exactly once, before any plane is created by any test here.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
    dir
}

fn new_plane(tag: &str, allow_write: bool) -> (AutomationControlPlane, std::path::PathBuf) {
    let root = temp_root();
    let snap = root.join(format!("snap-{tag}"));
    std::fs::create_dir_all(&snap).unwrap();
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    plane.snapshot_dir = Some(snap.canonicalize().unwrap());
    plane.wm.config_mut().api.allow_config_write = allow_write;
    (plane, snap)
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Direct,
    Tcp,
    Http,
}

struct Transport {
    kind: Kind,
    plane: Option<AutomationControlPlane>,
    addr: Option<std::net::SocketAddr>,
}

impl Transport {
    async fn new(kind: Kind, plane: AutomationControlPlane) -> Self {
        match kind {
            Kind::Direct => Self { kind, plane: Some(plane), addr: None },
            Kind::Tcp => {
                let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = l.local_addr().unwrap();
                tokio::spawn(serve_jsonrpc_tcp(l, plane, None));
                Self { kind, plane: None, addr: Some(addr) }
            }
            Kind::Http => {
                let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = l.local_addr().unwrap();
                tokio::spawn(serve_http_standalone(l, plane, Arc::from(TOKEN)));
                Self { kind, plane: None, addr: Some(addr) }
            }
        }
    }

    /// Sends one raw JSON-RPC line and returns the raw response body.
    async fn send(&mut self, line: &str) -> String {
        match self.kind {
            Kind::Direct => self.plane.as_mut().unwrap().handle_jsonrpc_line(line),
            Kind::Tcp => {
                let mut s = TcpStream::connect(self.addr.unwrap()).await.unwrap();
                s.write_all(format!("{line}\n").as_bytes()).await.unwrap();
                let mut out = String::new();
                let _ = tokio::time::timeout(
                    Duration::from_secs(5),
                    BufReader::new(s).read_line(&mut out),
                )
                .await;
                out.trim_end().to_string()
            }
            Kind::Http => {
                let addr = self.addr.unwrap();
                let mut s = TcpStream::connect(addr).await.unwrap();
                let req = format!(
                    "POST /rpc HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Length: {}\r\n\r\n{line}",
                    line.len()
                );
                s.write_all(req.as_bytes()).await.unwrap();
                let mut out = Vec::new();
                let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await;
                let text = String::from_utf8_lossy(&out).into_owned();
                // Non-200 answers keep their status line so differences stay visible.
                let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
                if head.starts_with("HTTP/1.1 200") {
                    body.to_string()
                } else {
                    format!("HTTP:{}", head.lines().next().unwrap_or(""))
                }
            }
        }
    }

    async fn call(&mut self, method: &str, params: &str) -> JsonValue {
        let line = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
        let raw = self.send(&line).await;
        JsonValue::parse(&raw).unwrap_or_else(|| panic!("{:?} {method}: not JSON: {raw}", self.kind))
    }
}

fn code(v: &JsonValue) -> Option<i64> {
    v.get("error").and_then(|e| e.get("code")).and_then(JsonValue::as_i64)
}
fn message(v: &JsonValue) -> String {
    v.get("error")
        .and_then(|e| e.get("message"))
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string()
}
fn result(v: &JsonValue) -> &JsonValue {
    v.get("result").unwrap_or_else(|| panic!("no result: {}", v.to_json_string()))
}

type Check = fn(&JsonValue);

struct Case {
    name: &'static str,
    method: &'static str,
    params: &'static str,
    check: Check,
}

fn ok_data<'a>(v: &'a JsonValue, key: &str) -> &'a JsonValue {
    result(v)
        .get("data")
        .and_then(|d| d.get(key))
        .unwrap_or_else(|| panic!("no data.{key}: {}", v.to_json_string()))
}

const KINDS: [Kind; 3] = [Kind::Direct, Kind::Tcp, Kind::Http];

fn cases() -> Vec<Case> {
    vec![
        Case { name: "workspace.status", method: "workspace.status", params: "{}",
            check: |v| { assert!(result(v).get("panes").is_some() || result(v).get("pane_count").is_some(), "{}", v.to_json_string()); } },
        Case { name: "workspace.snapshot (no file)", method: "workspace.snapshot", params: "{}",
            check: |v| { result(v); } },
        Case { name: "workspace.snapshot (confined file)", method: "workspace.snapshot", params: r#"{"ppm_path":"conf.ppm"}"#,
            check: |v| { result(v); } },
        Case { name: "workspace.snapshot (traversal refused)", method: "workspace.snapshot", params: r#"{"ppm_path":"../escape.ppm"}"#,
            check: |v| { assert!(code(v).is_some() && message(v).contains("ppm_path"), "{}", v.to_json_string()); } },
        Case { name: "wm.command :state", method: "wm.command", params: r#"{"command":":state"}"#,
            check: |v| { assert_eq!(ok_data(v, "pane_count").as_usize(), Some(1)); } },
        Case { name: "wm.command :get theme", method: "wm.command", params: r#"{"command":":get theme"}"#,
            check: |v| { assert!(ok_data(v, "value").as_str().is_some()); } },
        Case { name: "wm.command :split v", method: "wm.command", params: r#"{"command":":split v"}"#,
            check: |v| { assert_eq!(result(v).get("status").and_then(JsonValue::as_str), Some("handled"));
                         assert_eq!(result(v).get("pane_count").and_then(JsonValue::as_usize), Some(2)); } },
        Case { name: "wm.command :state after split", method: "wm.command", params: r#"{"command":"state"}"#,
            check: |v| { assert_eq!(ok_data(v, "pane_count").as_usize(), Some(2)); } },
        Case { name: "pane.send_keys active pane", method: "pane.send_keys", params: r#"{"keys":["a","b"]}"#,
            check: |v| { assert_eq!(result(v).get("count").and_then(JsonValue::as_usize), Some(2)); } },
        Case { name: "pane.send_keys bad chord", method: "pane.send_keys", params: r#"{"keys":["Nope+Nope+Nope"]}"#,
            check: |v| { assert!(message(v).contains("unrecognized key chord"), "{}", v.to_json_string()); } },
        Case { name: "pane.read active pane", method: "pane.read", params: "{}",
            check: |v| { assert!(matches!(result(v).get("lines"), Some(JsonValue::Array(a)) if !a.is_empty())); } },
        Case { name: "pane.read unknown pane", method: "pane.read", params: r#"{"pane_id":999}"#,
            check: |v| { assert!(message(v).contains("not found"), "{}", v.to_json_string()); } },
        Case { name: "events.poll", method: "events.poll", params: "{}",
            check: |v| { assert!(matches!(result(v).get("events"), Some(JsonValue::Array(_)))); } },
        Case { name: "unknown method", method: "no.such_method", params: "{}",
            check: |v| { assert!(code(v).is_some() && message(v).contains("Method not found"), "{}", v.to_json_string()); } },
        Case { name: "missing param", method: "wm.command", params: "{}",
            check: |v| { assert!(code(v).is_some() && message(v).contains("missing 'command'"), "{}", v.to_json_string()); } },
        Case { name: "not a colon command", method: "wm.command", params: r#"{"command":"definitely-not-a-command"}"#,
            check: |v| { assert!(message(v).contains("not a valid :colon command"), "{}", v.to_json_string()); } },
        Case { name: "write gate :set theme x", method: "wm.command", params: r#"{"command":":set theme x"}"#,
            check: |v| { assert!(message(v).contains(DENIED), "{}", v.to_json_string()); } },
        Case { name: "write gate :config save", method: "wm.command", params: r#"{"command":":config save"}"#,
            check: |v| { assert!(message(v).contains(DENIED)); } },
        Case { name: "api section never settable", method: "wm.command", params: r#"{"command":":set api.allow_config_write true"}"#,
            check: |v| { assert!(message(v).contains("[api]") || message(v).contains(DENIED), "{}", v.to_json_string()); } },
    ]
}

#[tokio::test]
async fn same_requests_give_identical_responses_on_every_transport() {
    let table = cases();
    let mut per_transport: Vec<(Kind, Vec<String>)> = Vec::new();
    for kind in KINDS {
        let (plane, snap) = new_plane(&format!("{kind:?}"), false);
        let mut t = Transport::new(kind, plane).await;
        let mut out = Vec::new();
        for c in &table {
            let v = t.call(c.method, c.params).await;
            (c.check)(&v);
            out.push(v.to_json_string());
        }
        assert!(snap.join("conf.ppm").exists(), "{kind:?}: confined snapshot not written");
        assert!(!snap.parent().unwrap().join("escape.ppm").exists());
        per_transport.push((kind, out));
    }
    let (_, base) = &per_transport[0];
    for (kind, got) in &per_transport[1..] {
        for (i, c) in table.iter().enumerate() {
            assert_eq!(&got[i], &base[i], "{kind:?} differs from Direct on case '{}'", c.name);
        }
    }
    assert_eq!(table.len(), 19);
}

#[tokio::test]
async fn write_gate_opens_with_allow_config_write_on_every_transport() {
    for kind in KINDS {
        let (plane, _snap) = new_plane(&format!("gate-{kind:?}"), true);
        let mut t = Transport::new(kind, plane).await;
        let v = t.call("wm.command", r#"{"command":":set font_size_pt 21"}"#).await;
        assert!(code(&v).is_none(), "{kind:?}: {}", v.to_json_string());
        let v = t.call("wm.command", r#"{"command":":get font_size_pt"}"#).await;
        assert_eq!(ok_data(&v, "value").to_json_string().trim_matches('"'), "21", "{kind:?}");
        // The [api] section stays immutable even when writes are enabled.
        let v = t.call("wm.command", r#"{"command":":set api.allow_config_write false"}"#).await;
        assert!(message(&v).contains("[api]"), "{kind:?}: {}", v.to_json_string());
    }
}

/// KNOWN DIFFERENCES between transports (asserted so a change is noticed, not hidden):
/// * JSON-RPC (direct/TCP/HTTP) maps every dispatcher failure to -32000; only parse (-32700) and
///   invalid-request (-32600) errors have specific codes. MCP `tools/call` maps `missing ...` to
///   -32602 and unknown tools to -32602, unknown MCP methods to -32601.
/// * HTTP forwards a syntactically bad body to the same handler on 200 (-32700) but answers an
///   empty body with HTTP 400.
#[tokio::test]
async fn error_shapes_per_transport() {
    for kind in KINDS {
        let (plane, _s) = new_plane(&format!("err-{kind:?}"), false);
        let mut t = Transport::new(kind, plane).await;
        let v = JsonValue::parse(&t.send("{not json").await).expect("json error body");
        assert_eq!(code(&v), Some(-32700), "{kind:?}");
        let v = JsonValue::parse(&t.send(r#"{"jsonrpc":"2.0","id":3}"#).await).unwrap();
        assert_eq!(code(&v), Some(-32600), "{kind:?}");
        let v = t.call("nope.nope", "{}").await;
        assert_eq!(code(&v), Some(-32000), "{kind:?}: unknown method code");
        assert_eq!(v.get("id").and_then(JsonValue::as_i64), Some(1));
        let v = t.call("wm.command", "{}").await;
        assert_eq!(code(&v), Some(-32000), "{kind:?}: missing param code");
    }
    // MCP uses the finer-grained codes.
    let (mut plane, _s) = new_plane("err-mcp", false);
    let mut mcp = |line: &str| JsonValue::parse(&plane.handle_mcp_line(line).unwrap()).unwrap();
    let v = mcp(r#"{"jsonrpc":"2.0","id":1,"method":"nope/nope"}"#);
    assert_eq!(code(&v), Some(-32601));
    let v = mcp(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_colon_command","arguments":{}}}"#);
    assert_eq!(code(&v), Some(-32602));
    let v = mcp(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"no_such_tool","arguments":{}}}"#);
    assert_eq!(code(&v), Some(-32602));
    // MCP drops unparseable lines silently (no response), unlike JSON-RPC's -32700.
    assert!(plane.handle_mcp_line("{not json").is_none());
}

#[tokio::test]
async fn http_empty_body_is_400() {
    let (plane, _s) = new_plane("http-empty", false);
    let mut t = Transport::new(Kind::Http, plane).await;
    assert!(t.send("").await.starts_with("HTTP:HTTP/1.1 400"));
}

/// The same `wm.command` yields equivalent results over RPC dispatch, HTTP `/rpc` and MCP.
#[tokio::test]
async fn wm_command_equivalent_over_rpc_http_and_mcp() {
    let cmds = [":state", ":get theme", ":split v", ":zoom", ":set theme x", ":bogus-not-a-command"];
    let (plane, _s) = new_plane("eq-direct", false);
    let mut direct = Transport::new(Kind::Direct, plane).await;
    let (plane, _s) = new_plane("eq-http", false);
    let mut http = Transport::new(Kind::Http, plane).await;
    let (mut mcp_plane, _s) = new_plane("eq-mcp", false);
    for cmd in cmds {
        let params = format!(r#"{{"command":"{cmd}"}}"#);
        let d = direct.call("wm.command", &params).await;
        let h = http.call("wm.command", &params).await;
        assert_eq!(d.to_json_string(), h.to_json_string(), "{cmd}: direct vs http");
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"bilterm_colon_command","arguments":{params}}}}}"#
        );
        let m = JsonValue::parse(&mcp_plane.handle_mcp_line(&line).unwrap()).unwrap();
        let r = m.get("result").expect("mcp result");
        let is_err = r.get("isError").and_then(JsonValue::as_bool).unwrap();
        let text = match r.get("content") {
            Some(JsonValue::Array(a)) => a[0].get("text").and_then(JsonValue::as_str).unwrap().to_string(),
            _ => panic!("no content"),
        };
        match d.get("result") {
            Some(res) => {
                assert!(!is_err, "{cmd}: mcp flagged error: {text}");
                assert_eq!(JsonValue::parse(&text).unwrap().to_json_string(), res.to_json_string(), "{cmd}: rpc vs mcp");
            }
            None => {
                assert!(is_err, "{cmd}: rpc errored but mcp did not");
                assert_eq!(text, message(&d), "{cmd}: error text rpc vs mcp");
            }
        }
    }
}

#[tokio::test]
async fn write_gate_text_identical_over_mcp() {
    let (mut plane, _s) = new_plane("gate-mcp", false);
    let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_colon_command","arguments":{"command":":set theme x"}}}"#;
    let m = JsonValue::parse(&plane.handle_mcp_line(line).unwrap()).unwrap();
    assert!(m.to_json_string().contains(DENIED));
    let line = line.replace(":set theme x", ":set font_size_pt 21");
    let (mut plane, _s) = new_plane("gate-mcp-on", true);
    let m = JsonValue::parse(&plane.handle_mcp_line(&line).unwrap()).unwrap();
    assert_eq!(m.get("result").and_then(|r| r.get("isError")).and_then(JsonValue::as_bool), Some(false));
}

/// Catalog entries (by command name) never run by the sweep, with the reason.
const SKIP_COMMANDS: &[(&str, &str)] = &[
    (":quit", "ends the application"),
    (":exit!", "ends the application"),
    (":close", "removes the pane (destructive)"),
    (":respawn", "restarts pane processes"),
    (":respawn-pane", "restarts pane processes"),
    (":detach", "detaches/ends the session"),
    (":attach", "attaches to a daemon session"),
    (":save", "writes session state"),
    (":session", "session save/restore/new/relaunch write or replace state"),
    (":ssh", "opens a network connection"),
    (":ssh-tmux", "opens a network connection"),
    (":remote", "remote connections"),
    (":record", "writes recordings to disk"),
    (":quake", "spawns/hides a dropdown shell"),
    (":guake", "spawns/hides a dropdown shell"),
    (":profile", "profile save/default write layout files"),
    (":autosave", "writes layout files"),
    (":layout", "layout autosave writes layout files"),
    (":theme", "theme import/config read user files"),
    (":itermcolors", "reads colour files from disk"),
    (":browse", "opens a file browser"),
    (":resume", "opens the saved-session picker over real state"),
    (":sessions", "lists real saved sessions"),
    (":mcp", "console checkpoint/push touch state"),
];

fn skipped(name: &str) -> bool {
    SKIP_COMMANDS.iter().any(|(n, _)| *n == name)
}

/// Every non-alias catalog example that is runnable as-is must return `handled` or a clean error
/// on a headless workspace; none may panic.
#[test]
fn catalog_commands_never_crash_over_wm_command() {
    let _ = temp_root();
    let (mut ran, mut errs, mut skipped_n) = (0usize, 0usize, 0usize);
    let mut failures = Vec::new();
    for info in bilterm::command_registry::catalog() {
        if info.alias_of.is_some() {
            continue;
        }
        for ex in &info.examples {
            if ex.ends_with(' ') || skipped(info.name) {
                skipped_n += 1;
                continue;
            }
            let ex = ex.to_string();
            let r = std::panic::catch_unwind(|| {
                let mut plane = AutomationControlPlane::new_headless(24, 80);
                plane.snapshot_dir = Some(temp_root());
                let params = JsonValue::obj([("command", JsonValue::String(ex.clone()))]);
                plane.dispatch_method("wm.command", &params)
            });
            match r {
                Err(_) => failures.push(format!("{ex}: PANIC")),
                Ok(Ok(v)) => {
                    let st = v.get("status").and_then(JsonValue::as_str).unwrap_or("");
                    if st == "handled" || st == "quit" {
                        ran += 1;
                    } else {
                        failures.push(format!("{ex}: odd status {st:?}"));
                    }
                }
                Ok(Err(e)) => {
                    assert!(!e.is_empty(), "{ex}: empty error");
                    errs += 1;
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(ran > 100, "only {ran} commands handled ({errs} clean errors, {skipped_n} skipped)");
    eprintln!("catalog sweep: {ran} handled, {errs} clean errors, {skipped_n} skipped");
}

/// `bilterm ctl <method>` can address every dispatcher method, and the dispatcher knows each.
#[test]
fn rpc_methods_const_matches_dispatcher_and_ctl_parser() {
    let _ = temp_root();
    // Drift: every `"x.y" =>` arm in dispatch_method is in RPC_METHODS and vice versa.
    let rpc_src = include_str!("../src/api/rpc.rs");
    let rpc_dispatch_src = include_str!("../src/api/rpc_dispatch.rs");
    let combined_src = format!("{rpc_src}\n{rpc_dispatch_src}");
    let body = combined_src
        .split("pub fn dispatch_method")
        .nth(1)
        .expect("dispatch_method in rpc.rs or rpc_dispatch.rs");
    let body = body
        .split("_ => Err(format!(\"Method not found")
        .next()
        .unwrap();
    let mut arms: Vec<String> = Vec::new();
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with('"') && t.contains("=>") {
            let lhs = t.split("=>").next().unwrap();
            for part in lhs.split('|') {
                arms.push(part.trim().trim_matches('"').to_string());
            }
        }
    }
    arms.sort();
    let mut listed: Vec<String> = bilterm::api::RPC_METHODS.iter().map(ToString::to_string).collect();
    listed.sort();
    assert_eq!(arms, listed, "RPC_METHODS drifted from dispatch_method");

    let default_ep = "unix:/tmp/none.sock";
    for m in bilterm::api::RPC_METHODS {
        // Bare method and endpoint+method (+params) forms both parse.
        for args in [vec![m.to_string()], vec!["127.0.0.1:1".into(), m.to_string(), "{}".into()]] {
            let (ep, method, params) = bilterm::api::parse_ctl_args(&args, default_ep);
            assert_eq!(method, *m);
            assert!(matches!(params, JsonValue::Object(_)));
            assert_eq!(ep, if args.len() == 1 { default_ep } else { "127.0.0.1:1" });
        }
        let mut plane = AutomationControlPlane::new_headless(24, 80);
        let params = JsonValue::obj([] as [(&str, JsonValue); 0]);
        if let Err(e) = plane.dispatch_method(m, &params) {
            assert!(!e.starts_with("Method not found"), "{m} is not dispatched");
        }
    }
    let (ep, method, _) = bilterm::api::parse_ctl_args(&[], default_ep);
    assert_eq!((ep.as_str(), method.as_str()), (default_ep, "workspace.status"));
    let (ep, method, p) = bilterm::api::parse_ctl_args(
        &["unix:/x.sock".into(), "wm.command".into(), r#"{"command":":state"}"#.into()],
        default_ep,
    );
    assert_eq!((ep.as_str(), method.as_str()), ("unix:/x.sock", "wm.command"));
    assert_eq!(p.get("command").and_then(JsonValue::as_str), Some(":state"));
}
