//! Executor for `:agents` (Phase 50-03): structured per-pane agent state via `set_command_data`.
//!
//! Data carries pane id, sanitised identity token, state, timing and profile name only; never
//! screen text or command arguments (T-50-06).

use std::time::Instant;

use crate::api::JsonValue;
use crate::wm::WindowManager;
use crate::wm::agents_cmd::AgentsOp;

/// Executes one `:agents` operation; always returns `false` (never quits).
pub(super) fn execute_agents_command(wm: &mut WindowManager, op: AgentsOp) -> bool {
    match op {
        AgentsOp::List => list(wm),
    }
    false
}

fn list(wm: &mut WindowManager) {
    if !wm.agents.has_updated() {
        wm.update_agent_states(Instant::now());
    }
    let agents: Vec<JsonValue> = wm
        .agent_statuses()
        .into_iter()
        .map(|s| {
            JsonValue::obj([
                ("pane_id", JsonValue::Number(s.pane_id as i64)),
                ("name", JsonValue::String(s.name)),
                ("state", JsonValue::String(s.state.as_str().to_string())),
                ("since_ms", JsonValue::Number(s.since_ms as i64)),
                ("profile", s.profile.map_or(JsonValue::Null, JsonValue::String)),
                ("reported", JsonValue::Bool(s.reported)),
            ])
        })
        .collect();
    let c = wm.agent_counts();
    let counts = JsonValue::obj([
        ("working", JsonValue::Number(c.working as i64)),
        ("blocked", JsonValue::Number(c.blocked as i64)),
        ("idle", JsonValue::Number(c.idle as i64)),
        ("done", JsonValue::Number(c.done as i64)),
        ("errored", JsonValue::Number(c.errored as i64)),
    ]);
    let data = JsonValue::obj([("agents", JsonValue::Array(agents)), ("counts", counts)]);
    wm.set_command_data(data.to_json_string());
    let msg = format!(
        "Agents: {} working · {} blocked · {} idle · {} done · {} errored",
        c.working, c.blocked, c.idle, c.done, c.errored
    );
    wm.active_pane_mut().set_status_message(Some(msg));
}
