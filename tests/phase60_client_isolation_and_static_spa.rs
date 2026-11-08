//! Phase 60 Wave 2 (`WEB-FE-03`, `WEB-FE-04`, `UNI-R01`): per-client view isolation, `--web-dir`
//! static hosting under `SEC-R01` confinement with SPA fallback, `--cors-origin`, and one web-surface
//! status across `:web`, `Action::Web`, JSON-RPC and MCP.

#[path = "support/phase60.rs"]
mod support;

use bilterm::action::{Action, WebOp};
use bilterm::api::static_serve::{serve_static_asset, StaticRoot};
use bilterm::api::{AutomationControlPlane, JsonValue, WebServeOptions};
use support::*;

fn plane() -> AutomationControlPlane {
    AutomationControlPlane::new_headless(24, 80)
}

fn spa_dir() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("index.html"), "<!doctype html><title>SPA</title>").unwrap();
    std::fs::create_dir(d.path().join("assets")).unwrap();
    std::fs::write(d.path().join("assets/app-1234.js"), "console.log(1)").unwrap();
    d
}

#[tokio::test]
async fn test_client_view_isolation() {
    let mut p = plane();
    let created = rpc(&mut p, "web.create", r#"{"name":"second"}"#);
    let second = created.get("agentId").or_else(|| created.get("agent").and_then(|a| a.get("id")));
    let second = second.and_then(JsonValue::as_str).expect("created agent id").to_string();
    let host_focus = |p: &mut AutomationControlPlane| {
        rpc(p, "workspace.view_model", "{}").get("z_stack").and_then(|z| z.get("focusedAgentId")).cloned()
    };
    let before = host_focus(&mut p);
    let opts = WebServeOptions::default();
    let surface = opts.surface.clone();
    let addr = start(p, opts).await;

    let mut a = WsClient::connect(addr, "/ws", true).await;
    let mut b = WsClient::connect(addr, "/ws", true).await;
    a.send_json(&hello("")).await;
    b.send_json(&hello("")).await;
    let id_a = a.wait_for("hello_ack").await.0.get("clientId").and_then(JsonValue::as_str).unwrap().to_string();
    let id_b = b.wait_for("hello_ack").await.0.get("clientId").and_then(JsonValue::as_str).unwrap().to_string();
    assert_ne!(id_a, id_b);
    assert_eq!(surface.clients().len(), 2);

    a.send_json(&format!(r#"{{"type":"client:focus","agentId":"{second}"}}"#)).await;
    let (view, _) = a.wait_for("client:view").await;
    assert_eq!(view.get("focusedAgentId").and_then(JsonValue::as_str), Some(second.as_str()));
    a.send_json(&format!(r#"{{"type":"client:scroll","agentId":"{second}","offset":7}}"#)).await;
    a.wait_for("client:view").await;
    a.send_json(&format!(r#"{{"type":"agent:resize","agentId":"{second}","cols":120,"rows":40}}"#)).await;
    b.send_json(r#"{"type":"client:scroll","agentId":"pane-1","offset":2}"#).await;
    let (view_b, _) = b.wait_for("client:view").await;
    assert_eq!(view_b.get("focusedAgentId"), Some(&JsonValue::Null), "B must not see A's focus");
    // Round-trip on A so its resize has been processed.
    a.send_json(r#"{"type":"ping","t":1}"#).await;
    a.wait_for("pong").await;

    let sa = surface.clients().get(&id_a).unwrap();
    let sb = surface.clients().get(&id_b).unwrap();
    let second_id: usize = second.trim_start_matches("pane-").parse().unwrap();
    assert_eq!(sa.focused_pane_id, Some(second_id));
    assert_eq!(sa.scroll_offsets.get(&second_id), Some(&7));
    assert_eq!(sa.pane_sizes.get(&second_id), Some(&(120, 40)));
    assert_eq!((sa.viewport_cols, sa.viewport_rows), (120, 40));
    assert_eq!(sb.focused_pane_id, None);
    assert_eq!(sb.scroll_offsets.get(&second_id), None);
    assert!(sb.pane_sizes.is_empty());

    // The host workspace focus is untouched by browser-local view state.
    let r = raw(addr, post(addr, "/rpc", r#"{"jsonrpc":"2.0","id":1,"method":"workspace.view_model","params":{}}"#).as_bytes()).await;
    let vm = JsonValue::parse(body(&r)).unwrap();
    let after = vm.get("result").and_then(|v| v.get("z_stack")).and_then(|z| z.get("focusedAgentId")).cloned();
    assert_eq!(after, before);

    // Disconnecting a client drops only its state.
    drop(a);
    for _ in 0..50 {
        if surface.clients().len() == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(surface.clients().client_ids(), vec![id_b]);
}

#[tokio::test]
async fn test_static_spa_path_confinement() {
    let d = spa_dir();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "TOPSECRET").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path().join("secret.txt"), d.path().join("leak.txt")).unwrap();
    let root = StaticRoot::new(d.path()).unwrap();

    assert_eq!(serve_static_asset(&root, "/assets/app-1234.js").status, 200);
    for hostile in ["/../secret.txt", "/%2e%2e/secret.txt", "/assets/%2E%2E/%2E%2E/etc/passwd", "/a\\..\\b", "/%00", "/C:/x"] {
        let a = serve_static_asset(&root, hostile);
        assert_eq!(a.status, 403, "{hostile}");
        assert!(!String::from_utf8_lossy(&a.body).contains("TOPSECRET"));
    }
    #[cfg(unix)]
    {
        let a = serve_static_asset(&root, "/leak.txt");
        assert_eq!(a.status, 404, "symlink escape must not be served");
        assert!(!String::from_utf8_lossy(&a.body).contains("TOPSECRET"));
    }
    assert!(StaticRoot::new(&d.path().join("index.html")).is_err(), "--web-dir must be a directory");

    // Over the wire, unauthenticated (static is browser-facing) and still confined.
    let opts = WebServeOptions { static_root: Some(root), ..WebServeOptions::default() };
    let addr = start(plane(), opts).await;
    let js = raw(addr, get(addr, "/assets/app-1234.js", false).as_bytes()).await;
    assert_eq!(status(&js), 200, "{js}");
    assert!(js.to_ascii_lowercase().contains("content-type: text/javascript"));
    assert!(js.contains("immutable"));
    let bad = raw(addr, get(addr, "/%2e%2e/%2e%2e/etc/passwd", false).as_bytes()).await;
    assert_eq!(status(&bad), 403, "{bad}");
    // API routes are never shadowed by static files and still need the token.
    assert_eq!(status(&raw(addr, get(addr, "/api/agents", false).as_bytes()).await), 401);
}

#[tokio::test]
async fn test_spa_index_html_fallback() {
    let d = spa_dir();
    let root = StaticRoot::new(d.path()).unwrap();
    for route in ["/", "/agents", "/agents/pane-1/logs", "/settings?tab=2"] {
        let a = serve_static_asset(&root, route);
        assert_eq!(a.status, 200, "{route}");
        assert_eq!(a.content_type, "text/html; charset=utf-8");
        assert_eq!(a.cache_control, "no-cache");
        assert!(String::from_utf8_lossy(&a.body).contains("SPA"));
    }
    // Missing assets (with an extension) are a real 404, not index.html.
    assert_eq!(serve_static_asset(&root, "/assets/missing.js").status, 404);

    let opts = WebServeOptions { static_root: Some(root), ..WebServeOptions::default() };
    let addr = start(plane(), opts).await;
    let r = raw(addr, get(addr, "/deep/link", false).as_bytes()).await;
    assert_eq!(status(&r), 200, "{r}");
    assert!(body(&r).contains("<title>SPA</title>"));
    let head = raw(addr, format!("HEAD / HTTP/1.1\r\nHost: {addr}\r\n\r\n").as_bytes()).await;
    assert_eq!(status(&head), 200);
    assert!(body(&head).is_empty(), "HEAD must not carry a body");
}

#[tokio::test]
async fn test_cors_preflight_and_headers() {
    const DEV: &str = "http://localhost:5173";
    let opts = WebServeOptions { cors_origin: Some(DEV.to_string()), ..WebServeOptions::default() };
    let addr = start(plane(), opts).await;

    let pre = format!(
        "OPTIONS /api/agents HTTP/1.1\r\nHost: {addr}\r\nOrigin: {DEV}\r\nAccess-Control-Request-Method: GET\r\nAccess-Control-Request-Headers: authorization\r\n\r\n"
    );
    let r = raw(addr, pre.as_bytes()).await;
    assert_eq!(status(&r), 204, "{r}");
    assert!(r.contains(&format!("Access-Control-Allow-Origin: {DEV}")), "{r}");
    assert!(r.contains("Access-Control-Allow-Headers: Content-Type, Authorization"));
    assert!(r.contains("Access-Control-Allow-Methods: GET, POST"));

    let req = format!("GET /api/agents HTTP/1.1\r\nHost: {addr}\r\nOrigin: {DEV}\r\nAuthorization: Bearer {TOKEN}\r\n\r\n");
    let r = raw(addr, req.as_bytes()).await;
    assert_eq!(status(&r), 200, "{r}");
    assert!(r.contains(&format!("Access-Control-Allow-Origin: {DEV}")));
    // 401s carry CORS too, so the dev app can read the error.
    let req = format!("GET /api/agents HTTP/1.1\r\nHost: {addr}\r\nOrigin: {DEV}\r\n\r\n");
    let r = raw(addr, req.as_bytes()).await;
    assert_eq!(status(&r), 401);
    assert!(r.contains("Access-Control-Allow-Origin"));

    // Any other origin is refused outright, without CORS headers.
    let evil = format!("GET /api/agents HTTP/1.1\r\nHost: {addr}\r\nOrigin: http://evil.example\r\nAuthorization: Bearer {TOKEN}\r\n\r\n");
    let r = raw(addr, evil.as_bytes()).await;
    assert_eq!(status(&r), 403);
    assert!(!r.contains("Access-Control-Allow"));

    // Without --cors-origin there is no preflight support and no CORS header anywhere.
    let addr2 = start(plane(), WebServeOptions::default()).await;
    let r = raw(addr2, format!("OPTIONS /api/agents HTTP/1.1\r\nHost: {addr2}\r\n\r\n").as_bytes()).await;
    assert_eq!(status(&r), 405);
    assert!(!r.contains("Access-Control-Allow"));
}

#[tokio::test]
async fn test_web_surface_uniformity() {
    let mut p = plane();
    let via_rpc = rpc(&mut p, "web.status", "{}");

    p.wm.apply(Action::Web(WebOp::Status)).unwrap();
    let via_action = JsonValue::parse(&p.wm.take_command_data().unwrap()).unwrap();

    let cmd = bilterm::colon_command::parse_colon_command(":web status").unwrap().unwrap();
    assert_eq!(cmd, Action::Web(WebOp::Status));
    p.wm.apply(cmd).unwrap();
    let via_colon = JsonValue::parse(&p.wm.take_command_data().unwrap()).unwrap();

    let mcp = p
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"bilterm_web_surface","arguments":{"action":"status"}}}"#)
        .unwrap();
    let mcp = JsonValue::parse(&mcp).unwrap();
    let text = mcp.get("result").and_then(|r| r.get("content")).and_then(JsonValue::as_array).unwrap()[0]
        .get("text")
        .and_then(JsonValue::as_str)
        .unwrap()
        .to_string();
    let via_mcp = JsonValue::parse(&text).unwrap();

    for (name, v) in [("action", &via_action), ("colon", &via_colon), ("mcp", &via_mcp)] {
        assert_eq!(v, &via_rpc, "{name} status differs from RPC");
    }
    for k in ["enabled", "port", "url", "active_clients"] {
        assert!(via_rpc.get(k).is_some(), "status missing {k}");
    }

    // A port change made on one surface is visible on all others.
    let set = bilterm::colon_command::parse_colon_command(":web port 9123").unwrap().unwrap();
    p.wm.apply(set).unwrap();
    let _ = p.wm.take_command_data();
    assert_eq!(rpc(&mut p, "web.status", "{}").get("port").and_then(JsonValue::as_i64), Some(9123));
    let r = p.handle_jsonrpc_line(r#"{"jsonrpc":"2.0","id":1,"method":"web.port","params":{"port":0}}"#);
    assert!(r.contains("\"error\""), "port 0 must be rejected: {r}");
    assert_eq!(rpc(&mut p, "web.status", "{}").get("port").and_then(JsonValue::as_i64), Some(9123));

    // While served, every surface reports the live listener.
    let opts = WebServeOptions::default();
    let surface = opts.surface.clone();
    let addr = start(p, opts).await;
    for _ in 0..50 {
        if surface.is_enabled() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let r = raw(addr, post(addr, "/rpc", r#"{"jsonrpc":"2.0","id":1,"method":"web.status","params":{}}"#).as_bytes()).await;
    let st = JsonValue::parse(body(&r)).unwrap();
    let st = st.get("result").unwrap();
    assert_eq!(st.get("enabled").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(st.get("port").and_then(JsonValue::as_i64), Some(i64::from(addr.port())));

    // `web.stop` shuts the listener down and the surface reports it.
    let _ = raw(addr, post(addr, "/rpc", r#"{"jsonrpc":"2.0","id":1,"method":"web.stop","params":{}}"#).as_bytes()).await;
    for _ in 0..100 {
        if !surface.is_enabled() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(!surface.is_enabled(), "web.stop must stop the listener");
}
