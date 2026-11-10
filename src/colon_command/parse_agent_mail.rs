//! Parser for `:agent` colon commands (Phase 52, `AGT-01..03`).

use super::ColonCommand;
use crate::action::Action;
use crate::wm::agent_mail::{AgentDeliveryMode, AgentMailOp};

pub(super) fn parse_agent_colon_command(
    head: &str,
    parts: &[&str],
    rest: &str,
) -> Option<Result<ColonCommand, String>> {
    if head != "agent" {
        return None;
    }

    let sub = match parts.get(1) {
        Some(s) => *s,
        None => {
            return Some(Err(
                "Usage: :agent <name|send|broadcast|inbox|ack|list|enable>".to_string(),
            ));
        }
    };

    match sub {
        "replay" => Some(super::parse_agents_hud::parse_replay(&parts[2..])),
        "name" => {
            // :agent name [pane_id] <name>
            let tail = &parts[2..];
            if tail.is_empty() {
                return Some(Err("Usage: :agent name [pane_id] <name>".to_string()));
            }
            if tail.len() == 1 {
                Some(Ok(Action::AgentMail(AgentMailOp::SetName {
                    pane_id: None,
                    name: tail[0].to_string(),
                })))
            } else if let Ok(pid) = tail[0].parse::<usize>() {
                Some(Ok(Action::AgentMail(AgentMailOp::SetName {
                    pane_id: Some(pid),
                    name: tail[1].to_string(),
                })))
            } else {
                Some(Err("Usage: :agent name [pane_id] <name>".to_string()))
            }
        }
        "send" => {
            // :agent send [--nudge|--file] <to> <body...>
            let mut delivery = AgentDeliveryMode::Mailbox;
            let mut idx = 2;
            if let Some(&flag) = parts.get(idx) {
                if flag == "--nudge" {
                    delivery = AgentDeliveryMode::Nudge;
                    idx += 1;
                } else if flag == "--file" {
                    delivery = AgentDeliveryMode::File;
                    idx += 1;
                }
            }
            let to = match parts.get(idx) {
                Some(t) => t.to_string(),
                None => {
                    return Some(Err(
                        "Usage: :agent send [--nudge|--file] <to> <body...>".to_string(),
                    ));
                }
            };
            idx += 1;
            // The body is whatever follows in rest
            // To be resilient to multiple spaces or flags, slice rest after `to`
            let body = if let Some(pos) = rest.find(&to) {
                let after = &rest[pos + to.len()..];
                after.trim_start().to_string()
            } else {
                parts[idx..].join(" ")
            };
            if body.is_empty() {
                return Some(Err(
                    "Usage: :agent send [--nudge|--file] <to> <body...>".to_string(),
                ));
            }
            Some(Ok(Action::AgentMail(AgentMailOp::Send {
                from: None,
                to,
                body,
                kind: "message".to_string(),
                delivery,
                reply_to: None,
            })))
        }
        "broadcast" => {
            // :agent broadcast [--tree <subtree>] <body...>
            let mut subtree = None;
            let mut idx = 2;
            if let Some(&flag) = parts.get(idx)
                && flag == "--tree"
            {
                idx += 1;
                if let Some(&tree) = parts.get(idx) {
                    subtree = Some(tree.to_string());
                    idx += 1;
                } else {
                    return Some(Err(
                        "Usage: :agent broadcast [--tree <subtree>] <body...>".to_string(),
                    ));
                }
            }
            let body = parts[idx..].join(" ");
            if body.is_empty() {
                return Some(Err(
                    "Usage: :agent broadcast [--tree <subtree>] <body...>".to_string(),
                ));
            }
            Some(Ok(Action::AgentMail(AgentMailOp::Broadcast {
                from: None,
                subtree,
                body,
                kind: "broadcast".to_string(),
            })))
        }
        "inbox" => {
            // :agent inbox [name] [--unread]
            let mut agent = None;
            let mut unread_only = false;
            for &p in &parts[2..] {
                if p == "--unread" {
                    unread_only = true;
                } else if agent.is_none() {
                    agent = Some(p.to_string());
                }
            }
            Some(Ok(Action::AgentMail(AgentMailOp::Inbox {
                agent,
                unread_only,
            })))
        }
        "ack" => {
            // :agent ack [name] [id|all]
            let tail = &parts[2..];
            let (agent, message_id) = match tail.len() {
                0 => (None, None),
                1 => {
                    if tail[0] == "all" {
                        (None, None)
                    } else if let Ok(id) = tail[0].parse::<u64>() {
                        (None, Some(id))
                    } else {
                        (Some(tail[0].to_string()), None)
                    }
                }
                _ => {
                    let ag = Some(tail[0].to_string());
                    if tail[1] == "all" {
                        (ag, None)
                    } else if let Ok(id) = tail[1].parse::<u64>() {
                        (ag, Some(id))
                    } else {
                        return Some(Err("Usage: :agent ack [name] [id|all]".to_string()));
                    }
                }
            };
            Some(Ok(Action::AgentMail(AgentMailOp::Ack {
                agent,
                message_id,
            })))
        }
        "list" => Some(Ok(Action::AgentMail(AgentMailOp::List))),
        "enable" => {
            // :agent enable [on|off]
            let opt = match parts.get(2).copied() {
                None => None,
                Some("on" | "true" | "1") => Some(true),
                Some("off" | "false" | "0") => Some(false),
                Some(_) => return Some(Err("Usage: :agent enable [on|off]".to_string())),
            };
            Some(Ok(Action::AgentMail(AgentMailOp::Enable(opt))))
        }
        _ => Some(Err(
            "Usage: :agent <name|send|broadcast|inbox|ack|list|enable>".to_string(),
        )),
    }
}
