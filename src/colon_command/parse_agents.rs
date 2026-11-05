//! Parser for the `:agents` view command (Phase 50-03).

use super::ColonCommand;
use crate::wm::agents_cmd::{AgentsOp, SnoopFilterKind, SnoopFilterOp, TopologyOp};
use crate::config::agents::EDGE_FADE_RANGE;
use crate::wm::agents_window::{GraphDir, NodeKey, WindowPane};

const USAGE: &str = "Usage: :agents [list|zoom [pane-id]|toggle [pane-id]|demo|close|dismiss|blur|select <left|right|up|down|node-key>|edge <next|prev|a b>|enter|move <dx> <dy>|resize <dw> <dh>|rect <x> <y> <w> <h>|size <cols>x<rows>|auto|max|split <20..80|+n|-n>|grab [on|off]|nudge <dir>|node-pin <key> <x> <y>|node-unpin [key]|pin [toggle]|unpin|scale <75..200|+n|-n>|focus [graph|snoop|compose]|line <+n|-n|seq>|copy|scroll <±n|top>|follow [on|off]|fade [secs]|topology [export <name>]|filter <agent [label]|pair [a b]|text kw|clear|remove agent|pair|text>]";

const FADE_USAGE: &str = "Usage: :agents fade [1..3600] (seconds for an edge to fade to its floor)";
const TOPOLOGY_USAGE: &str = "Usage: :agents topology [export <name>]";
const SCALE_USAGE: &str = "Usage: :agents scale <75..200> | +n | -n (GUI only)";

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
        ("scale", 1) if args[0].starts_with(['+', '-']) => match args[0].parse::<i16>() {
            Ok(n) => Some(Ok(AgentsOp::ScaleStep(n))),
            Err(_) => Some(Err(SCALE_USAGE.to_string())),
        },
        ("pin", 0) => Some(Ok(AgentsOp::Pin(Some(true)))),
        ("pin", 1) if args[0] == "toggle" => Some(Ok(AgentsOp::Pin(None))),
        ("unpin", 0) => Some(Ok(AgentsOp::Pin(Some(false)))),
        ("node-unpin", 0) => Some(Ok(AgentsOp::UnpinNode(None))),
        ("node-unpin", 1) => match node_key(args[0]) {
            Some(key) => Some(Ok(AgentsOp::UnpinNode(Some(key)))),
            None => usage("node-unpin [node-key]"),
        },
        ("scale", _) => Some(Err(SCALE_USAGE.to_string())),
        ("move" | "resize" | "rect" | "size" | "max" | "split" | "grab" | "nudge" | "node-pin" | "node-unpin" | "pin" | "unpin", _) => {
            usage(&format!("{verb} ... (see :agents)"))
        }
        _ => None,
    }
}

/// Conversation (snoop) pane verbs (Phase 50-10).
fn parse_snoop_verb(verb: &str, args: &[&str]) -> Option<Result<AgentsOp, String>> {
    let usage = |u: &str| Some(Err(format!("Usage: :agents {u}")));
    match (verb, args.len()) {
        ("focus", 0) => Some(Ok(AgentsOp::FocusPane(None))),
        ("focus", 1) => match args[0] {
            "graph" => Some(Ok(AgentsOp::FocusPane(Some(WindowPane::Graph)))),
            "snoop" => Some(Ok(AgentsOp::FocusPane(Some(WindowPane::Snoop)))),
            "compose" => Some(Ok(AgentsOp::FocusPane(Some(WindowPane::Compose)))),
            _ => usage("focus [graph|snoop|compose]"),
        },
        ("line", 1) if args[0].starts_with(['+', '-']) => match args[0].parse::<i16>() {
            Ok(n) => Some(Ok(AgentsOp::SnoopStep(n))),
            Err(_) => usage("line <+n|-n|seq>"),
        },
        ("line", 1) => match args[0].parse::<u64>() {
            Ok(seq) => Some(Ok(AgentsOp::SnoopSelect(seq))),
            Err(_) => usage("line <+n|-n|seq>"),
        },
        ("copy", 0) => Some(Ok(AgentsOp::Copy)),
        ("scroll", 1) if args[0] == "top" => Some(Ok(AgentsOp::SnoopScroll(i16::MIN))),
        ("scroll", 1) => match args[0].parse::<i16>() {
            Ok(n) if n != i16::MIN => Some(Ok(AgentsOp::SnoopScroll(n))),
            _ => usage("scroll <±n|top>"),
        },
        ("follow", 0) => Some(Ok(AgentsOp::Follow(None))),
        ("follow", 1) if matches!(args[0], "on" | "off") => Some(Ok(AgentsOp::Follow(Some(args[0] == "on")))),
        ("filter", _) => Some(parse_filter(args).map(AgentsOp::Filter)),
        ("focus", _) => usage("focus [graph|snoop|compose]"),
        ("line", _) => usage("line <+n|-n|seq>"),
        ("copy", _) => usage("copy"),
        ("scroll", _) => usage("scroll <±n|top>"),
        ("follow", _) => usage("follow [on|off]"),
        _ => None,
    }
}

const FILTER_USAGE: &str =
    "Usage: :agents filter agent [label] | pair [a b] | text <keyword> | clear | remove <agent|pair|text>";

/// `:agents filter ...` arguments.
fn parse_filter(args: &[&str]) -> Result<SnoopFilterOp, String> {
    let usage = || Err(FILTER_USAGE.to_string());
    match (args.first().copied(), args.len()) {
        (Some("agent"), 1) => Ok(SnoopFilterOp::AgentFromSelection),
        (Some("agent"), _) => Ok(SnoopFilterOp::Agent(args[1..].join(" "))),
        (Some("pair"), 1) => Ok(SnoopFilterOp::PairFromSelection),
        (Some("pair"), 3) => Ok(SnoopFilterOp::Pair(args[1].to_string(), args[2].to_string())),
        (Some("text"), n) if n >= 2 => Ok(SnoopFilterOp::Text(args[1..].join(" "))),
        (Some("clear"), 1) => Ok(SnoopFilterOp::Clear),
        (Some("remove"), 2) => match args[1] {
            "agent" => Ok(SnoopFilterOp::Remove(SnoopFilterKind::Agent)),
            "pair" => Ok(SnoopFilterOp::Remove(SnoopFilterKind::Pair)),
            "text" => Ok(SnoopFilterOp::Remove(SnoopFilterKind::Text)),
            _ => usage(),
        },
        _ => usage(),
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
        Some("toggle") if parts.len() <= 3 => match parts.get(2) {
            None => Ok(ColonCommand::Agents(AgentsOp::Toggle(None))),
            Some(raw) => match raw.trim_start_matches('#').parse::<crate::pane::PaneId>() {
                Ok(id) => Ok(ColonCommand::Agents(AgentsOp::Toggle(Some(id)))),
                Err(_) => Err("Usage: :agents toggle [pane-id]".to_string()),
            },
        },
        Some("demo") if parts.len() == 2 => Ok(ColonCommand::Agents(AgentsOp::Demo)),
        Some("close") if parts.len() == 2 => Ok(ColonCommand::Agents(AgentsOp::Close)),
        Some("dismiss") if parts.len() == 2 => Ok(ColonCommand::Agents(AgentsOp::Dismiss)),
        Some("blur") if parts.len() == 2 => Ok(ColonCommand::Agents(AgentsOp::Blur)),
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
        Some("fade") if parts.len() <= 3 => match parts.get(2).map(|raw| raw.parse::<u32>()) {
            None => Ok(ColonCommand::Agents(AgentsOp::Fade(None))),
            Some(Ok(n)) if EDGE_FADE_RANGE.contains(&n) => Ok(ColonCommand::Agents(AgentsOp::Fade(Some(n)))),
            _ => Err(FADE_USAGE.to_string()),
        },
        Some("topology") if parts.len() == 2 => Ok(ColonCommand::Agents(AgentsOp::Topology(TopologyOp::Show))),
        Some("topology") if parts.len() == 4 && parts[2] == "export" => {
            Ok(ColonCommand::Agents(AgentsOp::Topology(TopologyOp::Export(parts[3].to_string()))))
        }
        Some("topology") => Err(TOPOLOGY_USAGE.to_string()),
        // Absolute scale reuses the shared `PanelScaleOf` Action (`+n` / `-n` are `ScaleStep`).
        Some("scale") if parts.len() == 3 && !parts[2].starts_with(['+', '-']) => {
            match parts[2].trim_end_matches('%').parse::<u16>() {
                Ok(n) if (75..=200).contains(&n) => {
                    Ok(ColonCommand::PanelScaleOf(crate::ui::typography::PanelTarget::Agents, n))
                }
                _ => Err(SCALE_USAGE.to_string()),
            }
        }
        Some(verb) => match parse_snoop_verb(verb, &parts[2..]).or_else(|| parse_window_verb(verb, &parts[2..])) {
            Some(res) => res.map(ColonCommand::Agents),
            None => Err(USAGE.to_string()),
        },
        None => Err(USAGE.to_string()),
    })
}
