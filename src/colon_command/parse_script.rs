//! Parser for `:script <verb>` and `:studio <verb>` (Plan 51.2-03, WIN-03).

use super::ColonCommand;
use crate::wm::script_studio::ScriptStudioOp;

pub(super) const SCRIPT_USAGE: &str = "Usage: :script [open|close|toggle|dismiss|blur|run|step|reset|record [on|off]|edit [on|off]|add <line>|clear|clear-log|save [name]|load <name>|list|pin [on|off]|dock [on|off]|up|down|left|right|select <idx>|move <dx> <dy>|resize <dw> <dh>|rect <x> <y> <w> <h>]";

fn usage<T>() -> Result<T, String> {
    Err(SCRIPT_USAGE.to_string())
}

fn nums<T: std::str::FromStr>(args: &[&str]) -> Option<Vec<T>> {
    args.iter().map(|a| a.parse::<T>().ok()).collect()
}

fn parse_bool_opt(arg: &str) -> Option<Option<bool>> {
    match arg {
        "on" | "true" | "1" => Some(Some(true)),
        "off" | "false" | "0" => Some(Some(false)),
        "toggle" => Some(None),
        _ => None,
    }
}

/// Parses the `:script` or `:studio` colon command.
pub(super) fn parse_script_colon_command(head: &str, parts: &[&str], rest: &str) -> Option<Result<ColonCommand, String>> {
    if head != "script" && head != "studio" {
        return None;
    }

    use ScriptStudioOp as Op;
    let args = &parts[1..];
    let op = match args {
        [] => Op::Toggle,
        ["open"] => Op::Open,
        ["status"] => Op::Open,
        ["help"] => Op::Open,
        ["close"] => Op::Close,
        ["toggle"] => Op::Toggle,
        ["dismiss"] => Op::Dismiss,
        ["blur"] => Op::Blur,
        ["run"] => Op::RunAll,
        ["step"] => Op::StepNext,
        ["reset"] => Op::ResetStep,
        ["record"] => Op::Record(None),
        ["record", arg] => match parse_bool_opt(arg) {
            Some(opt) => Op::Record(opt),
            None => return Some(usage()),
        },
        ["edit"] => Op::EditMode(None),
        ["edit", arg] => match parse_bool_opt(arg) {
            Some(opt) => Op::EditMode(opt),
            None => return Some(usage()),
        },
        ["add", ..] => {
            // Find the slice of `rest` starting after head and "add"
            let prefix_len = head.len() + 1; // e.g. "script "
            let after_head = rest.get(prefix_len..)?.trim_start();
            let after_add = after_head.strip_prefix("add")?.trim_start();
            Op::AppendLine(after_add.to_string())
        }
        ["clear"] => Op::ClearScript,
        ["clear-log"] | ["clearlog"] => Op::ClearLog,
        ["save"] => Op::Save(None),
        ["save", name] => Op::Save(Some((*name).to_string())),
        ["load", name] => Op::Load((*name).to_string()),
        ["list"] => Op::ListSaved,
        ["pin"] => Op::Pin(Some(true)),
        ["unpin"] => Op::Pin(Some(false)),
        ["pin", arg] => match parse_bool_opt(arg) {
            Some(opt) => Op::Pin(opt),
            None => return Some(usage()),
        },
        ["dock"] => Op::Dock(Some(true)),
        ["undock"] | ["float"] => Op::Dock(Some(false)),
        ["dock", arg] => match parse_bool_opt(arg) {
            Some(opt) => Op::Dock(opt),
            None => return Some(usage()),
        },
        ["up"] => Op::CursorUp,
        ["down"] => Op::CursorDown,
        ["left"] => Op::CursorLeft,
        ["right"] => Op::CursorRight,
        ["select", idx_str] => match idx_str.parse::<usize>() {
            Ok(idx) => Op::SelectLine(idx),
            Err(_) => return Some(usage()),
        },
        ["insert", c_str] => {
            if let Some(c) = c_str.chars().next() {
                Op::InsertChar(c)
            } else {
                return Some(usage());
            }
        }
        ["backspace"] => Op::Backspace,
        ["newline"] => Op::NewLine,
        ["delete"] => Op::DeleteLine,
        ["move", a, b] => match nums::<i16>(&[a, b]) {
            Some(v) => Op::Move { dx: v[0], dy: v[1] },
            None => return Some(usage()),
        },
        ["resize", a, b] => match nums::<i16>(&[a, b]) {
            Some(v) => Op::Resize { dw: v[0], dh: v[1] },
            None => return Some(usage()),
        },
        ["rect", a, b, c, d] => match nums::<u16>(&[a, b, c, d]) {
            Some(v) => Op::SetRect { x: v[0], y: v[1], w: v[2], h: v[3] },
            None => return Some(usage()),
        },
        _ => return Some(usage()),
    };

    Some(Ok(ColonCommand::ScriptStudio(op)))
}
