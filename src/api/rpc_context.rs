//! JSON-RPC 2.0 handlers `context.set|get|list` and `agent.queue` (UNI-R01, Phase 68).
//! All lower to `Action` and run through `WindowManager::apply`.

use crate::action::Action;
use crate::api::json::JsonValue;
use crate::api::rpc::{AutomationControlPlane, SnapshotPolicy};

use crate::wm::context_queue_ops::{AgentGateOp, AgentQueueOp, ContextOp};

fn str_param<'a>(params: &'a JsonValue, name: &str) -> Result<&'a str, String> {
    params.get(name).and_then(JsonValue::as_str).ok_or_else(|| format!("missing '{name}' parameter"))
}

impl AutomationControlPlane {
    /// Applies `action` and returns the executor's structured payload, or the status error.
    fn apply_with_data(&mut self, action: Action) -> Result<JsonValue, String> {
        self.apply_with_policy(action, SnapshotPolicy::Confined).map_err(|e| e.to_string())?;
        let data = self.wm.take_command_data().and_then(|d| JsonValue::parse(&d));
        Ok(data.unwrap_or(JsonValue::Null))
    }

    /// `context.set` `{key, value}` -> `{key, revision}`.
    pub(super) fn rpc_context_set(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let key = str_param(params, "key")?.to_string();
        let value = str_param(params, "value")?.to_string();
        self.apply_with_data(Action::Context(ContextOp::Set { key, value }))
    }

    /// `context.get` `{key}` -> entry.
    pub(super) fn rpc_context_get(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let key = str_param(params, "key")?.to_string();
        self.apply_with_data(Action::Context(ContextOp::Get { key }))
    }

    /// `context.list` `{prefix?}` -> `{entries, revision}`.
    pub(super) fn rpc_context_list(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let prefix = params.get("prefix").and_then(JsonValue::as_str).map(str::to_string);
        self.apply_with_data(Action::Context(ContextOp::List { prefix }))
    }

    /// `agent.queue` `{op: list|add|clear|drop|approve|deny, text?, index?}`.
    pub(super) fn rpc_agent_queue(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let op = params.get("op").and_then(JsonValue::as_str).unwrap_or("list");
        let action = match op {
            "list" => Action::AgentQueue(AgentQueueOp::List),
            "clear" => Action::AgentQueue(AgentQueueOp::Clear),
            "add" => Action::AgentQueue(AgentQueueOp::Add(str_param(params, "text")?.to_string())),
            "drop" => {
                let n = params.get("index").and_then(JsonValue::as_usize).filter(|n| *n >= 1);
                Action::AgentQueue(AgentQueueOp::Drop(n.ok_or_else(|| "missing or invalid 'index' (1-based)".to_string())?))
            }
            "approve" => Action::AgentGate(AgentGateOp::Approve),
            "deny" => Action::AgentGate(AgentGateOp::Deny),
            other => return Err(format!("unknown agent.queue op: {other}")),
        };
        self.apply_with_data(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(cp: &mut AutomationControlPlane, m: &str, p: &[(&str, JsonValue)]) -> Result<JsonValue, String> {
        cp.dispatch_method(m, &JsonValue::obj(p.iter().map(|(k, v)| (*k, v.clone()))))
    }
    fn s(v: &str) -> JsonValue {
        JsonValue::String(v.to_string())
    }

    #[test]
    fn context_set_get_list() {
        let mut cp = AutomationControlPlane::new_headless(24, 80);
        call(&mut cp, "context.set", &[("key", s("a.b")), ("value", s("hi there"))]).unwrap();
        let got = call(&mut cp, "context.get", &[("key", s("a.b"))]).unwrap();
        assert_eq!(got.get("value").and_then(JsonValue::as_str), Some("hi there"));
        let list = call(&mut cp, "context.list", &[("prefix", s("a."))]).unwrap();
        assert_eq!(list.get("entries").and_then(JsonValue::as_array).map(<[_]>::len), Some(1));
        assert!(call(&mut cp, "context.get", &[("key", s("nope"))]).is_err());
        assert!(call(&mut cp, "context.set", &[("key", s("k"))]).is_err());
    }

    #[test]
    fn agent_queue_add_list_drop_clear() {
        let mut cp = AutomationControlPlane::new_headless(24, 80);
        call(&mut cp, "agent.queue", &[("op", s("add")), ("text", s("one"))]).unwrap();
        call(&mut cp, "agent.queue", &[("op", s("add")), ("text", s("two"))]).unwrap();
        let l = call(&mut cp, "agent.queue", &[("op", s("list"))]).unwrap();
        assert_eq!(l.get("items").and_then(JsonValue::as_array).map(<[_]>::len), Some(2));
        let d = call(&mut cp, "agent.queue", &[("op", s("drop")), ("index", JsonValue::Number(1))]).unwrap();
        assert_eq!(d.get("dropped").and_then(JsonValue::as_str), Some("one"));
        assert!(call(&mut cp, "agent.queue", &[("op", s("drop")), ("index", JsonValue::Number(5))]).is_err());
        call(&mut cp, "agent.queue", &[("op", s("clear"))]).unwrap();
        let l = call(&mut cp, "agent.queue", &[]).unwrap();
        assert_eq!(l.get("items").and_then(JsonValue::as_array).map(<[_]>::len), Some(0));
        assert!(call(&mut cp, "agent.queue", &[("op", s("bogus"))]).is_err());
        assert!(call(&mut cp, "agent.queue", &[("op", s("approve"))]).is_err(), "idle pane is not blocked");
    }
}
