//! Opt-in loopback HTTP/1.1 + SSE transport mirroring the JSON-RPC control plane (`bilterm --http 127.0.0.1:PORT`).
//!
//! Routes (every route requires `Authorization: Bearer <token>`):
//! - `GET /health`  -> `{"ok":true,"version":".."}`
//! - `POST /rpc`    -> body is a JSON-RPC 2.0 request; reply is the same JSON-RPC response
//!   (dispatched through the SAME `AutomationControlPlane` handler as the Unix/TCP server).
//! - `GET /events`  -> `text/event-stream`, fed by `events.poll` (drain) on the same plane.
//!
//! Security: loopback-only bind, constant-time token compare, Host/Origin checks (DNS-rebinding defence),
//! no CORS headers, 1 MiB body cap, 16 KiB header cap, 64 header lines, read timeout, connection cap.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

use super::json::JsonValue;
use super::rpc::AutomationControlPlane;
use crate::error::{BiltermError, Result};

/// One queued JSON-RPC request line plus its reply channel (shared with the Unix/TCP server loop).
pub type RpcRequest = (String, oneshot::Sender<String>);

/// Bound loopback listener plus bearer token for the optional HTTP transport.
pub struct HttpListen {
    pub listener: tokio::net::TcpListener,
    pub token: Arc<str>,
}

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
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(BiltermError::Config(
            "refusing to serve HTTP on a non-loopback listener".to_string(),
        ));
    }
    let permits = Arc::new(tokio::sync::Semaphore::new(MAX_CONNECTIONS));
    loop {
        let (stream, peer) = match listener.accept().await {
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
        let (tx, token) = (rpc_tx.clone(), token.clone());
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = handle_connection(stream, tx, token).await {
                tracing::debug!(%peer, error = %e, "HTTP connection closed");
            }
        });
    }
}

/// Serves HTTP with a dedicated plane (standalone / tests); the plane is owned by this loop.
pub async fn serve_http_standalone(
    listener: tokio::net::TcpListener,
    mut plane: AutomationControlPlane,
    token: Arc<str>,
) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<RpcRequest>(64);
    let server = serve_http(listener, tx, token);
    tokio::pin!(server);
    loop {
        tokio::select! {
            r = &mut server => return r,
            Some((line, reply)) = rx.recv() => { let _ = reply.send(plane.handle_jsonrpc_line(&line)); }
        }
    }
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
    let head = format!(
        "HTTP/1.1 {code} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n{extra}\r\n",
        reason(code),
        body.len()
    );
    w.write_all(head.as_bytes()).await?;
    w.write_all(body.as_bytes()).await?;
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

/// DNS-rebinding / cross-origin defence: Host must be loopback; any Origin must be same-origin.
fn check_origin(req: &Request) -> std::result::Result<(), HttpError> {
    let host = req
        .header("host")
        .ok_or(HttpError(403, "missing Host header"))?;
    if !is_loopback_host(host_only(host)) {
        return Err(HttpError(403, "Host not allowed"));
    }
    if let Some(origin) = req.header("origin") {
        let ok = origin
            .strip_prefix("http://")
            .is_some_and(|o| o.eq_ignore_ascii_case(host));
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

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    rpc_tx: mpsc::Sender<RpcRequest>,
    token: Arc<str>,
) -> std::io::Result<()> {
    let req = match tokio::time::timeout(READ_TIMEOUT, read_request(&mut stream)).await {
        Err(_) => return write_response(&mut stream, 408, "", &err_body("request timeout")).await,
        Ok(Err(HttpError(code, msg))) => {
            return write_response(&mut stream, code, "", &err_body(msg)).await;
        }
        Ok(Ok(r)) => r,
    };
    if let Err(HttpError(code, msg)) = check_origin(&req).and_then(|()| check_auth(&req, &token)) {
        let extra = if code == 401 {
            "WWW-Authenticate: Bearer\r\n"
        } else {
            ""
        };
        return write_response(&mut stream, code, extra, &err_body(msg)).await;
    }
    let path = req.path.split('?').next().unwrap_or("");
    match (req.method.as_str(), path) {
        ("GET", "/health") => {
            let body = JsonValue::obj([
                ("ok", JsonValue::Bool(true)),
                (
                    "version",
                    JsonValue::String(env!("CARGO_PKG_VERSION").to_string()),
                ),
            ])
            .to_json_string();
            write_response(&mut stream, 200, "", &body).await
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
                Some(resp) => write_response(&mut stream, 200, "", resp.trim_end()).await,
                None => {
                    write_response(&mut stream, 503, "", &err_body("control plane unavailable"))
                        .await
                }
            }
        }
        ("GET", "/events") => serve_sse(stream, rpc_tx).await,
        (_, "/health" | "/rpc" | "/events") => {
            write_response(&mut stream, 405, "", &err_body("method not allowed")).await
        }
        _ => write_response(&mut stream, 404, "", &err_body("not found")).await,
    }
}

/// Streams events drained via `events.poll` as SSE `data:` frames until the client disconnects.
async fn serve_sse(
    mut stream: tokio::net::TcpStream,
    rpc_tx: mpsc::Sender<RpcRequest>,
) -> std::io::Result<()> {
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
