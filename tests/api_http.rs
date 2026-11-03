//! Loopback HTTP + SSE transport (`bilterm --headless --http`): auth, limits, origin checks, RPC parity, SSE.

use std::sync::Arc;
use std::time::Duration;

use bilterm::api::{
    serve_http_standalone, validate_http_bind, AutomationControlPlane, AutomationEvent,
    MAX_HTTP_BODY_BYTES,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const TOKEN: &str = "test-token-0123456789";

async fn start(plane: AutomationControlPlane) -> std::net::SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(serve_http_standalone(l, plane, Arc::from(TOKEN)));
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
    resp.split(' ')
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0)
}

fn post_rpc(addr: std::net::SocketAddr, auth: Option<&str>, extra: &str, body: &str) -> Vec<u8> {
    let auth = auth
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    format!(
        "POST /rpc HTTP/1.1\r\nHost: {addr}\r\n{auth}{extra}Content-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

const STATUS_RPC: &str = r#"{"jsonrpc":"2.0","id":7,"method":"workspace.status","params":{}}"#;

#[tokio::test]
async fn valid_rpc_call_and_health() {
    let addr = start(AutomationControlPlane::new_headless(24, 80)).await;
    let r = raw(addr, &post_rpc(addr, Some(TOKEN), "", STATUS_RPC)).await;
    assert_eq!(status(&r), 200, "{r}");
    let body = r.split("\r\n\r\n").nth(1).unwrap();
    let mut direct = AutomationControlPlane::new_headless(24, 80);
    assert_eq!(body, direct.handle_jsonrpc_line(STATUS_RPC));

    let h = raw(
        addr,
        format!("GET /health HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {TOKEN}\r\n\r\n")
            .as_bytes(),
    )
    .await;
    assert_eq!(status(&h), 200);
    assert!(h.contains(r#""ok":true"#) && h.contains("version"), "{h}");
}

#[tokio::test]
async fn missing_or_bad_token_is_401() {
    let addr = start(AutomationControlPlane::new_headless(24, 80)).await;
    assert_eq!(
        status(&raw(addr, &post_rpc(addr, None, "", STATUS_RPC)).await),
        401
    );
    assert_eq!(
        status(&raw(addr, &post_rpc(addr, Some("wrong"), "", STATUS_RPC)).await),
        401
    );
    let h = raw(
        addr,
        format!("GET /health HTTP/1.1\r\nHost: {addr}\r\n\r\n").as_bytes(),
    )
    .await;
    assert_eq!(status(&h), 401);
}

#[test]
fn non_loopback_bind_refused() {
    assert!(validate_http_bind("0.0.0.0:8080").is_err());
    assert!(validate_http_bind("192.168.1.5:8080").is_err());
    assert!(validate_http_bind("example.com:80").is_err());
    assert!(validate_http_bind("127.0.0.1:8080").is_ok());
    assert!(validate_http_bind("[::1]:8080").is_ok());
    assert!(validate_http_bind("localhost:8080").is_ok());
}

#[tokio::test]
async fn non_loopback_listener_refused_by_server() {
    if let Ok(l) = TcpListener::bind("0.0.0.0:0").await {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        assert!(
            bilterm::api::serve_http(l, tx, Arc::from(TOKEN))
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn oversized_body_is_413() {
    let addr = start(AutomationControlPlane::new_headless(24, 80)).await;
    let req = format!(
        "POST /rpc HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Length: {}\r\n\r\n",
        MAX_HTTP_BODY_BYTES + 1
    );
    assert_eq!(status(&raw(addr, req.as_bytes()).await), 413);
}

#[tokio::test]
async fn malformed_requests_are_400() {
    let addr = start(AutomationControlPlane::new_headless(24, 80)).await;
    for bad in [
        "garbage\r\n\r\n".to_string(),
        "POST /rpc HTTP/9.9\r\n\r\n".to_string(),
        format!("POST /rpc HTTP/1.1\r\nHost: {addr}\r\nno-colon-header\r\n\r\n"),
        format!("POST /rpc HTTP/1.1\r\nHost: {addr}\r\nContent-Length: abc\r\n\r\n"),
        format!("POST /rpc HTTP/1.1\r\nHost: {addr}\r\nTransfer-Encoding: chunked\r\n\r\n"),
        format!(
            "POST /rpc HTTP/1.1\r\nHost: {addr}\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\nx"
        ),
    ] {
        let r = raw(addr, bad.as_bytes()).await;
        assert_eq!(status(&r), 400, "{bad:?} -> {r}");
    }
    let r = raw(addr, &post_rpc(addr, Some(TOKEN), "", "")).await;
    assert_eq!(status(&r), 400);
    let big = format!(
        "GET /health HTTP/1.1\r\nHost: {addr}\r\nX: {}\r\n\r\n",
        "a".repeat(20_000)
    );
    assert_eq!(status(&raw(addr, big.as_bytes()).await), 431);
}

#[tokio::test]
async fn bad_origin_or_host_is_403() {
    let addr = start(AutomationControlPlane::new_headless(24, 80)).await;
    let r = raw(
        addr,
        &post_rpc(addr, Some(TOKEN), "Origin: http://evil.example\r\n", STATUS_RPC),
    )
    .await;
    assert_eq!(status(&r), 403);
    assert!(!r.contains("Access-Control-Allow"));
    let rebinding = format!(
        "POST /rpc HTTP/1.1\r\nHost: evil.example:80\r\nAuthorization: Bearer {TOKEN}\r\nContent-Length: 0\r\n\r\n"
    );
    assert_eq!(status(&raw(addr, rebinding.as_bytes()).await), 403);
    let ok = raw(
        addr,
        &post_rpc(addr, Some(TOKEN), &format!("Origin: http://{addr}\r\n"), STATUS_RPC),
    )
    .await;
    assert_eq!(status(&ok), 200);
}

#[tokio::test]
async fn sse_delivers_event() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    plane.push_event(AutomationEvent::CommandSubmitted {
        pane_id: 1,
        command: "echo sse".into(),
    });
    let addr = start(plane).await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(
        format!("GET /events HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {TOKEN}\r\n\r\n")
            .as_bytes(),
    )
    .await
    .unwrap();
    let mut acc = String::new();
    let mut buf = [0u8; 1024];
    let res = tokio::time::timeout(Duration::from_secs(5), async {
        while !acc.contains("echo sse") {
            let n = s.read(&mut buf).await.unwrap();
            assert!(n > 0, "stream closed early: {acc}");
            acc.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
    })
    .await;
    assert!(res.is_ok(), "no SSE event: {acc}");
    assert!(
        acc.contains("text/event-stream") && acc.contains("data: "),
        "{acc}"
    );
}
