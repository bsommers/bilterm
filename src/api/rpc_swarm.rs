//! JSON-RPC 2.0 handlers `swarm.status|load|clear`
//! (UNI-R01, Phase 69). All lower to `Action` and run through `WindowManager::apply`.

use crate::action::Action;
use crate::api::json::JsonValue;
use crate::api::rpc::AutomationControlPlane;
use crate::wm::swarm_surface::SwarmOp;

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
}
