//! Parser for `:agent queue [list|add <text>|clear|drop <n>]` and `:agent approve|deny`
//! (UNI-R01, Phase 68). Extensible: future `:agent replay` / `:swarm` verbs slot into [`parse_agent_ops`].

use super::ColonCommand;
use crate::action::Action;
use crate::wm::context_queue_ops::{AgentGateOp, AgentQueueOp};

pub const AGENT_QUEUE_USAGE: &str = "Usage: :agent queue [list|add <text>|clear|drop <n>]";

/// Handles the Phase 68 `:agent` subverbs; `None` leaves everything else to the mail parser.
pub(super) fn parse_agent_ops(head: &str, parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    if head != "agent" {
        return None;
    }
    match parts.get(1).copied()? {
        "queue" | "q" => Some(parse_queue(&parts[2..])),
        "approve" if parts.len() == 2 => Some(Ok(Action::AgentGate(AgentGateOp::Approve))),
        "deny" if parts.len() == 2 => Some(Ok(Action::AgentGate(AgentGateOp::Deny))),
        "approve" | "deny" => Some(Err("Usage: :agent approve|deny".to_string())),
        _ => None,
    }
}

fn parse_queue(args: &[&str]) -> Result<ColonCommand, String> {
    let op = match args {
        [] | ["list" | "ls"] => AgentQueueOp::List,
        ["clear"] => AgentQueueOp::Clear,
        // Bare `add` / `drop` (palette prompt entries) parse; the executor reports the usage.
        ["add", text @ ..] => AgentQueueOp::Add(text.join(" ")),
        ["drop" | "remove"] => AgentQueueOp::Drop(0),
        ["drop" | "remove", n] => match n.parse::<usize>() {
            Ok(n) => AgentQueueOp::Drop(n),
            _ => return Err("agent queue drop: <n> must be a number >= 1".to_string()),
        },
        _ => return Err(AGENT_QUEUE_USAGE.to_string()),
    };
    Ok(Action::AgentQueue(op))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colon_command::parse_colon_command;

    fn p(s: &str) -> Option<Result<Action, String>> {
        parse_colon_command(s)
    }

    #[test]
    fn parses_queue_verbs() {
        assert_eq!(p(":agent queue"), Some(Ok(Action::AgentQueue(AgentQueueOp::List))));
        assert_eq!(p(":agent queue list"), Some(Ok(Action::AgentQueue(AgentQueueOp::List))));
        assert_eq!(p(":agent queue clear"), Some(Ok(Action::AgentQueue(AgentQueueOp::Clear))));
        assert_eq!(p(":agent queue drop 2"), Some(Ok(Action::AgentQueue(AgentQueueOp::Drop(2)))));
        assert_eq!(
            p(":agent queue add fix the build"),
            Some(Ok(Action::AgentQueue(AgentQueueOp::Add("fix the build".into()))))
        );
    }

    #[test]
    fn parses_gate_and_rejects_bad() {
        assert_eq!(p(":agent approve"), Some(Ok(Action::AgentGate(AgentGateOp::Approve))));
        assert_eq!(p(":agent deny"), Some(Ok(Action::AgentGate(AgentGateOp::Deny))));
        assert!(p(":agent queue drop x").unwrap().is_err());
        assert_eq!(p(":agent queue add"), Some(Ok(Action::AgentQueue(AgentQueueOp::Add(String::new())))));
        assert!(p(":agent approve now").unwrap().is_err());
    }

    #[test]
    fn mail_verbs_still_route() {
        assert_eq!(p(":agent list"), Some(Ok(Action::AgentMail(crate::wm::agent_mail::AgentMailOp::List))));
    }
}
