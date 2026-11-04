//! Executor for `:agents` (Phase 50-03): structured per-pane agent state via `set_command_data`.
//!
//! Data carries pane id, sanitised identity token, state, timing and profile name only; never
//! screen text or command arguments (T-50-06).

use std::time::{Duration, Instant};

use crate::api::JsonValue;
use crate::wm::WindowManager;
use crate::wm::agents_cmd::AgentsOp;

/// Window over which `:agents list` reports a pane's recent communication partners.
const PARTNER_WINDOW: Duration = Duration::from_secs(60);

/// Executes one `:agents` operation; always returns `false` (never quits).
pub(super) fn execute_agents_command(wm: &mut WindowManager, op: AgentsOp) -> bool {
    match op {
        AgentsOp::List => list(wm),
        AgentsOp::Zoom(focus) => zoom(wm, focus),
        AgentsOp::Close => wm.close_agents_window(),
        AgentsOp::Dismiss => dismiss(wm),
    }
    false
}

fn zoom(wm: &mut WindowManager, focus: Option<crate::pane::PaneId>) {
    wm.open_agents_window(focus);
    if let Some(id) = focus
        && wm.pane(id).is_none()
    {
        wm.active_pane_mut().set_status_message(Some(format!("No pane #{id}")));
    }
}

/// Esc: close an unpinned window; a pinned window only releases the keyboard.
fn dismiss(wm: &mut WindowManager) {
    if wm.agents_window().pinned {
        wm.agents_window_mut().focused = false;
    } else {
        wm.close_agents_window();
    }
}

fn list(wm: &mut WindowManager) {
    if !wm.agents.has_updated() {
        wm.update_agent_states(Instant::now());
    }
    let now = Instant::now();
    let statuses = wm.agent_statuses();
    let agents: Vec<JsonValue> = statuses
        .into_iter()
        .map(|s| {
            let partners: Vec<JsonValue> = wm
                .partners_of(s.pane_id, PARTNER_WINDOW, now)
                .into_iter()
                .map(|p| {
                    JsonValue::obj([
                        ("label", JsonValue::String(p.endpoint.label())),
                        ("count", JsonValue::Number(p.count as i64)),
                    ])
                })
                .collect();
            JsonValue::obj([
                ("pane_id", JsonValue::Number(s.pane_id as i64)),
                ("name", JsonValue::String(s.name)),
                ("state", JsonValue::String(s.state.as_str().to_string())),
                ("since_ms", JsonValue::Number(s.since_ms as i64)),
                ("profile", s.profile.map_or(JsonValue::Null, JsonValue::String)),
                ("reported", JsonValue::Bool(s.reported)),
                ("partners", JsonValue::Array(partners)),
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
    let bus = wm.traffic();
    let traffic = JsonValue::obj([
        ("latest_seq", JsonValue::Number(bus.latest_seq() as i64)),
        ("len", JsonValue::Number(bus.len() as i64)),
        ("dropped", JsonValue::Number(bus.dropped() as i64)),
    ]);
    let data = JsonValue::obj([
        ("agents", JsonValue::Array(agents)),
        ("counts", counts),
        ("traffic", traffic),
    ]);
    wm.set_command_data(data.to_json_string());
    let msg = format!(
        "Agents: {} working · {} blocked · {} idle · {} done · {} errored",
        c.working, c.blocked, c.idle, c.done, c.errored
    );
    wm.active_pane_mut().set_status_message(Some(msg));
}
