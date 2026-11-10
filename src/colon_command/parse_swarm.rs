//! Parser for `:swarm load|status|run|spawn|verify|diff|merge|clean` (Phase 69 wiring).

use super::ColonCommand;
use crate::action::Action;
use crate::wm::swarm_cmd::SwarmOp;

pub const SWARM_USAGE: &str = "Usage: :swarm load <plan.toml> | status | run | spawn <slug> [agent] [prompt] | verify <node> | diff <slug> | merge <slug> [--squash] | clean <slug> [--force]";

pub(super) fn parse(args: &[&str]) -> Result<ColonCommand, String> {
    let bad = || SWARM_USAGE.to_string();
    let flag = |rest: &[&str], f: &str| rest.contains(&f);
    let op = match args {
        [] | ["status"] => SwarmOp::Status,
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
