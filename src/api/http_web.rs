//! Web-surface routes for the HTTP transport (`WEB-FE-01..04`, Phase 60): `/api/*` REST adapters,
//! `/tokens.css`, the `/ws` upgrade, and the options that carry static/CORS/web-surface state.

use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use super::http::RpcRequest;
use super::json::JsonValue;
use super::output_ring::OutputHub;
use super::rpc::AutomationControlPlane;
use super::static_serve::StaticRoot;
use super::view_model::health_json;
use super::web_surface::WebSurfaceState;
use super::ws::{WsContext, rpc, serve_ws, upgrade_response};

/// Web-surface options for the HTTP server (`--web-dir`, `--cors-origin`, shared state).
#[derive(Clone, Debug, Default)]
pub struct WebServeOptions {
    pub surface: WebSurfaceState,
    pub hub: OutputHub,
    pub static_root: Option<StaticRoot>,
    pub cors_origin: Option<String>,
    /// Traffic meter of the bound plane's `WindowManager` (Phase 64); `None` = not metered.
    pub meter: Option<crate::traffic::TrafficMeter>,
}

impl WebServeOptions {
    /// Shares this server's state with `plane`: the plane's output hub is adopted and the plane's
    /// `WindowManager` reports this server's web surface (so `:web status` sees it).
    pub fn bind_plane(&mut self, plane: &mut AutomationControlPlane) {
        self.hub = plane.output_hub.clone();
        self.meter = Some(plane.wm.traffic_meter());
        self.surface.set_web_dir(self.static_root.as_ref().map(|r| r.path().to_path_buf()));
        self.surface.set_cors_origin(self.cors_origin.clone());
        plane.wm.set_web_surface(self.surface.clone());
    }
}

/// Protocol `ErrorBody` (`{ok:false, error:{code,message,retryable}}`) and its HTTP status.
pub fn error_body(message: &str) -> (u16, String) {
    let (status, code) = if message.contains("not found") {
        (404, "NOT_FOUND")
    } else if message.contains("unavailable") {
        (503, "BACKEND_DOWN")
    } else if message.contains("exceeds") {
        (413, "PAYLOAD_TOO_LARGE")
    } else {
        (400, "USAGE")
    };
    let body = JsonValue::obj([
        ("ok", JsonValue::Bool(false)),
        (
            "error",
            JsonValue::obj([
                ("code", JsonValue::String(code.to_string())),
                ("message", JsonValue::String(message.to_string())),
                ("retryable", JsonValue::Bool(status == 503)),
            ]),
        ),
    ]);
    (status, body.to_json_string())
}

fn query_param<'a>(full_path: &'a str, key: &str) -> Option<&'a str> {
    full_path
        .split_once('?')?
        .1
        .split('&')
        .find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v))
}

/// Routes `/api/*` (bearer-authenticated by the caller). Returns `(status, json body)`.
pub async fn api_route(
    tx: &mpsc::Sender<RpcRequest>,
    method: &str,
    full_path: &str,
    body: &[u8],
) -> (u16, String) {
    let path = full_path.split('?').next().unwrap_or("");
    let segs: Vec<&str> = path.trim_start_matches("/api/").split('/').filter(|s| !s.is_empty()).collect();
    let result = match (method, segs.as_slice()) {
        ("GET", ["health"]) => rpc(tx, "web.agents", JsonValue::obj([] as [(&str, JsonValue); 0]))
            .await
            .map(|r| health_json(r.get("agents").and_then(JsonValue::as_array).map_or(0, <[JsonValue]>::len))),
        ("GET", ["agents"]) => rpc(tx, "web.agents", JsonValue::obj([] as [(&str, JsonValue); 0])).await,
        ("GET", ["agents", id]) => rpc(tx, "web.agent", JsonValue::obj([("agentId", JsonValue::String((*id).to_string()))])).await,
        ("GET", ["agents", id, "logs"]) => {
            let mut p = vec![("agentId", JsonValue::String((*id).to_string()))];
            for (q, k) in [("beforeSeq", "beforeSeq"), ("afterSeq", "afterSeq"), ("limit", "limit")] {
                if let Some(n) = query_param(full_path, q).and_then(|v| v.parse::<i64>().ok()) {
                    p.push((k, JsonValue::Number(n)));
                }
            }
            rpc(tx, "web.logs", JsonValue::obj(p)).await
        }
        ("POST", ["search"]) => match std::str::from_utf8(body).ok().and_then(JsonValue::parse) {
            Some(params) => rpc(tx, "web.search", params).await,
            None => Err("search body must be a JSON object".to_string()),
        },
        (_, ["health" | "agents" | "search", ..]) => return (405, error_body("method not allowed").1),
        _ => Err("route not found".to_string()),
    };
    match result {
        Ok(v) => (200, v.to_json_string()),
        Err(e) => error_body(&e),
    }
}

/// `GET /tokens.css` body (CSS custom properties of the active appearance profile).
pub async fn tokens_css(tx: &mpsc::Sender<RpcRequest>) -> Option<String> {
    let vm = rpc(tx, "workspace.view_model", JsonValue::obj([] as [(&str, JsonValue); 0])).await.ok()?;
    vm.get("css_tokens").and_then(JsonValue::as_str).map(str::to_string)
}

/// Completes the `/ws` upgrade and runs the protocol bridge.
pub async fn upgrade(
    mut stream: super::metered::Metered<tokio::net::TcpStream>,
    header: impl Fn(&str) -> Option<String>,
    full_path: &str,
    header_authed: bool,
    rpc_tx: mpsc::Sender<RpcRequest>,
    token: Arc<str>,
    opts: &WebServeOptions,
) -> std::io::Result<()> {
    let head = match upgrade_response(&header) {
        Ok(h) => h,
        Err(msg) => {
            let body = error_body(msg).1;
            let resp = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(resp.as_bytes()).await?;
            return stream.flush().await;
        }
    };
    let query_authed = query_param(full_path, "token")
        .is_some_and(|t| crate::secure_fs::constant_time_eq(t.as_bytes(), token.as_bytes()));
    stream.write_all(head.as_bytes()).await?;
    stream.flush().await?;
    let ctx = WsContext {
        rpc_tx,
        hub: opts.hub.clone(),
        surface: opts.surface.clone(),
        token,
        pre_authed: header_authed || query_authed,
        meter: opts.meter.clone(),
    };
    // The handshake was counted on the HTTP channel; frames are counted on `ws:web-N`.
    serve_ws(stream.into_inner(), ctx).await
}
