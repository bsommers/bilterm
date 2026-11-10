//! JSON-RPC 2.0 handlers `swarm.status|load|clear`, `hud.toggle` and `agent.replay`
//! (UNI-R01, Phases 69/70). All lower to `Action` and run through `WindowManager::apply`.

use crate::action::Action;
use crate::api::json::JsonValue;
use crate::api::rpc::AutomationControlPlane;
use crate::wm::swarm_surface::{HudOp, ReplayOp, SwarmOp};

impl AutomationControlPlane {
    /// `swarm.status` -> `{loaded, nodes, waves, ...}`.
    pub(super) fn rpc_swarm_status(&mut self, _params: &JsonValue) -> Result<JsonValue, String> {
        self.apply_with_data(Action::Swarm(SwarmOp::Status))
    }

    /// `swarm.load` `{path}`.
    pub(super) fn rpc_swarm_load(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let path = params
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or("missing 'path' parameter")?;
        self.apply_with_data(Action::Swarm(SwarmOp::Load(path.to_string())))
    }

    /// `swarm.clear`.
    pub(super) fn rpc_swarm_clear(&mut self, _params: &JsonValue) -> Result<JsonValue, String> {
        self.apply_with_data(Action::Swarm(SwarmOp::Clear))
    }

    /// `hud.toggle` `{op?: toggle|open|close}` -> `{open, tiles}`.
    pub(super) fn rpc_hud_toggle(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let op = match params
            .get("op")
            .and_then(JsonValue::as_str)
            .unwrap_or("toggle")
        {
            "toggle" => HudOp::Toggle,
            "open" => HudOp::Open,
            "close" => HudOp::Close,
            other => return Err(format!("unknown hud op: {other}")),
        };
        self.apply_with_data(Action::AgentsHud(op))
    }

    /// `agent.replay` `{op?: show|prev|next|first|last}` -> turn summary.
    pub(super) fn rpc_agent_replay(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let op = match params
            .get("op")
            .and_then(JsonValue::as_str)
            .unwrap_or("show")
        {
            "show" => ReplayOp::Show,
            "prev" => ReplayOp::Prev,
            "next" => ReplayOp::Next,
            "first" => ReplayOp::First,
            "last" => ReplayOp::Last,
            other => return Err(format!("unknown replay op: {other}")),
        };
        self.apply_with_data(Action::AgentReplay(op))
    }
}
