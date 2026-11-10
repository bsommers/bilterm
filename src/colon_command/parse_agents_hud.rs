//! Parsers for the Phase 70 forms: `:agents hud ...`, `:agent replay ...`, `:agents watchdog`.
//!
//! Every non-matching form is the usage `Err`; numbers use checked `parse` calls only.

use super::ColonCommand;
use crate::pane::PaneId;
use crate::wm::agents_hud_cmd::{HudOp, ReplayOp, WatchdogOp};
use crate::wm::mission_control_hud::HudButtonKind;

pub(super) const HUD_USAGE: &str = "Usage: :agents hud [open|close|toggle|select <next|prev|pane-id>|page <next|prev>|approve [pane]|prompt [pane]|diff [pane]|zoom [pane]]";
pub(super) const REPLAY_USAGE: &str =
    "Usage: :agent replay [show]|<pane-id|close|prev|next|first|last|turn <id>|up [n]|down [n]>";
pub(super) const WATCHDOG_USAGE: &str = "Usage: :agents watchdog [status]";

fn pane_id(raw: &str) -> Option<PaneId> {
    raw.strip_prefix('#').unwrap_or(raw).parse::<PaneId>().ok()
}

fn press(kind: HudButtonKind, args: &[&str]) -> Result<ColonCommand, String> {
    let pane = match args {
        [] => None,
        [id] => Some(pane_id(id).ok_or(HUD_USAGE)?),
        _ => return Err(HUD_USAGE.to_string()),
    };
    Ok(ColonCommand::AgentsHud(HudOp::Press {
        pane,
        kind,
        generation: None,
    }))
}

/// Parses the words after `:agents hud`.
pub(super) fn parse_hud(args: &[&str]) -> Result<ColonCommand, String> {
    let op = match args {
        [] | ["open"] => HudOp::Open,
        ["close"] => HudOp::Close,
        ["toggle"] => HudOp::Toggle,
        ["select", "next"] => HudOp::Step(1),
        ["select", "prev"] => HudOp::Step(-1),
        ["select", id] => HudOp::SelectPane(pane_id(id).ok_or(HUD_USAGE)?),
        ["page", "next"] => HudOp::Page(1),
        ["page", "prev"] => HudOp::Page(-1),
        ["approve", rest @ ..] => return press(HudButtonKind::Approve, rest),
        ["prompt", rest @ ..] => return press(HudButtonKind::Prompt, rest),
        ["diff", rest @ ..] => return press(HudButtonKind::Diff, rest),
        ["zoom", rest @ ..] => return press(HudButtonKind::Zoom, rest),
        _ => return Err(HUD_USAGE.to_string()),
    };
    Ok(ColonCommand::AgentsHud(op))
}

/// Parses the words after `:agents watchdog`.
pub(super) fn parse_watchdog(args: &[&str]) -> Result<ColonCommand, String> {
    match args {
        [] | ["status"] => Ok(ColonCommand::AgentsWatchdog(WatchdogOp::Status)),
        _ => Err(WATCHDOG_USAGE.to_string()),
    }
}

/// Parses the words after `:agent replay`.
pub(super) fn parse_replay(args: &[&str]) -> Result<ColonCommand, String> {
    let scroll = |sign: i16, n: Option<&&str>| -> Result<ColonCommand, String> {
        let k = match n {
            None => 1,
            Some(raw) => raw
                .parse::<i16>()
                .ok()
                .filter(|k| (1..=1000).contains(k))
                .ok_or(REPLAY_USAGE)?,
        };
        Ok(ColonCommand::AgentsReplay(ReplayOp::Scroll(sign * k)))
    };
    let op = match args {
        [] | ["show"] => ReplayOp::Show,
        ["close"] => ReplayOp::Close,
        ["prev"] => ReplayOp::Prev,
        ["next"] => ReplayOp::Next,
        ["first"] => ReplayOp::First,
        ["last"] => ReplayOp::Last,
        ["turn", id] => ReplayOp::Goto(id.parse::<u64>().map_err(|_| REPLAY_USAGE.to_string())?),
        ["up", rest @ ..] if rest.len() <= 1 => return scroll(-1, rest.first()),
        ["down", rest @ ..] if rest.len() <= 1 => return scroll(1, rest.first()),
        [id] => ReplayOp::Open(pane_id(id).ok_or(REPLAY_USAGE)?),
        _ => return Err(REPLAY_USAGE.to_string()),
    };
    Ok(ColonCommand::AgentsReplay(op))
}
