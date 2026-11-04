//! Parser for the `:agents` view command (Phase 50-03).

use super::ColonCommand;
use crate::wm::agents_cmd::AgentsOp;

/// Returns `Some` only for head `agents`; everything else falls through unchanged.
pub(super) fn parse_agents_colon_command(
    head: &str,
    parts: &[&str],
) -> Option<Result<ColonCommand, String>> {
    if head != "agents" {
        return None;
    }
    Some(match parts.get(1).copied() {
        None | Some("list" | "ls" | "status") if parts.len() <= 2 => {
            Ok(ColonCommand::Agents(AgentsOp::List))
        }
        Some("zoom") if parts.len() <= 3 => match parts.get(2) {
            None => Ok(ColonCommand::Agents(AgentsOp::Zoom(None))),
            Some(raw) => match raw.trim_start_matches('#').parse::<crate::pane::PaneId>() {
                Ok(id) => Ok(ColonCommand::Agents(AgentsOp::Zoom(Some(id)))),
                Err(_) => Err("Usage: :agents zoom [pane-id]".to_string()),
            },
        },
        Some("close") if parts.len() == 2 => Ok(ColonCommand::Agents(AgentsOp::Close)),
        Some("dismiss") if parts.len() == 2 => Ok(ColonCommand::Agents(AgentsOp::Dismiss)),
        _ => Err("Usage: :agents [list|zoom [pane-id]|close|dismiss]".to_string()),
    })
}
