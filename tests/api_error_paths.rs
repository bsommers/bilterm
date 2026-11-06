//! Error-path coverage for the automation API: HTTP transport, JSON-RPC handlers, MCP tools,
//! snapshot/session confinement and the JSON parser.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use bilterm::api::{serve_http_standalone, AutomationControlPlane, JsonValue};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const TOKEN: &str = "test-token-0123456789";

/// One temp state dir for the whole test binary (session.save / snapshots never touch real state).
fn state_dir() -> &'static std::path::PathBuf {
    static DIR: OnceLock<std::path::PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("bilterm-errpaths-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        // SAFETY: set exactly once, before any test reads it, via this OnceLock.
        unsafe {
            std::env::set_var("XDG_STATE_HOME", &d);
        }
        d.clone()
    })
}

fn plane() -> AutomationControlPlane {
    state_dir();
    AutomationControlPlane::new_headless(24, 80)
}

// ---------------------------------------------------------------- HTTP

async fn start() -> std::net::SocketAddr {
    state_dir();
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(serve_http_standalone(
        l,
        AutomationControlPlane::new_headless(24, 80),
        Arc::from(TOKEN),
    ));
    addr
}

async fn raw(addr: std::net::SocketAddr, req: &[u8]) -> String {
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(req).await.unwrap();
    let mut out = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await;
    String::from_utf8_lossy(&out).into_owned()
}

fn status(resp: &str) -> u16 {
    resp.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0)
}

fn body_of(resp: &str) -> &str {
    resp.split("\r\n\r\n").nth(1).unwrap_or("")
}

fn req(addr: std::net::SocketAddr, method: &str, path: &str) -> Vec<u8> {
    format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes()
}

#[tokio::test]
async fn wrong_method_is_405_and_unknown_route_is_404() {
    let addr = start().await;
    for (m, p) in [
        ("PUT", "/rpc"),
        ("DELETE", "/rpc"),
        ("GET", "/rpc"),
        ("POST", "/health"),
        ("POST", "/events"),
        ("PATCH", "/health"),
    ] {
        let r = raw(addr, &req(addr, m, p)).await;
        assert_eq!(status(&r), 405, "{m} {p} -> {r}");
        assert!(body_of(&r).contains("method not allowed"), "{r}");
    }
    for (m, p) in [("GET", "/"), ("GET", "/nope"), ("POST", "/rpc/extra"), ("GET", "/healthz")] {
        let r = raw(addr, &req(addr, m, p)).await;
        assert_eq!(status(&r), 404, "{m} {p} -> {r}");
        assert!(body_of(&r).contains("not found"));
    }
    // A query string is ignored for routing.
    assert_eq!(status(&raw(addr, &req(addr, "GET", "/health?x=1")).await), 200);
}

#[tokio::test]
async fn missing_host_is_403() {
    let addr = start().await;
    let r = raw(
        addr,
        format!("GET /health HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\n\r\n").as_bytes(),
    )
    .await;
    assert_eq!(status(&r), 403, "{r}");
    assert!(body_of(&r).contains("missing Host header"), "{r}");
}

#[tokio::test]
async fn authorization_variants_are_401_with_challenge() {
    let addr = start().await;
    let short = &TOKEN[..TOKEN.len() - 1];
    let long = format!("{TOKEN}x");
    let cases = [
        "Authorization: Basic dGVzdDp0ZXN0".to_string(),
        "Authorization: Bearer".to_string(),
        "Authorization: Bearer ".to_string(),
        "Authorization: ".to_string(),
        format!("Authorization: {TOKEN}"),
        format!("Authorization: Bearer {short}"),
        format!("Authorization: Bearer {long}"),
        format!("Authorization: Bearer {}", "A".repeat(TOKEN.len())),
    ];
    for c in cases {
        let r = raw(addr, format!("GET /health HTTP/1.1\r\nHost: {addr}\r\n{c}\r\n\r\n").as_bytes()).await;
        assert_eq!(status(&r), 401, "{c:?} -> {r}");
        assert!(r.contains("WWW-Authenticate: Bearer"), "{r}");
    }
    // Scheme is case-insensitive.
    let ok = raw(
        addr,
        format!("GET /health HTTP/1.1\r\nHost: {addr}\r\nAuthorization: bEaReR {TOKEN}\r\n\r\n").as_bytes(),
    )
    .await;
    assert_eq!(status(&ok), 200);
}

#[tokio::test]
async fn framing_errors_have_specific_messages() {
    let addr = start().await;
    let a = format!("Host: {addr}\r\nAuthorization: Bearer {TOKEN}\r\n");
    let cases: Vec<(String, u16, &str)> = vec![
        (format!("POST /rpc HTTP/1.1\r\n{a}Content-Length: 1\r\nContent-Length: 1\r\n\r\nx"), 400, "duplicate content-length"),
        (format!("POST /rpc HTTP/1.1\r\n{a}Content-Length: 1\r\ncontent-length: 2\r\n\r\nx"), 400, "duplicate content-length"),
        (format!("POST /rpc HTTP/1.1\r\n{a}Transfer-Encoding: chunked\r\n\r\n0\r\n\r\n"), 400, "transfer-encoding is not supported"),
        (format!("POST /rpc HTTP/1.1\r\n{a}Content-Length: +5\r\n\r\nhello"), 400, "invalid content-length"),
        (format!("POST /rpc HTTP/1.1\r\n{a}Content-Length: \r\n\r\n"), 400, "invalid content-length"),
        (format!("POST /rpc HTTP/1.1\r\n{a}Content-Length: 99999999999999999999999\r\n\r\n"), 413, "body too large"),
        (format!("POST /rpc HTTP/1.1\r\n{a}Content-Length: 2\r\n\r\nabcdef"), 400, "unexpected data after body"),
        (format!("get /health HTTP/1.1\r\n{a}\r\n"), 400, "malformed request line"),
        (format!("GET health HTTP/1.1\r\n{a}\r\n"), 400, "malformed request line"),
        (format!("GET /a b HTTP/1.1\r\n{a}\r\n"), 400, "malformed request line"),
        (format!("GET /health HTTP/2\r\n{a}\r\n"), 400, "malformed request line"),
        (format!("GET /health HTTP/1.1\r\n{a}Bad Name: v\r\n\r\n"), 400, "malformed header name"),
        (format!("GET /health HTTP/1.1\r\n{a}: v\r\n\r\n"), 400, "malformed header name"),
        (format!("GET /health HTTP/1.1\r\n{a}nocolon\r\n\r\n"), 400, "malformed header"),
    ];
    for (bytes, code, msg) in cases {
        let r = raw(addr, bytes.as_bytes()).await;
        assert_eq!(status(&r), code, "{bytes:?} -> {r}");
        assert!(body_of(&r).contains(msg), "{bytes:?} expected {msg:?} in {r}");
    }
    // Invalid UTF-8 in the header block.
    let mut bad = format!("GET /health HTTP/1.1\r\n{a}X: ").into_bytes();
    bad.extend_from_slice(&[0xff, 0xfe]);
    bad.extend_from_slice(b"\r\n\r\n");
    let r = raw(addr, &bad).await;
    assert_eq!(status(&r), 400);
    assert!(body_of(&r).contains("invalid header encoding"), "{r}");
}

#[tokio::test]
async fn truncated_request_or_body_is_400() {
    let addr = start().await;
    // Client closes after a partial head.
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(b"GET /health HTTP/1.1\r\nHost: x").await.unwrap();
    s.shutdown().await.unwrap();
    let mut out = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await;
    let r = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(status(&r), 400, "{r}");
    assert!(body_of(&r).contains("incomplete request"));
    // Declared body longer than what is sent.
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(format!("POST /rpc HTTP/1.1\r\nHost: {addr}\r\nContent-Length: 10\r\n\r\nabc").as_bytes())
        .await
        .unwrap();
    s.shutdown().await.unwrap();
    let mut out = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await;
    let r = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(status(&r), 400, "{r}");
    assert!(body_of(&r).contains("incomplete body"));
}

#[tokio::test]
async fn header_limits_431() {
    let addr = start().await;
    // >16 KiB with the terminator present in the same write.
    let big = format!("GET /health HTTP/1.1\r\nHost: {addr}\r\nX: {}\r\n\r\n", "a".repeat(17_000));
    let r = raw(addr, big.as_bytes()).await;
    assert_eq!(status(&r), 431, "{r}");
    assert!(body_of(&r).contains("header section too large"));
    // >16 KiB and never terminated.
    let r = raw(addr, format!("GET /health HTTP/1.1\r\nX: {}", "b".repeat(20_000)).as_bytes()).await;
    assert_eq!(status(&r), 431, "{r}");
    // 65 headers is refused, 64 (Host + Authorization + 62) is accepted.
    let many = |n: usize| {
        let mut s = format!("GET /health HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {TOKEN}\r\n");
        for i in 0..n {
            s.push_str(&format!("X-{i}: v\r\n"));
        }
        s.push_str("\r\n");
        s
    };
    let r = raw(addr, many(63).as_bytes()).await; // 65 total
    assert_eq!(status(&r), 431, "{r}");
    assert!(body_of(&r).contains("too many headers"));
    assert_eq!(status(&raw(addr, many(62).as_bytes()).await), 200);
}

#[tokio::test]
async fn rpc_body_edge_cases() {
    let addr = start().await;
    let post = |body: &[u8]| {
        let mut v = format!(
            "POST /rpc HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        v.extend_from_slice(body);
        v
    };
    let r = raw(addr, &post(&[0xff, 0xfe, 0x20])).await;
    assert_eq!(status(&r), 400);
    assert!(body_of(&r).contains("body is not UTF-8"), "{r}");
    let r = raw(addr, &post(b"  \r\n ")).await;
    assert_eq!(status(&r), 400);
    assert!(body_of(&r).contains("empty body"), "{r}");
    let r = raw(addr, &post(b"not json")).await;
    assert_eq!(status(&r), 200);
    assert!(body_of(&r).contains("-32700") && body_of(&r).contains("Parse error"), "{r}");
    // HTTP/1.0 and an IPv6 loopback Host are accepted.
    let r = raw(addr, format!("GET /health HTTP/1.0\r\nHost: [::1]:80\r\nAuthorization: Bearer {TOKEN}\r\n\r\n").as_bytes()).await;
    assert_eq!(status(&r), 200, "{r}");
    let r = raw(addr, format!("GET /health HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\n\r\n").as_bytes()).await;
    assert_eq!(status(&r), 200, "{r}");
    let r = raw(addr, format!("GET /health HTTP/1.1\r\nHost: 10.0.0.1\r\nAuthorization: Bearer {TOKEN}\r\n\r\n").as_bytes()).await;
    assert_eq!(status(&r), 403);
    assert!(body_of(&r).contains("Host not allowed"));
    let r = raw(addr, req(addr, "GET", "/health").as_slice()).await;
    assert!(r.contains("Cache-Control: no-store") && r.contains("X-Content-Type-Options: nosniff"));
}

#[tokio::test]
async fn sse_streams_preamble_events_and_survives_disconnect() {
    let addr = start().await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(format!("GET /events?x=1 HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {TOKEN}\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut acc = String::new();
    let mut buf = [0u8; 512];
    tokio::time::timeout(Duration::from_secs(5), async {
        while !acc.contains(": bilterm events") {
            let n = s.read(&mut buf).await.unwrap();
            assert!(n > 0, "closed early: {acc}");
            acc.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
    })
    .await
    .expect("preamble");
    assert!(acc.starts_with("HTTP/1.1 200 OK") && acc.contains("Content-Type: text/event-stream"), "{acc}");
    drop(s);
    // Server still healthy after the client vanished.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(status(&raw(addr, &req(addr, "GET", "/health")).await), 200);
}

#[tokio::test]
async fn sixty_fifth_connection_is_dropped() {
    let addr = start().await;
    let mut held = Vec::new();
    for _ in 0..64 {
        held.push(TcpStream::connect(addr).await.unwrap());
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    // The 65th is accepted by the kernel then closed by the server without any response.
    let r = raw(addr, &req(addr, "GET", "/health")).await;
    assert_eq!(r, "", "{r}");
    // Releasing one slot makes the server usable again.
    held.pop();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(status(&raw(addr, &req(addr, "GET", "/health")).await), 200);
}

// ---------------------------------------------------------------- JSON-RPC

fn rpc(p: &mut AutomationControlPlane, method: &str, params: &str) -> JsonValue {
    let line = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    JsonValue::parse(&p.handle_jsonrpc_line(&line)).expect("json response")
}

fn err_of(v: &JsonValue) -> (i64, String) {
    let e = v.get("error").unwrap_or_else(|| panic!("expected error: {}", v.to_json_string()));
    (
        e.get("code").and_then(JsonValue::as_i64).unwrap(),
        e.get("message").and_then(JsonValue::as_str).unwrap().to_string(),
    )
}

fn expect_err(p: &mut AutomationControlPlane, method: &str, params: &str, needle: &str) {
    let (code, msg) = err_of(&rpc(p, method, params));
    assert_eq!(code, -32000, "{method} {params}");
    assert!(msg.contains(needle), "{method} {params}: {msg:?} lacks {needle:?}");
}

fn ok(p: &mut AutomationControlPlane, method: &str, params: &str) -> JsonValue {
    let v = rpc(p, method, params);
    v.get("result").cloned().unwrap_or_else(|| panic!("{method} {params}: {}", v.to_json_string()))
}

#[test]
fn envelope_errors() {
    let mut p = plane();
    let v = JsonValue::parse(&p.handle_jsonrpc_line("{")).unwrap();
    assert_eq!(err_of(&v), (-32700, "Parse error".to_string()));
    let v = JsonValue::parse(&p.handle_jsonrpc_line(r#"{"jsonrpc":"2.0","id":3}"#)).unwrap();
    assert_eq!(err_of(&v).0, -32600);
    assert_eq!(v.get("id").and_then(JsonValue::as_i64), Some(3));
    expect_err(&mut p, "no.such", "{}", "Method not found: no.such");
}

#[test]
fn pane_exec_errors() {
    let mut p = plane();
    expect_err(&mut p, "pane.exec", "{}", "missing 'command' parameter");
    expect_err(&mut p, "pane.exec", r#"{"command":5}"#, "missing 'command' parameter");
    expect_err(&mut p, "pane.exec", r#"{"command":null}"#, "missing 'command' parameter");
    expect_err(&mut p, "pane.exec", r#"{"command":"echo hi","pane_id":99}"#, "pane #99 not found");
}

#[test]
fn pane_read_and_feed_output_errors() {
    let mut p = plane();
    expect_err(&mut p, "pane.read", r#"{"pane_id":42}"#, "pane #42 not found");
    expect_err(&mut p, "pane.feed_output", "{}", "missing 'data' parameter");
    expect_err(&mut p, "pane.feed_output", r#"{"data":7}"#, "missing 'data' parameter");
    expect_err(&mut p, "pane.feed_output", r#"{"data":"x","pane_id":42}"#, "pane #42 not found");
    // Ill-typed pane_id silently falls back to the active pane.
    let r = ok(&mut p, "pane.read", r#"{"pane_id":"abc"}"#);
    assert_eq!(r.get("pane_id").and_then(JsonValue::as_usize), Some(p.wm.active_pane_id()));
}

#[test]
fn pane_send_keys_errors() {
    let mut p = plane();
    expect_err(&mut p, "pane.send_keys", r#"{"keys":["F99x"]}"#, "unrecognized key chord: F99x");
    expect_err(&mut p, "pane.send_keys", r#"{"keys":"Enter bogus-chord"}"#, "unrecognized key chord: bogus-chord");
    // Non-string members, numbers and a missing key list dispatch nothing.
    for params in [r#"{"keys":[1,null,true]}"#, r#"{"keys":42}"#, "{}"] {
        let r = ok(&mut p, "pane.send_keys", params);
        assert_eq!(r.get("count").and_then(JsonValue::as_usize), Some(0), "{params}");
        assert_eq!(r.get("status").and_then(JsonValue::as_str), Some("dispatched"));
    }
    let r = ok(&mut p, "pane.send_keys", r#"{"keys":"a b"}"#);
    assert_eq!(r.get("count").and_then(JsonValue::as_usize), Some(2));
}

#[test]
fn events_poll_drain_semantics() {
    let mut p = plane();
    ok(&mut p, "pane.exec", r#"{"command":"echo one"}"#);
    let n = |v: &JsonValue| v.get("events").and_then(JsonValue::as_array).map(<[_]>::len).unwrap();
    assert!(n(&ok(&mut p, "events.poll", r#"{"drain":false}"#)) >= 1);
    assert!(n(&ok(&mut p, "events.poll", r#"{"drain":false}"#)) >= 1, "non-drain keeps events");
    // Ill-typed `drain` falls back to the default (drain = true).
    assert!(n(&ok(&mut p, "events.poll", r#"{"drain":"no"}"#)) >= 1);
    assert_eq!(n(&ok(&mut p, "events.poll", "{}")), 0);
}

#[test]
fn wm_command_and_playbook_errors() {
    let mut p = plane();
    expect_err(&mut p, "wm.command", "{}", "missing 'command' parameter");
    expect_err(&mut p, "wm.command", r#"{"command":"definitely-not-a-command"}"#, "");
    expect_err(&mut p, "playbook.run", "{}", "missing 'toml' parameter");
    expect_err(&mut p, "playbook.run", r#"{"toml":"steps = 3"}"#, "invalid playbook TOML");
    expect_err(&mut p, "playbook.run", r#"{"toml":"[[steps]]\naction = \"bogus\""}"#, "step 1: unknown playbook action 'bogus'");
    expect_err(&mut p, "playbook.run", r#"{"toml":"[[steps]]\naction = \"colon\""}"#, "step 1: 'colon' requires 'command'");
    expect_err(&mut p, "playbook.run", r#"{"toml":"[[steps]]\naction = \"exec\""}"#, "step 1: 'exec' requires 'command'");
    expect_err(&mut p, "playbook.run", r#"{"toml":"[[steps]]\naction = \"export_ppm\""}"#, "step 1: 'export_ppm' requires 'path'");
    expect_err(&mut p, "playbook.run", r#"{"toml":"[[steps]]\naction = \"assert_screen_contains\"\nexpected = \"zzz-never\""}"#, "assertion failed");
    expect_err(&mut p, "playbook.run", r#"{"toml":"[[steps]]\naction = \"assert_trigger_fired\"\npattern = \"nope\""}"#, "trigger 'nope' did not fire");
    expect_err(&mut p, "playbook.run", r#"{"toml":"[[steps]]\naction = \"assert_exit_code\"\nexit_code = 3"}"#, "expected exit_code 3, got None");
    expect_err(&mut p, "playbook.run", r#"{"toml":"[[steps]]\naction = \"assert_screen_contains\"\npane_id = 77"}"#, "pane #77 not found");
    expect_err(&mut p, "playbook.run", r#"{"toml":"[[steps]]\naction = \"assert_exit_code\"\npane_id = 77"}"#, "pane #77 not found");
    // A passing playbook reports its steps.
    let r = ok(&mut p, "playbook.run", r#"{"toml":"name = \"ok\"\n[[steps]]\naction = \"feed_output\"\ndata = \"zebra\"\n[[steps]]\naction = \"assert_screen_contains\"\nexpected = \"zebra\""}"#);
    assert_eq!(r.get("passed").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(r.get("steps_completed").and_then(JsonValue::as_usize), Some(2));
}

#[test]
fn remote_and_fleet_errors() {
    let mut p = plane();
    expect_err(&mut p, "remote.connect_ssh", "{}", "missing 'target' parameter");
    expect_err(&mut p, "remote.connect_tmux", "{}", "missing 'target' parameter");
    expect_err(&mut p, "fleet.register", "{}", "missing 'name' parameter");
    expect_err(&mut p, "fleet.register", r#"{"name":"a"}"#, "missing 'destination' parameter");
    expect_err(&mut p, "fleet.register", r#"{"name":"a","destination":5}"#, "missing 'destination' parameter");
    expect_err(&mut p, "fleet.register", r#"{"name":"a","destination":"-oProxyCommand=x"}"#, "leading '-' not allowed");
    expect_err(&mut p, "fleet.register", r#"{"name":"a","destination":"a b"}"#, "invalid character");
    expect_err(&mut p, "fleet.register", r#"{"name":"a","destination":""}"#, "empty");
    expect_err(&mut p, "fleet.exec", "{}", "missing 'command' parameter");
    expect_err(&mut p, "fleet.exec", r#"{"command":3}"#, "missing 'command' parameter");
    expect_err(&mut p, "fleet.disconnect", "{}", "missing 'name' parameter");
    // Happy path + unknown disconnect + exec with no matching nodes.
    let r = ok(&mut p, "fleet.register", r#"{"name":"web1","destination":"user@host","tags":["a",1,"b"],"rpc_endpoint":"http://x"}"#);
    assert_eq!(r.get("node_count").and_then(JsonValue::as_usize), Some(1));
    let l = ok(&mut p, "fleet.list", "{}");
    let nodes = l.get("nodes").and_then(JsonValue::as_array).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].get("name").and_then(JsonValue::as_str), Some("web1"));
    assert_eq!(nodes[0].get("connected").and_then(JsonValue::as_bool), Some(true), "rpc_endpoint given => connected");
    let tags = nodes[0].get("tags").and_then(JsonValue::as_array).unwrap();
    assert_eq!(tags.len(), 2, "non-string tags are dropped");
    let r = ok(&mut p, "fleet.exec", r#"{"command":"uptime","selector":"tag:none"}"#);
    assert_eq!(r.get("panes").and_then(JsonValue::as_array).map(<[_]>::len), Some(0));
    let r = ok(&mut p, "fleet.disconnect", r#"{"name":"ghost"}"#);
    assert_eq!(r.get("removed").and_then(JsonValue::as_bool), Some(false));
    let r = ok(&mut p, "fleet.disconnect", r#"{"name":"web1"}"#);
    assert_eq!(r.get("removed").and_then(JsonValue::as_bool), Some(true));
}

#[test]
fn fontsize_and_canvas_errors() {
    let mut p = plane();
    expect_err(&mut p, "workspace.fontsize", r#"{"action":"banana"}"#, "invalid workspace.fontsize action: banana");
    expect_err(&mut p, "workspace.fontsize", r#"{"size":70000}"#, "size out of range: 70000");
    expect_err(&mut p, "workspace.fontsize", r#"{"scale_pct":70000}"#, "scale_pct out of range: 70000");
    let (_, m) = err_of(&rpc(&mut p, "workspace.fontsize", r#"{"command":"zzz-bad"}"#));
    assert!(!m.is_empty());
    // Non-numeric / wrong-typed members are ignored, action accepts aliases.
    let r = ok(&mut p, "workspace.fontsize", r#"{"size":"big","scale_pct":-3,"action":"  IN "}"#);
    assert!(r.get("font_size_pt").is_some());
    ok(&mut p, "workspace.fontsize", r#"{"action":"reset"}"#);
    let r = ok(&mut p, "workspace.fontsize", r#"{"action":"150%"}"#);
    assert!(r.get("font_scale_pct").and_then(JsonValue::as_i64).is_some());
    let (_, m) = err_of(&rpc(&mut p, "canvas.command", r#"{"command":"zzz-not-a-canvas-verb"}"#));
    assert!(!m.is_empty());
    let s = ok(&mut p, "canvas.status", "{}");
    assert_eq!(s.get("canvas_active").and_then(JsonValue::as_bool), Some(false));
    // A non-string command is ignored (status only).
    assert!(ok(&mut p, "canvas.command", r#"{"command":9}"#).get("primitives").is_some());
}

#[test]
fn session_save_restore_errors_and_confinement() {
    let mut p = plane();
    let sessions = state_dir().join("bilterm").join("sessions");
    expect_err(&mut p, "session.restore", r#"{"name":"never-saved-xyz"}"#, "No such file");
    // Non-string name falls back to "default".
    let r = ok(&mut p, "session.save", r#"{"name":12}"#);
    assert_eq!(r.get("session").and_then(JsonValue::as_str), Some("default"));
    // Path traversal in the name is sanitised to a plain stem inside the session dir.
    let r = ok(&mut p, "session.save", r#"{"name":"../../evil/../x"}"#);
    let path = std::path::PathBuf::from(r.get("path").and_then(JsonValue::as_str).unwrap());
    assert_eq!(path.parent().unwrap(), sessions.as_path(), "{path:?}");
    assert_eq!(path.file_name().unwrap(), "evilx.toml");
    assert!(!state_dir().join("evil").exists() && !state_dir().parent().unwrap().join("x.toml").exists());
    // Only-illegal characters collapse to "default"; restore of the sanitised name works.
    let r = ok(&mut p, "session.save", r#"{"name":"///"}"#);
    assert!(r.get("path").and_then(JsonValue::as_str).unwrap().ends_with("default.toml"));
    expect_err(&mut p, "session.restore", r#"{"name":"../../etc/passwd"}"#, "No such file");
    let r = ok(&mut p, "session.restore", r#"{"name":"evilx"}"#);
    assert_eq!(r.get("restored").and_then(JsonValue::as_bool), Some(true));
    // A corrupt session file is a config error, not a panic.
    std::fs::write(sessions.join("corrupt.toml"), "panes = [[[").unwrap();
    let (_, m) = err_of(&rpc(&mut p, "session.restore", r#"{"name":"corrupt"}"#));
    assert!(!m.is_empty());
}

#[test]
fn snapshot_traversal_is_refused_and_leaks_nothing() {
    let dir = std::env::temp_dir().join(format!("bilterm-errpaths-snap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let snaps = dir.join("snaps");
    let mut p = AutomationControlPlane::new_headless(24, 80).with_snapshot_dir(snaps.clone());
    const MSG: &str = "ppm_path must be a file name within the snapshot directory";
    for bad in ["../esc.ppm", "/tmp/esc-abs.ppm", "a/b.ppm", "a\\b.ppm", "..", ".", "", "x\u{0}.ppm"] {
        let params = JsonValue::obj([("ppm_path", JsonValue::String(bad.to_string()))]).to_json_string();
        expect_err(&mut p, "workspace.snapshot", &params, MSG);
        let pb = format!("[[steps]]\naction = \"export_ppm\"\npath = {}", JsonValue::String(bad.to_string()).to_json_string());
        let params = format!(r#"{{"toml":{}}}"#, JsonValue::String(pb).to_json_string());
        expect_err(&mut p, "playbook.run", &params, MSG);
    }
    assert!(!dir.join("esc.ppm").exists() && !std::path::Path::new("/tmp/esc-abs.ppm").exists());
    // wm.command snapshots are confined as well.
    expect_err(&mut p, "wm.command", r#"{"command":"snapshot ../esc2.ppm"}"#, "");
    assert!(!dir.join("esc2.ppm").exists());
    // A legit name works and lands inside the snapshot dir.
    ok(&mut p, "workspace.snapshot", r#"{"ppm_path":"good.ppm"}"#);
    assert!(std::fs::read(snaps.join("good.ppm")).unwrap().starts_with(b"P6"));
    // A snapshot dir that is a regular file is rejected.
    let file = dir.join("afile");
    std::fs::write(&file, "x").unwrap();
    let mut p2 = AutomationControlPlane::new_headless(24, 80).with_snapshot_dir(file.clone());
    expect_err(&mut p2, "workspace.snapshot", r#"{"ppm_path":"g.ppm"}"#, "is not a directory");
}

// ---------------------------------------------------------------- MCP

fn mcp(p: &mut AutomationControlPlane, line: &str) -> JsonValue {
    JsonValue::parse(&p.handle_mcp_line(line).expect("response")).unwrap()
}

fn tool(p: &mut AutomationControlPlane, name: &str, args: &str) -> JsonValue {
    mcp(p, &format!(r#"{{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{{"name":"{name}","arguments":{args}}}}}"#))
}

#[test]
fn mcp_missing_args_are_invalid_params_for_each_tool() {
    let mut p = plane();
    for (name, needle) in [
        ("bilterm_exec_command", "missing 'command' parameter"),
        ("bilterm_colon_command", "missing 'command' parameter"),
        ("bilterm_run_playbook", "missing 'toml' parameter"),
        ("bilterm_remote_connect", "missing 'target' parameter"),
        ("bilterm_fleet_exec", "missing 'command' parameter"),
    ] {
        let v = tool(&mut p, name, "{}");
        let (code, msg) = err_of(&v);
        assert_eq!(code, -32602, "{name}");
        assert_eq!(msg, format!("Invalid params: {needle}"), "{name}");
        assert!(v.get("result").is_none());
    }
}

#[test]
fn mcp_runtime_failures_use_iserror_shape() {
    let mut p = plane();
    let cases = [
        ("bilterm_exec_command", r#"{"command":"x","pane_id":99}"#, "pane #99 not found"),
        ("bilterm_inspect_pane", r#"{"pane_id":99}"#, "pane #99 not found"),
        ("bilterm_send_keys", r#"{"keys":["nonsense-key"]}"#, "unrecognized key chord: nonsense-key"),
        ("bilterm_workspace_fontsize", r#"{"action":"nope"}"#, "invalid workspace.fontsize action: nope"),
        ("bilterm_run_playbook", r#"{"toml":"steps = 1"}"#, "invalid playbook TOML"),
    ];
    for (name, args, needle) in cases {
        let v = tool(&mut p, name, args);
        let r = v.get("result").unwrap_or_else(|| panic!("{name}: {}", v.to_json_string()));
        assert_eq!(r.get("isError").and_then(JsonValue::as_bool), Some(true), "{name}");
        let content = r.get("content").and_then(JsonValue::as_array).unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0].get("type").and_then(JsonValue::as_str), Some("text"));
        let text = content[0].get("text").and_then(JsonValue::as_str).unwrap();
        assert!(text.contains(needle), "{name}: {text}");
    }
    // The two tools that cannot fail on bad args still succeed with isError=false.
    for name in ["bilterm_workspace_status", "bilterm_canvas"] {
        let v = tool(&mut p, name, "{}");
        assert_eq!(v.get("result").and_then(|r| r.get("isError")).and_then(JsonValue::as_bool), Some(false), "{name}");
    }
}

#[test]
fn mcp_envelope_errors() {
    let mut p = plane();
    let (c, m) = err_of(&tool(&mut p, "bilterm_nope", "{}"));
    assert_eq!((c, m.as_str()), (-32602, "Invalid params: unknown MCP tool: bilterm_nope"));
    let v = mcp(&mut p, r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_workspace_status","arguments":[1]}}"#);
    assert_eq!(err_of(&v), (-32602, "Invalid params: 'arguments' must be an object".to_string()));
    let v = mcp(&mut p, r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_workspace_status","arguments":"x"}}"#);
    assert_eq!(err_of(&v).0, -32602);
    let v = mcp(&mut p, r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{}}"#);
    assert_eq!(err_of(&v).1, "Invalid params: unknown MCP tool: ");
    // Missing `arguments` is allowed.
    let v = mcp(&mut p, r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_workspace_status"}}"#);
    assert_eq!(v.get("result").and_then(|r| r.get("isError")).and_then(JsonValue::as_bool), Some(false));
    let v = mcp(&mut p, r#"{"jsonrpc":"2.0","id":1,"method":"nope/nope"}"#);
    assert_eq!(err_of(&v), (-32601, "Method not found: nope/nope".to_string()));
    // Notifications (no id), invalid JSON and missing method yield no response.
    assert!(p.handle_mcp_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
    assert!(p.handle_mcp_line("{broken").is_none());
    assert!(p.handle_mcp_line(r#"{"jsonrpc":"2.0","id":1}"#).is_none());
    let v = mcp(&mut p, r#"{"jsonrpc":"2.0","id":"str-id","method":"ping"}"#);
    assert_eq!(v.get("id").and_then(JsonValue::as_str), Some("str-id"));
    let tools = mcp(&mut p, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
    assert_eq!(tools.get("result").and_then(|r| r.get("tools")).and_then(JsonValue::as_array).map(<[_]>::len), Some(14));
}

#[test]
fn parse_key_chord_rejects_garbage() {
    use bilterm::api::parse_key_chord_str;
    assert!(parse_key_chord_str("").is_none());
    assert!(parse_key_chord_str("Ctrl-").is_none());
    assert!(parse_key_chord_str("Enter").is_some());
    assert!(parse_key_chord_str("x").is_some());
}

// ---------------------------------------------------------------- JSON parser

#[test]
fn json_depth_limit() {
    let nest = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
    assert!(JsonValue::parse(&nest(100)).is_some());
    assert!(JsonValue::parse(&nest(130)).is_none(), "nesting beyond MAX_JSON_DEPTH rejected");
    assert!(JsonValue::parse(&nest(100_000)).is_none(), "no stack overflow");
    let obj = format!("{}1{}", r#"{"a":"#.repeat(200), "}".repeat(200));
    assert!(JsonValue::parse(&obj).is_none());
}

#[test]
fn json_strings_and_escapes() {
    let s = |t: &str| JsonValue::parse(t).and_then(|v| v.as_str().map(String::from));
    assert_eq!(s(r#""Aé中""#).as_deref(), Some("Aé中"));
    assert_eq!(s(r#""a\"b\\c\/d\n\r\t\b\f""#).as_deref(), Some("a\"b\\c/d\n\r\t\u{8}\u{c}"));
    assert_eq!(s("\"héllo 😀\"").as_deref(), Some("héllo 😀"));
    assert_eq!(s(r#""\u12""#), None, "short \\u escape");
    assert_eq!(s(r#""\uZZZZ""#), None, "non-hex \\u escape");
    assert_eq!(s(r#""abc"#), None, "unterminated string");
    assert_eq!(s(r#""abc\"#), None, "dangling backslash");
    assert_eq!(s(r#""\ud800""#), None, "lone surrogate is rejected, not mangled");
    // Round trip of control characters through the serializer.
    let v = JsonValue::String("a\u{1}\"\n".into());
    assert_eq!(JsonValue::parse(&v.to_json_string()), Some(v));
}

#[test]
fn json_surrogate_pair_escape() {
    assert_eq!(
        JsonValue::parse(r#""😀""#).and_then(|v| v.as_str().map(String::from)).as_deref(),
        Some("😀")
    );
}

#[test]
fn json_trailing_garbage_and_structure() {
    for bad in [
        "{} x", "1 2", "[1,2]]", "null null", "", "   ", "[1,]", "{\"a\":1,}", "{\"a\"}", "{\"a\" 1}", "{a:1}",
        "[1 2]", "[", "{", "{\"a\":", "tru", "nul", "'x'", "-", "{\"a\":1 \"b\":2}",
    ] {
        assert_eq!(JsonValue::parse(bad), None, "{bad:?}");
    }
    assert_eq!(JsonValue::parse("  [ 1 , 2 ]\n").unwrap().as_array().map(<[_]>::len), Some(2));
    assert_eq!(JsonValue::parse("{}"), Some(JsonValue::Object(Default::default())));
    // Duplicate keys: last wins.
    assert_eq!(JsonValue::parse(r#"{"a":1,"a":2}"#).unwrap().get("a").and_then(JsonValue::as_i64), Some(2));
    assert_eq!(JsonValue::parse("true"), Some(JsonValue::Bool(true)));
    assert_eq!(JsonValue::parse("false"), Some(JsonValue::Bool(false)));
    assert_eq!(JsonValue::parse("null"), Some(JsonValue::Null));
}

#[test]
fn json_numbers() {
    let n = |t: &str| JsonValue::parse(t).and_then(|v| v.as_i64());
    assert_eq!(n("0"), Some(0));
    assert_eq!(n("-17"), Some(-17));
    assert_eq!(n("9223372036854775807"), Some(i64::MAX));
    assert_eq!(n("-9223372036854775808"), Some(i64::MIN));
    assert_eq!(n("9223372036854775808"), None, "i64 overflow rejected");
    assert_eq!(n("99999999999999999999999999"), None);
    assert_eq!(n("-"), None);
    assert_eq!(n("-x"), None);
    // as_usize rejects negatives.
    assert_eq!(JsonValue::parse("-1").unwrap().as_usize(), None);
    assert_eq!(JsonValue::parse("7").unwrap().as_usize(), Some(7));
    assert_eq!(JsonValue::parse(r#""7""#).unwrap().as_i64(), None);
    assert_eq!(JsonValue::parse("7").unwrap().as_str(), None);
    assert_eq!(JsonValue::parse("7").unwrap().as_bool(), None);
    assert!(JsonValue::parse("7").unwrap().as_array().is_none());
    assert!(JsonValue::parse("7").unwrap().get("a").is_none());
}

#[test]
fn json_fractional_and_exponent_numbers_are_not_silently_truncated() {
    // Fractions/exponents parse as Float and never as a truncated integer.
    for t in ["1.5", "2e3", "1E2", "-0.5"] {
        assert!(matches!(JsonValue::parse(t), Some(JsonValue::Float(_))), "{t}");
    }
    assert_eq!(JsonValue::parse("1.5").unwrap().as_i64(), None);
}

#[test]
fn json_leading_zero_is_rejected() {
    assert_eq!(JsonValue::parse("01"), None, "RFC 8259 forbids leading zeros");
}
