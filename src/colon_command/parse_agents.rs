//! Parser for the `:agents` view command (Phase 50-03).

use super::ColonCommand;
use crate::wm::agents_cmd::AgentsOp;
use crate::wm::agents_window::{GraphDir, NodeKey};

const USAGE: &str = "Usage: :agents [list|zoom [pane-id]|close|dismiss|select <left|right|up|down|node-key>|edge <next|prev|a b>|enter|move <dx> <dy>|resize <dw> <dh>|rect <x> <y> <w> <h>|size <cols>x<rows>|auto|max|split <20..80|+n|-n>|grab [on|off]|nudge <dir>|node-pin <key> <x> <y>|node-unpin [key]|pin [toggle]|unpin]";

fn graph_dir(raw: &str) -> Option<GraphDir> {
    match raw {
        "left" => Some(GraphDir::Left),
        "right" => Some(GraphDir::Right),
        "up" => Some(GraphDir::Up),
        "down" => Some(GraphDir::Down),
        _ => None,
    }
}

/// `user`, `api`, `mcp`, `pane:<id>`, `session:<name>` or `agent:<name>`.
fn node_key(raw: &str) -> Option<NodeKey> {
    let ok = match raw {
        "user" | "api" | "mcp" => true,
        _ => match raw.split_once(':') {
            Some(("pane", id)) => id.parse::<crate::pane::PaneId>().is_ok(),
            Some(("session" | "agent", name)) => !name.is_empty(),
            _ => false,
        },
    };
    ok.then(|| NodeKey(raw.to_string()))
}

fn nums<T: std::str::FromStr>(parts: &[&str]) -> Option<Vec<T>> {
    parts.iter().map(|p| p.parse::<T>().ok()).collect()
}

/// `size` argument: `auto` or `<cols>x<rows>` (cols 20..=400, rows 6..=200).
fn parse_size(raw: &str) -> Option<Option<(u16, u16)>> {
    if raw == "auto" {
        return Some(None);
    }
    let (c, r) = raw.split_once('x')?;
    let (c, r) = (c.parse::<u16>().ok()?, r.parse::<u16>().ok()?);
    ((20..=400).contains(&c) && (6..=200).contains(&r)).then_some(Some((c, r)))
}

/// Window geometry, splitter and node verbs (Phase 50-09).
fn parse_window_verb(verb: &str, args: &[&str]) -> Option<Result<AgentsOp, String>> {
    let usage = |u: &str| Some(Err(format!("Usage: :agents {u}")));
    match (verb, args.len()) {
        ("move", 2) => match nums::<i16>(args) {
            Some(v) => Some(Ok(AgentsOp::Move { dx: v[0], dy: v[1] })),
            None => usage("move <dx> <dy>"),
        },
        ("resize", 2) => match nums::<i16>(args) {
            Some(v) => Some(Ok(AgentsOp::Resize { dw: v[0], dh: v[1] })),
            None => usage("resize <dw> <dh>"),
        },
        ("rect", 4) => match nums::<u16>(args) {
            Some(v) => Some(Ok(AgentsOp::SetRect { x: v[0], y: v[1], w: v[2], h: v[3] })),
            None => usage("rect <x> <y> <w> <h>"),
        },
        ("size", 1) => match parse_size(args[0]) {
            Some(size) => Some(Ok(AgentsOp::Size(size))),
            None => usage("size <cols>x<rows> | auto (cols 20..400, rows 6..200)"),
        },
        ("max", 0) => Some(Ok(AgentsOp::Maximize)),
        ("split", 1) => {
            let raw = args[0];
            if raw.starts_with(['+', '-']) {
                match raw.parse::<i16>() {
                    Ok(n) => Some(Ok(AgentsOp::SplitStep(n))),
                    Err(_) => usage("split <20..80 | +n | -n>"),
                }
            } else {
                match raw.parse::<u16>() {
                    Ok(n) if (20..=80).contains(&n) => Some(Ok(AgentsOp::Split(n))),
                    _ => usage("split <20..80 | +n | -n>"),
                }
            }
        }
        ("grab", 0) => Some(Ok(AgentsOp::Grab(None))),
        ("grab", 1) if matches!(args[0], "on" | "off") => Some(Ok(AgentsOp::Grab(Some(args[0] == "on")))),
        ("nudge", 1) => match graph_dir(args[0]) {
            Some(d) => Some(Ok(AgentsOp::NudgeNode(d))),
            None => usage("nudge <left|right|up|down>"),
        },
        ("node-pin", 3) => {
            let key = node_key(args[0]);
            match (key, nums::<u16>(&args[1..])) {
                (Some(key), Some(v)) if v.iter().all(|n| *n <= 1000) => {
                    Some(Ok(AgentsOp::PinNode { key, x_pm: v[0], y_pm: v[1] }))
                }
                _ => usage("node-pin <node-key> <x 0..1000> <y 0..1000>"),
            }
        }
        ("pin", 0) => Some(Ok(AgentsOp::Pin(Some(true)))),
        ("pin", 1) if args[0] == "toggle" => Some(Ok(AgentsOp::Pin(None))),
        ("unpin", 0) => Some(Ok(AgentsOp::Pin(Some(false)))),
        ("node-unpin", 0) => Some(Ok(AgentsOp::UnpinNode(None))),
        ("node-unpin", 1) => match node_key(args[0]) {
            Some(key) => Some(Ok(AgentsOp::UnpinNode(Some(key)))),
            None => usage("node-unpin [node-key]"),
        },
        ("move" | "resize" | "rect" | "size" | "max" | "split" | "grab" | "nudge" | "node-pin" | "node-unpin" | "pin" | "unpin", _) => {
            usage(&format!("{verb} ... (see :agents)"))
        }
        _ => None,
    }
}

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
        Some("select") if parts.len() == 3 => match (graph_dir(parts[2]), node_key(parts[2])) {
            (Some(dir), _) => Ok(ColonCommand::Agents(AgentsOp::Select(dir))),
            (None, Some(key)) => Ok(ColonCommand::Agents(AgentsOp::SelectNode(key))),
            (None, None) => Err(USAGE.to_string()),
        },
        Some("edge") if parts.len() == 3 && matches!(parts[2], "next" | "prev") => {
            Ok(ColonCommand::Agents(AgentsOp::CycleEdge(parts[2] == "next")))
        }
        Some("edge") if parts.len() == 4 => match (node_key(parts[2]), node_key(parts[3])) {
            (Some(a), Some(b)) if a != b => Ok(ColonCommand::Agents(AgentsOp::SelectEdge(a, b))),
            _ => Err("Usage: :agents edge <next|prev|node-key node-key>".to_string()),
        },
        Some("enter") if parts.len() == 2 => Ok(ColonCommand::Agents(AgentsOp::Activate)),
        Some(verb) => match parse_window_verb(verb, &parts[2..]) {
            Some(res) => res.map(ColonCommand::Agents),
            None => Err(USAGE.to_string()),
        },
        None => Err(USAGE.to_string()),
    })
}
