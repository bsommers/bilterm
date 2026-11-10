//! Executor for `Action::Swarm` (Phase 69 wiring): runs the shared op, publishes JSON, sets status.

use crate::wm::WindowManager;
use crate::wm::swarm_cmd::SwarmOp;

pub fn execute_swarm(wm: &mut WindowManager, op: SwarmOp) -> bool {
    let form = op.colon_form();
    match wm.swarm_apply(op) {
        Ok(data) => {
            wm.set_command_data(data.to_json_string());
            wm.active_pane_mut().set_status_message(Some(format!("{form}: ok")));
        }
        Err(e) => {
            let msg = format!("swarm: {e}");
            wm.record_automation_error(msg.clone());
            wm.active_pane_mut().set_status_message(Some(msg));
        }
    }
    false
}
