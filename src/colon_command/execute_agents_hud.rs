//! Executors for the Phase 70 colon forms (HUD-01..03): lower to the window manager methods,
//! show a status line, and leave a JSON summary as structured command data (RPC `wm.command`).

use crate::api::JsonValue;
use crate::wm::WindowManager;
use crate::wm::agents_hud_cmd::{HudOp, ReplayOp};

fn report(wm: &mut WindowManager, res: Result<Option<String>, String>, what: &str) {
    match res {
        Ok(Some(msg)) => wm.active_pane_mut().set_status_message(Some(msg)),
        Ok(None) => {}
        Err(err) => {
            wm.record_automation_error(err.clone());
            wm.active_pane_mut()
                .set_status_message(Some(format!("{what} error: {err}")));
        }
    }
}

/// Executes one `:agents hud` operation; always returns `false` (never quits).
pub(super) fn execute_hud(wm: &mut WindowManager, op: &HudOp) -> bool {
    let res = wm.agents_hud_apply(op);
    report(wm, res, "HUD");
    wm.set_command_data(crate::api::agents_hud_json::hud_snapshot(wm).to_json_string());
    false
}

/// Executes one `:agent replay` operation; always returns `false`.
pub(super) fn execute_replay(wm: &mut WindowManager, op: &ReplayOp) -> bool {
    let res = wm.agents_replay_apply(op);
    report(wm, res, "Replay");
    if let Some(pane) = wm.agents_hud().replay_pane() {
        wm.set_command_data(crate::api::agents_hud_json::replay_list(wm, pane).to_json_string());
    }
    false
}

/// Executes `:agents watchdog`; always returns `false`.
pub(super) fn execute_watchdog(wm: &mut WindowManager) -> bool {
    let json: JsonValue = crate::api::agents_hud_json::watchdog_status(wm);
    wm.set_command_data(json.to_json_string());
    let n = wm.agents_hud().alerts().count();
    let on = wm.config().agents.watchdog.enabled;
    wm.active_pane_mut().set_status_message(Some(format!(
        "Watchdog {}: {n} recent alert(s)",
        if on { "on" } else { "off" }
    )));
    false
}
