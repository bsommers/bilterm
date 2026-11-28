//! Loopback-only Artifact Studio & HTML/SVG preview server for `:editor preview` / `:board` (`EDIT-07`).
//!
//! Serves either:
//! - The interactive Shared Artifact Studio web app (`studio_web.rs`) wired live to `ContextBus`
//!   (`/api/state`, `/api/save`, `/api/create`, `/api/delete`, `/api/broadcast`), OR
//! - Raw `.html`/`.htm`/`.svg` buffer content directly for standalone HTML/JS/SVG artifacts
//!   (with `/api/*` disabled and a network-blocking sandbox CSP).
//!
//! Bound strictly to `127.0.0.1:0` with a 256-bit OS CSPRNG path token (`secure_fs::random_token_hex`)
//! verified in constant time, literal loopback `Host` (`127.0.0.1` / `[::1]`) and same-origin
//! `Origin`/`Content-Type` checks, bounded concurrent worker threads, and `Cache-Control: no-store`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::api::json::JsonValue;
use crate::editor::artifact::{
    ARTIFACT_KEY_PREFIX, artifact_bus_key, artifact_template, checklist_progress, sanitize_inline,
    sanitize_multiline,
};
use crate::editor::buffer::TextBuffer;
use crate::editor::io as editor_io;
use crate::editor::studio_web::studio_app_html;
use crate::wm::context_bus::{ContextBus, ContextBusError};

const MAX_HEAD_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 256 * 1024;
const MAX_PENDING_BROADCASTS: usize = 32;
const MAX_BROADCAST_NOTE_CHARS: usize = 512;
const MAX_CONCURRENT_CONNS: usize = 16;
const REQUEST_DEADLINE: Duration = Duration::from_secs(2);
const DROP_WORKER_WAIT: Duration = Duration::from_millis(300);

const RAW_PREVIEW_EXTRA_HEADERS: &str = concat!(
    "Content-Security-Policy: sandbox allow-scripts; default-src 'none'; ",
    "script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; connect-src 'none'\r\n",
);

const STUDIO_PAGE_EXTRA_HEADERS: &str = concat!(
    "Content-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; ",
    "style-src 'unsafe-inline'; img-src data:; connect-src 'self'; frame-ancestors 'none'\r\n",
    "X-Frame-Options: DENY\r\n",
    "Cross-Origin-Resource-Policy: same-origin\r\n",
);

#[derive(Debug)]
struct SharedPreview {
    bus: ContextBus,
    primary_name: Mutex<String>,
    active_seq: AtomicU64,
    raw_html: Mutex<Option<(String, &'static str)>>,
    pending_broadcasts: Mutex<Vec<(String, String)>>,
    save_lock: Mutex<()>,
    persist_dir: Option<PathBuf>,
    active_workers: AtomicUsize,
    stop: AtomicBool,
}

struct WorkerGuard {
    shared: Arc<SharedPreview>,
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.shared.active_workers.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Live handle to a loopback preview / Artifact Studio server owned by an editor window or `:board`.
#[derive(Debug)]
pub struct PreviewServer {
    url: String,
    redacted_url: String,
    shared: Arc<SharedPreview>,
    accept_thread: Option<JoinHandle<()>>,
}

impl PreviewServer {
    /// Starts a loopback server on an ephemeral port with a 256-bit OS CSPRNG path token.
    ///
    /// Checks CSPRNG token availability before binding the TCP listener.
    /// When `raw_body` is `Some((body, content_type))`, serves raw HTML/SVG directly with `/api/*`
    /// disabled; when `None`, serves the interactive Shared Artifact Studio web app backed by `bus`.
    /// When `persist_dir` is `Some`, Studio saves/creates/deletes also persist to `0600` `.md` files.
    pub fn start(
        bus: ContextBus,
        primary_name: String,
        raw_body: Option<(String, &'static str)>,
        persist_dir: Option<PathBuf>,
    ) -> std::io::Result<Self> {
        let token = crate::secure_fs::random_token_hex()?;
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let url = format!("http://127.0.0.1:{port}/{token}/");
        let prefix: String = token.chars().take(6).collect();
        let redacted_url = format!("http://127.0.0.1:{port}/{prefix}.../");
        let shared = Arc::new(SharedPreview {
            bus,
            primary_name: Mutex::new(primary_name),
            active_seq: AtomicU64::new(1),
            raw_html: Mutex::new(raw_body),
            pending_broadcasts: Mutex::new(Vec::new()),
            save_lock: Mutex::new(()),
            persist_dir,
            active_workers: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
        });
        let bg = Arc::clone(&shared);
        let tok = Arc::new(token);
        let handle = std::thread::Builder::new()
            .name("bilterm-editor-preview".into())
            .spawn(move || serve_loop(listener, tok, bg))?;
        Ok(Self { url, redacted_url, shared, accept_thread: Some(handle) })
    }

    /// Full loopback URL (`http://127.0.0.1:<port>/<token>/`) for passing only to the browser opener.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Redacted loopback URL (`http://127.0.0.1:<port>/<prefix>.../`) safe for RPC replies and status text.
    pub fn redacted_url(&self) -> &str {
        &self.redacted_url
    }

    /// Updates raw HTML/SVG body or primary artifact name in memory and bumps `active_seq` when switching docs.
    pub fn update(&self, primary_name: String, raw_body: Option<(String, &'static str)>) {
        if let Ok(mut g) = self.shared.primary_name.lock()
            && *g != primary_name
        {
            *g = primary_name;
            self.shared.active_seq.fetch_add(1, Ordering::Relaxed);
        }
        if let Ok(mut g) = self.shared.raw_html.lock() {
            *g = raw_body;
        }
    }

    /// Drains any `(artifact_name, note)` broadcast requests clicked inside the web UI.
    pub fn take_pending_broadcasts(&self) -> Vec<(String, String)> {
        self.shared
            .pending_broadcasts
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default()
    }
}

impl Drop for PreviewServer {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        // The accept loop polls every 20 ms on a non-blocking listener, so joining completes promptly.
        if let Some(h) = self.accept_thread.take() {
            let _ = h.join();
        }
        // Wait up to DROP_WORKER_WAIT for in-flight connection workers to finish; any stalled client
        // remaining after the bounded wait detaches and exits on its own REQUEST_DEADLINE.
        let deadline = Instant::now() + DROP_WORKER_WAIT;
        while self.shared.active_workers.load(Ordering::Acquire) > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Normalizes a user-entered title or filename (e.g. `"UI Spec 2"`, `"README.md"`) into a
/// lowercase, `validate_safe_name`-compliant artifact slug (`[a-z0-9_-]{1,64}`, no dots).
pub fn slugify_artifact_name(raw: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
        if out.len() >= 64 {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

/// Returns `true` for raw HTML / SVG paths whose buffer should be previewed directly.
pub fn is_raw_html_path(path: Option<&std::path::Path>) -> bool {
    path.and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "html" | "htm" | "svg"))
}

fn next_auto_doc_name(bus: &ContextBus) -> String {
    let existing = bus.list(Some(ARTIFACT_KEY_PREFIX));
    for n in 1..=999 {
        let cand = format!("doc-{n}");
        let key = artifact_bus_key(&cand);
        if !existing.iter().any(|e| e.key == key) {
            return cand;
        }
    }
    slugify_artifact_name(&format!("doc-{}", crate::secure_fs::unique_suffix()))
}

fn host_only(hostport: &str) -> &str {
    if let Some(rest) = hostport.strip_prefix('[') {
        return rest.split(']').next().unwrap_or("");
    }
    hostport.rsplit_once(':').map_or(hostport, |(h, _)| h)
}

/// Accepts only literal loopback IP addresses (`127.0.0.1`, `::1`, etc.), refusing `localhost`.
fn is_literal_loopback_host(h: &str) -> bool {
    h.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    host: Option<String>,
    origin: Option<String>,
    content_type: Option<String>,
    body: Vec<u8>,
}

enum ParseError {
    Empty,
    Status(&'static str, &'static str),
}

fn read_http_request<R: Read>(stream: &mut R) -> Result<HttpRequest, ParseError> {
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    let mut header_end: Option<usize> = None;
    let mut content_len: usize = 0;
    let mut scan_from: usize = 0;
    let deadline = Instant::now() + REQUEST_DEADLINE;

    loop {
        if Instant::now() >= deadline {
            return Err(ParseError::Status("408 Request Timeout", "request timed out"));
        }
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if header_end.is_none() {
                    let search_start = scan_from.saturating_sub(3);
                    if let Some(rel) = buf[search_start..].windows(4).position(|w| w == b"\r\n\r\n") {
                        let pos = search_start + rel;
                        if pos > MAX_HEAD_BYTES {
                            return Err(ParseError::Status(
                                "431 Request Header Fields Too Large",
                                "headers too large",
                            ));
                        }
                        header_end = Some(pos + 4);
                        let hdrs = std::str::from_utf8(&buf[..pos])
                            .map_err(|_| ParseError::Status("400 Bad Request", "invalid header encoding"))?;
                        let mut saw_cl = false;
                        for line in hdrs.lines().skip(1) {
                            let Some((k, v)) = line.split_once(':') else { continue };
                            let key = k.trim();
                            if key.eq_ignore_ascii_case("transfer-encoding") {
                                return Err(ParseError::Status(
                                    "400 Bad Request",
                                    "transfer-encoding not supported",
                                ));
                            }
                            if key.eq_ignore_ascii_case("content-length") {
                                if saw_cl {
                                    return Err(ParseError::Status(
                                        "400 Bad Request",
                                        "duplicate content-length",
                                    ));
                                }
                                saw_cl = true;
                                let parsed = v
                                    .trim()
                                    .parse::<usize>()
                                    .map_err(|_| ParseError::Status("400 Bad Request", "invalid content-length"))?;
                                if parsed > MAX_BODY_BYTES {
                                    return Err(ParseError::Status(
                                        "413 Payload Too Large",
                                        "payload too large",
                                    ));
                                }
                                content_len = parsed;
                            }
                        }
                    } else {
                        scan_from = buf.len();
                        if buf.len() > MAX_HEAD_BYTES {
                            return Err(ParseError::Status(
                                "431 Request Header Fields Too Large",
                                "headers too large",
                            ));
                        }
                    }
                }
                if let Some(he) = header_end {
                    if buf.len() >= he + content_len {
                        break;
                    }
                    if buf.len() > he + MAX_BODY_BYTES {
                        return Err(ParseError::Status("413 Payload Too Large", "payload too large"));
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                if Instant::now() >= deadline {
                    return Err(ParseError::Status("408 Request Timeout", "request timed out"));
                }
                continue;
            }
            Err(_) => break,
        }
    }

    if buf.is_empty() {
        return Err(ParseError::Empty);
    }
    let he = header_end.ok_or(ParseError::Status("400 Bad Request", "incomplete headers"))?;
    if buf.len() < he + content_len {
        return Err(ParseError::Status("400 Bad Request", "incomplete request body"));
    }
    let head = std::str::from_utf8(&buf[..he - 4])
        .map_err(|_| ParseError::Status("400 Bad Request", "invalid header encoding"))?;
    let mut lines = head.lines();
    let first = lines.next().ok_or(ParseError::Status("400 Bad Request", "empty request"))?;
    let mut parts = first.split_whitespace();
    let method = parts
        .next()
        .ok_or(ParseError::Status("400 Bad Request", "missing method"))?
        .to_string();
    let raw_path = parts
        .next()
        .ok_or(ParseError::Status("400 Bad Request", "missing path"))?;
    let path = raw_path.split('?').next().unwrap_or(raw_path).to_string();

    let mut host = None;
    let mut origin = None;
    let mut content_type = None;
    for line in lines {
        let Some((k, v)) = line.split_once(':') else { continue };
        let key = k.trim();
        let val = v.trim();
        if key.eq_ignore_ascii_case("host") {
            host = Some(val.to_string());
        } else if key.eq_ignore_ascii_case("origin") {
            origin = Some(val.to_string());
        } else if key.eq_ignore_ascii_case("content-type") {
            content_type = Some(val.to_string());
        }
    }

    Ok(HttpRequest {
        method,
        path,
        host,
        origin,
        content_type,
        body: buf[he..he + content_len].to_vec(),
    })
}

fn write_response<W: Write>(
    stream: &mut W,
    status: &str,
    ct: &str,
    body: &[u8],
    head_only: bool,
    extra_headers: &str,
) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\n{extra_headers}Connection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    if !head_only {
        let _ = stream.write_all(body);
    }
    let _ = stream.flush();
}

fn persist_artifact(shared: &SharedPreview, name: &str, text: &str) -> Result<(), String> {
    let Some(dir) = &shared.persist_dir else {
        return Ok(());
    };
    let path = editor_io::scratch_file(dir, name)?;
    editor_io::ensure_scratch_dir(dir).map_err(|e| format!("artifact dir {}: {e}", dir.display()))?;
    crate::secure_fs::write_private_file(&path, text.as_bytes())
        .map_err(|e| format!("persist {}: {e}", path.display()))
}

fn remove_persisted_artifact(shared: &SharedPreview, name: &str) -> Result<(), String> {
    let Some(dir) = &shared.persist_dir else {
        return Ok(());
    };
    let path = editor_io::scratch_file(dir, name)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("remove {}: {e}", path.display())),
    }
}

fn handle_api(sub: &str, method: &str, body: &[u8], shared: &SharedPreview) -> (&'static str, String) {
    match (method, sub) {
        ("GET", "/api/state") => {
            let entries = shared.bus.list(Some(ARTIFACT_KEY_PREFIX));
            let mut docs = Vec::with_capacity(entries.len());
            for e in entries {
                let name = e.key.strip_prefix(ARTIFACT_KEY_PREFIX).unwrap_or(&e.key).to_string();
                let tb = TextBuffer::from_text(&e.value);
                let (done, total) = checklist_progress(&tb);
                docs.push(JsonValue::obj([
                    ("name", JsonValue::String(name)),
                    ("key", JsonValue::String(e.key)),
                    ("text", JsonValue::String(e.value)),
                    ("revision", JsonValue::Number(e.revision as i64)),
                    ("done", JsonValue::Number(done as i64)),
                    ("total", JsonValue::Number(total as i64)),
                ]));
            }
            let active = shared.primary_name.lock().map(|g| g.clone()).unwrap_or_else(|_| "shared".into());
            let active_seq = shared.active_seq.load(Ordering::Relaxed);
            let payload = JsonValue::obj([
                ("ok", JsonValue::Bool(true)),
                ("active", JsonValue::String(active)),
                ("active_seq", JsonValue::Number(active_seq as i64)),
                ("docs", JsonValue::Array(docs)),
            ]);
            ("200 OK", payload.to_json_string())
        }
        ("POST", "/api/save") => {
            let Some(v) = std::str::from_utf8(body).ok().and_then(JsonValue::parse) else {
                return ("400 Bad Request", r#"{"ok":false,"error":"invalid json"}"#.into());
            };
            let name = v.get("name").and_then(JsonValue::as_str).unwrap_or("").to_ascii_lowercase();
            let raw_text = v.get("text").and_then(JsonValue::as_str).unwrap_or("");
            let clean_text = sanitize_multiline(raw_text);
            let Some(expected_rev) = v
                .get("expected_revision")
                .and_then(JsonValue::as_i64)
                .and_then(|n| u64::try_from(n).ok())
            else {
                return (
                    "400 Bad Request",
                    r#"{"ok":false,"error":"expected_revision required"}"#.into(),
                );
            };
            if let Err(e) = crate::secure_fs::validate_safe_name(&name) {
                return (
                    "400 Bad Request",
                    JsonValue::obj([("ok", JsonValue::Bool(false)), ("error", JsonValue::String(e))]).to_json_string(),
                );
            }
            let _guard = shared.save_lock.lock().unwrap_or_else(|e| e.into_inner());
            let key = artifact_bus_key(&name);
            let previous = shared.bus.get(&key).map(|e| e.value);
            let bus_res = shared.bus.set_if_revision(&key, &clean_text, None, expected_rev);
            let rev = match bus_res {
                Ok(rev) => rev,
                Err(ContextBusError::Conflict { actual, .. }) => {
                    return (
                        "409 Conflict",
                        JsonValue::obj([
                            ("ok", JsonValue::Bool(false)),
                            ("conflict", JsonValue::Bool(true)),
                            ("error", JsonValue::String("artifact updated concurrently".into())),
                            ("current_revision", JsonValue::Number(actual as i64)),
                        ])
                        .to_json_string(),
                    );
                }
                Err(e) => {
                    return (
                        "400 Bad Request",
                        JsonValue::obj([("ok", JsonValue::Bool(false)), ("error", JsonValue::String(e.to_string()))]).to_json_string(),
                    );
                }
            };
            if let Err(e) = persist_artifact(shared, &name, &clean_text) {
                tracing::warn!("artifact studio persist failed for '{name}': {e}");
                // Keep bus and disk consistent: restore the pre-save text (bumps the revision).
                if let Some(prev) = &previous {
                    let _ = shared.bus.set(&key, prev, None);
                }
                return (
                    "500 Internal Server Error",
                    JsonValue::obj([("ok", JsonValue::Bool(false)), ("error", JsonValue::String(e))]).to_json_string(),
                );
            }
            (
                "200 OK",
                JsonValue::obj([("ok", JsonValue::Bool(true)), ("revision", JsonValue::Number(rev as i64))]).to_json_string(),
            )
        }
        ("POST", "/api/create") => {
            let Some(v) = std::str::from_utf8(body).ok().and_then(JsonValue::parse) else {
                return ("400 Bad Request", r#"{"ok":false,"error":"invalid json"}"#.into());
            };
            let raw_name = v.get("name").and_then(JsonValue::as_str).unwrap_or("");
            let name = {
                let slug = slugify_artifact_name(raw_name);
                if slug.is_empty() { next_auto_doc_name(&shared.bus) } else { slug }
            };
            if let Err(e) = crate::secure_fs::validate_safe_name(&name) {
                return (
                    "400 Bad Request",
                    JsonValue::obj([("ok", JsonValue::Bool(false)), ("error", JsonValue::String(e))]).to_json_string(),
                );
            }
            let _guard = shared.save_lock.lock().unwrap_or_else(|e| e.into_inner());
            let key = artifact_bus_key(&name);
            if shared.bus.get(&key).is_none() {
                let initial = artifact_template(&name);
                if let Err(e) = shared.bus.set(&key, &initial, None) {
                    return (
                        "400 Bad Request",
                        JsonValue::obj([("ok", JsonValue::Bool(false)), ("error", JsonValue::String(e.to_string()))]).to_json_string(),
                    );
                }
                if let Err(e) = persist_artifact(shared, &name, &initial) {
                    let _ = shared.bus.delete(&key);
                    tracing::warn!("artifact studio persist failed for '{name}': {e}");
                    return (
                        "500 Internal Server Error",
                        JsonValue::obj([("ok", JsonValue::Bool(false)), ("error", JsonValue::String(e))]).to_json_string(),
                    );
                }
            }
            ("200 OK", JsonValue::obj([("ok", JsonValue::Bool(true)), ("name", JsonValue::String(name))]).to_json_string())
        }
        ("POST", "/api/delete") => {
            let Some(v) = std::str::from_utf8(body).ok().and_then(JsonValue::parse) else {
                return ("400 Bad Request", r#"{"ok":false,"error":"invalid json"}"#.into());
            };
            let name = v.get("name").and_then(JsonValue::as_str).unwrap_or("").to_ascii_lowercase();
            if let Err(e) = crate::secure_fs::validate_safe_name(&name) {
                return (
                    "400 Bad Request",
                    JsonValue::obj([("ok", JsonValue::Bool(false)), ("error", JsonValue::String(e))]).to_json_string(),
                );
            }
            let _guard = shared.save_lock.lock().unwrap_or_else(|e| e.into_inner());
            let del_key = artifact_bus_key(&name);
            let previous = shared.bus.get(&del_key).map(|e| e.value);
            let deleted = shared.bus.delete(&del_key);
            if deleted && let Err(e) = remove_persisted_artifact(shared, &name) {
                tracing::warn!("artifact studio delete persist failed for '{name}': {e}");
                // Keep bus and disk consistent: put the artifact back.
                if let Some(prev) = &previous {
                    let _ = shared.bus.set(&del_key, prev, None);
                }
                return (
                    "500 Internal Server Error",
                    JsonValue::obj([("ok", JsonValue::Bool(false)), ("error", JsonValue::String(e))]).to_json_string(),
                );
            }
            ("200 OK", JsonValue::obj([("ok", JsonValue::Bool(deleted))]).to_json_string())
        }
        ("POST", "/api/broadcast") => {
            let Some(v) = std::str::from_utf8(body).ok().and_then(JsonValue::parse) else {
                return ("400 Bad Request", r#"{"ok":false,"error":"invalid json"}"#.into());
            };
            let name = v.get("name").and_then(JsonValue::as_str).unwrap_or("").to_ascii_lowercase();
            if let Err(e) = crate::secure_fs::validate_safe_name(&name) {
                return (
                    "400 Bad Request",
                    JsonValue::obj([("ok", JsonValue::Bool(false)), ("error", JsonValue::String(e))]).to_json_string(),
                );
            }
            let raw_note = v.get("note").and_then(JsonValue::as_str).unwrap_or("");
            let note = sanitize_inline(raw_note, MAX_BROADCAST_NOTE_CHARS);
            if let Ok(mut q) = shared.pending_broadcasts.lock() {
                if q.len() >= MAX_PENDING_BROADCASTS {
                    return ("429 Too Many Requests", r#"{"ok":false,"error":"broadcast queue full"}"#.into());
                }
                q.push((name, note));
            }
            ("200 OK", r#"{"ok":true}"#.into())
        }
        _ => ("404 Not Found", r#"{"ok":false,"error":"not found"}"#.into()),
    }
}

/// Validates literal loopback `Host`, constant-time path token, same-origin CSRF defenses, and dispatches one HTTP exchange.
fn handle_http_exchange<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    token: &str,
    shared: &SharedPreview,
) {
    let req = match read_http_request(reader) {
        Ok(r) => r,
        Err(ParseError::Empty) => return,
        Err(ParseError::Status(status, msg)) => {
            write_response(writer, status, "text/plain; charset=utf-8", msg.as_bytes(), false, "");
            return;
        }
    };
    let head_only = req.method == "HEAD";

    // DNS-rebinding defense: Host must be present and a literal loopback IP (127.0.0.1 / [::1]).
    let Some(host) = req.host.as_deref() else {
        write_response(writer, "403 Forbidden", "text/plain; charset=utf-8", b"missing Host header", head_only, "");
        return;
    };
    if !is_literal_loopback_host(host_only(host)) {
        write_response(writer, "403 Forbidden", "text/plain; charset=utf-8", b"Host not allowed", head_only, "");
        return;
    }

    // Constant-time comparison of the first path segment against the CSPRNG token.
    let Some(after_slash) = req.path.strip_prefix('/') else {
        write_response(writer, "404 Not Found", "text/plain; charset=utf-8", b"Not Found", head_only, "");
        return;
    };
    let (seg, sub) = match after_slash.find('/') {
        Some(idx) => (&after_slash[..idx], &after_slash[idx..]),
        None => (after_slash, ""),
    };
    if !crate::secure_fs::constant_time_eq(seg.as_bytes(), token.as_bytes()) {
        write_response(writer, "404 Not Found", "text/plain; charset=utf-8", b"Not Found", head_only, "");
        return;
    }

    let raw = shared.raw_html.lock().ok().and_then(|g| g.clone());
    if sub.is_empty() || sub == "/" {
        if req.method != "GET" && req.method != "HEAD" {
            write_response(writer, "405 Method Not Allowed", "text/plain; charset=utf-8", b"", head_only, "");
            return;
        }
        if let Some((html, ct)) = raw {
            // Isolate raw previewed HTML/SVG in a sandboxed opaque origin with egress blocked.
            write_response(writer, "200 OK", ct, html.as_bytes(), head_only, RAW_PREVIEW_EXTRA_HEADERS);
        } else {
            let init = shared.primary_name.lock().map(|g| g.clone()).unwrap_or_else(|_| "shared".into());
            let base_path = format!("/{token}");
            let page = studio_app_html(&base_path, &init);
            write_response(
                writer,
                "200 OK",
                "text/html; charset=utf-8",
                page.as_bytes(),
                head_only,
                STUDIO_PAGE_EXTRA_HEADERS,
            );
        }
    } else if sub.starts_with("/api/") {
        // Never expose ContextBus mutation/read APIs alongside raw HTML/SVG previews.
        if raw.is_some() {
            write_response(writer, "404 Not Found", "application/json; charset=utf-8", br#"{"ok":false,"error":"not found"}"#, head_only, "");
            return;
        }
        if req.method == "POST" {
            if let Some(origin) = req.origin.as_deref() {
                let same_origin = origin
                    .strip_prefix("http://")
                    .is_some_and(|o| o.eq_ignore_ascii_case(host));
                if !same_origin {
                    write_response(
                        writer,
                        "403 Forbidden",
                        "application/json; charset=utf-8",
                        br#"{"ok":false,"error":"cross-origin request refused"}"#,
                        head_only,
                        "",
                    );
                    return;
                }
            }
            let is_json = req
                .content_type
                .as_deref()
                .and_then(|ct| ct.split(';').next())
                .is_some_and(|m| m.trim().eq_ignore_ascii_case("application/json"));
            if !is_json {
                write_response(
                    writer,
                    "415 Unsupported Media Type",
                    "application/json; charset=utf-8",
                    br#"{"ok":false,"error":"Content-Type must be application/json"}"#,
                    head_only,
                    "",
                );
                return;
            }
        }
        let (status, json) = handle_api(sub, &req.method, &req.body, shared);
        write_response(writer, status, "application/json; charset=utf-8", json.as_bytes(), head_only, "");
    } else {
        write_response(writer, "404 Not Found", "text/plain; charset=utf-8", b"Not Found", head_only, "");
    }
}

fn handle_client_stream(mut stream: TcpStream, token: Arc<String>, shared: Arc<SharedPreview>) {
    let _guard = WorkerGuard { shared: Arc::clone(&shared) };
    // On macOS/BSD, accepted streams inherit O_NONBLOCK from the listener.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(1000)));
    let Ok(mut reader) = stream.try_clone() else { return };
    handle_http_exchange(&mut reader, &mut stream, &token, &shared);
}

fn serve_loop(listener: TcpListener, token: Arc<String>, shared: Arc<SharedPreview>) {
    while !shared.stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let prev = shared.active_workers.fetch_add(1, Ordering::AcqRel);
                if prev >= MAX_CONCURRENT_CONNS {
                    shared.active_workers.fetch_sub(1, Ordering::AcqRel);
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
                    write_response(
                        &mut stream,
                        "503 Service Unavailable",
                        "text/plain; charset=utf-8",
                        b"too many concurrent connections",
                        false,
                        "",
                    );
                    continue;
                }
                let tok = Arc::clone(&token);
                let sh = Arc::clone(&shared);
                if std::thread::Builder::new()
                    .name("bilterm-preview-conn".into())
                    .spawn(move || handle_client_stream(stream, tok, sh))
                    .is_err()
                {
                    shared.active_workers.fetch_sub(1, Ordering::AcqRel);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn test_shared(raw: Option<(String, &'static str)>, persist_dir: Option<PathBuf>) -> SharedPreview {
        let bus = ContextBus::new();
        bus.set(&artifact_bus_key("plan"), "# Plan\nParagraph here\n- [ ] Task 1", None).unwrap();
        SharedPreview {
            bus,
            primary_name: Mutex::new("plan".into()),
            active_seq: AtomicU64::new(1),
            raw_html: Mutex::new(raw),
            pending_broadcasts: Mutex::new(Vec::new()),
            save_lock: Mutex::new(()),
            persist_dir,
            active_workers: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
        }
    }

    fn exchange(raw_req: &[u8], token: &str, shared: &SharedPreview) -> String {
        let mut reader = Cursor::new(raw_req);
        let mut out = Vec::new();
        handle_http_exchange(&mut reader, &mut out, token, shared);
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn http_request_parser_handles_get_post_and_rejects_malformed() {
        let get = b"GET /tok/api/state?x=1 HTTP/1.1\r\nHost: 127.0.0.1:9999\r\n\r\n";
        let req = read_http_request(&mut Cursor::new(&get[..])).unwrap_or_else(|_| panic!("GET failed"));
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/tok/api/state");
        assert!(req.body.is_empty());

        let post = b"POST /tok/api/create HTTP/1.1\r\nHost: 127.0.0.1:9999\r\nContent-Type: application/json\r\nContent-Length: 20\r\n\r\n{\"name\":\"UI Spec 1\"}";
        let req = read_http_request(&mut Cursor::new(&post[..])).unwrap_or_else(|_| panic!("POST failed"));
        assert_eq!(req.method, "POST");
        assert_eq!(req.body, br#"{"name":"UI Spec 1"}"#);

        // Truncated body -> 400
        let trunc = b"POST /tok/api/save HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 50\r\n\r\nshort";
        assert!(matches!(
            read_http_request(&mut Cursor::new(&trunc[..])),
            Err(ParseError::Status("400 Bad Request", _))
        ));

        // Oversized Content-Length -> 413
        let big = b"POST /tok/api/save HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 999999\r\n\r\n";
        assert!(matches!(
            read_http_request(&mut Cursor::new(&big[..])),
            Err(ParseError::Status("413 Payload Too Large", _))
        ));

        // Duplicate Content-Length -> 400
        let dup = b"POST /tok/api/save HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}";
        assert!(matches!(
            read_http_request(&mut Cursor::new(&dup[..])),
            Err(ParseError::Status("400 Bad Request", _))
        ));

        // Transfer-Encoding -> 400
        let chunked = b"POST /tok/api/save HTTP/1.1\r\nHost: 127.0.0.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n";
        assert!(matches!(
            read_http_request(&mut Cursor::new(&chunked[..])),
            Err(ParseError::Status("400 Bad Request", _))
        ));
    }

    #[test]
    fn security_guards_enforce_token_literal_host_csp_origin_content_type_and_raw_isolation() {
        let shared = test_shared(None, None);
        let tok = "deadbeef01234567";

        // Wrong token -> 404
        let res = exchange(b"GET /wrong/api/state HTTP/1.1\r\nHost: 127.0.0.1:8080\r\n\r\n", tok, &shared);
        assert!(res.starts_with("HTTP/1.1 404"), "{res}");

        // Non-loopback Host -> 403
        let res = exchange(b"GET /deadbeef01234567/api/state HTTP/1.1\r\nHost: evil.example:8080\r\n\r\n", tok, &shared);
        assert!(res.starts_with("HTTP/1.1 403"), "{res}");

        // Host: localhost:PORT is refused (literal loopback IPs only)
        let res = exchange(b"GET /deadbeef01234567/api/state HTTP/1.1\r\nHost: localhost:8080\r\n\r\n", tok, &shared);
        assert!(res.starts_with("HTTP/1.1 403"), "{res}");

        // Cross-origin POST -> 403
        let body = br#"{"name":"plan","text":"pwn"}"#;
        let req = format!(
            "POST /{tok}/api/save HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nOrigin: http://evil.example\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            std::str::from_utf8(body).unwrap()
        );
        let res = exchange(req.as_bytes(), tok, &shared);
        assert!(res.starts_with("HTTP/1.1 403"), "{res}");

        // Non-JSON Content-Type on POST -> 415
        let req = format!(
            "POST /{tok}/api/save HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            std::str::from_utf8(body).unwrap()
        );
        let res = exchange(req.as_bytes(), tok, &shared);
        assert!(res.starts_with("HTTP/1.1 415"), "{res}");

        // Studio page sets Referrer-Policy, frame-ancestors 'none', CORP same-origin, and iframe CSP connect-src 'none'
        let res = exchange(b"GET /deadbeef01234567/ HTTP/1.1\r\nHost: 127.0.0.1:8080\r\n\r\n", tok, &shared);
        assert!(res.starts_with("HTTP/1.1 200"), "{res}");
        assert!(res.contains("Referrer-Policy: no-referrer"), "{res}");
        assert!(res.contains("frame-ancestors 'none'"), "{res}");
        assert!(res.contains("Cross-Origin-Resource-Policy: same-origin"), "{res}");
        assert!(res.contains("connect-src 'none'"), "Studio iframe srcdoc CSP must block network egress");

        // Raw HTML preview disables /api/* and applies strict network-blocking sandbox CSP
        let raw_shared = test_shared(Some(("<h1>Hi</h1>".into(), "text/html; charset=utf-8")), None);
        let res = exchange(b"GET /deadbeef01234567/ HTTP/1.1\r\nHost: 127.0.0.1:8080\r\n\r\n", tok, &raw_shared);
        assert!(res.starts_with("HTTP/1.1 200"), "{res}");
        assert!(
            res.contains("Content-Security-Policy: sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; connect-src 'none'"),
            "{res}"
        );
        let api_res = exchange(b"GET /deadbeef01234567/api/state HTTP/1.1\r\nHost: 127.0.0.1:8080\r\n\r\n", tok, &raw_shared);
        assert!(api_res.starts_with("HTTP/1.1 404"), "{api_res}");
    }

    #[test]
    fn api_save_enforces_expected_revision_and_reports_persist_errors() {
        let shared = test_shared(None, None);
        let initial_rev = shared.bus.get(&artifact_bus_key("plan")).unwrap().revision;
        // Concurrent agent writes a newer revision
        let newer_rev = shared.bus.set(&artifact_bus_key("plan"), "# Newer Agent Text", None).unwrap();
        assert!(newer_rev > initial_rev);

        // Stale save with initial_rev must return 409 Conflict and keep newer agent text intact
        let stale_payload = format!(
            r#"{{"name":"plan","text":"stale overwrite","expected_revision":{initial_rev}}}"#
        );
        let (st, res) = handle_api("/api/save", "POST", stale_payload.as_bytes(), &shared);
        assert_eq!(st, "409 Conflict", "{res}");
        assert!(res.contains("\"conflict\":true"), "{res}");
        assert_eq!(shared.bus.get(&artifact_bus_key("plan")).unwrap().value, "# Newer Agent Text");

        // Save with matching revision succeeds
        let fresh_payload = format!(
            r#"{{"name":"plan","text":"clean merge","expected_revision":{newer_rev}}}"#
        );
        let (st, res) = handle_api("/api/save", "POST", fresh_payload.as_bytes(), &shared);
        assert_eq!(st, "200 OK", "{res}");
        assert_eq!(shared.bus.get(&artifact_bus_key("plan")).unwrap().value, "clean merge");

        // Unwritable persist_dir (a regular file instead of directory) yields a 500 error report
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let bad_shared = test_shared(None, Some(tmp.path().to_path_buf()));
        let bad_rev = bad_shared.bus.get(&artifact_bus_key("plan")).unwrap().revision;
        let bad_payload = format!(r#"{{"name":"plan","text":"fail","expected_revision":{bad_rev}}}"#);
        let (st, res) = handle_api("/api/save", "POST", bad_payload.as_bytes(), &bad_shared);
        assert_eq!(st, "500 Internal Server Error", "{res}");
        assert!(res.contains("\"ok\":false"), "{res}");
        // Failed persist rolls the bus back to the pre-save text.
        assert_eq!(bad_shared.bus.get(&artifact_bus_key("plan")).unwrap().value, "# Plan\nParagraph here\n- [ ] Task 1");

        // Missing expected_revision is refused rather than clobbering.
        let (st, res) = handle_api("/api/save", "POST", br#"{"name":"plan","text":"x"}"#, &shared);
        assert_eq!(st, "400 Bad Request", "{res}");
        assert!(res.contains("expected_revision required"), "{res}");
    }

    #[test]
    fn api_delete_rolls_back_bus_when_persist_removal_fails() {
        // A directory squatting on the artifact's file path makes remove_file fail with a
        // non-NotFound error on every OS.
        let dir = tempfile::tempdir().unwrap();
        let victim = editor_io::scratch_file(dir.path(), "plan").unwrap();
        std::fs::create_dir_all(&victim).unwrap();
        let shared = test_shared(None, Some(dir.path().to_path_buf()));
        let key = artifact_bus_key("plan");
        let before = shared.bus.get(&key).unwrap().value;
        let (st, res) = handle_api("/api/delete", "POST", br#"{"name":"plan"}"#, &shared);
        assert_eq!(st, "500 Internal Server Error", "{res}");
        assert!(res.contains("\"ok\":false"), "{res}");
        assert_eq!(shared.bus.get(&key).expect("bus entry restored").value, before);
    }

    #[test]
    fn slugify_strips_dots_and_api_crud_and_broadcast_work_end_to_end() {
        assert_eq!(slugify_artifact_name("README.md"), "readme-md");
        assert_eq!(slugify_artifact_name("v1.0 spec"), "v1-0-spec");
        assert!(crate::secure_fs::validate_safe_name(&slugify_artifact_name("v1.0 spec")).is_ok());

        let shared = test_shared(None, None);
        let (st, res) = handle_api("/api/create", "POST", br#"{"name":""}"#, &shared);
        assert_eq!(st, "200 OK");
        assert!(res.contains("\"name\":\"doc-1\""), "{res}");

        let (st, res) = handle_api("/api/create", "POST", br#"{"name":"v1.0 Spec"}"#, &shared);
        assert_eq!(st, "200 OK");
        assert!(res.contains("\"name\":\"v1-0-spec\""), "{res}");

        let rev = shared.bus.get(&artifact_bus_key("plan")).unwrap().revision;
        let payload = format!(r##"{{"name":"plan","text":"# Plan\nUpdated instruction paragraph!\n- [x] Task 1","expected_revision":{rev}}}"##);
        let (st, res) = handle_api("/api/save", "POST", payload.as_bytes(), &shared);
        assert_eq!(st, "200 OK");
        assert!(res.contains("\"ok\":true"), "{res}");

        let (st, state) = handle_api("/api/state", "GET", b"", &shared);
        assert_eq!(st, "200 OK");
        assert!(state.contains("Updated instruction paragraph!"), "{state}");
        assert!(state.contains("\"done\":1"), "{state}");
        assert!(state.contains("\"active\":\"plan\""), "{state}");
        assert!(state.contains("\"active_seq\":1"), "{state}");

        let (st, _) = handle_api("/api/broadcast", "POST", b"not-json", &shared);
        assert_eq!(st, "400 Bad Request");
        let (st, _) = handle_api("/api/broadcast", "POST", br#"{"name":"../bad","note":"hi"}"#, &shared);
        assert_eq!(st, "400 Bad Request");
        let (st, _) = handle_api("/api/broadcast", "POST", br#"{"name":"plan","note":"ok\u202eevil\u001b[31m"}"#, &shared);
        assert_eq!(st, "200 OK");
        let queued = shared.pending_broadcasts.lock().unwrap().clone();
        assert_eq!(queued, vec![("plan".to_string(), "okevil[31m".to_string())]);

        // Windows reserved device name rejected on /api/create
        let (st, _) = handle_api("/api/create", "POST", br#"{"name":"CON"}"#, &shared);
        assert_eq!(st, "400 Bad Request");

        let (st, del) = handle_api("/api/delete", "POST", br#"{"name":"doc-1"}"#, &shared);
        assert_eq!(st, "200 OK");
        assert!(del.contains("\"ok\":true"), "{del}");
    }

    #[test]
    fn stalled_client_does_not_block_second_request_and_redacted_url_hides_token() {
        let bus = ContextBus::new();
        bus.set(&artifact_bus_key("plan"), "# Plan", None).unwrap();
        let srv = PreviewServer::start(bus, "plan".into(), None, None).unwrap();

        let full = srv.url().to_string();
        let redacted = srv.redacted_url().to_string();
        let rest = full.strip_prefix("http://").unwrap();
        let (addr, path_with_slash) = rest.split_once('/').unwrap();
        let token = path_with_slash.trim_end_matches('/');
        assert_eq!(token.len(), 64);
        assert!(!redacted.contains(token), "redacted URL must not expose full token: {redacted}");
        assert!(redacted.ends_with(".../"), "{redacted}");

        // Client 1 connects and stalls without sending any bytes
        let _stalled = TcpStream::connect(addr).unwrap();

        // Client 2 must still receive 200 OK promptly without waiting for Client 1's 2s deadline
        let t0 = Instant::now();
        let mut fast = TcpStream::connect(addr).unwrap();
        fast.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let req = format!("GET /{token}/api/state HTTP/1.1\r\nHost: {addr}\r\n\r\n");
        fast.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        fast.read_to_string(&mut resp).unwrap();
        assert!(resp.starts_with("HTTP/1.1 200 OK"), "{resp}");
        assert!(
            t0.elapsed() < Duration::from_millis(1000),
            "second request was blocked by stalled client: {:?}",
            t0.elapsed()
        );
    }
}
