//! `GET /ws` RFC 6455 upgrade and `@agent-manager/protocol` v1 bridge (`WEB-FE-01`, `WEB-FE-03`).
//!
//! One task per connection. Every workspace read/write is a JSON-RPC call queued to the plane
//! owner (`web.*`), so the bridge never touches `WindowManager` state directly. Output streams
//! from the shared [`OutputHub`] with per-pane monotonic `seq`, `resumeFrom` replay and
//! `agent:gap`. View state (focus, scroll, viewport) lives in the [`ClientViewRegistry`] only.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

use super::http::RpcRequest;
use super::json::JsonValue;
use super::output_ring::{MAX_HISTORY_LIMIT, OutputHub};
use super::view_model::{CONTRACT_VERSION, agent_id, parse_agent_id, server_epoch};
use super::web_surface::WebSurfaceState;
use super::ws_codec::{self, Frame, MAX_FRAME_BYTES, Opcode};
use crate::pane::PaneId;

pub use super::ws_codec::{accept_key, decode_frame, encode_frame, encode_masked_frame, sha1};

/// Output pump interval while a client is connected.
pub const PUMP_INTERVAL: Duration = Duration::from_millis(50);
/// Connections idle (no frames at all) for this long are closed; clients ping every 15 s.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(90);
/// Most `agent:broadcast` targets (protocol `limits.maxBroadcastTargets`).
pub const MAX_BROADCAST_TARGETS: usize = 64;

/// Everything a WebSocket connection needs from the server.
#[derive(Clone)]
pub struct WsContext {
    pub rpc_tx: mpsc::Sender<RpcRequest>,
    pub hub: OutputHub,
    pub surface: WebSurfaceState,
    pub token: Arc<str>,
    /// Bearer token already verified on the upgrade request (header or `?token=`).
    pub pre_authed: bool,
    /// Traffic meter used to open the per-client `ws:web-N` channel (Phase 64).
    pub meter: Option<crate::traffic::TrafficMeter>,
}

/// Validates upgrade headers; returns the `101 Switching Protocols` response head.
pub fn upgrade_response(header: impl Fn(&str) -> Option<String>) -> Result<String, &'static str> {
    let has_token = |name: &str, tok: &str| {
        header(name).is_some_and(|v| v.split(',').any(|p| p.trim().eq_ignore_ascii_case(tok)))
    };
    if !has_token("upgrade", "websocket") || !has_token("connection", "upgrade") {
        return Err("expected Upgrade: websocket");
    }
    if header("sec-websocket-version").as_deref().map(str::trim) != Some("13") {
        return Err("unsupported Sec-WebSocket-Version (need 13)");
    }
    let key = header("sec-websocket-key").ok_or("missing Sec-WebSocket-Key")?;
    if !ws_codec::is_valid_client_key(&key) {
        return Err("invalid Sec-WebSocket-Key");
    }
    Ok(format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        accept_key(&key)
    ))
}

pub(super) async fn rpc(tx: &mpsc::Sender<RpcRequest>, method: &str, params: JsonValue) -> Result<JsonValue, String> {
    let req = JsonValue::obj([
        ("jsonrpc", JsonValue::String("2.0".into())),
        ("id", JsonValue::Number(1)),
        ("method", JsonValue::String(method.into())),
        ("params", params),
    ]);
    let (r_tx, r_rx) = oneshot::channel();
    tx.send((req.to_json_string(), r_tx)).await.map_err(|_| "control plane unavailable".to_string())?;
    let resp = r_rx.await.map_err(|_| "control plane unavailable".to_string())?;
    let v = JsonValue::parse(&resp).ok_or_else(|| "bad control plane reply".to_string())?;
    if let Some(r) = v.get("result") {
        return Ok(r.clone());
    }
    Err(v
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(JsonValue::as_str)
        .unwrap_or("control plane error")
        .to_string())
}

fn s(v: &str) -> JsonValue {
    JsonValue::String(v.to_string())
}

fn msg(ty: &str, fields: Vec<(&str, JsonValue)>) -> JsonValue {
    let mut all = vec![("type", s(ty))];
    all.extend(fields);
    JsonValue::obj(all)
}

fn error_code(message: &str) -> &'static str {
    if message.contains("not found") {
        "NOT_FOUND"
    } else if message.contains("exceeds") {
        "PAYLOAD_TOO_LARGE"
    } else if message.contains("unavailable") {
        "BACKEND_DOWN"
    } else if message.contains("read-only") || message.contains("refusing") {
        "FORBIDDEN"
    } else {
        "USAGE"
    }
}

fn error_msg(req_id: Option<&JsonValue>, agent: Option<&str>, code: &str, message: &str) -> JsonValue {
    let mut f = vec![("code", s(code)), ("message", s(message)), ("retryable", JsonValue::Bool(code == "BACKEND_DOWN"))];
    if let Some(r) = req_id {
        f.push(("reqId", r.clone()));
    }
    if let Some(a) = agent {
        f.push(("agentId", s(a)));
    }
    msg("agent:error", f)
}

/// Per-connection protocol state.
pub struct WsBridge {
    ctx: WsContext,
    client_id: Option<String>,
    authed: bool,
    sent: BTreeMap<PaneId, u64>,
    status: BTreeMap<PaneId, String>,
    known: BTreeSet<PaneId>,
    out: Vec<JsonValue>,
    close: Option<(u16, &'static str)>,
    tap: Option<crate::traffic::TrafficTap>,
}

impl WsBridge {
    pub fn new(ctx: WsContext) -> Self {
        let authed = ctx.pre_authed;
        Self { ctx, client_id: None, authed, sent: BTreeMap::new(), status: BTreeMap::new(), known: BTreeSet::new(), out: Vec::new(), close: None, tap: None }
    }

    /// Registered client id once `hello` succeeded.
    pub fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref()
    }

    fn agent_ids(agents: &JsonValue) -> Vec<(PaneId, JsonValue)> {
        agents
            .as_array()
            .unwrap_or(&[])
            .iter()
            .filter_map(|a| Some((parse_agent_id(a.get("id")?.as_str()?)?, a.clone())))
            .collect()
    }

    /// Streams new output and roster deltas; `initial` emits `agent:init_list` instead of deltas.
    async fn pump(&mut self, initial: Option<&BTreeMap<String, u64>>) {
        let Ok(r) = rpc(&self.ctx.rpc_tx, "web.pump", JsonValue::obj([] as [(&str, JsonValue); 0])).await else {
            return;
        };
        let agents = Self::agent_ids(r.get("agents").unwrap_or(&JsonValue::Null));
        let live: BTreeSet<PaneId> = agents.iter().map(|(id, _)| *id).collect();
        if initial.is_some() {
            self.out.push(msg("agent:init_list", vec![("agents", JsonValue::Array(agents.iter().map(|(_, a)| a.clone()).collect()))]));
        } else {
            for (id, a) in &agents {
                if !self.known.contains(id) {
                    self.out.push(msg("agent:added", vec![("agent", a.clone())]));
                }
            }
            for id in self.known.difference(&live) {
                self.out.push(msg("agent:removed", vec![("agentId", s(&agent_id(*id))), ("reason", s("gone"))]));
                self.sent.remove(id);
                self.status.remove(id);
            }
        }
        for (id, a) in &agents {
            let st = a.get("status").and_then(JsonValue::as_str).unwrap_or("idle").to_string();
            if initial.is_none() && self.known.contains(id) && self.status.get(id).is_some_and(|p| *p != st) {
                self.out.push(msg("agent:status_change", vec![("agentId", s(&agent_id(*id))), ("status", s(&st))]));
            }
            self.status.insert(*id, st);
        }
        self.known = live;
        for &id in &self.known {
            let resume = initial.and_then(|m| m.get(&agent_id(id)).copied());
            let replaying = initial.is_some();
            let after = resume.unwrap_or_else(|| self.sent.get(&id).copied().unwrap_or(0));
            if !replaying && after >= self.ctx.hub.last_seq(id) {
                continue;
            }
            let rep = self.ctx.hub.replay_after(id, after);
            if let Some((from, to)) = rep.gap
                && (resume.is_some() || !replaying)
            {
                self.out.push(msg("agent:gap", vec![("agentId", s(&agent_id(id))), ("fromSeq", JsonValue::Number(from as i64)), ("toSeq", JsonValue::Number(to as i64))]));
            }
            for c in rep.chunks {
                let mut f = vec![("agentId", s(&agent_id(id))), ("data", s(&c.data)), ("seq", JsonValue::Number(c.seq as i64))];
                if replaying {
                    f.push(("replay", JsonValue::Bool(true)));
                }
                self.out.push(msg("agent:output", f));
                self.sent.insert(id, c.seq);
            }
            self.sent.entry(id).or_insert(after);
        }
    }

    async fn on_hello(&mut self, m: &JsonValue) {
        if m.get("contract").and_then(JsonValue::as_i64) != Some(CONTRACT_VERSION) {
            self.out.push(error_msg(None, None, "USAGE", "unsupported contract version (bilterm speaks contract 1)"));
            self.close = Some((1002, "contract mismatch"));
            return;
        }
        if !self.authed {
            let ok = m
                .get("token")
                .and_then(JsonValue::as_str)
                .is_some_and(|t| crate::secure_fs::constant_time_eq(t.as_bytes(), self.ctx.token.as_bytes()));
            if !ok {
                self.out.push(error_msg(None, None, "UNAUTHENTICATED", "missing or invalid token"));
                self.close = Some((1008, "unauthenticated"));
                return;
            }
            self.authed = true;
        }
        if self.client_id.is_none() {
            let id = self.ctx.surface.clients().allocate_id();
            if !self.ctx.surface.clients().register(&id) {
                self.out.push(error_msg(None, None, "RATE_LIMITED", "too many web clients"));
                self.close = Some((1013, "too many clients"));
                return;
            }
            if let Some(m) = &self.ctx.meter {
                let origin = crate::traffic::Origin::Ws { client: id.clone() };
                self.tap = Some(m.open_channel(origin, &id, None, crate::traffic::Role::Wire));
            }
            self.client_id = Some(id);
        }
        let epoch = server_epoch();
        self.out.push(msg("hello_ack", vec![
            ("contract", JsonValue::Number(CONTRACT_VERSION)),
            ("epoch", s(&epoch)),
            ("server", JsonValue::obj([("name", s("bilterm")), ("version", s(crate::version_info::VERSION))])),
            ("auth", JsonValue::obj([("required", JsonValue::Bool(true))])),
            ("limits", JsonValue::obj([
                ("maxFrameBytes", JsonValue::Number(MAX_FRAME_BYTES as i64)),
                ("maxInputBytes", JsonValue::Number(super::rpc_web::MAX_WEB_INPUT_BYTES as i64)),
                ("maxBroadcastTargets", JsonValue::Number(MAX_BROADCAST_TARGETS as i64)),
            ])),
            ("backends", JsonValue::Array(vec![JsonValue::obj([
                ("id", s("bilterm")),
                ("label", s("Bilterm")),
                ("up", JsonValue::Bool(true)),
                ("capabilities", JsonValue::Array(Vec::new())),
            ])])),
            ("clientId", s(self.client_id.as_deref().unwrap_or(""))),
        ]));
        // resumeFrom is only meaningful within the same server epoch.
        let same_epoch = m.get("epoch").and_then(JsonValue::as_str).is_none_or(|e| e == epoch);
        let mut resume = BTreeMap::new();
        if same_epoch && let Some(JsonValue::Object(map)) = m.get("resumeFrom") {
            for (k, v) in map {
                if let Some(n) = v.as_i64().and_then(|n| u64::try_from(n).ok()) {
                    resume.insert(k.clone(), n);
                }
            }
        }
        self.sent.clear();
        self.pump(Some(&resume)).await;
    }

    async fn call_ack(&mut self, method: &str, params: JsonValue, op: &str, m: &JsonValue) {
        let req_id = m.get("reqId");
        let agent = m.get("agentId").and_then(JsonValue::as_str);
        match rpc(&self.ctx.rpc_tx, method, params).await {
            Ok(r) => {
                if let Some(rid) = req_id {
                    let mut f = vec![("reqId", rid.clone()), ("op", s(op)), ("ok", JsonValue::Bool(true))];
                    if let Some(a) = r.get("agent") {
                        f.push(("agent", a.clone()));
                    }
                    self.out.push(msg("agent:ack", f));
                }
                if op == "update"
                    && let (Some(a), Some(id)) = (r.get("agent"), agent)
                {
                    self.out.push(msg("agent:updated", vec![("agentId", s(id)), ("patch", a.clone())]));
                }
            }
            Err(e) => self.out.push(error_msg(req_id, agent, error_code(&e), &e)),
        }
    }

    /// Handles one decoded client text message.
    pub async fn on_text(&mut self, text: &str) {
        let Some(m) = JsonValue::parse(text) else {
            self.out.push(error_msg(None, None, "USAGE", "message is not JSON"));
            return;
        };
        let ty = m.get("type").and_then(JsonValue::as_str).unwrap_or("").to_string();
        if ty == "hello" {
            return self.on_hello(&m).await;
        }
        if self.client_id.is_none() {
            let code = if self.authed { "USAGE" } else { "UNAUTHENTICATED" };
            self.out.push(error_msg(None, None, code, "send hello first"));
            self.close = Some((1008, "hello required"));
            return;
        }
        let cid = self.client_id.clone().unwrap_or_default();
        let agent = m.get("agentId").and_then(JsonValue::as_str).unwrap_or("").to_string();
        let pane = parse_agent_id(&agent);
        let num = |k: &str| m.get(k).and_then(JsonValue::as_i64);
        match ty.as_str() {
            "ping" => self.out.push(msg("pong", vec![("t", m.get("t").cloned().unwrap_or(JsonValue::Null))])),
            "agent:input" => {
                let data = m.get("data").and_then(JsonValue::as_str).unwrap_or("");
                let p = JsonValue::obj([("agentId", s(&agent)), ("data", s(data))]);
                if let Err(e) = rpc(&self.ctx.rpc_tx, "web.input", p).await {
                    self.out.push(error_msg(m.get("reqId"), Some(&agent), error_code(&e), &e));
                }
            }
            "agent:resize" => {
                let (cols, rows) = (num("cols").unwrap_or(0), num("rows").unwrap_or(0));
                match (pane, u16::try_from(cols), u16::try_from(rows)) {
                    (Some(id), Ok(c), Ok(r)) if c > 0 && r > 0 => {
                        // WEB-FE-03: record this client's viewport only; the host layout is untouched.
                        self.ctx.surface.clients().set_pane_size(&cid, id, c, r);
                        self.ctx.surface.clients().update_viewport(&cid, c, r);
                    }
                    _ => self.out.push(error_msg(m.get("reqId"), Some(&agent), "USAGE", "agent:resize needs agentId, cols, rows")),
                }
            }
            "client:focus" => {
                self.ctx.surface.clients().set_focus(&cid, pane);
                self.push_client_view(&cid);
            }
            "client:scroll" => {
                if let (Some(id), Some(off)) = (pane, num("offset").and_then(|o| usize::try_from(o).ok())) {
                    self.ctx.surface.clients().set_scroll(&cid, id, off);
                }
                self.push_client_view(&cid);
            }
            "agent:create" => {
                let mut p = vec![("name", m.get("name").cloned().unwrap_or(JsonValue::Null))];
                if let Some(c) = m.get("initialCmd") {
                    p.push(("initialCmd", c.clone()));
                }
                self.call_ack("web.create", JsonValue::obj(p), "create", &m).await;
            }
            "agent:kill" => self.call_ack("web.kill", JsonValue::obj([("agentId", s(&agent))]), "kill", &m).await,
            "agent:update" => {
                let mut p = vec![("agentId", s(&agent))];
                if let Some(n) = m.get("name") {
                    p.push(("name", n.clone()));
                }
                self.call_ack("web.update", JsonValue::obj(p), "update", &m).await;
            }
            "agent:broadcast" => self.on_broadcast(&m).await,
            "agent:history" => {
                let Some(id) = pane else {
                    return self.out.push(error_msg(m.get("reqId"), Some(&agent), "USAGE", "agent:history needs agentId"));
                };
                let opt = |k: &str| num(k).and_then(|n| u64::try_from(n).ok());
                let limit = num("limit").and_then(|l| usize::try_from(l).ok()).unwrap_or(500).min(MAX_HISTORY_LIMIT);
                let (chunks, has_more) = self.ctx.hub.history(id, opt("beforeSeq"), opt("afterSeq"), limit);
                let chunks = chunks
                    .into_iter()
                    .map(|c| JsonValue::obj([("seq", JsonValue::Number(c.seq as i64)), ("ts", JsonValue::Number(c.ts as i64)), ("data", s(&c.data))]))
                    .collect();
                let mut f = vec![("agentId", s(&agent)), ("chunks", JsonValue::Array(chunks)), ("hasMore", JsonValue::Bool(has_more))];
                if let Some(r) = m.get("reqId") {
                    f.push(("reqId", r.clone()));
                }
                self.out.push(msg("agent:history_chunk", f));
            }
            other => self.out.push(error_msg(m.get("reqId"), None, "UNSUPPORTED", &format!("unsupported message type '{other}'"))),
        }
    }

    fn push_client_view(&mut self, cid: &str) {
        if let Some(v) = self.ctx.surface.clients().get(cid) {
            let focused = v.focused_pane_id.map_or(JsonValue::Null, |id| s(&agent_id(id)));
            let scroll = JsonValue::Object(v.scroll_offsets.iter().map(|(id, o)| (agent_id(*id), JsonValue::Number(*o as i64))).collect());
            self.out.push(msg("client:view", vec![("clientId", s(cid)), ("focusedAgentId", focused), ("scroll", scroll)]));
        }
    }

    async fn on_broadcast(&mut self, m: &JsonValue) {
        let targets: Vec<String> = m
            .get("targetAgentIds")
            .and_then(JsonValue::as_array)
            .unwrap_or(&[])
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        if targets.is_empty() || targets.len() > MAX_BROADCAST_TARGETS {
            return self.out.push(error_msg(m.get("reqId"), None, "USAGE", "targetAgentIds must have 1..=64 entries"));
        }
        let data = m.get("data").and_then(JsonValue::as_str).unwrap_or("").to_string();
        let mut results = Vec::new();
        for t in targets {
            let p = JsonValue::obj([("agentId", s(&t)), ("data", s(&data))]);
            let r = match rpc(&self.ctx.rpc_tx, "web.input", p).await {
                Ok(_) => JsonValue::obj([("agentId", s(&t)), ("status", s("sent"))]),
                Err(e) => JsonValue::obj([("agentId", s(&t)), ("status", s("failed")), ("code", s(error_code(&e))), ("message", s(&e))]),
            };
            results.push(r);
        }
        let mut f = vec![("op", s("broadcast")), ("ok", JsonValue::Bool(true)), ("results", JsonValue::Array(results))];
        f.push(("reqId", m.get("reqId").cloned().unwrap_or_else(|| s(""))));
        self.out.push(msg("agent:ack", f));
    }

    /// Drains queued outbound messages as encoded text frames.
    pub fn take_frames(&mut self) -> Vec<u8> {
        let mut wire = Vec::new();
        for m in self.out.drain(..) {
            wire.extend(encode_frame(Opcode::Text, m.to_json_string().as_bytes()));
        }
        wire
    }
}

impl Drop for WsBridge {
    fn drop(&mut self) {
        if let Some(id) = &self.client_id {
            self.ctx.surface.clients().unregister(id);
        }
    }
}

/// Pulls complete messages out of `buf`, reassembling fragments; control frames are answered inline.
fn drain_messages(bridge: &mut WsBridge, buf: &mut Vec<u8>, partial: &mut Option<Vec<u8>>, ctl: &mut Vec<u8>) -> Vec<String> {
    let mut texts = Vec::new();
    while bridge.close.is_none() {
        let (Frame { fin, opcode, payload }, used) = match decode_frame(buf, true) {
            Ok(Some(x)) => x,
            Ok(None) => break,
            Err(e) => {
                bridge.close = Some((e.close_code, e.reason));
                break;
            }
        };
        buf.drain(..used);
        match opcode {
            Opcode::Ping => ctl.extend(encode_frame(Opcode::Pong, &payload)),
            Opcode::Pong => {}
            Opcode::Close => bridge.close = Some((1000, "bye")),
            Opcode::Binary => bridge.close = Some((1003, "binary frames are not supported")),
            Opcode::Text | Opcode::Continuation => {
                let acc = match (opcode, partial.take()) {
                    (Opcode::Text, None) => payload,
                    (Opcode::Continuation, Some(mut prev)) => {
                        prev.extend_from_slice(&payload);
                        prev
                    }
                    _ => {
                        bridge.close = Some((1002, "unexpected continuation"));
                        break;
                    }
                };
                if acc.len() > MAX_FRAME_BYTES {
                    bridge.close = Some((1009, "message too large"));
                } else if !fin {
                    *partial = Some(acc);
                } else {
                    match String::from_utf8(acc) {
                        Ok(t) => texts.push(t),
                        Err(_) => bridge.close = Some((1007, "invalid UTF-8")),
                    }
                }
            }
        }
    }
    if buf.len() > MAX_FRAME_BYTES + 16 {
        bridge.close = Some((1009, "message too large"));
    }
    texts
}

/// Runs an upgraded connection until close, error, idle timeout or server stop.
pub async fn serve_ws(mut stream: tokio::net::TcpStream, ctx: WsContext) -> std::io::Result<()> {
    let surface = ctx.surface.clone();
    let mut bridge = WsBridge::new(ctx);
    let mut buf: Vec<u8> = Vec::new();
    let mut partial: Option<Vec<u8>> = None;
    let mut chunk = vec![0u8; 16 * 1024];
    let mut tick = tokio::time::interval(PUMP_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_rx = tokio::time::Instant::now();
    // Bytes seen before `hello` allocated the `web-N` channel are flushed once it exists.
    let mut pending = (0usize, 0usize);
    loop {
        let mut wire = Vec::new();
        let mut rx_n = 0usize;
        tokio::select! {
            r = stream.read(&mut chunk) => {
                let n = match r { Ok(0) | Err(_) => return Ok(()), Ok(n) => n };
                last_rx = tokio::time::Instant::now();
                rx_n = n;
                buf.extend_from_slice(&chunk[..n]);
                for text in drain_messages(&mut bridge, &mut buf, &mut partial, &mut wire) {
                    bridge.on_text(&text).await;
                    if bridge.close.is_some() {
                        break;
                    }
                }
            }
            _ = tick.tick() => {
                if surface.is_stop_requested() {
                    bridge.close = Some((1001, "server stopping"));
                } else if last_rx.elapsed() > IDLE_TIMEOUT {
                    bridge.close = Some((1001, "idle timeout"));
                } else if bridge.client_id.is_some() {
                    bridge.pump(None).await;
                }
            }
        }
        wire.extend(bridge.take_frames());
        let close_frame = bridge
            .close
            .take()
            .map(|(code, reason)| encode_frame(Opcode::Close, &ws_codec::close_payload(code, reason)));
        pending.0 += rx_n;
        pending.1 += wire.len() + close_frame.as_ref().map_or(0, Vec::len);
        if let Some(t) = &bridge.tap {
            if pending.0 > 0 {
                t.rx(std::mem::take(&mut pending.0));
            }
            if pending.1 > 0 {
                t.tx(std::mem::take(&mut pending.1));
            }
        }
        if !wire.is_empty() {
            stream.write_all(&wire).await?;
        }
        if let Some(frame) = close_frame {
            let _ = stream.write_all(&frame).await;
            let _ = stream.flush().await;
            return Ok(());
        }
    }
}
