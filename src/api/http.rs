//! Opt-in loopback HTTP/1.1 + SSE transport mirroring the JSON-RPC control plane (`bilterm --http 127.0.0.1:PORT`).
//!
//! Bearer-authenticated routes (`Authorization: Bearer <token>`):
//! - `GET /health`  -> `{ok,version,contract,epoch,backends,agents}` (`@agent-manager/protocol` health)
//! - `POST /rpc`    -> body is a JSON-RPC 2.0 request; reply is the same JSON-RPC response
//!   (dispatched through the SAME `AutomationControlPlane` handler as the Unix/TCP server).
//! - `GET /events`  -> `text/event-stream`, fed by `events.poll` (drain) on the same plane.
//! - `/api/*`       -> protocol REST adapters (`http_web::api_route`).
//!
//! Browser-facing routes (Phase 60, no bearer header; Host/Origin checks still apply):
//! - `GET /ws` (token via header, `?token=` or `hello.token`), `GET /tokens.css`,
//!   and static `--web-dir` files with SPA fallback.
//!
//! Security: loopback-only bind, constant-time token compare, Host/Origin checks (DNS-rebinding defence),
//! CORS only for the single `--cors-origin`, 1 MiB body cap, 16 KiB header cap, 64 header lines,
//! read timeout, connection cap.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

use super::agent_wait::{AgentWaiters, WAIT_TICK};
pub use super::http_web::WebServeOptions;
use super::json::JsonValue;
use super::metered::Metered;
use super::rpc::AutomationControlPlane;
use super::static_serve::{cors_headers, serve_static_asset};
use crate::error::{BiltermError, Result};
use crate::traffic::{Origin, Role};

/// One queued JSON-RPC request line plus its reply channel (shared with the Unix/TCP server loop).
pub type RpcRequest = (String, oneshot::Sender<String>);

/// Bound loopback listener plus bearer token for the optional HTTP transport.
pub struct HttpListen {
    pub listener: tokio::net::TcpListener,
    pub token: Arc<str>,
    /// Web-surface options (`--web-dir`, `--cors-origin`, shared surface state).
    pub web: WebServeOptions,
}

const STOP_POLL_INTERVAL: Duration = Duration::from_millis(200);

pub const MAX_HTTP_BODY_BYTES: usize = 1024 * 1024;
pub const MAX_HTTP_HEAD_BYTES: usize = 16 * 1024;
const MAX_HEADERS: usize = 64;
const MAX_CONNECTIONS: usize = 64;
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const SSE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const SSE_KEEPALIVE_EVERY: u32 = 60; // polls (~15 s)

/// Parses `--http` bind addresses; any non-loopback address is refused.
pub fn validate_http_bind(s: &str) -> Result<SocketAddr> {
    let addr: SocketAddr = if let Ok(a) = s.parse::<SocketAddr>() {
        a
    } else if let Some(p) = s.strip_prefix("localhost:") {
        let port: u16 = p
            .parse()
            .map_err(|_| BiltermError::Config(format!("invalid port in --http address: '{s}'")))?;
        SocketAddr::from(([127, 0, 0, 1], port))
    } else {
        return Err(BiltermError::Config(format!(
            "invalid --http address: '{s}' (must be 127.0.0.1:PORT, [::1]:PORT or localhost:PORT)"
        )));
    };
    if !addr.ip().is_loopback() {
        return Err(BiltermError::Config(format!(
            "refusing to serve HTTP on non-loopback address {addr}; --http is loopback-only"
        )));
    }
    Ok(addr)
}

/// Generates a 256-bit random bearer token (hex) from the OS CSPRNG.
pub fn generate_http_token() -> Result<String> {
    #[cfg(unix)]
    {
        use std::io::Read;
        let mut buf = [0u8; 32];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut buf)?;
        Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
    }
    #[cfg(not(unix))]
    {
        Err(BiltermError::Config(
            "HTTP token generation is unsupported on this platform".to_string(),
        ))
    }
}

/// Writes `token` to `<private runtime dir>/bilterm-http.token` (0600, atomic) and returns the path.
pub fn write_http_token_file(token: &str) -> Result<std::path::PathBuf> {
    let dir = crate::secure_fs::private_runtime_dir()?;
    let path = dir.join("bilterm-http.token");
    crate::secure_fs::write_private_file(&path, token.as_bytes())?;
    Ok(path)
}

/// Serves HTTP on `listener`, forwarding JSON-RPC lines to `rpc_tx` (the plane owner's channel).
pub async fn serve_http(
    listener: tokio::net::TcpListener,
    rpc_tx: mpsc::Sender<RpcRequest>,
    token: Arc<str>,
) -> Result<()> {
    serve_http_with(listener, rpc_tx, token, WebServeOptions::default()).await
}

/// [`serve_http`] with web-surface options. Returns `Ok(())` after `:web stop` / `web.stop`.
pub async fn serve_http_with(
    listener: tokio::net::TcpListener,
    rpc_tx: mpsc::Sender<RpcRequest>,
    token: Arc<str>,
    opts: WebServeOptions,
) -> Result<()> {
    let local = listener.local_addr()?;
    if !local.ip().is_loopback() {
        return Err(BiltermError::Config(
            "refusing to serve HTTP on a non-loopback listener".to_string(),
        ));
    }
    let opts = Arc::new(opts);
    opts.surface.mark_listening(local.port());
    let permits = Arc::new(tokio::sync::Semaphore::new(MAX_CONNECTIONS));
    let mut stop_tick = tokio::time::interval(STOP_POLL_INTERVAL);
    loop {
        let accepted = tokio::select! {
            a = listener.accept() => a,
            _ = stop_tick.tick() => {
                if opts.surface.is_stop_requested() {
                    opts.surface.mark_stopped();
                    return Ok(());
                }
                continue;
            }
        };
        let (stream, peer) = match accepted {
            Ok(x) => x,
            Err(e) => {
                tracing::warn!(error = %e, "HTTP accept failed");
                continue;
            }
        };
        let Ok(permit) = permits.clone().try_acquire_owned() else {
            drop(stream);
            continue;
        };
        let (tx, token, opts) = (rpc_tx.clone(), token.clone(), opts.clone());
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = handle_connection(stream, tx, token, opts).await {
                tracing::debug!(%peer, error = %e, "HTTP connection closed");
            }
        });
    }
}

/// Serves HTTP with a dedicated plane (standalone / tests); the plane is owned by this loop.
pub async fn serve_http_standalone(
    listener: tokio::net::TcpListener,
    plane: AutomationControlPlane,
    token: Arc<str>,
) -> Result<()> {
    serve_http_standalone_with(listener, plane, token, WebServeOptions::default()).await
}

/// [`serve_http_standalone`] with web-surface options bound to `plane`.
pub async fn serve_http_standalone_with(
    listener: tokio::net::TcpListener,
    mut plane: AutomationControlPlane,
    token: Arc<str>,
    mut opts: WebServeOptions,
) -> Result<()> {
    opts.bind_plane(&mut plane);
    let (tx, mut rx) = mpsc::channel::<RpcRequest>(64);
    let server = serve_http_with(listener, tx, token, opts);
    tokio::pin!(server);
    let mut waiters = AgentWaiters::default();
    let mut wait_tick = tokio::time::interval(WAIT_TICK);
    loop {
        tokio::select! {
            r = &mut server => return r,
            Some((line, reply)) = rx.recv() => waiters.handle(&mut plane, &line, reply),
            _ = wait_tick.tick(), if !waiters.is_empty() => waiters.poll(&plane),
        }
    }
}

fn next_label(kind: &str) -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!("{kind}#{}", SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1)
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

struct HttpError(u16, &'static str);

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

async fn write_response<W: AsyncWriteExt + Unpin>(
    w: &mut W,
    code: u16,
    extra: &str,
    body: &str,
) -> std::io::Result<()> {
    let r = Reply { code, content_type: "application/json", cache_control: "no-store", extra, head_only: false };
    write_typed(w, &r, body.as_bytes()).await
}

/// Response metadata for [`write_typed`].
struct Reply<'a> {
    code: u16,
    content_type: &'a str,
    cache_control: &'a str,
    extra: &'a str,
    head_only: bool,
}

async fn write_typed<W: AsyncWriteExt + Unpin>(w: &mut W, r: &Reply<'_>, body: &[u8]) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: {}\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n{}\r\n",
        r.code,
        reason(r.code),
        r.content_type,
        body.len(),
        r.cache_control,
        r.extra
    );
    w.write_all(head.as_bytes()).await?;
    if !r.head_only {
        w.write_all(body).await?;
    }
    w.flush().await
}

fn err_body(msg: &str) -> String {
    JsonValue::obj([("error", JsonValue::String(msg.to_string()))]).to_json_string()
}

async fn read_request<S: AsyncReadExt + Unpin>(
    s: &mut S,
) -> std::result::Result<Request, HttpError> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 2048];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HTTP_HEAD_BYTES {
            return Err(HttpError(431, "header section too large"));
        }
        let n = s
            .read(&mut chunk)
            .await
            .map_err(|_| HttpError(400, "read error"))?;
        if n == 0 {
            return Err(HttpError(400, "incomplete request"));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    if head_end > MAX_HTTP_HEAD_BYTES {
        return Err(HttpError(431, "header section too large"));
    }
    let head = std::str::from_utf8(&buf[..head_end])
        .map_err(|_| HttpError(400, "invalid header encoding"))?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split(' ');
    let (Some(method), Some(path), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(HttpError(400, "malformed request line"));
    };
    if !(version == "HTTP/1.1" || version == "HTTP/1.0")
        || method.is_empty()
        || !method.bytes().all(|b| b.is_ascii_uppercase())
        || !path.starts_with('/')
        || path.bytes().any(|b| b <= b' ' || b == 0x7f)
    {
        return Err(HttpError(400, "malformed request line"));
    }
    let mut headers = Vec::new();
    for line in lines {
        if headers.len() >= MAX_HEADERS {
            return Err(HttpError(431, "too many headers"));
        }
        let Some((k, v)) = line.split_once(':') else {
            return Err(HttpError(400, "malformed header"));
        };
        if k.is_empty() || k.bytes().any(|b| b <= b' ' || b == 0x7f) {
            return Err(HttpError(400, "malformed header name"));
        }
        headers.push((k.to_ascii_lowercase(), v.trim().to_string()));
    }
    let mut req = Request {
        method: method.to_string(),
        path: path.to_string(),
        headers,
        body: Vec::new(),
    };
    if req.header("transfer-encoding").is_some() {
        return Err(HttpError(400, "transfer-encoding is not supported"));
    }
    let mut cl_values = req.headers.iter().filter(|(k, _)| k == "content-length");
    let content_length = match (cl_values.next(), cl_values.next()) {
        (None, _) => 0usize,
        (Some((_, v)), None) => {
            if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
                return Err(HttpError(400, "invalid content-length"));
            }
            v.parse::<usize>()
                .map_err(|_| HttpError(413, "body too large"))?
        }
        _ => return Err(HttpError(400, "duplicate content-length")),
    };
    if content_length > MAX_HTTP_BODY_BYTES {
        return Err(HttpError(413, "body too large"));
    }
    let mut body = buf[head_end + 4..].to_vec();
    if body.len() > content_length {
        return Err(HttpError(400, "unexpected data after body"));
    }
    while body.len() < content_length {
        let n = s
            .read(&mut chunk)
            .await
            .map_err(|_| HttpError(400, "read error"))?;
        if n == 0 {
            return Err(HttpError(400, "incomplete body"));
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > content_length {
            return Err(HttpError(400, "unexpected data after body"));
        }
    }
    req.body = body;
    Ok(req)
}

fn host_only(hostport: &str) -> &str {
    if let Some(rest) = hostport.strip_prefix('[') {
        return rest.split(']').next().unwrap_or("");
    }
    hostport.rsplit_once(':').map_or(hostport, |(h, _)| h)
}

fn is_loopback_host(h: &str) -> bool {
    h.eq_ignore_ascii_case("localhost")
        || h.parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// DNS-rebinding / cross-origin defence: Host must be loopback; any Origin must be same-origin
/// or exactly the configured `--cors-origin`.
fn check_origin(req: &Request, cors_origin: Option<&str>) -> std::result::Result<(), HttpError> {
    let host = req
        .header("host")
        .ok_or(HttpError(403, "missing Host header"))?;
    if !is_loopback_host(host_only(host)) {
        return Err(HttpError(403, "Host not allowed"));
    }
    if let Some(origin) = req.header("origin") {
        let ok = origin
            .strip_prefix("http://")
            .is_some_and(|o| o.eq_ignore_ascii_case(host))
            || cors_origin.is_some_and(|c| c.eq_ignore_ascii_case(origin));
        if !ok {
            return Err(HttpError(403, "cross-origin request refused"));
        }
    }
    Ok(())
}

fn check_auth(req: &Request, token: &str) -> std::result::Result<(), HttpError> {
    let presented = req
        .header("authorization")
        .and_then(|v| v.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, t)| t.trim());
    match presented {
        Some(t) if crate::secure_fs::constant_time_eq(t.as_bytes(), token.as_bytes()) => Ok(()),
        _ => Err(HttpError(401, "missing or invalid bearer token")),
    }
}

async fn rpc_call(tx: &mpsc::Sender<RpcRequest>, line: String) -> Option<String> {
    let (r_tx, r_rx) = oneshot::channel();
    tx.send((line, r_tx)).await.ok()?;
    r_rx.await.ok()
}

fn is_api_path(path: &str) -> bool {
    matches!(path, "/health" | "/rpc" | "/events") || path == "/api" || path.starts_with("/api/")
}

/// Browser-facing routes served before bearer auth. Returns the stream back when not handled.
async fn serve_pre_auth(
    mut stream: Metered<tokio::net::TcpStream>,
    req: &Request,
    rpc_tx: &mpsc::Sender<RpcRequest>,
    token: &Arc<str>,
    opts: &WebServeOptions,
    cors: &str,
) -> std::io::Result<Option<Metered<tokio::net::TcpStream>>> {
    let path = req.path.split('?').next().unwrap_or("");
    let (method, head_only) = (req.method.as_str(), req.method == "HEAD");
    if method == "OPTIONS" {
        let code = if cors.is_empty() { 405 } else { 204 };
        let r = Reply { code, content_type: "text/plain", cache_control: "no-store", extra: cors, head_only: false };
        write_typed(&mut stream, &r, b"").await?;
        return Ok(None);
    }
    if method == "GET" && path == "/ws" {
        let header_authed = check_auth(req, token).is_ok();
        let header = |n: &str| req.header(n).map(str::to_string);
        super::http_web::upgrade(stream, header, &req.path, header_authed, rpc_tx.clone(), token.clone(), opts)
            .await?;
        return Ok(None);
    }
    if matches!(method, "GET" | "HEAD") && path == "/tokens.css" {
        let Some(css) = super::http_web::tokens_css(rpc_tx).await else {
            write_response(&mut stream, 503, cors, &err_body("control plane unavailable")).await?;
            return Ok(None);
        };
        let r = Reply { code: 200, content_type: "text/css; charset=utf-8", cache_control: "no-cache", extra: cors, head_only };
        write_typed(&mut stream, &r, css.as_bytes()).await?;
        return Ok(None);
    }
    if let Some(root) = opts.static_root.as_ref().filter(|_| matches!(method, "GET" | "HEAD") && !is_api_path(path)) {
        let a = serve_static_asset(root, &req.path);
        let r = Reply { code: a.status, content_type: a.content_type, cache_control: a.cache_control, extra: cors, head_only };
        write_typed(&mut stream, &r, &a.body).await?;
        return Ok(None);
    }
    Ok(Some(stream))
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    rpc_tx: mpsc::Sender<RpcRequest>,
    token: Arc<str>,
    opts: Arc<WebServeOptions>,
) -> std::io::Result<()> {
    // One `Origin::Http` channel per connection (rolled up when it closes); every request read
    // and response write goes through the `Metered` wrapper (hooks H20/H21).
    let tap = match opts.meter.as_ref() {
        Some(m) => m.open_channel(Origin::Http, &next_label("http"), None, Role::Wire),
        None => crate::traffic::ChannelCounters::with_flags(0),
    };
    let mut stream = Metered::new(stream, tap);
    let req = match tokio::time::timeout(READ_TIMEOUT, read_request(&mut stream)).await {
        Err(_) => return write_response(&mut stream, 408, "", &err_body("request timeout")).await,
        Ok(Err(HttpError(code, msg))) => {
            return write_response(&mut stream, code, "", &err_body(msg)).await;
        }
        Ok(Ok(r)) => r,
    };
    if let Err(HttpError(code, msg)) = check_origin(&req, opts.cors_origin.as_deref()) {
        return write_response(&mut stream, code, "", &err_body(msg)).await;
    }
    let cors = cors_headers(opts.cors_origin.as_deref());
    let Some(mut stream) = serve_pre_auth(stream, &req, &rpc_tx, &token, &opts, &cors).await? else {
        return Ok(());
    };
    if let Err(HttpError(code, msg)) = check_auth(&req, &token) {
        let extra = format!("WWW-Authenticate: Bearer\r\n{cors}");
        return write_response(&mut stream, code, &extra, &err_body(msg)).await;
    }
    let path = req.path.split('?').next().unwrap_or("");
    if path == "/api" || path.starts_with("/api/") {
        let (code, body) = super::http_web::api_route(&rpc_tx, &req.method, &req.path, &req.body).await;
        return write_response(&mut stream, code, &cors, &body).await;
    }
    match (req.method.as_str(), path) {
        ("GET", "/health") => {
            let (code, body) = super::http_web::api_route(&rpc_tx, "GET", "/api/health", &[]).await;
            write_response(&mut stream, code, &cors, &body).await
        }
        ("POST", "/rpc") => {
            let Ok(text) = std::str::from_utf8(&req.body) else {
                return write_response(&mut stream, 400, "", &err_body("body is not UTF-8")).await;
            };
            let line = text.trim();
            if line.is_empty() {
                return write_response(&mut stream, 400, "", &err_body("empty body")).await;
            }
            match rpc_call(&rpc_tx, line.to_string()).await {
                Some(resp) => write_response(&mut stream, 200, &cors, resp.trim_end()).await,
                None => {
                    write_response(&mut stream, 503, "", &err_body("control plane unavailable"))
                        .await
                }
            }
        }
        ("GET", "/events") => serve_sse(stream, rpc_tx, &opts).await,
        (_, "/health" | "/rpc" | "/events") => {
            write_response(&mut stream, 405, "", &err_body("method not allowed")).await
        }
        _ => write_response(&mut stream, 404, "", &err_body("not found")).await,
    }
}

/// Streams events drained via `events.poll` as SSE `data:` frames until the client disconnects.
async fn serve_sse(
    mut stream: Metered<tokio::net::TcpStream>,
    rpc_tx: mpsc::Sender<RpcRequest>,
    opts: &WebServeOptions,
) -> std::io::Result<()> {
    // SSE frames and the EOF probe are counted on their own `Origin::Sse` channel (H22/H23).
    if let Some(m) = opts.meter.as_ref() {
        stream.set_tap(m.open_channel(Origin::Sse, &next_label("sse"), None, Role::Wire));
    }
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n: bilterm events\n\n")
        .await?;
    stream.flush().await?;
    let poll = r#"{"jsonrpc":"2.0","id":0,"method":"events.poll","params":{"drain":true}}"#;
    let mut idle = 0u32;
    let mut seq = 0u64;
    let mut probe = [0u8; 1];
    loop {
        let Some(resp) = rpc_call(&rpc_tx, poll.to_string()).await else {
            return Ok(());
        };
        let mut out = String::new();
        if let Some(events) = JsonValue::parse(&resp)
            .and_then(|v| v.get("result").and_then(|r| r.get("events")).cloned())
        {
            for ev in events.as_array().unwrap_or(&[]) {
                seq += 1;
                out.push_str(&format!("id: {seq}\ndata: {}\n\n", ev.to_json_string()));
            }
        }
        idle = if out.is_empty() { idle + 1 } else { 0 };
        if idle >= SSE_KEEPALIVE_EVERY {
            idle = 0;
            out.push_str(": keepalive\n\n");
        }
        if !out.is_empty() {
            stream.write_all(out.as_bytes()).await?;
            stream.flush().await?;
        }
        // Sleep, but notice client EOF promptly.
        tokio::select! {
            _ = tokio::time::sleep(SSE_POLL_INTERVAL) => {}
            r = stream.read(&mut probe) => {
                if matches!(r, Ok(0) | Err(_)) { return Ok(()); }
            }
        }
    }
}
