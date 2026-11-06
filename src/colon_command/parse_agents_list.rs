//! Parser for `:agents list <verb>` (Phase 51-01, 51-CONTRACT.md section 3).
//!
//! Every non-matching form (unknown verb, wrong argument count, out-of-range number) is the usage
//! `Err`; nothing is partially parsed and no input can panic. Numbers use checked `parse` calls
//! only (no `as` casts on untrusted input).

use super::ColonCommand;
use crate::pane::PaneId;
use crate::ui::typography::PanelTarget;
use crate::wm::agents_list_cmd::{AgentsListOp, ListFilter};

pub(super) const AGENTS_LIST_USAGE: &str = "Usage: :agents list [open|close|toggle|dismiss|blur|select <up|down|first|last|page-up|page-down|pane-id>|jump [pane-id]|zoom [pane-id]|pin [toggle]|unpin|scale <75..200|+n|-n>|filter <all|agents|toggle>|dock [toggle]|float|move <dx> <dy>|resize <dw> <dh>|rect <x> <y> <w> <h>|size <cols>x<rows>|auto|scroll <±n>]";

fn usage<T>() -> Result<T, String> {
    Err(AGENTS_LIST_USAGE.to_string())
}

/// Pane id with an optional `#`; capped at `u32` so absurd ids are a usage error.
fn pane_id(raw: &str) -> Option<PaneId> {
    let n = raw.strip_prefix('#').unwrap_or(raw).parse::<u32>().ok()?;
    PaneId::try_from(n).ok()
}

fn nums<T: std::str::FromStr>(args: &[&str]) -> Option<Vec<T>> {
    args.iter().map(|a| a.parse::<T>().ok()).collect()
}

/// `size` argument: `auto` or `<cols>x<rows>` (cols 20..=400, rows 6..=200, as the traffic window).
fn parse_size(raw: &str) -> Option<Option<(u16, u16)>> {
    if raw == "auto" {
        return Some(None);
    }
    let (c, r) = raw.split_once('x')?;
    let (c, r) = (c.parse::<u16>().ok()?, r.parse::<u16>().ok()?);
    ((20..=400).contains(&c) && (6..=200).contains(&r)).then_some(Some((c, r)))
}

fn parse_select(arg: &str) -> Option<AgentsListOp> {
    Some(match arg {
        "up" => AgentsListOp::Step(-1),
        "down" => AgentsListOp::Step(1),
        "first" => AgentsListOp::First,
        "last" => AgentsListOp::Last,
        "page-up" => AgentsListOp::Page(-1),
        "page-down" => AgentsListOp::Page(1),
        other => AgentsListOp::SelectPane(pane_id(other)?),
    })
}

/// Parses the words after `:agents list`.
pub(super) fn parse(args: &[&str]) -> Result<ColonCommand, String> {
    use AgentsListOp as Op;
    let op = match args {
        ["open"] => Op::Open,
        ["close"] => Op::Close,
        ["toggle"] => Op::Toggle,
        ["dismiss"] => Op::Dismiss,
        ["blur"] => Op::Blur,
        ["select", arg] => match parse_select(arg) {
            Some(op) => op,
            None => return usage(),
        },
        ["jump"] => Op::Jump(None),
        ["jump", id] => Op::Jump(Some(pane_id(id).ok_or(AGENTS_LIST_USAGE)?)),
        ["zoom"] => Op::Zoom(None),
        ["zoom", id] => Op::Zoom(Some(pane_id(id).ok_or(AGENTS_LIST_USAGE)?)),
        ["pin"] => Op::Pin(Some(true)),
        ["unpin"] => Op::Pin(Some(false)),
        ["pin", "toggle"] => Op::Pin(None),
        ["scale", arg] if arg.starts_with(['+', '-']) => match arg.parse::<i16>() {
            Ok(n) => Op::ScaleStep(n),
            Err(_) => return usage(),
        },
        ["scale", arg] => match arg.strip_suffix('%').unwrap_or(arg).parse::<u16>() {
            Ok(n) if (75..=200).contains(&n) => {
                return Ok(ColonCommand::PanelScaleOf(PanelTarget::AgentsList, n));
            }
            _ => return usage(),
        },
        ["filter", "all"] => Op::Filter(Some(ListFilter::All)),
        ["filter", "agents"] => Op::Filter(Some(ListFilter::Agents)),
        ["filter", "toggle"] => Op::Filter(None),
        ["dock"] => Op::Dock(Some(true)),
        ["dock", "toggle"] => Op::Dock(None),
        ["float"] => Op::Dock(Some(false)),
        ["move", a, b] => match nums::<i16>(&[a, b]) {
            Some(v) => Op::Move { dx: v[0], dy: v[1] },
            None => return usage(),
        },
        ["resize", a, b] => match nums::<i16>(&[a, b]) {
            Some(v) => Op::Resize { dw: v[0], dh: v[1] },
            None => return usage(),
        },
        ["rect", a, b, c, d] => match nums::<u16>(&[a, b, c, d]) {
            Some(v) => Op::SetRect { x: v[0], y: v[1], w: v[2], h: v[3] },
            None => return usage(),
        },
        ["size", raw] => match parse_size(raw) {
            Some(size) => Op::Size(size),
            None => return usage(),
        },
        ["scroll", n] => match n.parse::<i16>() {
            Ok(n) => Op::Scroll(n),
            Err(_) => return usage(),
        },
        _ => return usage(),
    };
    Ok(ColonCommand::AgentsList(op))
}
