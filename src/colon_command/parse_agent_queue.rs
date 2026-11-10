//! Parser for `:agent queue [list|add <text>|clear|drop <n>]` and `:agent approve|deny`
//! (UNI-R01, Phase 68). `:swarm` lives here too; `:agents hud` / `:agent replay` are in `parse_agents_hud`. Entry point: [`parse_agent_ops`].

use super::ColonCommand;
use crate::action::Action;
use crate::wm::context_queue_ops::{AgentGateOp, AgentQueueOp};
use crate::wm::swarm_surface::SwarmOp;

pub const AGENT_QUEUE_USAGE: &str = "Usage: :agent queue [list|add <text>|clear|drop <n>]";

/// Handles the Phase 68 `:agent` subverbs; `None` leaves everything else to the mail parser.
pub(super) fn parse_agent_ops(head: &str, parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    match head {
        "agent" => {}
        "swarm" => return Some(parse_swarm(&parts[1..])),
        _ => return None,
    }
    match parts.get(1).copied()? {
        "queue" | "q" => Some(parse_queue(&parts[2..])),
        "approve" if parts.len() == 2 => Some(Ok(Action::AgentGate(AgentGateOp::Approve))),
        "deny" if parts.len() == 2 => Some(Ok(Action::AgentGate(AgentGateOp::Deny))),
        "approve" | "deny" => Some(Err("Usage: :agent approve|deny".to_string())),
        _ => None,
    }
}

const SWARM_USAGE: &str = "Usage: :swarm [status|load <plan.toml>|clear|run|spawn <slug> [agent] [prompt]|verify <node>|diff <slug>|merge <slug> [--squash]|clean <slug> [--force]]";

fn parse_swarm(args: &[&str]) -> Result<ColonCommand, String> {
    let bad = || SWARM_USAGE.to_string();
    let flag = |rest: &[&str], f: &str| rest.contains(&f);
    let op = match args {
        [] | ["status"] => SwarmOp::Status,
        ["clear"] => SwarmOp::Clear,
        ["run"] => SwarmOp::Run,
        // Bare verbs (palette prompt entries) parse; the empty argument errors at execute time.
        ["load", path @ ..] => SwarmOp::Load(path.join(" ")),
        ["verify", rest @ ..] if rest.len() <= 1 => SwarmOp::Verify(rest.first().copied().unwrap_or("").to_string()),
        ["diff", rest @ ..] if rest.len() <= 1 => SwarmOp::Diff(rest.first().copied().unwrap_or("").to_string()),
        ["spawn", rest @ ..] => SwarmOp::Spawn {
            slug: rest.first().copied().unwrap_or("").to_string(),
            agent: rest.get(1).map_or("claude", |a| *a).to_string(),
            prompt: rest.iter().skip(2).copied().collect::<Vec<_>>().join(" "),
        },
        ["merge", rest @ ..] if rest.iter().filter(|a| !a.starts_with("--")).count() <= 1 && rest.iter().all(|a| !a.starts_with("--") || *a == "--squash") => {
            SwarmOp::Merge { slug: rest.iter().find(|a| !a.starts_with("--")).copied().unwrap_or("").to_string(), squash: flag(rest, "--squash") }
        }
        ["clean", rest @ ..] if rest.iter().filter(|a| !a.starts_with("--")).count() <= 1 && rest.iter().all(|a| !a.starts_with("--") || *a == "--force") => {
            SwarmOp::Clean { slug: rest.iter().find(|a| !a.starts_with("--")).copied().unwrap_or("").to_string(), force: flag(rest, "--force") }
        }
        _ => return Err(bad()),
    };
    Ok(Action::Swarm(op))
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
    use crate::wm::agents_hud_cmd::{HudOp, ReplayOp};

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
    fn parses_swarm_hud_replay() {
        assert_eq!(p(":swarm"), Some(Ok(Action::Swarm(SwarmOp::Status))));
        assert_eq!(p(":swarm load a b.toml"), Some(Ok(Action::Swarm(SwarmOp::Load("a b.toml".into())))));
        assert!(p(":swarm bogus").unwrap().is_err());
        assert_eq!(p(":agents hud open"), Some(Ok(Action::AgentsHud(HudOp::Open))));
        assert_eq!(p(":agents hud close"), Some(Ok(Action::AgentsHud(HudOp::Close))));
        assert_eq!(p(":agents hud toggle"), Some(Ok(Action::AgentsHud(HudOp::Toggle))));
        assert_eq!(p(":agent replay show"), Some(Ok(Action::AgentsReplay(ReplayOp::Show))));
        for (w, op) in [("prev", ReplayOp::Prev), ("next", ReplayOp::Next), ("first", ReplayOp::First), ("last", ReplayOp::Last)] {
            assert_eq!(p(&format!(":agent replay {w}")), Some(Ok(Action::AgentsReplay(op))));
        }
        assert!(p(":agent replay x").unwrap().is_err());
    }

    #[test]
    fn mail_verbs_still_route() {
        assert_eq!(p(":agent list"), Some(Ok(Action::AgentMail(crate::wm::agent_mail::AgentMailOp::List))));
    }
}
