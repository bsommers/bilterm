//! JSON-RPC handlers for the web surface (`workspace.view_model`, `web.*`; `WEB-FE-01..04`, Phase 60).
//!
//! The HTTP/WS server never touches the `WindowManager` directly: every web operation is a
//! JSON-RPC call queued to the plane owner, and user-visible mutations lower to `Action`s.

use super::json::JsonValue;
use super::rpc::{AutomationControlPlane, SnapshotPolicy, extract_vt_row_text};
use super::view_model::{self, WorkspaceViewModelDto, agent_json, agents_json, parse_agent_id};
use crate::action::{Action, WebOp};
use crate::pane::PaneId;

/// Largest `web.input` payload (protocol `limits.maxInputBytes`).
pub const MAX_WEB_INPUT_BYTES: usize = 64 * 1024;
/// Most search hits one `web.search` returns.
pub const MAX_SEARCH_RESULTS: usize = 500;

fn agent_param(params: &JsonValue) -> Result<PaneId, String> {
    let raw = params
        .get("agentId")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .or_else(|| params.get("pane_id").and_then(JsonValue::as_usize).map(|n| n.to_string()))
        .ok_or_else(|| "missing 'agentId' parameter".to_string())?;
    parse_agent_id(&raw).ok_or_else(|| format!("invalid agentId '{raw}'"))
}

fn opt_u64(params: &JsonValue, key: &str) -> Option<u64> {
    params.get(key).and_then(JsonValue::as_i64).and_then(|n| u64::try_from(n).ok())
}

impl AutomationControlPlane {
    pub(super) fn dispatch_web(&mut self, method: &str, params: &JsonValue) -> Result<JsonValue, String> {
        match method {
            "workspace.view_model" => Ok(WorkspaceViewModelDto::from_wm_with_hub(&self.wm, Some(&self.output_hub)).to_json()),
            "web.status" => self.web_op(WebOp::Status),
            "web.open" => self.web_op(WebOp::OpenBrowser),
            "web.stop" => self.web_op(WebOp::Stop),
            "web.port" => {
                let port = params
                    .get("port")
                    .and_then(JsonValue::as_i64)
                    .and_then(|p| u16::try_from(p).ok())
                    .filter(|p| *p > 0)
                    .ok_or_else(|| "missing or invalid 'port' (1-65535)".to_string())?;
                self.web_op(WebOp::SetPort(port))
            }
            "web.pump" => Ok(self.web_pump()),
            "web.agents" => Ok(JsonValue::obj([
                ("ok", JsonValue::Bool(true)),
                ("agents", JsonValue::Array(agents_json(&self.wm, Some(&self.output_hub)))),
            ])),
            "web.agent" => {
                let id = agent_param(params)?;
                let agent = agent_json(&self.wm, id, Some(&self.output_hub)).ok_or_else(|| format!("agent pane-{id} not found"))?;
                Ok(JsonValue::obj([("ok", JsonValue::Bool(true)), ("agent", agent)]))
            }
            "web.logs" => {
                let id = agent_param(params)?;
                if self.wm.pane(id).is_none() {
                    return Err(format!("agent pane-{id} not found"));
                }
                let limit = params.get("limit").and_then(JsonValue::as_usize).unwrap_or(500).min(2000);
                let (lines, more) = view_model::log_lines_json(&self.output_hub, id, opt_u64(params, "beforeSeq"), opt_u64(params, "afterSeq"), limit);
                Ok(JsonValue::obj([
                    ("ok", JsonValue::Bool(true)),
                    ("lines", JsonValue::Array(lines)),
                    ("hasMore", JsonValue::Bool(more)),
                ]))
            }
            "web.search" => Ok(self.web_search(params)),
            "web.input" => {
                let id = agent_param(params)?;
                let data = params.get("data").and_then(JsonValue::as_str).ok_or_else(|| "missing 'data' parameter".to_string())?;
                if data.len() > MAX_WEB_INPUT_BYTES {
                    return Err(format!("input exceeds {MAX_WEB_INPUT_BYTES} bytes"));
                }
                let pane = self.wm.pane_mut(id).ok_or_else(|| format!("agent pane-{id} not found"))?;
                if pane.is_read_only() {
                    return Err(format!("agent pane-{id} is read-only"));
                }
                pane.write_input(data.as_bytes()).map_err(|e| e.to_string())?;
                Ok(JsonValue::obj([("ok", JsonValue::Bool(true)), ("bytes", JsonValue::Number(data.len() as i64))]))
            }
            "web.create" => self.web_create(params),
            "web.kill" => {
                let id = agent_param(params)?;
                if self.wm.pane(id).is_none() {
                    return Err(format!("agent pane-{id} not found"));
                }
                if self.wm.pane_count() <= 1 {
                    return Err("refusing to kill the last pane".to_string());
                }
                if !self.wm.remove_pane(id) {
                    return Err(format!("could not remove pane-{id}"));
                }
                self.output_hub.retain(&self.wm.panes().keys().copied().collect::<Vec<_>>());
                Ok(JsonValue::obj([("ok", JsonValue::Bool(true)), ("agentId", JsonValue::String(view_model::agent_id(id)))]))
            }
            "web.update" => {
                let id = agent_param(params)?;
                if let Some(name) = params.get("name").and_then(JsonValue::as_str) {
                    self.rename_pane(id, name)?;
                }
                let agent = agent_json(&self.wm, id, Some(&self.output_hub)).ok_or_else(|| format!("agent pane-{id} not found"))?;
                Ok(JsonValue::obj([("ok", JsonValue::Bool(true)), ("agent", agent)]))
            }
            other => Err(format!("Method not found: {other}")),
        }
    }

    fn web_op(&mut self, op: WebOp) -> Result<JsonValue, String> {
        self.apply_with_policy(Action::Web(op), SnapshotPolicy::Trusted)?;
        let data = self.wm.take_command_data().unwrap_or_default();
        Ok(JsonValue::parse(&data).unwrap_or_else(|| self.wm.web_surface().status().to_json()))
    }

    fn rename_pane(&mut self, id: PaneId, name: &str) -> Result<(), String> {
        if self.wm.pane(id).is_none() {
            return Err(format!("agent pane-{id} not found"));
        }
        let op = crate::wm::window_manage::WindowManageOp::Rename { window_id: Some(id), new_name: name.to_string() };
        self.apply_with_policy(Action::WindowManage(op), SnapshotPolicy::Trusted).map(|_| ())
    }

    /// Drives PTYs, mirrors new output into the hub, and reports `{agents, seqs}`.
    pub fn web_pump(&mut self) -> JsonValue {
        let ids: Vec<PaneId> = self.wm.panes().keys().copied().collect();
        for &id in &ids {
            if let Some(p) = self.wm.pane_mut(id) {
                p.set_output_tap(true);
            }
        }
        self.wm.poll_all_ptys();
        self.drain_output_taps();
        let live: Vec<PaneId> = self.wm.panes().keys().copied().collect();
        self.output_hub.retain(&live);
        let seqs = JsonValue::Object(
            live.iter().map(|&id| (view_model::agent_id(id), JsonValue::Number(self.output_hub.last_seq(id) as i64))).collect(),
        );
        JsonValue::obj([
            ("agents", JsonValue::Array(agents_json(&self.wm, Some(&self.output_hub)))),
            ("seqs", seqs),
        ])
    }

    /// Moves tapped pane output into the hub (also called after `pane.feed_output`).
    pub fn drain_output_taps(&mut self) {
        let ids: Vec<PaneId> = self.wm.panes().keys().copied().collect();
        for id in ids {
            let Some(p) = self.wm.pane_mut(id) else { continue };
            let bytes = p.take_output_tap();
            if !bytes.is_empty() {
                self.output_hub.push(id, &bytes);
            }
        }
    }

    fn web_create(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let before: Vec<PaneId> = self.wm.panes().keys().copied().collect();
        self.apply_with_policy(Action::SplitVertical, SnapshotPolicy::Trusted)?;
        let new_id = self
            .wm
            .panes()
            .keys()
            .copied()
            .find(|id| !before.contains(id))
            .ok_or_else(|| "pane was not created".to_string())?;
        if let Some(name) = params.get("name").and_then(JsonValue::as_str).filter(|n| !n.trim().is_empty()) {
            self.rename_pane(new_id, name)?;
        }
        if let Some(cmd) = params.get("initialCmd").and_then(JsonValue::as_str).filter(|c| !c.is_empty()) {
            if cmd.len() > MAX_WEB_INPUT_BYTES {
                return Err(format!("initialCmd exceeds {MAX_WEB_INPUT_BYTES} bytes"));
            }
            if let Some(p) = self.wm.pane_mut(new_id) {
                p.write_input(format!("{cmd}\r").as_bytes()).map_err(|e| e.to_string())?;
            }
        }
        if let Some(p) = self.wm.pane_mut(new_id) {
            p.set_output_tap(true);
        }
        let agent = agent_json(&self.wm, new_id, Some(&self.output_hub)).ok_or_else(|| "created pane vanished".to_string())?;
        Ok(JsonValue::obj([("ok", JsonValue::Bool(true)), ("agent", agent)]))
    }

    /// Line search over retained output (falling back to visible rows) as protocol `SearchHit`s.
    fn web_search(&mut self, params: &JsonValue) -> JsonValue {
        let q = params.get("q").and_then(JsonValue::as_str).unwrap_or("");
        let case = params.get("caseSensitive").and_then(JsonValue::as_bool).unwrap_or(false);
        let limit = params.get("limit").and_then(JsonValue::as_usize).unwrap_or(100).clamp(1, MAX_SEARCH_RESULTS);
        let needle = if case { q.to_string() } else { q.to_lowercase() };
        let mut hits = Vec::new();
        let mut truncated = false;
        if !needle.is_empty() {
            'panes: for (&id, pane) in self.wm.panes() {
                let (mut lines, _) = view_model::log_lines_json(&self.output_hub, id, None, None, 2000);
                if lines.is_empty() {
                    let (rows, _) = pane.vt().size();
                    lines = (0..rows)
                        .map(|r| {
                            JsonValue::obj([
                                ("agentId", JsonValue::String(view_model::agent_id(id))),
                                ("seq", JsonValue::Number(0)),
                                ("line", JsonValue::Number(i64::from(r))),
                                ("ts", JsonValue::Number(0)),
                                ("text", JsonValue::String(extract_vt_row_text(pane.vt(), r))),
                            ])
                        })
                        .collect();
                }
                for line in lines {
                    let text = line.get("text").and_then(JsonValue::as_str).unwrap_or("");
                    let hay = if case { text.to_string() } else { text.to_lowercase() };
                    let Some(idx) = hay.find(&needle) else { continue };
                    if hits.len() >= limit {
                        truncated = true;
                        break 'panes;
                    }
                    let mut hit = line.clone();
                    if let JsonValue::Object(m) = &mut hit {
                        m.insert("matchIndex".into(), JsonValue::Number(idx as i64));
                        m.insert("matchLength".into(), JsonValue::Number(needle.len() as i64));
                    }
                    hits.push(hit);
                }
            }
        }
        JsonValue::obj([
            ("ok", JsonValue::Bool(true)),
            ("results", JsonValue::Array(hits)),
            ("truncated", JsonValue::Bool(truncated)),
        ])
    }
}
