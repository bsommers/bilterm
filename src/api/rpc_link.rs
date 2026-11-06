//! `remote.link` RPC method (Phase 51.1-11, UNI-R01 API leg) and the honest connect result.
//!
//! `remote.link {op?}` builds `:ssh-tmux info|reconnect|disconnect` from a closed `op` set, runs
//! it through `parse_colon_command` and `apply_with_policy`, and returns the `wm.command` result
//! plus `"link"` = [`link_summary_json`]. The colon string is assembled only from the validated
//! op, never from user text, so no parameter can carry a second command.

use super::json::JsonValue;
use super::rpc::{AutomationControlPlane, SnapshotPolicy};
use crate::colon_command::execute_link::link_summary_json;
use crate::colon_command::parse_colon_command;
use crate::input::AppAction;

fn bad(reason: &str) -> String {
    format!("invalid remote.link parameter: {reason}")
}

/// Maps the validated `op` to its colon form (`status` is the default).
fn colon_for(params: &JsonValue) -> Result<&'static str, String> {
    if !matches!(params, JsonValue::Object(_) | JsonValue::Null) {
        return Err(bad("params must be an object"));
    }
    match params.get("op") {
        None => Ok(":ssh-tmux info"),
        Some(JsonValue::String(s)) => match s.as_str() {
            "status" => Ok(":ssh-tmux info"),
            "reconnect" => Ok(":ssh-tmux reconnect"),
            "disconnect" => Ok(":ssh-tmux disconnect"),
            _ => Err(bad("op must be one of status, reconnect, disconnect")),
        },
        Some(_) => Err(bad("op must be a string")),
    }
}

impl AutomationControlPlane {
    /// `remote.link`: show, reconnect or stop the persistent remote tmux link.
    pub(super) fn rpc_remote_link(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let cmd = colon_for(params)?;
        let action = parse_colon_command(cmd)
            .ok_or_else(|| format!("not a valid :colon command: {cmd}"))??;
        let app_action = self
            .apply_with_policy(action, SnapshotPolicy::Confined)
            .map_err(|e| format!("action execution error: {e}"))?;
        let status = if app_action == AppAction::Quit { "quit" } else { "handled" };
        let mut out = vec![
            ("status", JsonValue::String(status.to_string())),
            ("command", JsonValue::String(cmd.to_string())),
            ("active_pane_id", JsonValue::Number(self.wm.active_pane_id() as i64)),
            ("pane_count", JsonValue::Number(self.wm.pane_count() as i64)),
        ];
        if let Some(data) = self.wm.take_command_data().and_then(|d| JsonValue::parse(&d)) {
            out.push(("data", data));
        }
        out.push(("link", link_summary_json(&self.wm.tmux_link_summary())));
        Ok(JsonValue::obj(out))
    }

    /// Honest result of `remote.connect_tmux` / `remote.connect_ssh {tmux_session}`: the link is
    /// only `connecting` until the control stream starts.
    pub(super) fn connect_tmux_result(
        &self,
        pane_id: usize,
        target: &str,
        session: &str,
    ) -> JsonValue {
        let summary = self.wm.tmux_link_summary();
        JsonValue::obj([
            ("status", JsonValue::String("connecting".to_string())),
            ("state", JsonValue::String(summary.state.to_string())),
            ("pane_id", JsonValue::Number(pane_id as i64)),
            ("target", JsonValue::String(target.to_string())),
            ("session", JsonValue::String(session.to_string())),
            ("link", link_summary_json(&summary)),
        ])
    }
}
