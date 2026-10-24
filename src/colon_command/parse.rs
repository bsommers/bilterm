use crate::colon_command::ColonCommand;
use crate::pane::PromptMode;
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::{CaretStyle, SpotlightMode};
use crate::ui::glyphs::GlyphPack;
use crate::ui::theme::ThemePreset;
use crate::wm::WorkspaceLayoutMode;

/// Parses an input string starting with `:` into a `ColonCommand`.
/// Returns `None` if `input` does not start with `:` or is not a recognized internal command.
pub fn parse_colon_command(input: &str) -> Option<Result<ColonCommand, String>> {
    let trimmed = input.trim();
    let rest = trimmed.strip_prefix(':')?;
    let parts: Vec<&str> = rest.split_whitespace().collect();
    let head = *parts.first()?;

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
            Some("stack") | Some("cascade") => {
                Some(Ok(ColonCommand::LayoutMode(WorkspaceLayoutMode::Stacked)))
            }
            Some("tile") | Some("tiled") => {
                Some(Ok(ColonCommand::LayoutMode(WorkspaceLayoutMode::Tiled)))
            }
            _ => Some(Err("Usage: :layout tile | stack".to_string())),
        },
        "quake" | "guake" => Some(Ok(ColonCommand::QuakeToggle)),
        "theme" => match parts.get(1).and_then(|s| ThemePreset::from_name(s)) {
            Some(preset) => Some(Ok(ColonCommand::Theme(preset))),
            None => Some(Err(
                "Usage: :theme turbo-blue | turbo-cyan | 3278 | 3279 | mono".to_string(),
            )),
        },
        "glyphs" => match parts.get(1).and_then(|s| GlyphPack::from_name(s)) {
            Some(pack) => Some(Ok(ColonCommand::Glyphs(pack))),
            None => Some(Err(
                "Usage: :glyphs unicode | cp437 | nerd-font | ascii".to_string(),
            )),
        },
        "cursor" => match parts.get(1).and_then(|s| CaretStyle::from_name(s)) {
            Some(style) => Some(Ok(ColonCommand::Cursor(style))),
            None => Some(Err(
                "Usage: :cursor block | hollow | underscore | bar".to_string(),
            )),
        },
        "outline" | "border" => match (parts.get(1).copied(), parts.get(2).copied()) {
            (Some("window" | "win"), Some(style_str)) => {
                OutlineStyle::from_name(style_str)
                    .map(|s| Ok(ColonCommand::Outline { window: Some(s), prompt: None }))
                    .or_else(|| Some(Err("Unknown outline style".to_string())))
            }
            (Some("prompt" | "cmd"), Some(style_str)) => {
                OutlineStyle::from_name(style_str)
                    .map(|s| Ok(ColonCommand::Outline { window: None, prompt: Some(s) }))
                    .or_else(|| Some(Err("Unknown outline style".to_string())))
            }
            (Some(style_str), None) => {
                OutlineStyle::from_name(style_str)
                    .map(|s| Ok(ColonCommand::Outline { window: Some(s), prompt: Some(s) }))
                    .or_else(|| {
                        Some(Err(
                            "Usage: :outline [window|prompt] <solid|double|rounded|thick|dotted|dashed|quadruple|ascii>"
                                .to_string(),
                        ))
                    })
            }
            _ => Some(Err(
                "Usage: :outline [window|prompt] <solid|double|rounded|thick|dotted|dashed|quadruple|ascii>"
                    .to_string(),
            )),
        },
        "crosshair" => {
            let (enabled, fullscreen) = match parts.get(1).copied() {
                Some("on" | "true") => (Some(true), None),
                Some("off" | "false") => (Some(false), None),
                Some("full" | "fullscreen") => (Some(true), Some(true)),
                Some("pane" | "local") => (Some(true), Some(false)),
                _ => (None, None),
            };
            Some(Ok(ColonCommand::CrosshairToggle {
                enabled,
                fullscreen,
            }))
        }
        "spotlight" | "spot" | "focus-light" => {
            let mut mode = None;
            let mut pixelated = None;
            let mut fullscreen = None;
            let mut rx = None;
            let mut ry = None;
            let mut delta = None;
            let mut feather_pct = None;

            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "off" | "none" | "false" => mode = Some(SpotlightMode::Off),
                    "circle" | "circular" | "round" | "on" | "true" => {
                        mode = Some(SpotlightMode::Circle);
                    }
                    "ellipse" | "elliptical" | "oval" => mode = Some(SpotlightMode::Ellipse),
                    "pixel" | "pixelated" | "block" | "blocks" | "cell" => pixelated = Some(true),
                    "fuzzy" | "smooth" | "soft" => pixelated = Some(false),
                    "full" | "fullscreen" => fullscreen = Some(true),
                    "pane" | "local" => fullscreen = Some(false),
                    "+" | "grow" | "larger" | "bigger" => delta = Some((4, 1)),
                    "-" | "shrink" | "smaller" => delta = Some((-4, -1)),
                    "size" | "radius" => {
                        let Some(rx_val) = parts.get(i + 1).and_then(|s| s.parse::<u16>().ok()) else {
                            return Some(Err(
                                "Usage: :spotlight size <rx_cols> [ry_rows]".to_string(),
                            ));
                        };
                        rx = Some(rx_val.clamp(4, 120));
                        i += 1;
                        if let Some(ry_val) = parts.get(i + 1).and_then(|s| s.parse::<u16>().ok()) {
                            ry = Some(ry_val.clamp(2, 60));
                            i += 1;
                        }
                    }
                    "feather" | "edge" => {
                        let Some(raw) = parts.get(i + 1) else {
                            return Some(Err(
                                "Usage: :spotlight feather <5..100 | 0.05..1.0>".to_string(),
                            ));
                        };
                        let pct = if let Ok(f) = raw.parse::<f32>() {
                            if f <= 1.0 {
                                (f * 100.0).round() as u8
                            } else {
                                f.round() as u8
                            }
                        } else {
                            return Some(Err(
                                "Usage: :spotlight feather <5..100 | 0.05..1.0>".to_string(),
                            ));
                        };
                        feather_pct = Some(pct.clamp(5, 100));
                        i += 1;
                    }
                    other => {
                        if let Ok(rx_val) = other.parse::<u16>() {
                            rx = Some(rx_val.clamp(4, 120));
                            if let Some(ry_val) = parts.get(i + 1).and_then(|s| s.parse::<u16>().ok())
                            {
                                ry = Some(ry_val.clamp(2, 60));
                                i += 1;
                            }
                        } else {
                            return Some(Err(
                                "Usage: :spotlight [circle|ellipse|off] [fuzzy|pixel] [full|pane] [+|-] [size <rx> [ry]] [feather <pct>]"
                                    .to_string(),
                            ));
                        }
                    }
                }
                i += 1;
            }

            Some(Ok(ColonCommand::Spotlight {
                mode,
                pixelated,
                fullscreen,
                rx,
                ry,
                delta,
                feather_pct,
            }))
        }
        "focalzoom" | "fzoom" | "lens" | "magnify" => {
            let mut mode = None;
            let mut zoom_pct = None;
            let mut step_pct = None;
            let mut pixelated = None;
            let mut fullscreen = None;
            let mut rx = None;
            let mut ry = None;
            let mut delta_size = None;
            let mut feather_pct = None;

            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "off" | "none" | "false" | "1x" | "1.0x" | "100%" => {
                        mode = Some(SpotlightMode::Off);
                    }
                    "circle" | "circular" | "round" => mode = Some(SpotlightMode::Circle),
                    "ellipse" | "elliptical" | "oval" | "on" | "true" => {
                        mode = Some(SpotlightMode::Ellipse);
                    }
                    "subtle" | "low" => zoom_pct = Some(125),
                    "medium" | "med" => zoom_pct = Some(150),
                    "high" => zoom_pct = Some(200),
                    "ultra" | "max" => zoom_pct = Some(300),
                    "pixel" | "pixelated" | "block" | "blocks" | "cell" => pixelated = Some(true),
                    "fuzzy" | "smooth" | "soft" => pixelated = Some(false),
                    "full" | "fullscreen" => fullscreen = Some(true),
                    "pane" | "local" => fullscreen = Some(false),
                    "+" | "in" | "up" => step_pct = Some(25),
                    "-" | "out" | "down" => step_pct = Some(-25),
                    "grow" | "larger" | "bigger" => delta_size = Some((4, 1)),
                    "shrink" | "smaller" => delta_size = Some((-4, -1)),
                    "size" | "radius" => {
                        let Some(rx_val) = parts.get(i + 1).and_then(|s| s.parse::<u16>().ok())
                        else {
                            return Some(Err(
                                "Usage: :focalzoom size <rx_cols> [ry_rows]".to_string(),
                            ));
                        };
                        rx = Some(rx_val.clamp(4, 120));
                        i += 1;
                        if let Some(ry_val) = parts.get(i + 1).and_then(|s| s.parse::<u16>().ok()) {
                            ry = Some(ry_val.clamp(2, 60));
                            i += 1;
                        }
                    }
                    "feather" | "edge" => {
                        let Some(raw) = parts.get(i + 1) else {
                            return Some(Err(
                                "Usage: :focalzoom feather <5..100 | 0.05..1.0>".to_string(),
                            ));
                        };
                        let pct = if let Ok(f) = raw.parse::<f32>() {
                            if f <= 1.0 {
                                (f * 100.0).round() as u8
                            } else {
                                f.round() as u8
                            }
                        } else {
                            return Some(Err(
                                "Usage: :focalzoom feather <5..100 | 0.05..1.0>".to_string(),
                            ));
                        };
                        feather_pct = Some(pct.clamp(5, 100));
                        i += 1;
                    }
                    other => {
                        let cleaned = other.trim_end_matches('x').trim_end_matches('%');
                        if let Ok(val) = cleaned.parse::<f32>() {
                            let pct = if val <= 6.0 {
                                (val * 100.0).round() as u16
                            } else {
                                val.round() as u16
                            };
                            if pct <= 100 {
                                mode = Some(SpotlightMode::Off);
                            } else {
                                zoom_pct = Some(pct.clamp(105, 400));
                            }
                        } else {
                            return Some(Err(
                                "Usage: :focalzoom [1.25x|1.5x|2x|3x|off] [circle|ellipse] [fuzzy|pixel] [+|-] [grow|shrink] [size <rx> [ry]] [feather <pct>]"
                                    .to_string(),
                            ));
                        }
                    }
                }
                i += 1;
            }

            Some(Ok(ColonCommand::FocalZoom {
                mode,
                zoom_pct,
                step_pct,
                pixelated,
                fullscreen,
                rx,
                ry,
                delta_size,
                feather_pct,
            }))
        }
        "shadows" | "shadow" => {
            let arg = parts.get(1).and_then(|s| ShadowStyle::from_name(s));
            Some(Ok(ColonCommand::Shadows(arg)))
        }
        "fkeybar" => {
            let arg = match parts.get(1).copied() {
                Some("on") | Some("true") => Some(true),
                Some("off") | Some("false") => Some(false),
                _ => None,
            };
            Some(Ok(ColonCommand::FKeyBarToggle(arg)))
        }
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
        "menubar" | "menu" => {
            let arg = match parts.get(1).copied() {
                Some("on" | "true") => Some(true),
                Some("off" | "false") => Some(false),
                _ => None,
            };
            Some(Ok(ColonCommand::MenuBarToggle(arg)))
        }
        "session" | "workspace" => match parts.get(1).copied() {
            Some("save") => {
                let name = parts.get(2).copied().unwrap_or("default").to_string();
                Some(Ok(ColonCommand::SessionSave(name)))
            }
            Some("restore" | "load") => {
                let name = parts.get(2).copied().unwrap_or("default").to_string();
                Some(Ok(ColonCommand::SessionRestore(name)))
            }
            None | Some("list" | "ls") => Some(Ok(ColonCommand::SessionList)),
            _ => Some(Err(
                "Usage: :session save [name] | restore [name] | list".to_string(),
            )),
        },
        "crt" | "scanlines" => {
            let arg = match parts.get(1).copied() {
                Some("on" | "true") => Some(true),
                Some("off" | "false") => Some(false),
                _ => None,
            };
            Some(Ok(ColonCommand::CrtToggle(arg)))
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
            Some("stop" | "end") => Some(Ok(ColonCommand::RecordStop)),
            _ => Some(Err("Usage: :record start [path.cast] | stop".to_string())),
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
        "clear" => Some(Ok(ColonCommand::Clear)),
        "scrollback" | "sb" => match parts.get(1).copied() {
            None | Some("info" | "status") => Some(Ok(ColonCommand::Scrollback { clear: false })),
            Some("clear") => Some(Ok(ColonCommand::Scrollback { clear: true })),
            _ => Some(Err("Usage: :scrollback [clear]".to_string())),
        },
        "raw" => Some(Ok(ColonCommand::RawToggle)),
        "quit" | "q" => Some(Ok(ColonCommand::Quit)),
        _ => None,
    }
}
