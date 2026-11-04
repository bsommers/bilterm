//! Parser for the `:agents` view command (Phase 50-03).

use super::ColonCommand;
use crate::wm::agents_cmd::AgentsOp;
use crate::wm::agents_window::GraphDir;

const USAGE: &str =
    "Usage: :agents [list|zoom [pane-id]|close|dismiss|select <left|right|up|down>|edge <next|prev>|enter]";

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
        Some("select") if parts.len() == 3 => {
            let dir = match parts[2] {
                "left" => GraphDir::Left,
                "right" => GraphDir::Right,
                "up" => GraphDir::Up,
                "down" => GraphDir::Down,
                _ => return Some(Err(USAGE.to_string())),
            };
            Ok(ColonCommand::Agents(AgentsOp::Select(dir)))
        }
        Some("edge") if parts.len() == 3 && matches!(parts[2], "next" | "prev") => {
            Ok(ColonCommand::Agents(AgentsOp::CycleEdge(parts[2] == "next")))
        }
        Some("enter") if parts.len() == 2 => Ok(ColonCommand::Agents(AgentsOp::Activate)),
        _ => Err(USAGE.to_string()),
    })
}
