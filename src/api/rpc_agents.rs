//! `agents.list` and `agents.demo` RPC methods (Phase 51-10, UNI-R01 API leg).
//!
//! Both build the colon form (`:agents list [op args..]` / `:agents demo [stop]`) from a closed,
//! bounded parameter set, run it through `parse_colon_command` and `apply_with_policy`, and add a
//! small summary object. There is no second implementation of any behavior, and no argument can
//! carry a second command (closed `op` set, per-token charset, length and count caps).

use super::json::JsonValue;
use super::rpc::{AutomationControlPlane, SnapshotPolicy};
use crate::colon_command::parse_colon_command;
use crate::input::AppAction;
use crate::wm::agents_list_cmd::ListFilter;

/// Every `op` the list window accepts (`list` / absent is the data read).
const OPS: &[&str] = &[
    "open", "close", "toggle", "dismiss", "blur", "select", "jump", "zoom", "pin", "unpin",
    "scale", "filter", "dock", "float", "move", "resize", "rect", "size", "scroll",
];
const MAX_TOKENS: usize = 8;
const MAX_TOKEN_LEN: usize = 32;

fn bad_list(reason: &str) -> String {
    format!("invalid agents.list parameter: {reason}")
}

fn token_ok(t: &str) -> bool {
    !t.is_empty()
        && t.len() <= MAX_TOKEN_LEN
        && !t.starts_with(':')
        && t.chars().all(|c| c.is_ascii_alphanumeric() || "+-_.#%:".contains(c))
}

/// Normalises `args` (array of strings/integers or one whitespace-separated string) to tokens.
fn arg_tokens(args: Option<&JsonValue>) -> Result<Vec<String>, String> {
    let tokens: Vec<String> = match args {
        None | Some(JsonValue::Null) => Vec::new(),
        Some(JsonValue::String(s)) => s.split_whitespace().map(str::to_string).collect(),
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| match item {
                JsonValue::String(s) => Ok(s.clone()),
                JsonValue::Number(n) => Ok(n.to_string()),
                _ => Err(bad_list("args must be strings or integers")),
            })
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(bad_list("args must be an array or a string")),
    };
    if tokens.len() > MAX_TOKENS {
        return Err(bad_list("too many args"));
    }
    if !tokens.iter().all(|t| token_ok(t)) {
        return Err(bad_list("an arg token has a disallowed character, is empty or too long"));
    }
    Ok(tokens)
}

fn is_object_or_null(params: &JsonValue) -> bool {
    matches!(params, JsonValue::Object(_) | JsonValue::Null)
}

impl AutomationControlPlane {
    /// `agents.list`: every list-window operation plus the data read.
    pub(super) fn rpc_agents_list(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        if !is_object_or_null(params) {
            return Err(bad_list("params must be an object"));
        }
        let op = match params.get("op") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::String(s)) if s == "list" => None,
            Some(JsonValue::String(s)) if OPS.contains(&s.as_str()) => Some(s.as_str()),
            Some(JsonValue::String(_)) => return Err(bad_list("unknown op")),
            Some(_) => return Err(bad_list("op must be a string")),
        };
        let tokens = arg_tokens(params.get("args"))?;
        let mut cmd = String::from(":agents list");
        match op {
            Some(op) => {
                cmd.push(' ');
                cmd.push_str(op);
                for t in &tokens {
                    cmd.push(' ');
                    cmd.push_str(t);
                }
            }
            None if !tokens.is_empty() => return Err(bad_list("args need an op")),
            None => {}
        }
        let mut out = self.run_agents_command(cmd)?;
        let s = self.wm.agents_list_summary();
        out.push((
            "agents_list",
            JsonValue::obj([
                ("active", JsonValue::Bool(s.active)),
                ("pinned", JsonValue::Bool(s.pinned)),
                ("focused", JsonValue::Bool(s.focused)),
                ("docked", JsonValue::Bool(s.docked)),
                (
                    "filter",
                    JsonValue::String(
                        match s.filter {
                            ListFilter::All => "all",
                            ListFilter::Agents => "agents",
                        }
                        .to_string(),
                    ),
                ),
                (
                    "selected",
                    s.selected.map_or(JsonValue::Null, |id| JsonValue::Number(id as i64)),
                ),
                ("row_count", JsonValue::Number(s.row_count as i64)),
                ("scale_pct", JsonValue::Number(i64::from(s.scale_pct))),
            ]),
        ));
        Ok(JsonValue::obj(out))
    }

    /// `agents.demo`: start (default) or stop the agent traffic demo.
    pub(super) fn rpc_agents_demo(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let bad = |reason: &str| format!("invalid agents.demo parameter: {reason}");
        if !is_object_or_null(params) {
            return Err(bad("params must be an object"));
        }
        let stop = match params.get("stop") {
            None | Some(JsonValue::Null) => false,
            Some(JsonValue::Bool(b)) => *b,
            Some(_) => return Err(bad("stop must be a boolean")),
        };
        let cmd = if stop { ":agents demo stop" } else { ":agents demo" };
        let mut out = self.run_agents_command(cmd.to_string())?;
        let d = self.wm.agents_demo_summary();
        let ids = |v: &[usize]| JsonValue::Array(v.iter().map(|&i| JsonValue::Number(i as i64)).collect());
        out.push((
            "demo",
            JsonValue::obj([
                ("running", JsonValue::Bool(d.running)),
                ("endpoints", ids(&d.endpoints)),
                ("spawned", ids(&d.spawned)),
            ]),
        ));
        Ok(JsonValue::obj(out))
    }

    /// Runs one assembled colon command; returns the `wm.command` result fields.
    fn run_agents_command(
        &mut self,
        cmd: String,
    ) -> Result<Vec<(&'static str, JsonValue)>, String> {
        let action = parse_colon_command(&cmd)
            .ok_or_else(|| format!("not a valid :colon command: {cmd}"))??;
        let app_action = self
            .apply_with_policy(action, SnapshotPolicy::Confined)
            .map_err(|e| format!("action execution error: {e}"))?;
        let status = if app_action == AppAction::Quit { "quit" } else { "handled" };
        let mut out = vec![
            ("status", JsonValue::String(status.to_string())),
            ("command", JsonValue::String(cmd)),
            ("active_pane_id", JsonValue::Number(self.wm.active_pane_id() as i64)),
            ("pane_count", JsonValue::Number(self.wm.pane_count() as i64)),
        ];
        if let Some(data) = self.wm.take_command_data().and_then(|d| JsonValue::parse(&d)) {
            out.push(("data", data));
        }
        Ok(out)
    }
}
