//! Executor for `:ssh-tmux reconnect|disconnect|info|prefill` (Phase 51.1, UNI-R01).
//!
//! No verb errors without a link: each sets a one-line status (exact strings of contract
//! section 9) so headless sweeps can run every example. `Info` also stores the structured
//! summary through `set_command_data`; `link_summary_json` is shared with `remote.link`.

use crate::api::JsonValue;
use crate::tmux::link::{LinkExitSummary, LinkSummary};
use crate::wm::WindowManager;
use crate::wm::link_cmd::RemoteLinkOp;

/// Executes one link verb; always returns `false` (never quits).
pub(super) fn execute_link_command(wm: &mut WindowManager, op: RemoteLinkOp) -> bool {
    match op {
        RemoteLinkOp::Prefill => {
            wm.prefill_ssh_tmux_prompt();
        }
        RemoteLinkOp::Reconnect => {
            wm.link_reconnect();
        }
        RemoteLinkOp::Disconnect => {
            wm.link_disconnect();
        }
        RemoteLinkOp::Info => info(wm),
    }
    false
}

fn info(wm: &mut WindowManager) {
    let summary = wm.tmux_link_summary();
    wm.set_command_data(link_summary_json(&summary).to_json_string());
    let line = match (&summary.dest, &summary.session) {
        (Some(dest), Some(session)) => format!(
            "Link {dest}/{session}: {} (attempt {}/{})",
            summary.state, summary.attempt, summary.max_attempts
        ),
        _ => "No tmux link (use :ssh-tmux <dest> [session])".to_string(),
    };
    wm.active_pane_mut().set_status_message(Some(line));
}

fn opt_str(s: &Option<String>) -> JsonValue {
    s.as_ref().map_or(JsonValue::Null, |v| JsonValue::String(v.clone()))
}

fn exit_json(e: &LinkExitSummary) -> JsonValue {
    JsonValue::obj([
        ("cause", JsonValue::String(e.cause.clone())),
        ("class", JsonValue::String(e.class.to_string())),
        ("code", e.code.map_or(JsonValue::Null, |c| JsonValue::Number(i64::from(c)))),
        ("reason", opt_str(&e.reason)),
        ("tail", JsonValue::Array(e.tail.iter().cloned().map(JsonValue::String).collect())),
        ("at_ms", JsonValue::Number(e.at_ms as i64)),
    ])
}

/// Keys: `state`, `dest`, `session`, `attempt`, `max_attempts`, `retry_in_ms`, `primary_pane`,
/// `panes` [{`pane`,`tmux_pane`}], `stale_panes`, `last_exit` {...} or null.
pub fn link_summary_json(s: &LinkSummary) -> JsonValue {
    JsonValue::obj([
        ("state", JsonValue::String(s.state.to_string())),
        ("dest", opt_str(&s.dest)),
        ("session", opt_str(&s.session)),
        ("attempt", JsonValue::Number(i64::from(s.attempt))),
        ("max_attempts", JsonValue::Number(i64::from(s.max_attempts))),
        ("retry_in_ms", s.retry_in_ms.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64))),
        ("primary_pane", s.primary_pane.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64))),
        (
            "panes",
            JsonValue::Array(
                s.panes
                    .iter()
                    .map(|(p, t)| {
                        JsonValue::obj([
                            ("pane", JsonValue::Number(*p as i64)),
                            ("tmux_pane", JsonValue::Number(*t as i64)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "stale_panes",
            JsonValue::Array(s.stale_panes.iter().map(|p| JsonValue::Number(*p as i64)).collect()),
        ),
        ("last_exit", s.last_exit.as_ref().map_or(JsonValue::Null, exit_json)),
    ])
}
