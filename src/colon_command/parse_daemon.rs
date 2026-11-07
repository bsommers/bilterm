//! Parser for `:daemon` colon commands (`DAEM-05..08`, Phase 55).
//!
//! Syntax:
//! - `:daemon` | `:daemon list`
//! - `:daemon status [name]`
//! - `:daemon start [name]`
//! - `:daemon stop [name] [--force]`
//! - `:daemon switch <name>`
//! - `:daemon prune`

use super::ColonCommand;
use crate::action::DaemonSupervisorOp;

const USAGE: &str = "Usage: :daemon [list | status [name] | start [name] | stop [name] [--force] | switch <name> | prune]";

/// Parses `:daemon` command invocations into a [`ColonCommand`].
pub(super) fn parse_daemon_colon_command(
    head: &str,
    parts: &[&str],
) -> Option<Result<ColonCommand, String>> {
    if head != "daemon" {
        return None;
    }

    let sub = parts.get(1).copied().unwrap_or("list");
    match sub {
        "list" | "ls" => {
            if parts.len() > 2 {
                return Some(Err(USAGE.to_string()));
            }
            Some(Ok(ColonCommand::DaemonSupervisor(DaemonSupervisorOp::List)))
        }
        "status" => {
            let name = parts.get(2).map(|s| s.to_string());
            if parts.len() > 3 {
                return Some(Err(USAGE.to_string()));
            }
            Some(Ok(ColonCommand::DaemonSupervisor(
                DaemonSupervisorOp::Status { name },
            )))
        }
        "start" => {
            let name = parts.get(2).map(|s| s.to_string());
            if parts.len() > 3 {
                return Some(Err(USAGE.to_string()));
            }
            Some(Ok(ColonCommand::DaemonSupervisor(
                DaemonSupervisorOp::Start { name },
            )))
        }
        "stop" => {
            let mut name = None;
            let mut force = false;
            for &arg in &parts[2..] {
                if arg == "--force" || arg == "-f" {
                    force = true;
                } else if name.is_none() {
                    name = Some(arg.to_string());
                } else {
                    return Some(Err(USAGE.to_string()));
                }
            }
            Some(Ok(ColonCommand::DaemonSupervisor(
                DaemonSupervisorOp::Stop { name, force },
            )))
        }
        "switch" => {
            let Some(name) = parts.get(2) else {
                return Some(Err(
                    "Usage: :daemon switch <name> (missing daemon name)".to_string(),
                ));
            };
            if parts.len() > 3 {
                return Some(Err(USAGE.to_string()));
            }
            Some(Ok(ColonCommand::DaemonSupervisor(
                DaemonSupervisorOp::Switch {
                    name: (*name).to_string(),
                },
            )))
        }
        "prune" => {
            if parts.len() > 2 {
                return Some(Err(USAGE.to_string()));
            }
            Some(Ok(ColonCommand::DaemonSupervisor(
                DaemonSupervisorOp::Prune,
            )))
        }
        _ => Some(Err(USAGE.to_string())),
    }
}
