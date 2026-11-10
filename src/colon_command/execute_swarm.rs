//! Executors for `Action::Swarm`, `Action::AgentsHud`, `Action::AgentReplay` (UNI-R01, Phases 69/70).

use crate::api::json::JsonValue;
use crate::swarm::dag::NodeState;
use crate::swarm::{SwarmOrchestrator, SwarmPlan};
use crate::wm::WindowManager;
use crate::wm::agent_state::AgentState;
use crate::wm::mission_control_hud::AgentCardTelemetry;
use crate::wm::swarm_surface::{HudOp, ReplayOp, SwarmOp};

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
                wm.swarm_surface_mut().swarm = Some(o);
                emit(wm, format!("swarm: loaded {n} node(s)"), data);
            }
            Err(e) => fail(wm, e),
        },
        SwarmOp::Clear => {
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

/// Runs one `:agents hud` op; opening refreshes one card per tracked pane.
pub fn execute_hud(wm: &mut WindowManager, op: HudOp) -> bool {
    let open = match op {
        HudOp::Toggle => !wm.swarm_surface().hud_open,
        HudOp::Open => true,
        HudOp::Close => false,
    };
    if open {
        let cards: Vec<(usize, String, AgentState)> = wm
            .panes()
            .iter()
            .map(|(id, p)| {
                (
                    *id,
                    p.title().to_string(),
                    wm.agent_state(*id).unwrap_or(AgentState::Idle),
                )
            })
            .collect();
        let hud = &mut wm.swarm_surface_mut().hud;
        for (id, title, state) in cards {
            hud.upsert_card(AgentCardTelemetry::new(id, &title, state));
        }
    }
    wm.swarm_surface_mut().hud_open = open;
    let tiles = wm.swarm_surface().hud.len();
    emit(
        wm,
        format!(
            "agents hud: {} ({tiles} tile(s))",
            if open { "open" } else { "closed" }
        ),
        JsonValue::obj([
            ("open", JsonValue::Bool(open)),
            ("tiles", JsonValue::Number(tiles as i64)),
        ]),
    );
    false
}

/// Runs one `:agent replay` op against the active pane's log (read-only; never touches the PTY).
pub fn execute_replay(wm: &mut WindowManager, op: ReplayOp) -> bool {
    let id = wm.active_pane_id();
    let log = wm.swarm_surface_mut().replay.entry(id).or_default();
    let moved = match op {
        ReplayOp::Show => true,
        ReplayOp::Prev => log.prev(),
        ReplayOp::Next => log.next(),
        ReplayOp::First => log.first(),
        ReplayOp::Last => log.last(),
    };
    let turns = log.len();
    let cur = log
        .current()
        .map(|t| (t.turn_id, t.prompt.clone(), t.files.clone()));
    let data = JsonValue::obj([
        ("pane_id", JsonValue::Number(id as i64)),
        ("turns", JsonValue::Number(turns as i64)),
        ("moved", JsonValue::Bool(moved)),
        (
            "turn_id",
            cur.as_ref()
                .map_or(JsonValue::Null, |c| JsonValue::Number(c.0 as i64)),
        ),
        (
            "prompt",
            cur.as_ref()
                .map_or(JsonValue::Null, |c| JsonValue::String(c.1.clone())),
        ),
        (
            "files",
            JsonValue::Array(cur.map_or_else(Vec::new, |c| {
                c.2.into_iter().map(JsonValue::String).collect()
            })),
        ),
    ]);
    emit(wm, format!("agent replay: {turns} turn(s)"), data);
    false
}
