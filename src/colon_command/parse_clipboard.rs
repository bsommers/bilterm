//! Parser for `:clipboard <verb>` and `:clips <verb>` (`CLIP-03`, `CLIP-04`, Phase 57).

use super::ColonCommand;
use crate::pane::PaneId;
use crate::wm::clipboard_registry::ClipboardFilterScope;
use crate::wm::clipboard_window::ClipboardWindowOp;

pub const CLIPBOARD_USAGE: &str = "Usage: :clipboard [open|close|toggle|dismiss|blur|pin [toggle]|unpin|dock [toggle]|float|all|session [<name>|cycle]|window [<id>|cycle]|pinned|media|tab <next|prev>|filter [<query>]|clear|paste [<idx>]|yank|copy <text>|pop|pin-entry [<id>]|delete [<id>]|persist [on|off]|up|down|page-up|page-down|first|last|select <idx>|scroll <delta>|preview-scroll <delta>|move <dx> <dy>|resize <dw> <dh>|rect <x> <y> <w> <h>|size auto|scale <+|-|delta>]";

fn usage<T>() -> Result<T, String> {
    Err(CLIPBOARD_USAGE.to_string())
}

/// Parses arguments after `:clipboard`, `:clips`, `:clip`, `:cb`, `:buffers`, `:yank-history`, or `:yank-ring`.
pub(super) fn parse(args: &[&str]) -> Result<ColonCommand, String> {
    use ClipboardWindowOp as Op;
    let op = match args {
        [] | ["toggle"] => Op::Toggle,
        ["open" | "show" | "list" | "ls"] => Op::Open { scope: None },
        ["close" | "hide"] => Op::Close,
        ["dismiss"] => Op::Dismiss,
        ["blur"] => Op::Blur,
        ["pin"] | ["pin", "on" | "true" | "1"] => Op::Pin(Some(true)),
        ["unpin"] | ["pin", "off" | "false" | "0"] => Op::Pin(Some(false)),
        ["pin", "toggle"] => Op::Pin(None),
        ["dock"] | ["dock", "on" | "true" | "1"] => Op::Dock(Some(true)),
        ["float" | "undock"] | ["dock", "off" | "false" | "0"] => Op::Dock(Some(false)),
        ["dock", "toggle"] => Op::Dock(None),
        ["all"] => Op::Open {
            scope: Some(ClipboardFilterScope::All),
        },
        ["session" | "sess"] => Op::Open {
            scope: Some(ClipboardFilterScope::Session),
        },
        ["session" | "sess", "cycle" | "next"] => Op::CycleSessionFilter,
        ["session" | "sess", "active" | "current" | "reset"] => Op::FilterSession(None),
        ["session" | "sess", name] => Op::FilterSession(Some((*name).to_string())),
        ["window" | "win" | "pane"] => Op::Open {
            scope: Some(ClipboardFilterScope::Window),
        },
        ["window" | "win" | "pane", "cycle" | "next"] => Op::CycleWindowFilter,
        ["window" | "win" | "pane", "active" | "current" | "reset"] => Op::FilterWindow(None),
        ["window" | "win" | "pane", id_str] => {
            let id = id_str
                .trim_start_matches('#')
                .parse::<PaneId>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::FilterWindow(Some(id))
        }
        ["pinned" | "pins"] => Op::Open {
            scope: Some(ClipboardFilterScope::Pinned),
        },
        ["media" | "images" | "video"] => Op::Open {
            scope: Some(ClipboardFilterScope::Media),
        },
        ["tab", "next"] => Op::CycleScope(1),
        ["tab", "prev"] => Op::CycleScope(-1),
        ["filter" | "search" | "find"] => Op::SearchClear,
        ["filter" | "search" | "find", "clear" | "off" | "reset"] => Op::SearchClear,
        ["filter" | "search" | "find", rest @ ..] if !rest.is_empty() => {
            Op::SearchSet(rest.join(" "))
        }
        ["clear" | "reset"] => Op::ClearUnpinned,
        ["paste"] => Op::PasteSelected,
        ["paste", idx_str] => {
            let idx = idx_str
                .trim_start_matches('#')
                .parse::<usize>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::PasteIndex(idx)
        }
        ["yank"] => Op::YankSelected,
        ["copy" | "add" | "push", rest @ ..] if !rest.is_empty() => Op::CopyText(rest.join(" ")),
        ["pop" | "cycle" | "next"] => Op::PopYankRing,
        ["pin-entry"] => Op::TogglePinSelected,
        ["pin-entry", id_str] => {
            let id = id_str
                .trim_start_matches('#')
                .parse::<u64>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::PinEntryId(id, None)
        }
        ["pin-entry", id_str, flag_str] => {
            let id = id_str
                .trim_start_matches('#')
                .parse::<u64>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            let pinned = match flag_str.to_ascii_lowercase().as_str() {
                "on" | "true" | "1" | "pin" => Some(true),
                "off" | "false" | "0" | "unpin" => Some(false),
                "toggle" => None,
                _ => return usage(),
            };
            Op::PinEntryId(id, pinned)
        }
        ["delete" | "del" | "rm"] => Op::DeleteSelected,
        ["delete" | "del" | "rm", id_str] => {
            let id = id_str
                .trim_start_matches('#')
                .parse::<u64>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::DeleteEntryId(id)
        }
        ["persist" | "save"] => Op::SetPersist(None),
        ["persist" | "save", "on" | "true" | "enable" | "1"] => Op::SetPersist(Some(true)),
        ["persist" | "save", "off" | "false" | "disable" | "0"] => Op::SetPersist(Some(false)),
        ["up" | "prev"] => Op::Step(-1),
        ["down"] => Op::Step(1),
        ["step", delta_str] => {
            let delta = delta_str
                .parse::<i32>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::Step(delta)
        }
        ["page-up"] => Op::Page(-1),
        ["page-down"] => Op::Page(1),
        ["first" | "home"] => Op::First,
        ["last" | "end"] => Op::Last,
        ["select", idx_str] => {
            let idx = idx_str
                .parse::<usize>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::SelectRow(idx.saturating_sub(1))
        }
        ["scroll", delta_str] => {
            let delta = delta_str
                .parse::<i32>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::Scroll(delta)
        }
        ["preview-scroll", delta_str] => {
            let delta = delta_str
                .parse::<i32>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::PreviewScroll(delta)
        }
        ["move", dx_str, dy_str] => {
            let dx = dx_str
                .parse::<i16>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            let dy = dy_str
                .parse::<i16>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::Move { dx, dy }
        }
        ["resize", dw_str, dh_str] => {
            let dw = dw_str
                .parse::<i16>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            let dh = dh_str
                .parse::<i16>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::Resize { dw, dh }
        }
        ["rect", x_str, y_str, w_str, h_str] => {
            let x = x_str
                .parse::<u16>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            let y = y_str
                .parse::<u16>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            let w = w_str
                .parse::<u16>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            let h = h_str
                .parse::<u16>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::SetRect { x, y, w, h }
        }
        ["size", "auto" | "reset"] => Op::SizeAuto,
        ["scale", "+" | "up" | "in"] => Op::Scale(10),
        ["scale", "-" | "down" | "out"] => Op::Scale(-10),
        ["scale", delta_str] => {
            let delta = delta_str
                .trim_end_matches('%')
                .parse::<i16>()
                .map_err(|_| CLIPBOARD_USAGE.to_string())?;
            Op::Scale(delta)
        }
        ["raw-ctrl-v" | "raw"] => {
            return Ok(ColonCommand::SelectCommand(Some(
                "clipboard raw-ctrl-v".to_string(),
            )));
        }
        [idx_str] => {
            if let Ok(idx) = idx_str.trim_start_matches('#').parse::<usize>() {
                Op::PasteIndex(idx)
            } else {
                return usage();
            }
        }
        _ => return usage(),
    };
    Ok(ColonCommand::ClipboardWindow(op))
}
