//! Executor for `:daemon` colon commands (`DAEM-05..08`, Phase 55).

use crate::action::DaemonSupervisorOp;
use crate::api::json::JsonValue;
use crate::daemon::registry::DaemonRegistry;
use crate::daemon::supervisor::{
    daemon_lock_path_in, ensure_started, stop_daemon, DaemonLockRecord, DaemonStartResult,
};
use crate::secure_fs::private_runtime_dir;
use crate::wm::WindowManager;

/// Executes a daemon supervisor operation against the local runtime directory.
pub fn execute_daemon_command(wm: &mut WindowManager, op: &DaemonSupervisorOp) -> bool {
    let runtime_dir = match private_runtime_dir() {
        Ok(dir) => dir,
        Err(e) => {
            wm.active_pane_mut()
                .set_status_message(Some(format!("Daemon error: {e}")));
            return false;
        }
    };

    match op {
        DaemonSupervisorOp::List => {
            let active = DaemonRegistry::list_active(&runtime_dir);
            let mut list_json = Vec::new();
            for item in &active {
                list_json.push(JsonValue::obj([
                    ("id", JsonValue::String(item.id.clone())),
                    ("name", JsonValue::String(item.name.clone())),
                    ("pid", JsonValue::Number(item.pid as i64)),
                    ("version", JsonValue::String(item.version.clone())),
                    ("started_at_ms", JsonValue::Number(item.started_at_ms as i64)),
                    (
                        "socket_path",
                        JsonValue::String(item.socket_path.to_string_lossy().into_owned()),
                    ),
                    (
                        "sessions_count",
                        JsonValue::Number(item.sessions_count as i64),
                    ),
                    ("panes_count", JsonValue::Number(item.panes_count as i64)),
                    ("agents_count", JsonValue::Number(item.agents_count as i64)),
                    ("is_current", JsonValue::Bool(item.is_current())),
                ]));
            }
            let res_json = JsonValue::obj([("daemons", JsonValue::Array(list_json))]);
            wm.set_command_data(res_json.to_json_string());

            let summary = if active.is_empty() {
                "No background daemons currently running".to_string()
            } else {
                let items: Vec<String> = active
                    .iter()
                    .map(|d| {
                        format!(
                            "{}(pid {}, {} sess, {} panes)",
                            d.name, d.pid, d.sessions_count, d.panes_count
                        )
                    })
                    .collect();
                format!("Active daemons ({}): {}", active.len(), items.join(", "))
            };
            wm.active_pane_mut().set_status_message(Some(summary));
            wm.open_daemon_window();
        }
        DaemonSupervisorOp::Status { name } => {
            let target = name.as_deref().unwrap_or("default");
            let active = DaemonRegistry::list_active(&runtime_dir);
            let found = active.into_iter().find(|d| d.name == target);

            let lock_path = daemon_lock_path_in(&runtime_dir, target).ok();
            let lock_rec = lock_path
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|s| DaemonLockRecord::from_json(&s).ok());

            if let Some(d) = found {
                let res = JsonValue::obj([
                    ("name", JsonValue::String(d.name.clone())),
                    ("pid", JsonValue::Number(d.pid as i64)),
                    ("version", JsonValue::String(d.version.clone())),
                    (
                        "socket_path",
                        JsonValue::String(d.socket_path.to_string_lossy().into_owned()),
                    ),
                    (
                        "sessions_count",
                        JsonValue::Number(d.sessions_count as i64),
                    ),
                    ("panes_count", JsonValue::Number(d.panes_count as i64)),
                    ("agents_count", JsonValue::Number(d.agents_count as i64)),
                    ("status", JsonValue::String("running".to_string())),
                ]);
                wm.set_command_data(res.to_json_string());
                wm.active_pane_mut().set_status_message(Some(format!(
                    "Daemon '{}': running (PID {}, {} sessions, {} panes)",
                    d.name, d.pid, d.sessions_count, d.panes_count
                )));
            } else if let Some(rec) = lock_rec {
                let res = JsonValue::obj([
                    ("name", JsonValue::String(rec.name.clone())),
                    ("pid", JsonValue::Number(rec.pid as i64)),
                    ("status", JsonValue::String("locked".to_string())),
                ]);
                wm.set_command_data(res.to_json_string());
                wm.active_pane_mut().set_status_message(Some(format!(
                    "Daemon '{}': lock held by PID {}",
                    rec.name, rec.pid
                )));
            } else {
                let res = JsonValue::obj([
                    ("name", JsonValue::String(target.to_string())),
                    ("status", JsonValue::String("stopped".to_string())),
                ]);
                wm.set_command_data(res.to_json_string());
                wm.active_pane_mut()
                    .set_status_message(Some(format!("Daemon '{target}': not running")));
            }
            wm.open_daemon_window();
        }
        DaemonSupervisorOp::Start { name } => {
            let target = name.as_deref().unwrap_or("default");
            match ensure_started(&runtime_dir, target) {
                Ok(DaemonStartResult::Started { pid, socket_path }) => {
                    let res = JsonValue::obj([
                        ("name", JsonValue::String(target.to_string())),
                        ("status", JsonValue::String("started".to_string())),
                        ("pid", JsonValue::Number(pid as i64)),
                        (
                            "socket_path",
                            JsonValue::String(socket_path.to_string_lossy().into_owned()),
                        ),
                    ]);
                    wm.set_command_data(res.to_json_string());
                    wm.active_pane_mut().set_status_message(Some(format!(
                        "Daemon '{target}' started (PID {pid})"
                    )));
                }
                Ok(DaemonStartResult::AlreadyRunning { pid, socket_path }) => {
                    let res = JsonValue::obj([
                        ("name", JsonValue::String(target.to_string())),
                        ("status", JsonValue::String("already_running".to_string())),
                        ("pid", JsonValue::Number(pid as i64)),
                        (
                            "socket_path",
                            JsonValue::String(socket_path.to_string_lossy().into_owned()),
                        ),
                    ]);
                    wm.set_command_data(res.to_json_string());
                    wm.active_pane_mut().set_status_message(Some(format!(
                        "Daemon '{target}' is already running (PID {pid})"
                    )));
                }
                Err(e) => {
                    let res = JsonValue::obj([
                        ("name", JsonValue::String(target.to_string())),
                        ("status", JsonValue::String("error".to_string())),
                        ("error", JsonValue::String(e.to_string())),
                    ]);
                    wm.set_command_data(res.to_json_string());
                    wm.active_pane_mut().set_status_message(Some(format!(
                        "Failed to start daemon '{target}': {e}"
                    )));
                }
            }
        }
        DaemonSupervisorOp::Stop { name, force } => {
            let target = name.as_deref().unwrap_or("default");
            match stop_daemon(&runtime_dir, target, *force) {
                Ok(stopped) => {
                    let status = if stopped { "stopped" } else { "not_running" };
                    let res = JsonValue::obj([
                        ("name", JsonValue::String(target.to_string())),
                        ("status", JsonValue::String(status.to_string())),
                    ]);
                    wm.set_command_data(res.to_json_string());
                    let msg = if stopped {
                        format!("Daemon '{target}' stopped")
                    } else {
                        format!("Daemon '{target}' was not running")
                    };
                    wm.active_pane_mut().set_status_message(Some(msg));
                }
                Err(e) => {
                    let res = JsonValue::obj([
                        ("name", JsonValue::String(target.to_string())),
                        ("status", JsonValue::String("error".to_string())),
                        ("error", JsonValue::String(e.to_string())),
                    ]);
                    wm.set_command_data(res.to_json_string());
                    wm.active_pane_mut().set_status_message(Some(format!(
                        "Failed to stop daemon '{target}': {e}"
                    )));
                }
            }
        }
        DaemonSupervisorOp::Switch { name } => {
            let res = JsonValue::obj([
                ("name", JsonValue::String(name.clone())),
                ("status", JsonValue::String("switched".to_string())),
            ]);
            wm.set_command_data(res.to_json_string());
            wm.active_pane_mut().set_status_message(Some(format!(
                "Active daemon context switched to '{name}'"
            )));
        }
        DaemonSupervisorOp::Prune => {
            let pruned = DaemonRegistry::prune_dead(&runtime_dir);
            let res = JsonValue::obj([
                ("status", JsonValue::String("pruned".to_string())),
                ("pruned_count", JsonValue::Number(pruned as i64)),
            ]);
            wm.set_command_data(res.to_json_string());
            wm.active_pane_mut().set_status_message(Some(format!(
                "Pruned {pruned} dead daemon record(s)"
            )));
        }
    }
    if !matches!(op, DaemonSupervisorOp::List | DaemonSupervisorOp::Status { .. })
        && wm.daemon_window().active
    {
        wm.refresh_daemon_window();
    }
    false
}
