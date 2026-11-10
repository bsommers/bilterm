//! Headless JSON-RPC 2.0 daemon listeners, Unix domain socket / TCP security gates, and client helpers.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use super::agent_wait::{AgentWaiters, WAIT_TICK};
use super::http::{HttpListen, RpcRequest};
use super::json::JsonValue;
use super::rpc::AutomationControlPlane;
use crate::error::{BiltermError, Result};
use crate::traffic::{Origin, Role};

/// Listening endpoint for the headless JSON-RPC automation daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenEndpoint {
    Unix(PathBuf),
    Tcp(SocketAddr),
}

/// Headless JSON-RPC daemon configuration with an optional bearer token.
///
/// Formatted output explicitly redacts the secret token to prevent accidental log leaks.
#[derive(Clone, PartialEq, Eq)]
pub struct ListenConfig {
    pub endpoint: ListenEndpoint,
    pub token: Option<String>,
}

impl std::fmt::Debug for ListenConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListenConfig")
            .field("endpoint", &self.endpoint)
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// Default loopback TCP listener endpoint used when no Unix socket path is available.
pub const DEFAULT_TCP_LISTEN: &str = "127.0.0.1:7370";

/// JSON-RPC 2.0 error code returned when authentication fails or is required before requests.
pub const RPC_AUTH_ERROR_CODE: i64 = -32001;

/// Conservative maximum byte length for Unix domain socket paths (`sockaddr_un::sun_path`).
///
/// POSIX systems specify 104 bytes on macOS / BSD and 108 bytes on Linux, inclusive of the NUL
/// terminator.
const MAX_SOCKET_PATH_BYTES: usize = 103;

/// Resolves the default socket path inside `dir`, enforcing the conservative path length limit.
pub fn socket_path_in(dir: &Path) -> std::io::Result<PathBuf> {
    let path = dir.join("bilterm.sock");
    if path.as_os_str().len() >= MAX_SOCKET_PATH_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "default socket path '{}' exceeds Unix socket path length limit ({} bytes); pass --listen unix:PATH with a shorter path",
                path.display(),
                MAX_SOCKET_PATH_BYTES
            ),
        ));
    }
    Ok(path)
}

/// Returns the default Unix domain socket path in Bilterm's private runtime directory.
pub fn default_socket_path() -> std::io::Result<PathBuf> {
    #[cfg(unix)]
    {
        let dir = crate::secure_fs::private_runtime_dir()?;
        socket_path_in(&dir)
    }
    #[cfg(not(unix))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Unix domain sockets not supported on non-Unix platforms",
        ))
    }
}

/// Reads a bearer token from a private 0600 file.
pub fn load_token_file(path: &Path) -> Result<String> {
    crate::secure_fs::read_secret_file(path)
        .map_err(|e| BiltermError::Config(format!("token file {}: {e}", path.display())))
}

/// Validates and normalizes listen configuration according to security policy (SEC-R01).
#[cfg_attr(not(unix), allow(unused_variables))]
pub fn validate_listen_config(
    listen: Option<&str>,
    allow_remote: bool,
    token: Option<String>,
    default_unix_path: Option<PathBuf>,
) -> Result<ListenConfig> {
    match listen {
        None => {
            #[cfg(unix)]
            {
                if let Some(path) = default_unix_path {
                    Ok(ListenConfig {
                        endpoint: ListenEndpoint::Unix(path),
                        token,
                    })
                } else {
                    Err(BiltermError::Config(
                        "default private runtime directory is unavailable; specify --listen unix:PATH"
                            .to_string(),
                    ))
                }
            }
            #[cfg(not(unix))]
            {
                let addr: SocketAddr = DEFAULT_TCP_LISTEN.parse().map_err(|e| {
                    BiltermError::Config(format!("invalid default listen address: {e}"))
                })?;
                if token.is_none() {
                    return Err(BiltermError::Config(
                        "default TCP listener requires a bearer token via --token-file (mode 0600)"
                            .to_string(),
                    ));
                }
                Ok(ListenConfig {
                    endpoint: ListenEndpoint::Tcp(addr),
                    token,
                })
            }
        }
        Some(s) => {
            let unix_path = s
                .strip_prefix("unix://")
                .or_else(|| s.strip_prefix("unix:"))
                .or_else(|| if s.starts_with('/') { Some(s) } else { None });

            if let Some(path_str) = unix_path {
                #[cfg(unix)]
                {
                    return Ok(ListenConfig {
                        endpoint: ListenEndpoint::Unix(PathBuf::from(path_str)),
                        token,
                    });
                }
                #[cfg(not(unix))]
                {
                    return Err(BiltermError::Config(
                        "Unix domain sockets are not supported on non-Unix platforms".to_string(),
                    ));
                }
            }

            let addr: SocketAddr = if let Ok(addr) = s.parse::<SocketAddr>() {
                addr
            } else if let Some(port_str) = s.strip_prefix("localhost:") {
                let port: u16 = port_str.parse().map_err(|_| {
                    BiltermError::Config(format!("invalid port in listen address: '{s}'"))
                })?;
                SocketAddr::from(([127, 0, 0, 1], port))
            } else {
                return Err(BiltermError::Config(format!(
                    "invalid listen address: '{s}' (must be IP:PORT, localhost:PORT, or unix:PATH)"
                )));
            };

            if addr.ip().is_loopback() {
                // Loopback TCP is reachable by every local user and by browsers (cross-protocol
                // requests), so it needs a bearer token just like remote listeners.
                if token.is_none() {
                    return Err(BiltermError::Config(
                        "TCP listener requires a bearer token via --token-file (mode 0600); use the default Unix socket for tokenless local access"
                            .to_string(),
                    ));
                }
                Ok(ListenConfig {
                    endpoint: ListenEndpoint::Tcp(addr),
                    token,
                })
            } else {
                if !allow_remote {
                    return Err(BiltermError::Config(format!(
                        "refusing to listen on non-loopback address {addr} without --allow-remote"
                    )));
                }
                if token.is_none() {
                    return Err(BiltermError::Config(
                        "--allow-remote requires a bearer token via --token-file (mode 0600)"
                            .to_string(),
                    ));
                }
                Ok(ListenConfig {
                    endpoint: ListenEndpoint::Tcp(addr),
                    token,
                })
            }
        }
    }
}

async fn rpc_roundtrip<S>(
    stream: S,
    token: Option<&str>,
    request_line: String,
) -> Result<String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let (rd, mut wr) = tokio::io::split(stream);
    let mut reader = BufReader::new(rd);

    if let Some(tok) = token {
        let auth_req = JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", JsonValue::Number(0)),
            ("method", JsonValue::String("auth.login".to_string())),
            (
                "params",
                JsonValue::obj([("token", JsonValue::String(tok.to_string()))]),
            ),
        ])
        .to_json_string();

        wr.write_all(format!("{auth_req}\n").as_bytes()).await?;
        wr.flush().await?;

        let mut auth_reply = String::new();
        reader.read_line(&mut auth_reply).await?;
        if let Some(parsed) = JsonValue::parse(auth_reply.trim()) {
            if parsed.get("error").is_some() {
                return Err(BiltermError::Config("authentication failed".to_string()));
            }
        } else {
            return Err(BiltermError::Config(
                "authentication failed: invalid server response".to_string(),
            ));
        }
    }

    wr.write_all(format!("{request_line}\n").as_bytes()).await?;
    wr.flush().await?;

    let mut reply = String::new();
    reader.read_line(&mut reply).await?;
    Ok(reply.trim_end_matches(['\r', '\n']).to_string())
}

/// Sends a single line-delimited JSON-RPC 2.0 request with an optional bearer token
/// to a TCP (`host:port`) or Unix domain socket path and returns the JSON-RPC response string (`bilterm ctl`).
/// Parses `bilterm ctl` positional arguments (after `-t/--token-file` removal) into
/// `(endpoint, method, params)`. The endpoint is optional: a first argument without a `:`
/// (`workspace.status`) is the method, so `bilterm ctl workspace.status` works against the default
/// endpoint. Unparsable JSON params fall back to `{}` (legacy behaviour).
pub fn parse_ctl_args(rest: &[String], default_endpoint: &str) -> (String, String, JsonValue) {
    let (endpoint, tail) = match rest.first() {
        Some(first) if first.contains(':') => (first.as_str(), &rest[1..]),
        _ => (default_endpoint, rest),
    };
    let method = tail.first().map_or("workspace.status", String::as_str);
    let params = tail
        .get(1)
        .and_then(|raw| JsonValue::parse(raw))
        .unwrap_or_else(|| JsonValue::obj([] as [(&str, JsonValue); 0]));
    (endpoint.to_string(), method.to_string(), params)
}

pub async fn send_rpc_request_with_token(
    endpoint: &str,
    method: &str,
    params: JsonValue,
    token: Option<&str>,
) -> Result<String> {
    let req = JsonValue::obj([
        ("jsonrpc", JsonValue::String("2.0".to_string())),
        ("id", JsonValue::Number(1)),
        ("method", JsonValue::String(method.to_string())),
        ("params", params),
    ])
    .to_json_string();

    let unix_prefix = endpoint
        .strip_prefix("unix://")
        .or_else(|| endpoint.strip_prefix("unix:"));

    if let Some(path) = unix_prefix {
        #[cfg(unix)]
        {
            let stream = tokio::net::UnixStream::connect(path).await?;
            rpc_roundtrip(stream, token, req).await
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(BiltermError::Config(
                "Use host:port TCP endpoint on non-Unix platforms".to_string(),
            ))
        }
    } else if endpoint.parse::<SocketAddr>().is_ok()
        || endpoint.starts_with("localhost:")
        || (endpoint.contains(':') && !Path::new(endpoint).exists())
    {
        let stream = tokio::net::TcpStream::connect(endpoint).await?;
        rpc_roundtrip(stream, token, req).await
    } else {
        #[cfg(unix)]
        {
            let stream = tokio::net::UnixStream::connect(endpoint).await?;
            rpc_roundtrip(stream, token, req).await
        }
        #[cfg(not(unix))]
        {
            Err(BiltermError::Config(
                "Use host:port TCP endpoint on non-Unix platforms".to_string(),
            ))
        }
    }
}

/// Sends a single line-delimited JSON-RPC 2.0 request to a TCP (`host:port`) or Unix domain socket path
/// and returns the JSON-RPC response string (`bilterm ctl`).
pub async fn send_rpc_request(endpoint: &str, method: &str, params: JsonValue) -> Result<String> {
    send_rpc_request_with_token(endpoint, method, params, None).await
}

/// Maximum length of one JSON-RPC request line; longer lines close the offending connection.
pub(super) const MAX_RPC_LINE_BYTES: u64 = 1024 * 1024;

/// Trait abstracting transport listener accept loops for generic JSON-RPC serving.
trait RpcAcceptor {
    type Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static;
    fn accept(
        &self,
    ) -> impl std::future::Future<Output = std::io::Result<(Self::Stream, String)>> + Send;
}

impl RpcAcceptor for tokio::net::TcpListener {
    type Stream = tokio::net::TcpStream;
    async fn accept(&self) -> std::io::Result<(Self::Stream, String)> {
        let (stream, peer) = self.accept().await?;
        Ok((stream, peer.to_string()))
    }
}

#[cfg(unix)]
impl RpcAcceptor for tokio::net::UnixListener {
    type Stream = tokio::net::UnixStream;
    async fn accept(&self) -> std::io::Result<(Self::Stream, String)> {
        let (stream, _addr) = self.accept().await?;
        let cred = stream.peer_cred()?;
        let peer_uid = cred.uid();
        let my_uid = crate::secure_fs::current_euid();
        if peer_uid != my_uid {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("peer UID {peer_uid} does not match daemon UID {my_uid}"),
            ));
        }
        Ok((stream, format!("unix:uid={peer_uid}")))
    }
}

#[cfg(unix)]
struct SocketFileGuard(PathBuf);

#[cfg(unix)]
impl Drop for SocketFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Runs a headless JSON-RPC 2.0 daemon configured according to security policy (SEC-R01).
pub async fn run_headless_server(config: ListenConfig, plane: AutomationControlPlane) -> Result<()> {
    run_headless_server_with_http(config, plane, None).await
}

/// Like [`run_headless_server`], additionally serving the opt-in loopback HTTP+SSE transport
/// (listener + bearer token) against the SAME control plane.
pub async fn run_headless_server_with_http(
    config: ListenConfig,
    plane: AutomationControlPlane,
    http: Option<HttpListen>,
) -> Result<()> {
    let token = config.token.map(std::sync::Arc::from);
    match config.endpoint {
        ListenEndpoint::Tcp(addr) => {
            let listener = tokio::net::TcpListener::bind(addr).await?;
            tracing::info!(endpoint = %addr, "JSON-RPC daemon listening on TCP");
            serve_loop(listener, plane, token, http).await
        }
        #[cfg(unix)]
        ListenEndpoint::Unix(path) => {
            use std::os::unix::fs::FileTypeExt;
            use std::os::unix::fs::PermissionsExt;

            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            if path.exists() || std::fs::symlink_metadata(&path).is_ok() {
                match tokio::net::UnixStream::connect(&path).await {
                    Ok(_) => {
                        return Err(BiltermError::Config(format!(
                            "another bilterm daemon is already listening on {}",
                            path.display()
                        )));
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::ConnectionRefused
                            || e.kind() == std::io::ErrorKind::NotFound =>
                    {
                        let meta = std::fs::symlink_metadata(&path)?;
                        if !meta.file_type().is_socket() {
                            return Err(BiltermError::Config(format!(
                                "refusing to overwrite non-socket file at {}",
                                path.display()
                            )));
                        }
                        std::fs::remove_file(&path)?;
                    }
                    Err(e) => {
                        return Err(BiltermError::Config(format!(
                            "another bilterm daemon is already listening on {} (or socket probe failed: {e})",
                            path.display()
                        )));
                    }
                }
            }

            let listener = tokio::net::UnixListener::bind(&path)?;
            let _guard = SocketFileGuard(path.clone());
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            tracing::info!(endpoint = %path.display(), "JSON-RPC daemon listening on Unix socket");
            serve_loop(listener, plane, token, http).await
        }
        #[cfg(not(unix))]
        ListenEndpoint::Unix(_) => Err(BiltermError::Config(
            "Unix domain sockets not supported on non-Unix platforms".to_string(),
        )),
    }
}

/// Runs a headless JSON-RPC 2.0 TCP automation server
/// (`bilterm --headless --listen 127.0.0.1:PORT --token-file PATH`). TCP listeners require a token.
pub async fn run_headless_tcp_server(
    bind_addr: &str,
    token: String,
    rows: u16,
    cols: u16,
) -> Result<()> {
    let cfg = validate_listen_config(Some(bind_addr), false, Some(token), None)?;
    run_headless_server(cfg, AutomationControlPlane::new_headless(rows, cols)).await
}

/// Serves JSON-RPC over a TCP listener with an optional bearer token gate.
pub async fn serve_jsonrpc_tcp(
    listener: tokio::net::TcpListener,
    plane: AutomationControlPlane,
    token: Option<std::sync::Arc<str>>,
) -> Result<()> {
    serve_loop(listener, plane, token, None).await
}

/// Serves JSON-RPC over `listener` without requiring authentication (legacy wrapper).
pub async fn serve_jsonrpc(
    listener: tokio::net::TcpListener,
    plane: AutomationControlPlane,
) -> Result<()> {
    serve_jsonrpc_tcp(listener, plane, None).await
}

/// Serves JSON-RPC over a Unix domain socket listener with an optional bearer token gate.
#[cfg(unix)]
pub async fn serve_jsonrpc_unix(
    listener: tokio::net::UnixListener,
    plane: AutomationControlPlane,
    token: Option<std::sync::Arc<str>>,
) -> Result<()> {
    serve_loop(listener, plane, token, None).await
}

async fn serve_loop<A: RpcAcceptor>(
    acceptor: A,
    mut plane: AutomationControlPlane,
    token: Option<std::sync::Arc<str>>,
    http: Option<HttpListen>,
) -> Result<()> {
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel::<RpcRequest>(64);
    let meter = plane.wm.traffic_meter();
    let mut conn_seq = 0u64;
    if let Some(mut h) = http {
        h.web.bind_plane(&mut plane);
        let tx = req_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = super::http::serve_http_with(h.listener, tx, h.token, h.web).await {
                tracing::warn!(error = %e, "HTTP transport stopped");
            }
        });
    }

    let mut waiters = AgentWaiters::default();
    let mut wait_tick = tokio::time::interval(WAIT_TICK);
    wait_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            accepted = acceptor.accept() => {
                match accepted {
                    Ok((stream, peer)) => {
                        let req_tx = req_tx.clone();
                        let token = token.clone();
                        conn_seq += 1;
                        let origin = match peer.parse::<std::net::SocketAddr>() {
                            Ok(a) => Origin::RpcTcp { remote: !a.ip().is_loopback() },
                            Err(_) if peer.starts_with("unix:") => Origin::RpcUnix,
                            Err(_) => Origin::RpcTcp { remote: true },
                        };
                        let tap = meter.open_channel(origin, &format!("rpc#{conn_seq}"), None, Role::Wire);
                        let stream = super::metered::Metered::new(stream, tap);
                        tokio::spawn(async move {
                            if let Err(e) = serve_jsonrpc_connection(stream, req_tx, token).await {
                                tracing::warn!(%peer, error = %e, "JSON-RPC connection closed");
                            }
                        });
                    }
                    Err(e) => tracing::warn!(error = %e, "JSON-RPC accept failed"),
                }
            }
            Some((line, reply_tx)) = req_rx.recv() => {
                waiters.handle(&mut plane, &line, reply_tx);
            }
            _ = wait_tick.tick(), if !waiters.is_empty() => waiters.poll(&plane),
        }
    }
}

/// Returns true when `line` looks like an HTTP request line (`METHOD SP ...` or `... HTTP/1.x`).
///
/// JSON objects/arrays are never treated as HTTP, so request payloads may mention `HTTP/1.1`.
fn looks_like_http(line: &str) -> bool {
    if line.starts_with('{') || line.starts_with('[') {
        return false;
    }
    const METHODS: [&str; 9] = [
        "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH", "CONNECT", "TRACE",
    ];
    let starts_with_method = METHODS.iter().any(|m| {
        line.len() > m.len()
            && line.as_bytes()[..m.len()].eq_ignore_ascii_case(m.as_bytes())
            && line.as_bytes()[m.len()] == b' '
    });
    starts_with_method || line.contains(" HTTP/1.")
}

async fn serve_jsonrpc_connection<S>(
    stream: S,
    req_tx: tokio::sync::mpsc::Sender<(String, tokio::sync::oneshot::Sender<String>)>,
    token: Option<std::sync::Arc<str>>,
) -> std::io::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    let (rd, mut wr) = tokio::io::split(stream);
    let mut reader = BufReader::new(rd);
    let mut buf = Vec::new();
    let mut authed = token.is_none();

    loop {
        buf.clear();
        let n = (&mut reader)
            .take(MAX_RPC_LINE_BYTES + 1)
            .read_until(b'\n', &mut buf)
            .await?;
        if n == 0 {
            return Ok(());
        }
        if buf.last() != Some(&b'\n') && n as u64 > MAX_RPC_LINE_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "JSON-RPC request line exceeds 1 MiB",
            ));
        }
        let line = String::from_utf8_lossy(&buf);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // A non-JSON line (or anything shaped like an HTTP request) means the peer is not
        // speaking line-delimited JSON-RPC, e.g. a browser POST aimed at a loopback port. Reply
        // once and close so that later lines (such as an HTTP body) are never dispatched.
        let parsed = match JsonValue::parse(trimmed).filter(|_| !looks_like_http(trimmed)) {
            Some(val) => val,
            None => {
                let err_resp = if authed {
                    AutomationControlPlane::error_response(JsonValue::Null, -32700, "Parse error")
                } else {
                    AutomationControlPlane::error_response(
                        JsonValue::Null,
                        RPC_AUTH_ERROR_CODE,
                        "authentication required: send auth.login first",
                    )
                }
                .to_json_string();
                wr.write_all(format!("{err_resp}\n").as_bytes()).await?;
                wr.flush().await?;
                return Ok(());
            }
        };

        let req_id = parsed.get("id").cloned().unwrap_or(JsonValue::Null);
        let method = parsed.get("method").and_then(|m| m.as_str()).unwrap_or("");

        if method == "auth.login" {
            let client_token = parsed
                .get("params")
                .and_then(|p| p.get("token"))
                .and_then(|t| t.as_str());
            let success = match (&token, client_token) {
                (Some(server_token), Some(cand)) => {
                    crate::secure_fs::constant_time_eq(server_token.as_bytes(), cand.as_bytes())
                }
                (None, _) => true,
                (Some(_), None) => false,
            };

            if success {
                authed = true;
                let resp = JsonValue::obj([
                    ("jsonrpc", JsonValue::String("2.0".to_string())),
                    ("id", req_id),
                    (
                        "result",
                        if token.is_some() {
                            JsonValue::obj([("authenticated", JsonValue::Bool(true))])
                        } else {
                            JsonValue::obj([
                                ("authenticated", JsonValue::Bool(true)),
                                ("required", JsonValue::Bool(false)),
                            ])
                        },
                    ),
                ])
                .to_json_string();
                wr.write_all(format!("{resp}\n").as_bytes()).await?;
                wr.flush().await?;
            } else {
                let resp = AutomationControlPlane::error_response(
                    req_id,
                    RPC_AUTH_ERROR_CODE,
                    "authentication failed",
                )
                .to_json_string();
                wr.write_all(format!("{resp}\n").as_bytes()).await?;
                wr.flush().await?;
                return Ok(());
            }
            continue;
        }

        if !authed {
            let resp = AutomationControlPlane::error_response(
                req_id,
                RPC_AUTH_ERROR_CODE,
                "authentication required: send auth.login first",
            )
            .to_json_string();
            wr.write_all(format!("{resp}\n").as_bytes()).await?;
            wr.flush().await?;
            return Ok(());
        }

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if req_tx.send((trimmed.to_string(), reply_tx)).await.is_err() {
            return Ok(());
        }
        let Ok(resp) = reply_rx.await else {
            return Ok(());
        };
        wr.write_all(format!("{resp}\n").as_bytes()).await?;
        wr.flush().await?;
    }
}

/// Spawns a tokio accept loop that forwards every request line from `listener` to `req_tx`.
///
/// Used by the always-on foreground RPC socket (`LIVE-01`): the foreground event loop owns the
/// `WindowManager`, receives `(line, reply)` pairs and answers them itself. Peer uid is checked
/// per connection exactly like the headless daemon; no bearer token is used on the private
/// `0600` Unix socket.
#[cfg(unix)]
pub(super) fn spawn_unix_acceptor(
    listener: tokio::net::UnixListener,
    req_tx: tokio::sync::mpsc::Sender<RpcRequest>,
    meter: crate::traffic::TrafficMeter,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut seq = 0u64;
        loop {
            match RpcAcceptor::accept(&listener).await {
                Ok((stream, _peer)) => {
                    seq += 1;
                    let tap = meter.open_channel(Origin::RpcUnix, &format!("live#{seq}"), None, Role::Wire);
                    let stream = super::metered::Metered::new(stream, tap);
                    let tx = req_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = serve_jsonrpc_connection(stream, tx, None).await {
                            tracing::debug!(error = %e, "live RPC connection closed");
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!(error = %e, "live RPC accept failed");
                    if e.kind() != std::io::ErrorKind::PermissionDenied {
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
        }
    })
}
