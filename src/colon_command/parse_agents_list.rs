//! Parser for `:agents list <verb>` (Phase 51-01, 51-CONTRACT.md section 3).

use super::ColonCommand;
use crate::wm::agents_list_cmd::AgentsListOp;

pub(super) const AGENTS_LIST_USAGE: &str = "Usage: :agents list [open|close|toggle|dismiss|blur]";

/// Parses the words after `:agents list`; anything unknown is the usage error (never a panic).
pub(super) fn parse(args: &[&str]) -> Result<ColonCommand, String> {
    let op = match args {
        ["open"] => AgentsListOp::Open,
        ["close"] => AgentsListOp::Close,
        ["toggle"] => AgentsListOp::Toggle,
        ["dismiss"] => AgentsListOp::Dismiss,
        ["blur"] => AgentsListOp::Blur,
        _ => return Err(AGENTS_LIST_USAGE.to_string()),
    };
    Ok(ColonCommand::AgentsList(op))
}
