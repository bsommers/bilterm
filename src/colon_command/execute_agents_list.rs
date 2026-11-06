//! Executor for `:agents list <verb>` (Phase 51-01, 51-08): lowers to
//! [`WindowManager::agents_list_apply`], shows its status text and leaves the window summary as
//! structured command data (RPC `wm.command` returns it as `data`).

use crate::api::JsonValue;
use crate::wm::WindowManager;
use crate::wm::agents_list_cmd::{AgentsListOp, ListFilter};

/// Executes one `:agents list` operation; always returns `false` (never quits).
pub(super) fn execute_agents_list_command(wm: &mut WindowManager, op: AgentsListOp) -> bool {
    if let Some(msg) = wm.agents_list_apply(&op) {
        status(wm, msg);
    }
    wm.set_command_data(summary_json(wm).to_json_string());
    false
}

/// The window summary scalars only: no pane names, no screen text (T-51-21).
pub(super) fn summary_json(wm: &WindowManager) -> JsonValue {
    let s = wm.agents_list_summary();
    let filter = match s.filter {
        ListFilter::All => "all",
        ListFilter::Agents => "agents",
    };
    JsonValue::obj([
        ("active", JsonValue::Bool(s.active)),
        ("pinned", JsonValue::Bool(s.pinned)),
        ("focused", JsonValue::Bool(s.focused)),
        ("docked", JsonValue::Bool(s.docked)),
        ("filter", JsonValue::String(filter.to_string())),
        ("selected", s.selected.map_or(JsonValue::Null, |id| JsonValue::Number(i64::try_from(id).unwrap_or(i64::MAX)))),
        ("row_count", JsonValue::Number(i64::try_from(s.row_count).unwrap_or(i64::MAX))),
        ("scale_pct", JsonValue::Number(i64::from(s.scale_pct))),
    ])
}

fn status(wm: &mut WindowManager, msg: String) {
    wm.active_pane_mut().set_status_message(Some(msg));
}
