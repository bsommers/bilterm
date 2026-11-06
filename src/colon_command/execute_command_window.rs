//! Executor for `:commands` / `:palette` window operations (WIN-02, Phase 51.2-02).

use crate::wm::WindowManager;
use crate::wm::command_window::CommandWindowOp;

pub(super) fn execute_command_window(wm: &mut WindowManager, op: CommandWindowOp) -> bool {
    if let Some(msg) = wm.command_window_apply(&op) {
        wm.active_pane_mut().set_status_message(Some(msg));
    }
    false
}
