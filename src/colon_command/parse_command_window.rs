//! Parser for `:commands <verb>` and `:palette <verb>` (WIN-02, Phase 51.2-02).

use super::ColonCommand;
use crate::wm::command_window::CommandWindowOp;

pub const COMMANDS_USAGE: &str = "Usage: :commands [open|close|toggle|dismiss|blur|pin [toggle]|unpin|dock [toggle]|float|clear|query <query>|run [command]|move <dx> <dy>|resize <dw> <dh>|rect <x> <y> <w> <h>]";

fn usage<T>() -> Result<T, String> {
    Err(COMMANDS_USAGE.to_string())
}

/// Parses the arguments after `:commands` or `:palette`.
pub(super) fn parse(args: &[&str]) -> Result<ColonCommand, String> {
    use CommandWindowOp as Op;
    let op = match args {
        [] | ["toggle"] => Op::Toggle,
        ["open"] => Op::Open,
        ["close"] => Op::Close,
        ["dismiss"] => Op::Dismiss,
        ["blur"] => Op::Blur,
        ["pin"] => Op::Pin(Some(true)),
        ["unpin"] => Op::Pin(Some(false)),
        ["pin", "toggle"] => Op::Pin(None),
        ["dock"] => Op::Dock(Some(true)),
        ["float"] => Op::Dock(Some(false)),
        ["dock", "toggle"] => Op::Dock(None),
        ["clear"] => Op::ClearLog,
        ["run"] => Op::ExecuteSelected,
        ["run", rest @ ..] if !rest.is_empty() => Op::ExecuteCommand(rest.join(" ")),
        ["query"] => Op::QueryClear,
        ["query", rest @ ..] if !rest.is_empty() => Op::SetQuery(rest.join(" ")),
        ["move", dx, dy] => {
            let dx = dx.parse::<i16>().map_err(|_| COMMANDS_USAGE.to_string())?;
            let dy = dy.parse::<i16>().map_err(|_| COMMANDS_USAGE.to_string())?;
            Op::Move { dx, dy }
        }
        ["resize", dw, dh] => {
            let dw = dw.parse::<i16>().map_err(|_| COMMANDS_USAGE.to_string())?;
            let dh = dh.parse::<i16>().map_err(|_| COMMANDS_USAGE.to_string())?;
            Op::Resize { dw, dh }
        }
        ["rect", x, y, w, h] => {
            let x = x.parse::<u16>().map_err(|_| COMMANDS_USAGE.to_string())?;
            let y = y.parse::<u16>().map_err(|_| COMMANDS_USAGE.to_string())?;
            let w = w.parse::<u16>().map_err(|_| COMMANDS_USAGE.to_string())?;
            let h = h.parse::<u16>().map_err(|_| COMMANDS_USAGE.to_string())?;
            Op::SetRect { x, y, w, h }
        }
        _ => return usage(),
    };
    Ok(ColonCommand::CommandWindow(op))
}
