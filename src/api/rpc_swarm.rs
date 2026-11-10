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

    /// `swarm.run` / `swarm.spawn` / `swarm.verify` / `swarm.diff` / `swarm.merge` / `swarm.clean`.
    pub(super) fn rpc_swarm_runtime(&mut self, method: &str, p: &JsonValue) -> Result<JsonValue, String> {
        let text = |k: &str| p.get(k).and_then(JsonValue::as_str).map(str::to_string).ok_or_else(|| format!("missing '{k}' parameter"));
        let flag = |k: &str| p.get(k).and_then(JsonValue::as_bool).unwrap_or(false);
        let op = match method {
            "swarm.run" => SwarmOp::Run,
            "swarm.spawn" => SwarmOp::Spawn {
                slug: text("slug")?,
                agent: text("agent").unwrap_or_else(|_| "claude".to_string()),
                prompt: text("prompt").unwrap_or_default(),
            },
            "swarm.verify" => SwarmOp::Verify(text("node")?),
            "swarm.diff" => SwarmOp::Diff(text("slug")?),
            "swarm.merge" => SwarmOp::Merge { slug: text("slug")?, squash: flag("squash") },
            "swarm.clean" => SwarmOp::Clean { slug: text("slug")?, force: flag("force") },
            _ => return Err(format!("unknown method {method}")),
        };
        self.apply_with_data(Action::Swarm(op))
    }
}
