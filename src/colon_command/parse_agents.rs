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
        _ => Err("Usage: :agents [list]".to_string()),
    })
}
