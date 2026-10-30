use crate::colon_command::ColonCommand;
use crate::colon_command::parse_visual::parse_visual_colon_command;
use crate::pane::{ExitProtectionPolicy, PaneScrollMode, PromptMode};
use crate::wm::WorkspaceLayoutMode;

/// Parses an input string starting with `:` into a `ColonCommand`.
/// Returns `None` if `input` does not start with `:` or is not a recognized internal command.
pub fn parse_colon_command(input: &str) -> Option<Result<ColonCommand, String>> {
    let trimmed = input.trim();
    let rest = trimmed.strip_prefix(':')?;
    let parts: Vec<&str> = rest.split_whitespace().collect();
    let head = *parts.first()?;

    if let Some(res) = parse_visual_colon_command(head, &parts) {
        return Some(res);
    }

    match head {
        "prompt" => match parts.get(1).copied() {
            Some("top") => Some(Ok(ColonCommand::PromptTop)),
            Some("bottom") => Some(Ok(ColonCommand::PromptBottom)),
            Some("split-top") | Some("splittop") | Some("fixed-top") => {
                Some(Ok(ColonCommand::PromptSplitTop))
            }
            Some("split-bottom") | Some("split-bot") | Some("splitbottom") | Some("fixed-bottom") => {
                Some(Ok(ColonCommand::PromptSplitBottom))
            }
            Some("split") => Some(Ok(ColonCommand::PromptSplitToggle)),
            Some("fixed" | "docked" | "fix") => {
                Some(Ok(ColonCommand::PromptModeSet(Some(PromptMode::Fixed))))
            }
            Some("floating") => Some(Ok(ColonCommand::PromptModeSet(Some(PromptMode::Floating)))),
            Some("terminal" | "regular" | "raw" | "off" | "hide") => {
                Some(Ok(ColonCommand::PromptModeSet(Some(PromptMode::RegularTerminal))))
            }
            Some("mode" | "cycle") => Some(Ok(ColonCommand::PromptModeSet(None))),
            Some("float") => {
                if let Some(row) = parts.get(2).and_then(|s| s.parse::<u16>().ok()) {
                    Some(Ok(ColonCommand::PromptFloat(row)))
                } else {
                    Some(Ok(ColonCommand::PromptFloat(4)))
                }
            }
            Some("height") => match parts.get(2).and_then(|s| s.parse::<u16>().ok()) {
                Some(rows) => Some(Ok(ColonCommand::PromptHeight(rows.clamp(3, 12)))),
                None => Some(Err("Usage: :prompt height <3..12>".to_string())),
            },
            Some("width") => match parts.get(2).copied() {
                Some("full" | "auto" | "reset") => Some(Ok(ColonCommand::PromptWidth(None))),
                Some(s) => match s.parse::<u16>().ok() {
                    Some(cols) => Some(Ok(ColonCommand::PromptWidth(Some(cols.max(16))))),
                    None => Some(Err("Usage: :prompt width <16..cols | full>".to_string())),
                },
                None => Some(Err("Usage: :prompt width <16..cols | full>".to_string())),
            },
            _ => Some(Err(
                "Usage: :prompt top | bottom | fixed | floating | terminal | split-top | split-bottom | split | float [row] | height <3..12> | width <cols|full>"
                    .to_string(),
            )),
        },
        "mode" => match parts.get(1).copied() {
            None | Some("cycle" | "toggle" | "next") => Some(Ok(ColonCommand::PromptModeSet(None))),
            Some("float" | "floating") => {
                Some(Ok(ColonCommand::PromptModeSet(Some(PromptMode::Floating))))
            }
            Some("fixed" | "fix" | "docked" | "split") => {
                Some(Ok(ColonCommand::PromptModeSet(Some(PromptMode::Fixed))))
            }
            Some("terminal" | "term" | "regular" | "raw" | "off") => Some(Ok(
                ColonCommand::PromptModeSet(Some(PromptMode::RegularTerminal)),
            )),
            _ => Some(Err(
                "Usage: :mode [float | fixed | terminal | cycle]".to_string(),
            )),
        },
        "help" | "h" | "?" => Some(Ok(ColonCommand::HelpToggle)),
        "history" | "hist" => match parts.get(1).copied() {
            None | Some("popup" | "list" | "show") => Some(Ok(ColonCommand::HistoryPopupToggle)),
            Some("save" | "export") => {
                let path = parts.get(2).map(|s| (*s).to_string());
                Some(Ok(ColonCommand::HistorySave(path)))
            }
            Some("on" | "enable" | "true") => Some(Ok(ColonCommand::HistoryToggle(Some(true)))),
            Some("off" | "disable" | "false") => Some(Ok(ColonCommand::HistoryToggle(Some(false)))),
            Some("toggle") => Some(Ok(ColonCommand::HistoryToggle(None))),
            Some("runs" | "segments") => Some(Ok(ColonCommand::HistoryRuns)),
            Some("max-files" | "files") => match parts.get(2).and_then(|s| s.parse::<usize>().ok()) {
                Some(n) => Some(Ok(ColonCommand::HistoryMaxFiles(n.max(1)))),
                None => Some(Err("Usage: :history max-files <count>".to_string())),
            },
            _ => Some(Err(
                "Usage: :history [popup | save [path] | on | off | runs | max-files <N>]".to_string(),
            )),
        },
        "block" => match parts.get(1).copied() {
            Some("prev") => Some(Ok(ColonCommand::BlockPrev)),
            Some("next") => Some(Ok(ColonCommand::BlockNext)),
            _ => Some(Err("Usage: :block prev | next".to_string())),
        },
        "split" | "sp" => match parts.get(1).copied() {
            Some("h") | Some("horizontal") | None => Some(Ok(ColonCommand::SplitHorizontal)),
            Some("v") | Some("vertical") => Some(Ok(ColonCommand::SplitVertical)),
            _ => Some(Err("Usage: :split h | v".to_string())),
        },
        "vsplit" | "vs" => Some(Ok(ColonCommand::SplitVertical)),
        "deck" => match parts.get(1).copied() {
            Some("prev") => Some(Ok(ColonCommand::DeckPrev)),
            Some("next") | None => Some(Ok(ColonCommand::DeckNext)),
            _ => Some(Err("Usage: :deck next | prev".to_string())),
        },
        "focus" | "raise" => match parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
            Some(id) => Some(Ok(ColonCommand::FocusPane(id))),
            None => Some(Err("Usage: :focus <pane_id>".to_string())),
        },
        "zoom" => Some(Ok(ColonCommand::ZoomToggle)),
        "close" => Some(Ok(ColonCommand::ClosePane)),
        "layout" => match parts.get(1).copied() {
            Some("stack") => {
                Some(Ok(ColonCommand::LayoutMode(WorkspaceLayoutMode::Stacked)))
            }
            Some("tile") | Some("tiled") => {
                Some(Ok(ColonCommand::LayoutMode(WorkspaceLayoutMode::Tiled)))
            }
            Some("cascade") if parts.len() == 2 => {
                Some(Ok(ColonCommand::LayoutMode(WorkspaceLayoutMode::Stacked)))
            }
            Some(
                "save"
                | "load"
                | "switch"
                | "use"
                | "select"
                | "apply"
                | "default"
                | "set-default"
                | "autosave"
                | "auto"
                | "list"
                | "ls"
                | "profiles"
                | "status"
                | "next"
                | "prev"
                | "previous"
                | "delete"
                | "rm"
                | "remove",
            ) => Some(Ok(ColonCommand::LayoutProfileCommand(parts[1..].join(" ")))),
            Some("profile") => {
                let sub = if parts.len() > 2 {
                    format!("switch {}", parts[2..].join(" "))
                } else {
                    "list".to_string()
                };
                Some(Ok(ColonCommand::LayoutProfileCommand(sub)))
            }
            Some(
                name @ ("single"
                | "dual-v"
                | "dual-h"
                | "main-side"
                | "quad"
                | "side-by-side"
                | "top-bottom"
                | "grid-4"),
            ) => Some(Ok(ColonCommand::LayoutProfileCommand(format!("switch {name}")))),
            _ => Some(Err(
                "Usage: :layout tile | stack | save [name] | switch <profile> | default [name] | autosave [on|off] | list | next | prev"
                    .to_string(),
            )),
        },
        "profile" | "profiles" => {
            let sub = if parts.len() > 1 {
                parts[1..].join(" ")
            } else {
                "list".to_string()
            };
            Some(Ok(ColonCommand::LayoutProfileCommand(sub)))
        }
        "autosave" => {
            let sub = if parts.len() > 1 {
                format!("autosave {}", parts[1..].join(" "))
            } else {
                "autosave".to_string()
            };
            Some(Ok(ColonCommand::LayoutProfileCommand(sub)))
        }
        "quake" | "guake" => Some(Ok(ColonCommand::QuakeToggle)),
        "search" | "find" => match parts.get(1).copied() {
            None | Some("off" | "clear") => Some(Ok(ColonCommand::SearchOff)),
            Some("next" | "n") => Some(Ok(ColonCommand::SearchNext)),
            Some("prev" | "p") => Some(Ok(ColonCommand::SearchPrev)),
            Some(_) => {
                let query = parts[1..].join(" ");
                Some(Ok(ColonCommand::SearchQuery(query)))
            }
        },
        "copy" | "vi" => Some(Ok(ColonCommand::CopyModeToggle)),
        "links" | "url" | "urls" => Some(Ok(ColonCommand::LinksYank)),
        "inspector" | "inspect" => {
            let arg = match parts.get(1).copied() {
                Some("on" | "true" | "open") => Some(true),
                Some("off" | "false" | "close") => Some(false),
                _ => None,
            };
            Some(Ok(ColonCommand::InspectorToggle(arg)))
        }
        "hints" | "hint" | "fields" => Some(Ok(ColonCommand::HintsToggle)),
        "lock" | "readonly" | "prot" => {
            let arg = match parts.get(1).copied() {
                Some("on" | "true") => Some(true),
                Some("off" | "false") => Some(false),
                _ => None,
            };
            Some(Ok(ColonCommand::LockToggle(arg)))
        }
        "broadcast" | "bcast" => {
            let arg = match parts.get(1).copied() {
                Some("on" | "true") => Some(true),
                Some("off" | "false") => Some(false),
                _ => None,
            };
            Some(Ok(ColonCommand::BroadcastToggle(arg)))
        }
        "session" | "workspace" | "sessions" => match parts.get(1).copied() {
            Some("save") => {
                let name = parts.get(2).copied().unwrap_or("default").to_string();
                Some(Ok(ColonCommand::SessionSave(name)))
            }
            Some("restore" | "load") => {
                let name = parts.get(2).copied().unwrap_or("default").to_string();
                Some(Ok(ColonCommand::SessionRestore(name)))
            }
            Some(
                "new"
                | "create"
                | "blank"
                | "clean"
                | "rename"
                | "name"
                | "switch"
                | "open"
                | "move"
                | "reorder"
                | "order"
                | "swap"
                | "up"
                | "down"
                | "top"
                | "bottom"
                | "delete"
                | "rm"
                | "remove"
                | "branch"
                | "spawn"
                | "graft"
                | "detach-subtree"
                | "promote"
                | "tree"
                | "checkpoint"
                | "journal",
            ) => {
                let sub = parts[1..].join(" ");
                Some(Ok(ColonCommand::SessionTreeCommand(sub)))
            }
            None | Some("list" | "ls") => Some(Ok(ColonCommand::SessionList)),
            _ => Some(Err(
                "Usage: :session new [name] | blank [name] | rename [old] <new> | switch <name> | move [name] <up|down|top|bottom|idx> | reorder <s1> <s2>... | save [name] | restore [name] | delete <name> | list | tree"
                    .to_string(),
            )),
        },
        "new-session" | "newsession" => {
            let sub = if parts.len() > 1 {
                format!("new {}", parts[1..].join(" "))
            } else {
                "new".to_string()
            };
            Some(Ok(ColonCommand::SessionTreeCommand(sub)))
        }
        "rename-session" | "renamesession" => {
            let sub = if parts.len() > 1 {
                format!("rename {}", parts[1..].join(" "))
            } else {
                "rename".to_string()
            };
            Some(Ok(ColonCommand::SessionTreeCommand(sub)))
        }
        "palette" | "commands" | "cmd" => Some(Ok(ColonCommand::PaletteToggle)),
        "notifications" | "notify" | "bell" => Some(Ok(ColonCommand::NotificationsClear)),
        "trigger" | "triggers" => match parts.get(1).copied() {
            Some("add") => {
                let pat = parts.get(2..).map(|s| s.join(" ")).unwrap_or_default();
                if pat.trim().is_empty() {
                    Some(Err("Usage: :trigger add <pattern> | clear".to_string()))
                } else {
                    Some(Ok(ColonCommand::TriggerAdd(pat)))
                }
            }
            Some("clear" | "off" | "reset") => Some(Ok(ColonCommand::TriggerClear)),
            _ => Some(Err("Usage: :trigger add <pattern> | clear".to_string())),
        },
        "record" | "rec" | "asciinema" => match parts.get(1).copied() {
            None | Some("start") => {
                let path = parts.get(2).map(|s| (*s).to_string());
                Some(Ok(ColonCommand::RecordStart(path)))
            }
            Some("workspace" | "ws" | "all") => {
                let path = parts.get(2).map(|s| (*s).to_string());
                Some(Ok(ColonCommand::RecordWorkspaceStart(path)))
            }
            Some("pane") => {
                let path = parts.get(2).map(|s| (*s).to_string());
                Some(Ok(ColonCommand::RecordPaneStart(path)))
            }
            Some("stop" | "end") => Some(Ok(ColonCommand::RecordStop)),
            _ => Some(Err(
                "Usage: :record [start|workspace|pane] [path.cast] | stop".to_string(),
            )),
        },
        "ssh" => {
            let Some(&target) = parts.get(1) else {
                return Some(Err("Usage: :ssh <user@host|profile> [port]".to_string()));
            };
            let port = parts.get(2).and_then(|p| p.parse::<u16>().ok());
            Some(Ok(ColonCommand::SshConnect {
                target: target.to_string(),
                port,
            }))
        }
        "ssh-tmux" | "sshtmux" => {
            let Some(&target) = parts.get(1) else {
                return Some(Err(
                    "Usage: :ssh-tmux <user@host|profile> [session]".to_string(),
                ));
            };
            let session = parts.get(2).copied().unwrap_or("bilterm").to_string();
            Some(Ok(ColonCommand::SshTmuxConnect {
                target: target.to_string(),
                session,
            }))
        }
        "remote" | "fleet" | "herdr" => match parts.get(1).copied() {
            None | Some("list" | "ls") => Some(Ok(ColonCommand::RemoteList)),
            Some("add" | "register") => {
                let (Some(&name), Some(&dest)) = (parts.get(2), parts.get(3)) else {
                    return Some(Err(
                        "Usage: :remote add <name> <user@host> [@tag1,@tag2]".to_string(),
                    ));
                };
                let tags = parts
                    .get(4)
                    .map(|raw| {
                        raw.split(',')
                            .map(|t| t.trim().trim_start_matches('@').to_string())
                            .filter(|t| !t.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                Some(Ok(ColonCommand::RemoteAdd {
                    name: name.to_string(),
                    destination: dest.to_string(),
                    tags,
                }))
            }
            Some("exec" | "run") => {
                let Some(&selector) = parts.get(2) else {
                    return Some(Err(
                        "Usage: :remote exec <all|local|remote|@tag|node> <command>".to_string(),
                    ));
                };
                let cmd_str = parts.get(3..).map(|s| s.join(" ")).unwrap_or_default();
                if cmd_str.trim().is_empty() {
                    Some(Err(
                        "Usage: :remote exec <all|local|remote|@tag|node> <command>".to_string(),
                    ))
                } else {
                    Some(Ok(ColonCommand::RemoteExec {
                        selector: selector.to_string(),
                        command: cmd_str,
                    }))
                }
            }
            Some("disconnect" | "rm" | "remove") => {
                let Some(&name) = parts.get(2) else {
                    return Some(Err("Usage: :remote disconnect <name>".to_string()));
                };
                Some(Ok(ColonCommand::RemoteDisconnect(name.to_string())))
            }
            _ => Some(Err(
                "Usage: :remote list | add <name> <user@host> [@tags] | exec <selector> <cmd> | disconnect <name>"
                    .to_string(),
            )),
        },
        "yank" => Some(Ok(ColonCommand::Yank)),
        "paste" | "p" => Some(Ok(ColonCommand::PasteClipboard)),
        "clear" => Some(Ok(ColonCommand::Clear)),
        "scrollback" | "sb" => match parts.get(1).copied() {
            None | Some("info" | "status") => Some(Ok(ColonCommand::Scrollback { clear: false })),
            Some("clear") => Some(Ok(ColonCommand::Scrollback { clear: true })),
            _ => Some(Err("Usage: :scrollback [clear]".to_string())),
        },
        "scroll" | "scrl" | "pager" => match parts.get(1).copied() {
            None | Some("cycle") => Some(Ok(ColonCommand::ScrollModeSet(None))),
            Some("toggle" | "hold") => Some(Ok(ColonCommand::ScrollModeTogglePage)),
            Some("next" | "advance" | "down" | "fwd") => Some(Ok(ColonCommand::ScrollPageAdvance)),
            Some("prev" | "back" | "up") => Some(Ok(ColonCommand::ScrollPageBack)),
            Some("mode") => match parts.get(2).copied() {
                None | Some("cycle") => Some(Ok(ColonCommand::ScrollModeSet(None))),
                Some(tok) => match PaneScrollMode::parse(tok) {
                    Some(m) => Some(Ok(ColonCommand::ScrollModeSet(Some(m)))),
                    None => Some(Err(
                        "Usage: :scroll [smooth | page | block | instant | toggle | next | prev]"
                            .to_string(),
                    )),
                },
            },
            Some(tok) => match PaneScrollMode::parse(tok) {
                Some(m) => Some(Ok(ColonCommand::ScrollModeSet(Some(m)))),
                None => Some(Err(
                    "Usage: :scroll [smooth | page | block | instant | toggle | next | prev]"
                        .to_string(),
                )),
            },
        },
        "more" => Some(Ok(ColonCommand::ScrollPageAdvance)),
        "raw" => Some(Ok(ColonCommand::RawToggle)),
        "resize" | "winresize" => {
            let amt = parts
                .get(2)
                .and_then(|s| s.parse::<i16>().ok())
                .unwrap_or(4);
            match parts.get(1).copied() {
                None | Some("right" | "l" | "+w" | "wider" | ">") => {
                    Some(Ok(ColonCommand::PaneResize { dw: amt, dh: 0 }))
                }
                Some("left" | "h" | "-w" | "narrower" | "<") => {
                    Some(Ok(ColonCommand::PaneResize { dw: -amt, dh: 0 }))
                }
                Some("down" | "j" | "+h" | "taller" | "+") => Some(Ok(ColonCommand::PaneResize {
                    dw: 0,
                    dh: amt.clamp(1, 20),
                })),
                Some("up" | "k" | "-h" | "shorter" | "-") => Some(Ok(ColonCommand::PaneResize {
                    dw: 0,
                    dh: -amt.clamp(1, 20),
                })),
                Some("reset" | "equalize" | "=") => Some(Ok(ColonCommand::WindowResetGeometry)),
                Some(raw_dw) => {
                    if let Ok(dw) = raw_dw.parse::<i16>() {
                        let dh = parts
                            .get(2)
                            .and_then(|s| s.parse::<i16>().ok())
                            .unwrap_or(0);
                        Some(Ok(ColonCommand::PaneResize { dw, dh }))
                    } else {
                        Some(Err(
                            "Usage: :resize <left|right|up|down|+w|-w|+h|-h|reset|<dw> [dh]> [amount]"
                                .to_string(),
                        ))
                    }
                }
            }
        }
        "move" | "winmove" => {
            let amt = parts
                .get(2)
                .and_then(|s| s.parse::<i16>().ok())
                .unwrap_or(4);
            match parts.get(1).copied() {
                None | Some("right" | "l") => Some(Ok(ColonCommand::PaneMove { dx: amt, dy: 0 })),
                Some("left" | "h") => Some(Ok(ColonCommand::PaneMove { dx: -amt, dy: 0 })),
                Some("down" | "j") => Some(Ok(ColonCommand::PaneMove {
                    dx: 0,
                    dy: amt.clamp(1, 20),
                })),
                Some("up" | "k") => Some(Ok(ColonCommand::PaneMove {
                    dx: 0,
                    dy: -amt.clamp(1, 20),
                })),
                Some("reset" | "cascade" | "=") => Some(Ok(ColonCommand::WindowResetGeometry)),
                Some(raw_dx) => {
                    if let Ok(dx) = raw_dx.parse::<i16>() {
                        let dy = parts
                            .get(2)
                            .and_then(|s| s.parse::<i16>().ok())
                            .unwrap_or(0);
                        Some(Ok(ColonCommand::PaneMove { dx, dy }))
                    } else {
                        Some(Err(
                            "Usage: :move <left|right|up|down|reset|<dx> [dy]> [amount]"
                                .to_string(),
                        ))
                    }
                }
            }
        }
        "cascade" | "equalize" => Some(Ok(ColonCommand::WindowResetGeometry)),
        "exit-guard" | "exitguard" | "exit-protection" => {
            let tok = parts.get(1).copied().unwrap_or("guard");
            match ExitProtectionPolicy::from_name(tok) {
                Some(pol) => Some(Ok(ColonCommand::ExitProtectionSet(pol))),
                None => Some(Err(
                    "Usage: :exit-guard [guard | respawn | detach | off]".to_string(),
                )),
            }
        }
        "mcp" | "master-control" => {
            let sub = if parts.len() > 1 {
                parts[1..].join(" ")
            } else {
                String::new()
            };
            Some(Ok(ColonCommand::McpCommand(sub)))
        }
        "detach" | "d" => Some(Ok(ColonCommand::DetachClient)),
        "attach" | "a" | "resume" => {
            let name = parts.get(1).copied().unwrap_or("default").to_string();
            Some(Ok(ColonCommand::SessionRestore(name)))
        }
        "respawn-pane" | "respawn" => Some(Ok(ColonCommand::RespawnPaneInPlace)),
        "quit!" | "q!" | "exit!" => Some(Ok(ColonCommand::ForceQuit)),
        "quit" | "q" => Some(Ok(ColonCommand::Quit)),
        _ => None,
    }
}
