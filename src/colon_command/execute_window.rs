//! Execution dispatch for `:window` and `:session` management commands (`SES-01..05`).

use crate::wm::WindowManager;
use crate::wm::window_manage::{SessionManageOp, WindowManageOp};

/// Dispatches a `:window ...` management operation to [`WindowManager::apply_window_manage`].
pub fn execute_window_manage(wm: &mut WindowManager, op: WindowManageOp) -> bool {
    match wm.apply_window_manage(&op) {
        Ok(msg) => {
            if !msg.is_empty() {
                wm.active_pane_mut().set_status_message(Some(msg));
            }
        }
        Err(err) => {
            wm.record_automation_error_msg(err.clone());
            wm.active_pane_mut().set_status_message(Some(err));
        }
    }
    false
}

/// Dispatches a `:session ...` management operation to [`WindowManager::apply_session_manage`].
pub fn execute_session_manage(wm: &mut WindowManager, op: SessionManageOp) -> bool {
    match wm.apply_session_manage(&op) {
        Ok(msg) => {
            if !msg.is_empty() {
                wm.active_pane_mut().set_status_message(Some(msg));
            }
        }
        Err(err) => {
            wm.record_automation_error_msg(err.clone());
            wm.active_pane_mut().set_status_message(Some(err));
        }
    }
    false
}
