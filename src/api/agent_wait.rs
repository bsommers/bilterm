//! `agent.wait`: wait for an inbox message without blocking the request loop.
//!
//! The socket/HTTP serve loops own the `AutomationControlPlane` and answer requests one at a time,
//! so a wait that sleeps would stall every other client, including the one sending the awaited
//! reply. Instead, a wait whose message is not yet present is parked in [`AgentWaiters`] along
//! with its reply channel and deadline. The loop then checks parked waiters after each request it
//! handles and on a periodic tick, and answers each one with the message or with a timeout once
//! its deadline passes.
//!
//! The synchronous `dispatch_method("agent.wait")` path (MCP stdio, playbooks, direct callers)
//! checks once and answers right away. Nothing else can touch the inbox while a sequential caller
//! is blocked, so sleeping there would only add latency before the same answer.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

use super::json::JsonValue;
use super::rpc::AutomationControlPlane;
use crate::wm::agent_mail::AgentMessage;

/// Default wait when the request omits `timeout_ms`.
const DEFAULT_TIMEOUT_MS: u64 = 5000;
/// Upper bound on any single wait.
const MAX_TIMEOUT_MS: u64 = 10_000;
/// Upper bound on concurrently parked waits; any wait past this limit is answered immediately.
const MAX_PENDING_WAITS: usize = 64;
/// How often the serve loops re-check parked waits for deadlines.
pub(super) const WAIT_TICK: Duration = Duration::from_millis(50);

/// A resolved `agent.wait` request: target inbox, optional `reply_to` filter and deadline.
pub(super) struct AgentWaitSpec {
    agent: String,
    reply_to: Option<u64>,
    deadline: Instant,
}

impl AgentWaitSpec {
    fn timeout_response(&self) -> JsonValue {
        JsonValue::obj([
            ("status", JsonValue::String("timeout".to_string())),
            ("agent", JsonValue::String(self.agent.clone())),
        ])
    }
}

/// `selector` / `until` select the pane-state form of `agent.wait` (never parked).
fn wants_state_wait(params: &JsonValue) -> bool {
    params.get("selector").is_some() || params.get("until").is_some()
}

fn received_response(msg: &AgentMessage) -> JsonValue {
    JsonValue::obj([
        ("status", JsonValue::String("received".to_string())),
        (
            "message",
            JsonValue::obj([
                ("id", JsonValue::Number(msg.id as i64)),
                ("from", JsonValue::String(msg.from.clone())),
                ("to", JsonValue::String(msg.to.clone())),
                ("body", JsonValue::String(msg.body.clone())),
                ("kind", JsonValue::String(msg.kind.clone())),
                (
                    "reply_to",
                    msg.reply_to
                        .map_or(JsonValue::Null, |id| JsonValue::Number(id as i64)),
                ),
                ("hop_count", JsonValue::Number(msg.hop_count as i64)),
                ("timestamp_ms", JsonValue::Number(msg.timestamp_ms as i64)),
                ("acked", JsonValue::Bool(msg.acked)),
                (
                    "file_path",
                    msg.file_path
                        .clone()
                        .map_or(JsonValue::Null, JsonValue::String),
                ),
            ]),
        ),
    ])
}

impl AutomationControlPlane {
    /// `agent.wait` (synchronous form): one inbox check, then the received or timeout result.
    pub(super) fn rpc_agent_wait(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        if wants_state_wait(params) {
            return self.rpc_agent_wait_state(params);
        }
        let spec = self.agent_wait_spec(params)?;
        Ok(self
            .agent_wait_check(&spec)
            .unwrap_or_else(|| spec.timeout_response()))
    }

    fn agent_wait_spec(&self, params: &JsonValue) -> Result<AgentWaitSpec, String> {
        let agent = params
            .get("agent")
            .and_then(JsonValue::as_str)
            .or_else(|| self.wm.agent_mail().name_for_pane(self.wm.active_pane_id()))
            .ok_or_else(|| "missing 'agent' parameter".to_string())?
            .to_string();
        let reply_to = params
            .get("reply_to")
            .and_then(JsonValue::as_i64)
            .and_then(|n| u64::try_from(n).ok());
        let timeout_ms = params
            .get("timeout_ms")
            .and_then(JsonValue::as_i64)
            .and_then(|n| u64::try_from(n).ok())
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);
        Ok(AgentWaitSpec {
            agent,
            reply_to,
            deadline: Instant::now() + Duration::from_millis(timeout_ms),
        })
    }

    /// The `received` result for the first unread message matching `spec`, if one exists.
    fn agent_wait_check(&self, spec: &AgentWaitSpec) -> Option<JsonValue> {
        self.wm
            .agent_mail()
            .get_inbox(&spec.agent, true)
            .iter()
            .find(|m| spec.reply_to.is_none_or(|rid| m.reply_to == Some(rid)))
            .map(received_response)
    }
}

struct PendingWait {
    id: JsonValue,
    spec: AgentWaitSpec,
    reply: oneshot::Sender<String>,
}

/// `agent.wait` requests parked by a serve loop until their message arrives or they time out.
#[derive(Default)]
pub(super) struct AgentWaiters {
    pending: Vec<PendingWait>,
}

impl AgentWaiters {
    pub(super) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Handles one request line: parks an unsatisfied `agent.wait`, answers everything else
    /// directly, then re-checks parked waits (the request may have delivered their message).
    pub(super) fn handle(
        &mut self,
        plane: &mut AutomationControlPlane,
        line: &str,
        reply: oneshot::Sender<String>,
    ) {
        match self.parkable_wait(plane, line) {
            Some((id, spec)) => self.pending.push(PendingWait { id, spec, reply }),
            None => {
                let _ = reply.send(plane.handle_jsonrpc_line(line));
            }
        }
        self.poll(plane);
    }

    /// Answers every parked wait whose message has arrived or whose deadline has passed, and
    /// drops any whose client has gone away.
    pub(super) fn poll(&mut self, plane: &AutomationControlPlane) {
        let now = Instant::now();
        for w in std::mem::take(&mut self.pending) {
            if w.reply.is_closed() {
                continue;
            }
            let result = match plane.agent_wait_check(&w.spec) {
                Some(received) => received,
                None if now >= w.spec.deadline => w.spec.timeout_response(),
                None => {
                    self.pending.push(w);
                    continue;
                }
            };
            let line = AutomationControlPlane::result_response(w.id, result).to_json_string();
            let _ = w.reply.send(line);
        }
    }

    /// `Some((id, spec))` when `line` is a valid `agent.wait` whose message is not yet present and
    /// there is room to park it. Every other case goes through the normal dispatcher.
    fn parkable_wait(
        &self,
        plane: &AutomationControlPlane,
        line: &str,
    ) -> Option<(JsonValue, AgentWaitSpec)> {
        if self.pending.len() >= MAX_PENDING_WAITS {
            return None;
        }
        let req = JsonValue::parse(line)?;
        if req.get("method").and_then(JsonValue::as_str) != Some("agent.wait") {
            return None;
        }
        let params = req
            .get("params")
            .cloned()
            .unwrap_or_else(|| JsonValue::Object(BTreeMap::new()));
        if wants_state_wait(&params) {
            return None;
        }
        let spec = plane.agent_wait_spec(&params).ok()?;
        if plane.agent_wait_check(&spec).is_some() {
            return None;
        }
        let id = req.get("id").cloned().unwrap_or(JsonValue::Null);
        Some((id, spec))
    }
}

