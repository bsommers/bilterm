//! Executor for `Action::Swarm` (UNI-R01, Phase 69).

use crate::api::json::JsonValue;
use crate::swarm::dag::NodeState;
use crate::swarm::{SwarmOrchestrator, SwarmPlan};
use crate::wm::WindowManager;
use crate::wm::swarm_surface::SwarmOp;

/// Largest plan file `:swarm load` accepts.
const MAX_PLAN_BYTES: u64 = 256 * 1024;

fn fail(wm: &mut WindowManager, msg: String) {
    wm.record_automation_error(msg.clone());
    wm.active_pane_mut().set_status_message(Some(msg));
}

fn emit(wm: &mut WindowManager, status: String, data: JsonValue) {
    wm.set_command_data(data.to_json_string());
    wm.active_pane_mut().set_status_message(Some(status));
}

fn state_name(s: Option<NodeState>) -> &'static str {
    match s {
        Some(NodeState::Pending) => "pending",
        Some(NodeState::Running) => "running",
        Some(NodeState::Verifying) => "verifying",
        Some(NodeState::Passed) => "passed",
        Some(NodeState::Failed) => "failed",
        None => "unknown",
    }
}

fn load_plan(path: &str) -> Result<SwarmPlan, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("swarm load: {e}"))?;
    if !meta.is_file() || meta.len() > MAX_PLAN_BYTES {
        return Err(format!(
            "swarm load: '{path}' is not a regular file under {MAX_PLAN_BYTES} bytes"
        ));
    }
    let src = std::fs::read_to_string(path).map_err(|e| format!("swarm load: {e}"))?;
    SwarmPlan::parse(&src).map_err(|e| format!("swarm load: {e}"))
}

fn swarm_json(o: &SwarmOrchestrator) -> JsonValue {
    let plan = o.plan();
    let nodes = plan
        .nodes
        .iter()
        .map(|n| {
            JsonValue::obj([
                ("id", JsonValue::String(n.id.clone())),
                (
                    "state",
                    JsonValue::String(state_name(o.scheduler().state(&n.id)).to_string()),
                ),
            ])
        })
        .collect();
    let waves = plan
        .waves()
        .into_iter()
        .map(|w| JsonValue::Array(w.into_iter().map(JsonValue::String).collect()))
        .collect();
    JsonValue::obj([
        ("loaded", JsonValue::Bool(true)),
        ("nodes", JsonValue::Array(nodes)),
        ("waves", JsonValue::Array(waves)),
        ("finished", JsonValue::Bool(o.is_finished())),
        ("max_parallel", JsonValue::Number(plan.max_parallel as i64)),
    ])
}

/// Runs one `:swarm` op; never quits.
pub fn execute_swarm(wm: &mut WindowManager, op: SwarmOp) -> bool {
    wm.swarm_poll(); // apply finished background verifications before any op
    match op {
        SwarmOp::Status => {
            let (status, data) = match &wm.swarm_surface().swarm {
                Some(o) => (
                    format!("swarm: {} node(s)", o.plan().nodes.len()),
                    swarm_json(o),
                ),
                None => (
                    "swarm: no plan loaded".to_string(),
                    JsonValue::obj([("loaded", JsonValue::Bool(false))]),
                ),
            };
            emit(wm, status, data);
        }
        SwarmOp::Load(path) if path.trim().is_empty() => {
            fail(wm, "Usage: :swarm load <plan.toml>".to_string())
        }
        SwarmOp::Load(path) => match load_plan(path.trim()) {
            Ok(plan) => {
                let o = SwarmOrchestrator::new(plan);
                let data = swarm_json(&o);
                let n = o.plan().nodes.len();
                wm.swarm_surface_mut().runtime.reset();
                wm.swarm_surface_mut().swarm = Some(o);
                emit(wm, format!("swarm: loaded {n} node(s)"), data);
            }
            Err(e) => fail(wm, e),
        },
        op @ (SwarmOp::Run
        | SwarmOp::Spawn { .. }
        | SwarmOp::Verify(_)
        | SwarmOp::Diff(_)
        | SwarmOp::Merge { .. }
        | SwarmOp::Clean { .. }) => {
            let form = op.colon_form();
            match wm.swarm_apply(op) {
                Ok(data) => emit(wm, format!("{form}: ok"), data),
                Err(e) => fail(wm, format!("swarm: {e}")),
            }
        }
        SwarmOp::Clear => {
            wm.swarm_surface_mut().runtime.reset();
            let had = wm.swarm_surface_mut().swarm.take().is_some();
            emit(
                wm,
                "swarm: cleared".to_string(),
                JsonValue::obj([("cleared", JsonValue::Bool(had))]),
            );
        }
    }
    false
}
