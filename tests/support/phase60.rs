//! Shared raw-socket helpers for the Phase 60 web-surface tests (HTTP + RFC 6455 client).
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bilterm::api::ws_codec::{decode_frame, encode_masked_frame, Opcode};
use bilterm::api::{serve_http_standalone_with, AutomationControlPlane, JsonValue, WebServeOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub const TOKEN: &str = "phase60-token-0123456789";
pub const WS_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

pub async fn start(plane: AutomationControlPlane, opts: WebServeOptions) -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(serve_http_standalone_with(l, plane, Arc::from(TOKEN), opts));
    addr
}

pub async fn raw(addr: SocketAddr, req: &[u8]) -> String {
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(req).await.unwrap();
    let mut out = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await;
    String::from_utf8_lossy(&out).into_owned()
}

pub fn status(resp: &str) -> u16 {
    resp.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0)
}

pub fn body(resp: &str) -> &str {
    resp.split_once("\r\n\r\n").map_or("", |(_, b)| b)
}

pub fn get(addr: SocketAddr, path: &str, auth: bool) -> String {
    let a = if auth { format!("Authorization: Bearer {TOKEN}\r\n") } else { String::new() };
    format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\n{a}\r\n")
}

pub fn post(addr: SocketAddr, path: &str, json: &str) -> String {
    format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{json}",
        json.len()
    )
}

pub fn rpc(plane: &mut AutomationControlPlane, method: &str, params: &str) -> JsonValue {
    let line = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    let resp = JsonValue::parse(&plane.handle_jsonrpc_line(&line)).expect("json-rpc response");
    resp.get("result").cloned().unwrap_or_else(|| panic!("{method} failed: {}", resp.to_json_string()))
}

/// A minimal masked-frame WebSocket client.
pub struct WsClient {
    pub stream: TcpStream,
    buf: Vec<u8>,
    pub handshake: String,
}

impl WsClient {
    pub async fn connect(addr: SocketAddr, path: &str, bearer: bool) -> Self {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let a = if bearer { format!("Authorization: Bearer {TOKEN}\r\n") } else { String::new() };
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {WS_KEY}\r\n{a}\r\n"
        );
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let end = loop {
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
            let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut chunk))
                .await
                .expect("handshake timeout")
                .unwrap();
            assert!(n > 0, "connection closed during handshake: {}", String::from_utf8_lossy(&buf));
            buf.extend_from_slice(&chunk[..n]);
        };
        let handshake = String::from_utf8_lossy(&buf[..end]).into_owned();
        let rest = buf[end..].to_vec();
        Self { stream, buf: rest, handshake }
    }

    pub async fn send_json(&mut self, json: &str) {
        let f = encode_masked_frame(Opcode::Text, json.as_bytes(), [0x12, 0x34, 0x56, 0x78]);
        self.stream.write_all(&f).await.unwrap();
    }

    pub async fn send_raw(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).await.unwrap();
    }

    /// Next frame (opcode, payload); `None` on EOF/timeout.
    pub async fn next_frame(&mut self) -> Option<(Opcode, Vec<u8>)> {
        let mut chunk = [0u8; 8192];
        loop {
            if let Some((f, used)) = decode_frame(&self.buf, false).expect("server frame") {
                self.buf.drain(..used);
                return Some((f.opcode, f.payload));
            }
            let n = tokio::time::timeout(Duration::from_secs(5), self.stream.read(&mut chunk)).await.ok()?.ok()?;
            if n == 0 {
                return None;
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    /// Next text message parsed as JSON.
    pub async fn next_msg(&mut self) -> Option<JsonValue> {
        loop {
            let (op, p) = self.next_frame().await?;
            match op {
                Opcode::Text => return JsonValue::parse(std::str::from_utf8(&p).ok()?),
                Opcode::Close => return None,
                _ => {}
            }
        }
    }

    /// Reads messages until one of type `ty` arrives; returns it plus everything seen before it.
    pub async fn wait_for(&mut self, ty: &str) -> (JsonValue, Vec<JsonValue>) {
        let mut seen = Vec::new();
        loop {
            let m = self.next_msg().await.unwrap_or_else(|| {
                panic!("no '{ty}' message; saw {:?}", seen.iter().map(JsonValue::to_json_string).collect::<Vec<_>>())
            });
            if msg_type(&m) == ty {
                return (m, seen);
            }
            seen.push(m);
        }
    }
}

pub fn msg_type(m: &JsonValue) -> &str {
    m.get("type").and_then(JsonValue::as_str).unwrap_or("")
}

pub fn hello(extra: &str) -> String {
    format!(r#"{{"type":"hello","contract":1{extra}}}"#)
}
