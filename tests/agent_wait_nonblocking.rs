//! `agent.wait` over the socket server must not block the request loop: a reply sent by another
//! client during the wait completes it, and unrelated requests are answered while it is pending.

use std::time::{Duration, Instant};

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue, serve_jsonrpc_tcp};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

async fn start() -> std::net::SocketAddr {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    plane.wm.apply(Action::SplitVertical).unwrap();
    plane.wm.set_agent_messaging_enabled(true);
    plane.wm.agent_mail_mut().register_name(1, "alice").unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(serve_jsonrpc_tcp(l, plane, None));
    addr
}

/// One request on a fresh connection; returns the parsed response.
async fn call(addr: std::net::SocketAddr, id: i64, method: &str, params: &str) -> JsonValue {
    let s = TcpStream::connect(addr).await.unwrap();
    let (rd, mut wr) = tokio::io::split(s);
    let line = format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#);
    wr.write_all(format!("{line}\n").as_bytes()).await.unwrap();
    let mut reply = String::new();
    tokio::time::timeout(Duration::from_secs(15), BufReader::new(rd).read_line(&mut reply))
        .await
        .expect("reply within 15s")
        .unwrap();
    JsonValue::parse(reply.trim()).expect("json reply")
}

fn status(v: &JsonValue) -> Option<&str> {
    v.get("result")?.get("status")?.as_str()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reply_sent_during_wait_completes_it() {
    let addr = start().await;
    let started = Instant::now();
    let waiter = tokio::spawn(async move {
        call(addr, 7, "agent.wait", r#"{"agent":"bob","reply_to":42,"timeout_ms":8000}"#).await
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    let sent = call(
        addr,
        8,
        "agent.send",
        r#"{"from":"alice","to":"bob","body":"answer","reply_to":42}"#,
    )
    .await;
    assert!(sent.get("result").is_some(), "send failed: {sent:?}");

    let resp = waiter.await.unwrap();
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(3), "wait took {elapsed:?}");
    assert_eq!(resp.get("id").and_then(JsonValue::as_i64), Some(7));
    assert_eq!(status(&resp), Some("received"), "{resp:?}");
    let msg = resp.get("result").and_then(|r| r.get("message")).unwrap();
    assert_eq!(msg.get("body").and_then(JsonValue::as_str), Some("answer"));
    assert_eq!(msg.get("reply_to").and_then(JsonValue::as_i64), Some(42));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unrelated_request_is_answered_while_wait_pending() {
    let addr = start().await;
    let waiter = tokio::spawn(async move {
        call(addr, 1, "agent.wait", r#"{"agent":"bob","timeout_ms":1500}"#).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let t = Instant::now();
    let st = call(addr, 2, "workspace.status", "{}").await;
    assert!(st.get("result").is_some(), "{st:?}");
    assert!(t.elapsed() < Duration::from_millis(500), "status took {:?}", t.elapsed());
    assert!(!waiter.is_finished(), "wait should still be pending");

    // A message for a different inbox must not complete the wait.
    let _ = call(addr, 3, "agent.send", r#"{"from":"bob","to":"alice","body":"x"}"#).await;
    assert!(!waiter.is_finished(), "message for another agent must not complete the wait");

    let resp = waiter.await.unwrap();
    assert_eq!(status(&resp), Some("timeout"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timeout_response_shape_is_unchanged() {
    let addr = start().await;
    let t = Instant::now();
    let resp = call(addr, 5, "agent.wait", r#"{"agent":"bob","timeout_ms":200}"#).await;
    let elapsed = t.elapsed();
    assert!(elapsed >= Duration::from_millis(200), "returned early: {elapsed:?}");
    assert!(elapsed < Duration::from_secs(2), "returned late: {elapsed:?}");
    let expected = JsonValue::obj([
        ("jsonrpc", JsonValue::String("2.0".to_string())),
        ("id", JsonValue::Number(5)),
        (
            "result",
            JsonValue::obj([
                ("status", JsonValue::String("timeout".to_string())),
                ("agent", JsonValue::String("bob".to_string())),
            ]),
        ),
    ]);
    assert_eq!(resp, expected);
}
