//! Live agent / pane / workspace control RPC (`HERDR-01..03`).
//!
//! `agent.prompt|approve|last_turn` plus the selector form of `agent.wait` (see
//! `agent_wait.rs`), `pane.list|split|focus|zoom|send` and `workspace.list|select|new|rename`.
//! Every mutation lowers to an existing `Action` applied through `WindowManager::apply`
//! (`SplitHorizontal`, `FocusPane`, `ZoomToggle`, `SessionRestore`, `SessionManage`,
//! `DrawerNav(NewSession)`) or to the pane's own prompt-injection path; there is no second
//! implementation of any behavior. Selectors are resolved by `selector.rs`.

use std::time::Instant;

use super::json::JsonValue;
use super::playbook::AutomationEvent;
use super::rpc::AutomationControlPlane;
use super::rpc::SnapshotPolicy;
use crate::action::Action;
use crate::pane::PaneId;
use crate::wm::traffic::{TrafficEndpoint, TrafficKind};

/// Maximum bytes returned by `agent.last_turn`.
const LAST_TURN_MAX_BYTES: usize = 16 * 1024;
/// Maximum accepted prompt / send text.
const MAX_TEXT_BYTES: usize = 256 * 1024;

fn num(n: usize) -> JsonValue {
    JsonValue::Number(n as i64)
}

fn s(v: &str) -> JsonValue {
    JsonValue::String(v.to_string())
}

fn need_text(params: &JsonValue) -> Result<&str, String> {
    let t = params
        .get("text")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "missing 'text' parameter".to_string())?;
    if t.len() > MAX_TEXT_BYTES {
        return Err("'text' exceeds 256 KiB".to_string());
    }
    Ok(t)
}

impl AutomationControlPlane {
    fn state_name(&self, id: PaneId) -> &'static str {
        self.wm.agent_state(id).map_or("unknown", |st| st.as_str())
    }

    fn traffic_source(&mut self, params: &JsonValue) -> Result<TrafficEndpoint, String> {
        match params.get("from_pane").and_then(JsonValue::as_usize) {
            Some(from) if self.wm.pane_mut(from).is_some() => Ok(TrafficEndpoint::Pane(from)),
            Some(from) => Err(format!("from_pane #{from} not found")),
            None => Ok(TrafficEndpoint::Api),
        }
    }

    /// Resolves the target and rejects read-only and `!ECHO` password panes.
    fn writable_target(&mut self, params: &JsonValue) -> Result<(PaneId, bool), String> {
        let id = self.target_pane(params)?;
        let allow_echo = params.get("allow_echo_lock").and_then(JsonValue::as_bool).unwrap_or(false);
        let pane = self.wm.pane_mut(id).ok_or_else(|| format!("pane #{id} not found"))?;
        if pane.is_read_only() {
            return Err(format!("pane #{id} is locked read-only (PROT)"));
        }
        let redact = pane.is_injection_password_locked();
        if redact && !allow_echo {
            return Err(format!("pane #{id} is in !ECHO LOCK password prompt; set allow_echo_lock=true to override"));
        }
        Ok((id, redact))
    }

    /// `agent.prompt`: smart prompt injection into the selected agent pane.
    pub(super) fn rpc_agent_prompt(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let text = need_text(params)?.to_string();
        let source = self.traffic_source(params)?;
        let (id, redact) = self.writable_target(params)?;
        let kind = self
            .wm
            .pane_mut(id)
            .ok_or_else(|| format!("pane #{id} not found"))?
            .inject_prompt(&text)
            .map_err(|e| e.to_string())?;
        self.wm.record_traffic_with(source, TrafficEndpoint::Pane(id), TrafficKind::Exec, &text, redact, Instant::now());
        self.wm.agents_hud_note_prompt(id, &text);
        self.push_event(AutomationEvent::CommandSubmitted { pane_id: id, command: text });
        Ok(JsonValue::obj([
            ("status", s("injected")),
            ("pane_id", num(id)),
            ("ref", JsonValue::String(format!("w1:p{id}"))),
            ("mode", s(kind.as_str())),
            ("state", s(self.state_name(id))),
        ]))
    }

    /// `agent.approve`: answers a blocked agent's approval prompt (default Enter; `answer`:
    /// `yes`/`y`, `no`/`n`, `esc`). Heuristic: it needs the pane to be `blocked` unless `force`.
    /// Shares [`crate::wm::WindowManager::agent_approve_pane`] with the HUD Approve button.
    pub(super) fn rpc_agent_approve(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let source = self.traffic_source(params)?;
        let id = self.target_pane(params)?;
        let force = params.get("force").and_then(JsonValue::as_bool).unwrap_or(false);
        let allow_echo = params.get("allow_echo_lock").and_then(JsonValue::as_bool).unwrap_or(false);
        let answer = params.get("answer").and_then(JsonValue::as_str).unwrap_or("yes").to_ascii_lowercase();
        let state = self.wm.agent_approve_pane(id, &answer, force, allow_echo, source)?;
        Ok(JsonValue::obj([
            ("status", s("approved")),
            ("pane_id", num(id)),
            ("answer", JsonValue::String(answer)),
            ("was", s(state)),
        ]))
    }

    /// `agent.wait` selector form: one non-blocking state check (`met` / `pending`). Clients
    /// (`bilterm agent wait`) poll until their own deadline.
    pub(super) fn rpc_agent_wait_state(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let id = self.target_pane(params)?;
        let until = params.get("until").and_then(JsonValue::as_str).unwrap_or("idle").to_ascii_lowercase();
        let state = self.wm.agent_state(id);
        let met = match (until.as_str(), state) {
            ("attention", Some(st)) => st.is_attention(),
            (u, Some(st)) if ["working", "blocked", "idle", "done", "errored"].contains(&u) => st.as_str() == u,
            (u, _) if ["working", "blocked", "idle", "done", "errored", "attention"].contains(&u) => false,
            (u, _) => return Err(format!("unknown 'until' state '{u}'")),
        };
        Ok(JsonValue::obj([
            ("status", s(if met { "met" } else { "pending" })),
            ("pane_id", num(id)),
            ("state", s(self.state_name(id))),
            ("until", JsonValue::String(until)),
        ]))
    }

    /// `agent.last_turn`: structured extraction of the last turn from the pane screen.
    ///
    /// Shell panes report the last command block (command, exit code, duration). Every pane also
    /// reports `response`: the visible rows after the last row that echoes the submitted
    /// text (best effort heuristic), bounded to 16 KiB.
    pub(super) fn rpc_agent_last_turn(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let id = self.target_pane(params)?;
        let max = params
            .get("max_bytes")
            .and_then(JsonValue::as_usize)
            .map_or(LAST_TURN_MAX_BYTES, |m| m.clamp(1, LAST_TURN_MAX_BYTES));
        let pane = self.wm.pane(id).ok_or_else(|| format!("pane #{id} not found"))?;
        let (rows, _) = pane.vt().size();
        let lines: Vec<String> = (0..rows).map(|r| pane.vt().row_text(r).trim_end().to_string()).collect();
        let block = pane.vt().command_blocks().last().cloned();
        let anchor = block.as_ref().map(|b| b.command.trim().chars().take(40).collect::<String>()).filter(|c| !c.is_empty());
        let start = anchor
            .as_ref()
            .and_then(|a| lines.iter().rposition(|l| l.contains(a.as_str())))
            .map_or(0, |i| i + 1);
        let mut response = lines[start.min(lines.len())..].join("\n").trim().to_string();
        if response.len() > max {
            let mut cut = response.len() - max;
            while !response.is_char_boundary(cut) {
                cut += 1;
            }
            response = response[cut..].to_string();
        }
        let (command, exit_code, duration_ms) = match &block {
            Some(b) => (
                JsonValue::String(b.command.clone()),
                b.exit_code.map_or(JsonValue::Null, |c| JsonValue::Number(i64::from(c))),
                b.duration_ms.map_or(JsonValue::Null, |d| JsonValue::Number(d as i64)),
            ),
            None => (JsonValue::Null, JsonValue::Null, JsonValue::Null),
        };
        let kind = pane.prompt_injection_kind().as_str();
        Ok(JsonValue::obj([
            ("pane_id", num(id)),
            ("ref", JsonValue::String(format!("w1:p{id}"))),
            ("state", s(self.state_name(id))),
            ("mode", s(kind)),
            ("command", command),
            ("exit_code", exit_code),
            ("duration_ms", duration_ms),
            ("anchored", JsonValue::Bool(anchor.is_some() && start > 0)),
            ("response", JsonValue::String(response)),
        ]))
    }

    /// `pane.list`: every pane with its hierarchical ref and agent state.
    pub(super) fn rpc_pane_list(&mut self) -> Result<JsonValue, String> {
        self.wm.update_agent_states(Instant::now());
        let active = self.wm.active_pane_id();
        let statuses = self.wm.agent_statuses();
        let mut out = Vec::new();
        for (&id, pane) in self.wm.panes() {
            let st = statuses.iter().find(|x| x.pane_id == id);
            let (rows, cols) = pane.vt().size();
            out.push(JsonValue::obj([
                ("id", num(id)),
                ("ref", JsonValue::String(format!("w1:p{id}"))),
                ("title", JsonValue::String(pane.title().to_string())),
                ("focused", JsonValue::Bool(id == active)),
                ("state", s(st.map_or("unknown", |x| x.state.as_str()))),
                ("profile", st.and_then(|x| x.profile.clone()).map_or(JsonValue::Null, JsonValue::String)),
                ("name", st.map_or(JsonValue::Null, |x| JsonValue::String(x.name.clone()))),
                ("mode", s(pane.prompt_injection_kind().as_str())),
                ("rows", JsonValue::Number(i64::from(rows))),
                ("cols", JsonValue::Number(i64::from(cols))),
            ]));
        }
        Ok(JsonValue::obj([("workspace", s("w1")), ("active_pane_id", num(active)), ("panes", JsonValue::Array(out))]))
    }

    fn apply_trusted(&mut self, action: Action) -> Result<(), String> {
        self.apply_with_policy(action, SnapshotPolicy::Trusted).map(|_| ())
    }

    fn focus_if_selected(&mut self, params: &JsonValue) -> Result<(), String> {
        if params.get("selector").is_some() || params.get("pane_id").is_some() {
            let id = self.target_pane(params)?;
            self.apply_trusted(Action::FocusPane(id))?;
        }
        Ok(())
    }

    /// `pane.split` (`direction`: `h`/`horizontal`/`v`/`vertical`, default `h`).
    pub(super) fn rpc_pane_split(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let action = match params.get("direction").and_then(JsonValue::as_str).unwrap_or("h") {
            "h" | "horizontal" => Action::SplitHorizontal,
            "v" | "vertical" => Action::SplitVertical,
            other => return Err(format!("invalid direction '{other}' (h or v)")),
        };
        self.focus_if_selected(params)?;
        self.apply_trusted(action)?;
        Ok(JsonValue::obj([
            ("status", s("split")),
            ("active_pane_id", num(self.wm.active_pane_id())),
            ("pane_count", num(self.wm.pane_count())),
        ]))
    }

    /// `pane.focus`.
    pub(super) fn rpc_pane_focus(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        if params.get("selector").is_none() && params.get("pane_id").is_none() {
            return Err("missing 'selector' parameter".to_string());
        }
        self.focus_if_selected(params)?;
        Ok(JsonValue::obj([("status", s("focused")), ("active_pane_id", num(self.wm.active_pane_id()))]))
    }

    /// `pane.zoom`: toggles zoom on the selected (or active) pane.
    pub(super) fn rpc_pane_zoom(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        self.focus_if_selected(params)?;
        self.apply_trusted(Action::ZoomToggle)?;
        Ok(JsonValue::obj([
            ("status", s("ok")),
            ("zoomed", self.wm.zoomed_pane().map_or(JsonValue::Null, num)),
            ("active_pane_id", num(self.wm.active_pane_id())),
        ]))
    }

    /// `pane.send`: raw text to the pane's input (`enter`: append CR). No history/prompt path.
    pub(super) fn rpc_pane_send(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let text = need_text(params)?.to_string();
        let source = self.traffic_source(params)?;
        let (id, redact) = self.writable_target(params)?;
        let enter = params.get("enter").and_then(JsonValue::as_bool).unwrap_or(false);
        let mut bytes = text.clone().into_bytes();
        if enter {
            bytes.push(b'\r');
        }
        self.wm
            .pane_mut(id)
            .ok_or_else(|| format!("pane #{id} not found"))?
            .write_input(&bytes)
            .map_err(|e| e.to_string())?;
        self.wm.record_traffic_with(source, TrafficEndpoint::Pane(id), TrafficKind::Keys, &text, redact, Instant::now());
        Ok(JsonValue::obj([("status", s("sent")), ("pane_id", num(id)), ("bytes", num(bytes.len()))]))
    }

    /// `workspace.list`: `w1` is the live workspace (the active session); the rest are saved.
    pub(super) fn rpc_workspace_list(&mut self) -> Result<JsonValue, String> {
        let active = self.wm.config().history_session.clone();
        let mut rows = vec![JsonValue::obj([
            ("id", s("w1")),
            ("name", JsonValue::String(active.clone())),
            ("active", JsonValue::Bool(true)),
            ("live", JsonValue::Bool(true)),
            ("pane_count", num(self.wm.pane_count())),
        ])];
        for (i, name) in self.wm.list_sessions().into_iter().filter(|n| *n != active).enumerate() {
            rows.push(JsonValue::obj([
                ("id", JsonValue::String(format!("w{}", i + 2))),
                ("name", JsonValue::String(name)),
                ("active", JsonValue::Bool(false)),
                ("live", JsonValue::Bool(false)),
            ]));
        }
        Ok(JsonValue::obj([("workspaces", JsonValue::Array(rows))]))
    }

    /// `workspace.select` by saved-session `name` or `w<N>` id.
    pub(super) fn rpc_workspace_select(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let raw = params
            .get("name")
            .or_else(|| params.get("workspace"))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'name' parameter".to_string())?
            .trim()
            .to_string();
        let active = self.wm.config().history_session.clone();
        let target = match raw.strip_prefix(['w', 'W']).and_then(|n| n.parse::<usize>().ok()) {
            Some(1) => active.clone(),
            Some(n) => self
                .wm
                .list_sessions()
                .into_iter()
                .filter(|x| *x != active)
                .nth(n - 2)
                .ok_or_else(|| format!("workspace w{n} not found"))?,
            None => raw,
        };
        if target != active {
            self.apply_trusted(Action::SessionRestore(target.clone()))?;
        }
        Ok(JsonValue::obj([("status", s("selected")), ("name", JsonValue::String(target))]))
    }

    /// `workspace.new` (`name` required): new session via the drawer's `NewSession`, then renamed.
    pub(super) fn rpc_workspace_new(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let name = params
            .get("name")
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .ok_or_else(|| "missing 'name' parameter".to_string())?
            .to_string();
        crate::secure_fs::validate_safe_name(&name)?;
        self.apply_trusted(Action::DrawerNav(crate::wm::drawer::DrawerNavOp::NewSession))?;
        self.rename_active_session(&name)?;
        Ok(JsonValue::obj([("status", s("created")), ("name", JsonValue::String(name))]))
    }

    /// `workspace.rename` (`name`) renames the active workspace/session.
    pub(super) fn rpc_workspace_rename(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let name = params
            .get("name")
            .or_else(|| params.get("new_name"))
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .ok_or_else(|| "missing 'name' parameter".to_string())?
            .to_string();
        crate::secure_fs::validate_safe_name(&name)?;
        self.rename_active_session(&name)?;
        Ok(JsonValue::obj([("status", s("renamed")), ("name", JsonValue::String(name))]))
    }

    fn rename_active_session(&mut self, name: &str) -> Result<(), String> {
        if self.wm.config().history_session == name {
            return Ok(());
        }
        self.apply_trusted(Action::SessionManage(crate::wm::window_manage::SessionManageOp::Rename {
            old_name: None,
            new_name: name.to_string(),
        }))
    }
}
