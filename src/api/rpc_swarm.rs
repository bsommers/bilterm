//! JSON-RPC `swarm.*` handlers (Phase 69 wiring); `bilterm ctl swarm.status` reaches these.

use crate::api::json::JsonValue;
use crate::api::rpc::AutomationControlPlane;
use crate::wm::swarm_cmd::SwarmOp;

fn text(p: &JsonValue, k: &str) -> Result<String, String> {
    p.get(k).and_then(JsonValue::as_str).map(str::to_string).ok_or_else(|| format!("missing '{k}' parameter"))
}

fn flag(p: &JsonValue, k: &str) -> bool {
    p.get(k).and_then(JsonValue::as_bool).unwrap_or(false)
}

impl AutomationControlPlane {
    /// Dispatches `swarm.<verb>` to the same [`SwarmOp`] the colon command uses.
    pub(super) fn rpc_swarm(&mut self, method: &str, p: &JsonValue) -> Result<JsonValue, String> {
        let op = match method {
            "swarm.load" => SwarmOp::Load(text(p, "path")?),
            "swarm.status" => SwarmOp::Status,
            "swarm.run" => SwarmOp::Run,
            "swarm.spawn" => SwarmOp::Spawn {
                slug: text(p, "slug")?,
                agent: text(p, "agent").unwrap_or_else(|_| "claude".to_string()),
                prompt: text(p, "prompt").unwrap_or_default(),
            },
            "swarm.verify" => SwarmOp::Verify(text(p, "node")?),
            "swarm.diff" => SwarmOp::Diff(text(p, "slug")?),
            "swarm.merge" => SwarmOp::Merge { slug: text(p, "slug")?, squash: flag(p, "squash") },
            "swarm.clean" => SwarmOp::Clean { slug: text(p, "slug")?, force: flag(p, "force") },
            _ => return Err(format!("unknown method {method}")),
        };
        self.wm.swarm_apply(op)
    }
}
