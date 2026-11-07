//! Parser for `:window` and `:session` management commands (`SES-01..05`).

use crate::action::Action as ColonCommand;
use crate::pane::PaneId;
use crate::wm::window_manage::{
    BulkWindowOp, SessionManageOp, WindowManageOp, WindowSelectTarget,
};

/// Parses a `:window ...` management command. Returns:
/// - `Some(Ok(cmd))` if it was a window management subcommand (`move`, `rename`, `select`, `bulk`, `undo`),
/// - `Some(Err(msg))` if it was a window management subcommand with invalid syntax,
/// - `None` if it was not a window management subcommand (falls through to visual/style).
pub fn parse_window_manage_command(parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    let sub = parts.first().copied()?;
    match sub {
        "move" => Some(parse_window_move(&parts[1..])),
        "rename" => Some(parse_window_rename(&parts[1..])),
        "select" => Some(parse_window_select(&parts[1..])),
        "bulk" => Some(parse_window_bulk(&parts[1..])),
        "undo" => {
            if parts.len() > 1 {
                Some(Err("Usage: :window undo".to_string()))
            } else {
                Some(Ok(ColonCommand::WindowManage(WindowManageOp::Undo)))
            }
        }
        _ => None,
    }
}

/// Parses a `:session ...` management command. Returns:
/// - `Some(Ok(cmd))` if it was `:session rename`, `:session move`, or `:session undo`,
/// - `Some(Err(msg))` if syntax error,
/// - `None` if it should fall through to other `:session` subcommands.
pub fn parse_session_manage_command(parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    let sub = parts.first().copied()?;
    match sub {
        "undo" => {
            if parts.len() > 1 {
                Some(Err("Usage: :session undo".to_string()))
            } else {
                Some(Ok(ColonCommand::SessionManage(SessionManageOp::Undo)))
            }
        }
        "rename" | "name" => Some(parse_session_rename(&parts[1..])),
        "move" => Some(parse_session_move(&parts[1..])),
        _ => None,
    }
}

pub(crate) fn parse_window_move(args: &[&str]) -> Result<ColonCommand, String> {
    let mut switch = false;
    let mut positional = Vec::new();

    for arg in args {
        if *arg == "--switch" || *arg == "-s" {
            switch = true;
        } else {
            positional.push(*arg);
        }
    }

    match positional.len() {
        1 => Ok(ColonCommand::WindowManage(WindowManageOp::Move {
            window_id: None,
            target_session: positional[0].to_string(),
            switch,
        })),
        2 => {
            if let Ok(id) = positional[0].parse::<PaneId>() {
                Ok(ColonCommand::WindowManage(WindowManageOp::Move {
                    window_id: Some(id),
                    target_session: positional[1].to_string(),
                    switch,
                }))
            } else {
                Err(format!(
                    "Invalid window id '{}': must be a positive integer",
                    positional[0]
                ))
            }
        }
        _ => Err("Usage: :window move [--switch] [window_id] <target_session>".to_string()),
    }
}

pub(crate) fn parse_window_rename(args: &[&str]) -> Result<ColonCommand, String> {
    match args.len() {
        1 => Ok(ColonCommand::WindowManage(WindowManageOp::Rename {
            window_id: None,
            new_name: args[0].to_string(),
        })),
        2 => {
            if let Ok(id) = args[0].parse::<PaneId>() {
                Ok(ColonCommand::WindowManage(WindowManageOp::Rename {
                    window_id: Some(id),
                    new_name: args[1].to_string(),
                }))
            } else {
                // If not numeric ID, assume whole remainder is the new name with spaces
                Ok(ColonCommand::WindowManage(WindowManageOp::Rename {
                    window_id: None,
                    new_name: args.join(" "),
                }))
            }
        }
        n if n > 2 => {
            if let Ok(id) = args[0].parse::<PaneId>() {
                Ok(ColonCommand::WindowManage(WindowManageOp::Rename {
                    window_id: Some(id),
                    new_name: args[1..].join(" "),
                }))
            } else {
                Ok(ColonCommand::WindowManage(WindowManageOp::Rename {
                    window_id: None,
                    new_name: args.join(" "),
                }))
            }
        }
        _ => Err("Usage: :window rename [window_id] <new_name>".to_string()),
    }
}

pub(crate) fn parse_window_select(args: &[&str]) -> Result<ColonCommand, String> {
    let sub = args.first().copied().unwrap_or("");
    match sub {
        "toggle" => {
            let Some(id_str) = args.get(1) else {
                return Err("Usage: :window select toggle <window_id>".to_string());
            };
            let id = id_str.parse::<PaneId>().map_err(|_| {
                format!("Invalid window id '{id_str}': must be a positive integer")
            })?;
            Ok(ColonCommand::WindowManage(WindowManageOp::Select(
                WindowSelectTarget::Toggle(id),
            )))
        }
        "all" => Ok(ColonCommand::WindowManage(WindowManageOp::Select(
            WindowSelectTarget::All,
        ))),
        "none" | "clear" => Ok(ColonCommand::WindowManage(WindowManageOp::Select(
            WindowSelectTarget::None,
        ))),
        "invert" => Ok(ColonCommand::WindowManage(WindowManageOp::Select(
            WindowSelectTarget::Invert,
        ))),
        "range" => {
            if args.len() < 3 {
                return Err("Usage: :window select range <from_id> <to_id>".to_string());
            }
            let from = args[1].parse::<PaneId>().map_err(|_| {
                format!("Invalid from_id '{}': must be an integer", args[1])
            })?;
            let to = args[2].parse::<PaneId>().map_err(|_| {
                format!("Invalid to_id '{}': must be an integer", args[2])
            })?;
            Ok(ColonCommand::WindowManage(WindowManageOp::Select(
                WindowSelectTarget::Range { from, to },
            )))
        }
        "explicit" => {
            let mut ids = Vec::new();
            for arg in &args[1..] {
                let id = arg.parse::<PaneId>().map_err(|_| {
                    format!("Invalid window id '{arg}': must be an integer")
                })?;
                ids.push(id);
            }
            Ok(ColonCommand::WindowManage(WindowManageOp::Select(
                WindowSelectTarget::Explicit(ids),
            )))
        }
        _ => Err("Usage: :window select <toggle <id> | all | none | clear | invert | range <from> <to> | explicit <ids...>>".to_string()),
    }
}

pub(crate) fn parse_window_bulk(args: &[&str]) -> Result<ColonCommand, String> {
    let sub = args.first().copied().unwrap_or("");
    match sub {
        "move" => {
            let Some(&session) = args.get(1) else {
                return Err("Usage: :window bulk move <target_session>".to_string());
            };
            Ok(ColonCommand::WindowManage(WindowManageOp::Bulk(
                BulkWindowOp::Move {
                    target_session: session.to_string(),
                },
            )))
        }
        "close" => {
            let force = args.iter().any(|&a| a == "--force" || a == "-f");
            let op = if force {
                BulkWindowOp::ForceClose
            } else {
                BulkWindowOp::Close
            };
            Ok(ColonCommand::WindowManage(WindowManageOp::Bulk(op)))
        }
        "rename-prefix" | "prefix" => {
            let Some(&prefix) = args.get(1) else {
                return Err("Usage: :window bulk rename-prefix <prefix>".to_string());
            };
            Ok(ColonCommand::WindowManage(WindowManageOp::Bulk(
                BulkWindowOp::RenamePrefix {
                    prefix: prefix.to_string(),
                },
            )))
        }
        "tile" => Ok(ColonCommand::WindowManage(WindowManageOp::Bulk(
            BulkWindowOp::Tile,
        ))),
        "pin" => {
            let flag = match args.get(1).copied() {
                Some("on" | "true" | "1") => Some(true),
                Some("off" | "false" | "0") => Some(false),
                _ => None,
            };
            Ok(ColonCommand::WindowManage(WindowManageOp::Bulk(
                BulkWindowOp::Pin(flag),
            )))
        }
        _ => Err("Usage: :window bulk <move <session> | close [--force] | rename-prefix <prefix> | tile | pin [on|off]>".to_string()),
    }
}

pub(crate) fn parse_session_rename(args: &[&str]) -> Result<ColonCommand, String> {
    match args.len() {
        0 => Ok(ColonCommand::SessionManage(SessionManageOp::Rename {
            old_name: None,
            new_name: String::new(),
        })),
        1 => Ok(ColonCommand::SessionManage(SessionManageOp::Rename {
            old_name: None,
            new_name: args[0].to_string(),
        })),
        2 => Ok(ColonCommand::SessionManage(SessionManageOp::Rename {
            old_name: Some(args[0].to_string()),
            new_name: args[1].to_string(),
        })),
        _ => Err("Usage: :session rename [old_name] <new_name>".to_string()),
    }
}

pub(crate) fn parse_session_move(args: &[&str]) -> Result<ColonCommand, String> {
    match args.len() {
        1 => Ok(ColonCommand::SessionManage(SessionManageOp::MoveOrder {
            name: None,
            target_pos: args[0].to_string(),
        })),
        2 => Ok(ColonCommand::SessionManage(SessionManageOp::MoveOrder {
            name: Some(args[0].to_string()),
            target_pos: args[1].to_string(),
        })),
        _ => Err("Usage: :session move [name] <up|down|top|bottom|1..N>".to_string()),
    }
}
