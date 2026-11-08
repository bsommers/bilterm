//! Phase 60 Wave 1 (`WEB-FE-01`, `WEB-FE-02`): design-token CSS export, extended health, the
//! `workspace.view_model` RPC + `/api/*` adapters, the RFC 6455 `/ws` bridge and output seq/resume.

#[path = "support/phase60.rs"]
mod support;

use bilterm::api::ws_codec::{accept_key, encode_frame, Opcode};
use bilterm::api::{AutomationControlPlane, JsonValue, WebServeOptions};
use support::*;

fn plane() -> AutomationControlPlane {
    AutomationControlPlane::new_headless(24, 80)
}

#[tokio::test]
async fn test_design_tokens_css_export() {
    let mut p = plane();
    let vm = rpc(&mut p, "workspace.view_model", "{}");
    let css = vm.get("css_tokens").and_then(JsonValue::as_str).unwrap().to_string();
    assert!(css.starts_with(":root {"), "{css}");
    assert!(css.trim_end().ends_with('}'));
    let vars: Vec<&str> = css.lines().filter(|l| l.trim_start().starts_with("--color-")).collect();
    assert!(vars.len() >= 10, "expected the token roles, got {vars:?}");
    for v in &vars {
        let val = v.split_once(':').unwrap().1.trim().trim_end_matches(';');
        assert!(
            (val.starts_with('#') && val.len() == 7) || val.starts_with("rgba("),
            "bad css colour {v}"
        );
    }

    let addr = start(p, WebServeOptions::default()).await;
    // Unauthenticated: browsers load the stylesheet with a plain <link>.
    let r = raw(addr, get(addr, "/tokens.css", false).as_bytes()).await;
    assert_eq!(status(&r), 200, "{r}");
    assert!(r.to_ascii_lowercase().contains("content-type: text/css"), "{r}");
    assert_eq!(body(&r), css);
}

#[tokio::test]
async fn test_extended_health_endpoint() {
    let addr = start(plane(), WebServeOptions::default()).await;
    for path in ["/health", "/api/health"] {
        let r = raw(addr, get(addr, path, true).as_bytes()).await;
        assert_eq!(status(&r), 200, "{path}: {r}");
        let h = JsonValue::parse(body(&r)).unwrap();
        assert_eq!(h.get("ok").and_then(JsonValue::as_bool), Some(true));
        assert_eq!(h.get("contract").and_then(JsonValue::as_i64), Some(1));
        assert!(h.get("version").and_then(JsonValue::as_str).is_some());
        assert!(h.get("epoch").and_then(JsonValue::as_str).unwrap().starts_with("bilterm-"));
        let b = &h.get("backends").and_then(JsonValue::as_array).unwrap()[0];
        assert_eq!(b.get("id").and_then(JsonValue::as_str), Some("bilterm"));
        assert_eq!(b.get("up").and_then(JsonValue::as_bool), Some(true));
        assert!(h.get("agents").and_then(JsonValue::as_i64).unwrap() >= 1);
        // Health still requires the bearer token.
        assert_eq!(status(&raw(addr, get(addr, path, false).as_bytes()).await), 401, "{path}");
    }
}

#[tokio::test]
async fn test_workspace_view_model_rpc() {
    let mut p = plane();
    let vm = rpc(&mut p, "workspace.view_model", "{}");
    for k in ["contract", "epoch", "layout_mode", "split_tree", "z_stack", "agents", "utility_windows", "css_tokens"] {
        assert!(vm.get(k).is_some(), "view model missing {k}: {}", vm.to_json_string());
    }
    let agents = vm.get("agents").and_then(JsonValue::as_array).unwrap();
    assert!(!agents.is_empty());
    let a = &agents[0];
    let id = a.get("id").and_then(JsonValue::as_str).unwrap().to_string();
    assert!(id.starts_with("pane-"), "{id}");
    for k in ["name", "mode", "status", "unreadOutput", "createdAt", "lastActivity", "capabilities", "meta"] {
        assert!(a.get(k).is_some(), "agent missing {k}");
    }
    let tree = vm.get("split_tree").unwrap();
    assert!(matches!(tree.get("type").and_then(JsonValue::as_str), Some("leaf" | "split")));
    let focused = vm.get("z_stack").and_then(|z| z.get("focusedAgentId")).and_then(JsonValue::as_str);
    assert_eq!(focused, Some(id.as_str()));

    let addr = start(p, WebServeOptions::default()).await;
    let list = raw(addr, get(addr, "/api/agents", true).as_bytes()).await;
    assert_eq!(status(&list), 200, "{list}");
    assert!(body(&list).contains(&id));
    let one = raw(addr, get(addr, &format!("/api/agents/{id}"), true).as_bytes()).await;
    assert_eq!(status(&one), 200, "{one}");
    let missing = raw(addr, get(addr, "/api/agents/pane-9999", true).as_bytes()).await;
    assert_eq!(status(&missing), 404, "{missing}");
    assert!(body(&missing).contains("NOT_FOUND"));
    let logs = raw(addr, get(addr, &format!("/api/agents/{id}/logs?limit=10"), true).as_bytes()).await;
    assert_eq!(status(&logs), 200, "{logs}");
    let search = raw(addr, post(addr, "/api/search", r#"{"query":"zzz-no-match"}"#).as_bytes()).await;
    assert_eq!(status(&search), 200, "{search}");
    assert_eq!(status(&raw(addr, get(addr, "/api/agents", false).as_bytes()).await), 401);
}

#[tokio::test]
async fn test_websocket_handshake_and_frames() {
    let addr = start(plane(), WebServeOptions::default()).await;

    // Bad upgrade (no key) is a plain 400.
    let bad = format!("GET /ws HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\n\r\n");
    assert_eq!(status(&raw(addr, bad.as_bytes()).await), 400);

    // Token in the hello message (browser path: no custom headers on WebSocket).
    let mut ws = WsClient::connect(addr, "/ws", false).await;
    assert!(ws.handshake.starts_with("HTTP/1.1 101"), "{}", ws.handshake);
    assert!(ws.handshake.contains(&format!("Sec-WebSocket-Accept: {}", accept_key(WS_KEY))));
    ws.send_json(&hello(&format!(r#","token":"{TOKEN}""#))).await;
    let (ack, _) = ws.wait_for("hello_ack").await;
    assert_eq!(ack.get("contract").and_then(JsonValue::as_i64), Some(1));
    assert!(ack.get("clientId").and_then(JsonValue::as_str).unwrap().starts_with("web-"));
    let (init, _) = ws.wait_for("agent:init_list").await;
    assert!(!init.get("agents").and_then(JsonValue::as_array).unwrap().is_empty());

    ws.send_json(r#"{"type":"ping","t":42}"#).await;
    let (pong, _) = ws.wait_for("pong").await;
    assert_eq!(pong.get("t").and_then(JsonValue::as_i64), Some(42));

    // WebSocket-level ping is answered with a pong frame carrying the same payload.
    ws.send_raw(&bilterm::api::ws_codec::encode_masked_frame(Opcode::Ping, b"hi", [9, 9, 9, 9])).await;
    loop {
        let (op, p) = ws.next_frame().await.expect("pong frame");
        if op == Opcode::Pong {
            assert_eq!(p, b"hi");
            break;
        }
    }

    // Wrong token -> UNAUTHENTICATED error + close.
    let mut bad = WsClient::connect(addr, "/ws", false).await;
    bad.send_json(&hello(r#","token":"nope""#)).await;
    let (err, _) = bad.wait_for("agent:error").await;
    assert!(err.to_json_string().contains("UNAUTHENTICATED"));

    // Unmasked client frame is a protocol error (close 1002).
    let mut um = WsClient::connect(addr, &format!("/ws?token={TOKEN}"), false).await;
    um.send_raw(&encode_frame(Opcode::Text, hello("").as_bytes())).await;
    loop {
        match um.next_frame().await {
            Some((Opcode::Close, p)) => {
                assert_eq!(u16::from_be_bytes([p[0], p[1]]), 1002);
                break;
            }
            Some(_) => {}
            None => panic!("expected close 1002"),
        }
    }
}

#[tokio::test]
async fn test_output_sequence_and_resume_gap() {
    let mut p = plane();
    let pump = rpc(&mut p, "web.pump", "{}");
    let (agent, _) = match pump.get("seqs") {
        Some(JsonValue::Object(m)) => m.iter().next().map(|(k, v)| (k.clone(), v.clone())).unwrap(),
        other => panic!("seqs: {other:?}"),
    };
    let pane_id: usize = agent.trim_start_matches("pane-").parse().unwrap();

    // ~360 KiB of output in 60 KiB chunks overflows the 256 KiB ring, evicting the oldest seqs.
    let chunk = "x".repeat(60 * 1024);
    let mut last = 0;
    for _ in 0..6 {
        rpc(&mut p, "pane.feed_output", &format!(r#"{{"pane_id":{pane_id},"data":"{chunk}"}}"#));
        let r = rpc(&mut p, "web.pump", "{}");
        let seq = r.get("seqs").and_then(|s| s.get(&agent)).and_then(JsonValue::as_i64).unwrap();
        assert!(seq > last, "seq must be monotonic: {seq} <= {last}");
        last = seq;
    }

    let addr = start(p, WebServeOptions::default()).await;

    // Resuming from seq 1 (already evicted) reports a gap, then replays what is retained.
    let mut ws = WsClient::connect(addr, "/ws", true).await;
    ws.send_json(&hello(&format!(r#","resumeFrom":{{"{agent}":1}}"#))).await;
    let (gap, _) = ws.wait_for("agent:gap").await;
    assert_eq!(gap.get("agentId").and_then(JsonValue::as_str), Some(agent.as_str()));
    let (from, to) = (
        gap.get("fromSeq").and_then(JsonValue::as_i64).unwrap(),
        gap.get("toSeq").and_then(JsonValue::as_i64).unwrap(),
    );
    assert!(from >= 2 && to >= from, "gap {from}..{to}");
    let mut seqs = Vec::new();
    while seqs.last() != Some(&last) {
        let (out, _) = ws.wait_for("agent:output").await;
        assert_eq!(out.get("replay").and_then(JsonValue::as_bool), Some(true));
        seqs.push(out.get("seq").and_then(JsonValue::as_i64).unwrap());
    }
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "replay not contiguous: {seqs:?}");
    assert_eq!(seqs[0], to + 1);

    // Resuming at the head replays nothing and reports no gap.
    let mut fresh = WsClient::connect(addr, "/ws", true).await;
    fresh.send_json(&hello(&format!(r#","resumeFrom":{{"{agent}":{last}}}"#))).await;
    let (_, _) = fresh.wait_for("agent:init_list").await;
    fresh.send_json(r#"{"type":"ping","t":1}"#).await;
    let (_, between) = fresh.wait_for("pong").await;
    assert!(
        between.iter().all(|m| !matches!(msg_type(m), "agent:gap" | "agent:output")),
        "unexpected replay: {:?}",
        between.iter().map(JsonValue::to_json_string).collect::<Vec<_>>()
    );
}
