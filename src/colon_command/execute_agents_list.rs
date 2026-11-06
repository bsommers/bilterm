//! Executor for `:agents list <verb>` (Phase 51-01): lowers to [`WindowManager::agents_list_apply`].

use crate::wm::WindowManager;
use crate::wm::agents_list_cmd::AgentsListOp;

/// Executes one `:agents list` operation; always returns `false` (never quits).
pub(super) fn execute_agents_list_command(wm: &mut WindowManager, op: AgentsListOp) -> bool {
    if let Some(msg) = wm.agents_list_apply(&op) {
        status(wm, msg);
    }
    false
}

fn status(wm: &mut WindowManager, msg: String) {
    wm.active_pane_mut().set_status_message(Some(msg));
}
