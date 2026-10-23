use crate::pane::history_store::HistorySegment;
use crate::pane::{CompositePane, PromptAnchor, PromptMode};
use crate::remote::{FleetSelector, RemoteHostProfile};
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::{CaretStyle, SpotlightMode};
use crate::ui::glyphs::GlyphPack;
use crate::ui::theme::ThemePreset;
use crate::vt::encode_osc52_clipboard;
use crate::wm::split_tree::SplitDirection;
use crate::wm::{WindowManager, WorkspaceLayoutMode};

/// Parsed in-prompt `:command` (`PANE-03`, `PANE-05`, `PTY-04`, `WM-01..05`, `UI-01..05`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColonCommand {
    PromptTop,
    PromptBottom,
    PromptSplitTop,
    PromptSplitBottom,
    PromptSplitToggle,
    PromptModeSet(Option<PromptMode>),
    PromptFloat(u16),
    PromptHeight(u16),
    PromptWidth(Option<u16>),
    BlockPrev,
    BlockNext,
    Yank,
    Clear,
    /// `:scrollback` (report) / `:scrollback clear` (free history).
    Scrollback { clear: bool },
    RawToggle,
    SplitHorizontal,
    SplitVertical,
    DeckNext,
    DeckPrev,
    FocusPane(usize),
    ZoomToggle,
    ClosePane,
    LayoutMode(WorkspaceLayoutMode),
    QuakeToggle,
    HelpToggle,
    HistoryPopupToggle,
    HistorySave(Option<String>),
    HistoryToggle(Option<bool>),
    HistoryMaxFiles(usize),
    HistoryRuns,
    Theme(ThemePreset),
    Glyphs(GlyphPack),
    Cursor(CaretStyle),
    Outline {
        window: Option<OutlineStyle>,
        prompt: Option<OutlineStyle>,
    },
    CrosshairToggle {
        enabled: Option<bool>,
        fullscreen: Option<bool>,
    },
    Spotlight {
        mode: Option<SpotlightMode>,
        pixelated: Option<bool>,
        fullscreen: Option<bool>,
        rx: Option<u16>,
        ry: Option<u16>,
        delta: Option<(i16, i16)>,
        feather_pct: Option<u8>,
    },
    FocalZoom {
        mode: Option<SpotlightMode>,
        zoom_pct: Option<u16>,
        step_pct: Option<i16>,
        pixelated: Option<bool>,
        fullscreen: Option<bool>,
        rx: Option<u16>,
        ry: Option<u16>,
        delta_size: Option<(i16, i16)>,
        feather_pct: Option<u8>,
    },
    Shadows(Option<ShadowStyle>),
    FKeyBarToggle(Option<bool>),
    SearchQuery(String),
    SearchNext,
    SearchPrev,
    SearchOff,
    CopyModeToggle,
    LinksYank,
    InspectorToggle(Option<bool>),
    HintsToggle,
    LockToggle(Option<bool>),
    BroadcastToggle(Option<bool>),
    MenuBarToggle(Option<bool>),
    SessionSave(String),
    SessionRestore(String),
    SessionList,
    CrtToggle(Option<bool>),
    PaletteToggle,
    NotificationsClear,
    TriggerAdd(String),
    TriggerClear,
    RecordStart(Option<String>),
    RecordStop,
    SshConnect {
        target: String,
        port: Option<u16>,
    },
    SshTmuxConnect {
        target: String,
        session: String,
    },
    RemoteAdd {
        name: String,
        destination: String,
        tags: Vec<String>,
    },
    RemoteList,
    RemoteExec {
        selector: String,
        command: String,
    },
    RemoteDisconnect(String),
    Quit,
}

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

/// Executes a parsed `ColonCommand` against a single `pane`. Returns `true` if the application should quit.
/// Formats the `:history runs` status line: segment count plus the 3 most recent runs.
fn format_history_runs(session: &str, segments: &[HistorySegment]) -> String {
    if segments.is_empty() {
        return format!("History session '{session}': no recorded runs");
    }
    let recent: Vec<String> = segments
        .iter()
        .rev()
        .take(3)
        .map(|s| format!("{} @{} ({} cmds)", s.run_id, s.started_at, s.count))
        .collect();
    format!(
        "History session '{session}': {} runs; recent: {}",
        segments.len(),
        recent.join(", ")
    )
}

pub fn execute_colon_command(pane: &mut CompositePane, cmd: ColonCommand) -> bool {
    match cmd {
        ColonCommand::PromptTop => {
            pane.set_prompt_anchor(PromptAnchor::Top);
            pane.status_message = Some("Prompt pinned to TOP".to_string());
            false
        }
        ColonCommand::PromptBottom => {
            pane.set_prompt_anchor(PromptAnchor::Bottom);
            pane.status_message = Some("Prompt pinned to BOTTOM".to_string());
            false
        }
        ColonCommand::PromptSplitTop => {
            pane.set_prompt_anchor(PromptAnchor::SplitTop);
            pane.status_message = Some("Prompt split at TOP (non-overlapping)".to_string());
            false
        }
        ColonCommand::PromptSplitBottom => {
            pane.set_prompt_anchor(PromptAnchor::SplitBottom);
            pane.status_message = Some("Prompt split at BOTTOM (non-overlapping)".to_string());
            false
        }
        ColonCommand::PromptSplitToggle => {
            pane.toggle_split_prompt();
            pane.status_message = Some("Toggled split/floating prompt mode".to_string());
            false
        }
        ColonCommand::PromptModeSet(mode_opt) => {
            match mode_opt {
                Some(mode) => pane.set_prompt_mode(mode),
                None => {
                    pane.cycle_prompt_mode();
                }
            }
            false
        }
        ColonCommand::PromptFloat(y_offset) => {
            pane.set_prompt_anchor(PromptAnchor::Floating { y_offset });
            pane.status_message = Some(format!("Prompt floating at row +{y_offset}"));
            false
        }
        ColonCommand::PromptHeight(rows) => {
            pane.set_prompt_height(rows);
            pane.status_message = Some(format!("Prompt height set to {rows} rows"));
            false
        }
        ColonCommand::PromptWidth(cols_opt) => {
            pane.set_prompt_width(cols_opt);
            pane.status_message = Some(match cols_opt {
                Some(c) => format!("Prompt width set to {c} cols"),
                None => "Prompt width reset to full width".to_string(),
            });
            false
        }
        ColonCommand::HistoryPopupToggle => {
            pane.prompt.toggle_history_popup();
            false
        }
        ColonCommand::HistorySave(path_opt) => {
            let custom = path_opt.as_deref().map(std::path::Path::new);
            match pane
                .history_store
                .export_to_temp_file(&pane.prompt.history, custom)
            {
                Ok(saved_path) => {
                    pane.status_message = Some(format!("Saved history to {}", saved_path.display()));
                }
                Err(err) => {
                    pane.status_message = Some(format!("History save error: {err}"));
                }
            }
            false
        }
        ColonCommand::HistoryToggle(val) => {
            let next = val.unwrap_or(!pane.history_store.enabled);
            pane.history_store.enabled = next;
            pane.status_message = Some(format!(
                "Command history persistence: {}",
                if next { "ON" } else { "OFF" }
            ));
            false
        }
        ColonCommand::HistoryMaxFiles(max_f) => {
            pane.history_store.max_files = max_f.max(1);
            let _ = pane.history_store.prune_excess_files();
            pane.status_message = Some(format!("History max session files: {}", max_f.max(1)));
            false
        }
        ColonCommand::HistoryRuns => {
            pane.status_message = Some(format_history_runs(
                &pane.history_store.session_name,
                &pane.history_store.segments(),
            ));
            false
        }
        ColonCommand::BlockPrev => {
            if let Some(block) = pane.vt.jump_to_prev_command_block() {
                pane.status_message =
                    Some(format!("Block #{}: {}", block.index + 1, block.command));
            } else {
                pane.status_message = Some("No command blocks in scrollback".to_string());
            }
            false
        }
        ColonCommand::BlockNext => {
            if let Some(block) = pane.vt.jump_to_next_command_block() {
                pane.status_message =
                    Some(format!("Block #{}: {}", block.index + 1, block.command));
            } else {
                pane.status_message = Some("Returned to live output".to_string());
            }
            false
        }
        ColonCommand::Yank => {
            let text_to_yank = if let Some(block) = pane.vt.selected_command_block() {
                block.command.clone()
            } else if let Some(last) = pane.vt.command_blocks().last() {
                last.command.clone()
            } else {
                pane.vt.screen().contents()
            };
            let seq = encode_osc52_clipboard(&text_to_yank);
            pane.last_osc52_sequence = Some(seq);
            pane.status_message = Some(format!("Yanked {} bytes via OSC 52", text_to_yank.len()));
            false
        }
        ColonCommand::Clear => {
            pane.process_pty_output(b"\x1b[2J\x1b[H");
            pane.vt.reset_scrollback();
            pane.status_message = Some("Canvas cleared".to_string());
            false
        }
        ColonCommand::RawToggle => {
            pane.toggle_manual_raw_override();
            false
        }
        ColonCommand::SearchQuery(query) => {
            pane.set_canvas_search(&query);
            false
        }
        ColonCommand::SearchNext => {
            pane.next_canvas_search_match();
            false
        }
        ColonCommand::SearchPrev => {
            pane.prev_canvas_search_match();
            false
        }
        ColonCommand::SearchOff => {
            pane.clear_canvas_search();
            pane.status_message = Some("Cleared canvas search".to_string());
            false
        }
        ColonCommand::CopyModeToggle => {
            if pane.copy_mode.active {
                pane.exit_copy_mode();
            } else {
                pane.enter_copy_mode();
            }
            false
        }
        ColonCommand::LinksYank => {
            let link = pane
                .vt
                .detect_visible_urls()
                .last()
                .map(|u| u.url.clone())
                .or_else(|| pane.vt.osc8_links().last().map(|l| l.uri.clone()));
            if let Some(url) = link {
                let seq = encode_osc52_clipboard(&url);
                pane.last_osc52_sequence = Some(seq);
                pane.prompt.yank_buffer = url.clone();
                pane.status_message = Some(format!("Yanked URL: {url}"));
            } else {
                pane.status_message = Some("No URLs detected on canvas".to_string());
            }
            false
        }
        ColonCommand::HintsToggle => {
            if pane.hints_mode.active {
                pane.exit_hints_mode();
            } else {
                pane.enter_hints_mode();
            }
            false
        }
        ColonCommand::LockToggle(val) => {
            let next = val.unwrap_or(!pane.read_only);
            pane.set_read_only(next);
            false
        }
        ColonCommand::NotificationsClear => {
            let count = pane.vt.notifications().len();
            pane.vt.clear_notifications();
            pane.status_message = Some(format!("Cleared {count} notification(s)"));
            false
        }
        ColonCommand::RecordStart(path_opt) => {
            let path = path_opt
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!("bilterm-pane-{}.cast", pane.id))
                });
            pane.vt.start_recording(path.clone());
            pane.status_message = Some(format!("⏺ Recording asciicast v2 to {}", path.display()));
            false
        }
        ColonCommand::RecordStop => {
            match pane.vt.stop_recording() {
                Ok(Some(path)) => {
                    pane.status_message =
                        Some(format!("Saved asciicast v2 recording to {}", path.display()));
                }
                Ok(None) => {
                    pane.status_message = Some("No active recording on this pane".to_string());
                }
                Err(err) => {
                    pane.status_message = Some(format!("Recording save error: {err}"));
                }
            }
            false
        }
        ColonCommand::Quit => true,
        _ => false,
    }
}

/// Executes a parsed `ColonCommand` against the multi-pane `WindowManager`.
pub fn execute_wm_colon_command(wm: &mut WindowManager, cmd: ColonCommand) -> bool {
    match cmd {
        ColonCommand::SplitHorizontal => {
            let _ = wm.split_focused(SplitDirection::Horizontal);
            false
        }
        ColonCommand::SplitVertical => {
            let _ = wm.split_focused(SplitDirection::Vertical);
            false
        }
        ColonCommand::DeckNext => {
            wm.shuffle_deck_next();
            false
        }
        ColonCommand::DeckPrev => {
            wm.shuffle_deck_prev();
            false
        }
        ColonCommand::FocusPane(id) => {
            wm.focus_and_raise(id);
            false
        }
        ColonCommand::ZoomToggle => {
            wm.toggle_zoom();
            false
        }
        ColonCommand::ClosePane => {
            wm.close_focused_pane();
            false
        }
        ColonCommand::LayoutMode(mode) => {
            wm.layout_mode = mode;
            false
        }
        ColonCommand::QuakeToggle => {
            wm.toggle_quick_terminal();
            false
        }
        ColonCommand::HelpToggle => {
            wm.toggle_help();
            false
        }
        ColonCommand::HistoryToggle(val) => {
            let next = val.unwrap_or(!wm.config.history_enabled);
            wm.config.history_enabled = next;
            wm.sync_history_config();
            wm.active_pane_mut().status_message = Some(format!(
                "Command history persistence: {}",
                if next { "ON" } else { "OFF" }
            ));
            false
        }
        ColonCommand::Scrollback { clear } => {
            let note = if clear {
                if wm.active_pane_mut().vt.clear_scrollback() {
                    "History cleared. "
                } else {
                    "Cannot clear while the alternate screen is active. "
                }
            } else {
                ""
            };
            let report = wm.scrollback_report();
            wm.active_pane_mut().status_message = Some(format!("{note}{report}"));
            false
        }
        ColonCommand::HistoryMaxFiles(max_f) => {
            wm.config.history_max_files = max_f.max(1);
            wm.sync_history_config();
            wm.active_pane_mut().status_message =
                Some(format!("History max session files: {}", max_f.max(1)));
            false
        }
        ColonCommand::Theme(preset) => {
            wm.config.theme = preset;
            wm.config.custom_palette_override = None;
            wm.notify_theme_changed();
            false
        }
        ColonCommand::InspectorToggle(val) => {
            wm.inspector_visible = val.unwrap_or(!wm.inspector_visible);
            if wm.inspector_visible {
                wm.inspector_scroll = 0;
            }
            false
        }
        ColonCommand::BroadcastToggle(val) => {
            let next = val.unwrap_or(!wm.broadcast_input);
            wm.set_broadcast_input(next);
            false
        }
        ColonCommand::MenuBarToggle(val) => {
            wm.config.show_top_menubar = val.unwrap_or(!wm.config.show_top_menubar);
            if !wm.config.show_top_menubar {
                wm.menubar_open_category = None;
            }
            false
        }
        ColonCommand::SessionSave(name) => {
            match wm.save_session(&name) {
                Ok(path) => {
                    wm.active_pane_mut().status_message =
                        Some(format!("Saved session '{}' to {}", name, path.display()));
                }
                Err(err) => {
                    wm.active_pane_mut().status_message =
                        Some(format!("Session save error: {err}"));
                }
            }
            false
        }
        ColonCommand::SessionRestore(name) => {
            match wm.restore_session(&name) {
                Ok(count) => {
                    wm.active_pane_mut().status_message =
                        Some(format!("Restored session '{name}' ({count} pane(s))"));
                }
                Err(err) => {
                    wm.active_pane_mut().status_message =
                        Some(format!("Session restore error: {err}"));
                }
            }
            false
        }
        ColonCommand::SessionList => {
            let sessions = wm.list_sessions();
            wm.active_pane_mut().status_message = Some(if sessions.is_empty() {
                "Saved sessions: (none)".to_string()
            } else {
                format!("Saved sessions: {}", sessions.join(", "))
            });
            false
        }
        ColonCommand::CrtToggle(val) => {
            let next = val.unwrap_or(!wm.config.crt_scanlines);
            wm.config.crt_scanlines = next;
            wm.active_pane_mut().status_message = Some(format!(
                "CRT Phosphor Scanlines: {}",
                if next { "ON" } else { "OFF" }
            ));
            false
        }
        ColonCommand::PaletteToggle => {
            wm.toggle_palette();
            false
        }
        ColonCommand::TriggerAdd(pat) => {
            if !wm.config.triggers.contains(&pat) {
                wm.config.triggers.push(pat.clone());
            }
            wm.active_pane_mut().status_message = Some(format!(
                "Added output highlight trigger '{}' ({} active)",
                pat,
                wm.config.triggers.len()
            ));
            false
        }
        ColonCommand::TriggerClear => {
            wm.config.triggers.clear();
            wm.active_pane_mut().status_message =
                Some("Cleared all output highlight triggers".to_string());
            false
        }
        ColonCommand::SshConnect { target, port } => {
            match wm.spawn_ssh_pane(&target, port, Vec::new()) {
                Ok(new_id) => {
                    wm.active_pane_mut().status_message =
                        Some(format!("🌐 Connected SSH pane #{new_id} -> {target}"));
                }
                Err(err) => {
                    wm.active_pane_mut().status_message = Some(format!("SSH connect error: {err}"));
                }
            }
            false
        }
        ColonCommand::SshTmuxConnect { target, session } => {
            match wm.connect_ssh_tmux(&target, &session, Vec::new()) {
                Ok(new_id) => {
                    wm.active_pane_mut().status_message = Some(format!(
                        "🌐 Attached remote tmux -CC '{session}' on {target} (pane #{new_id})"
                    ));
                }
                Err(err) => {
                    wm.active_pane_mut().status_message =
                        Some(format!("SSH tmux connect error: {err}"));
                }
            }
            false
        }
        ColonCommand::RemoteAdd {
            name,
            destination,
            tags,
        } => {
            let mut profile = RemoteHostProfile::new(name.clone(), destination.clone());
            profile.tags = tags.clone();
            wm.fleet.register_profile(profile);
            wm.active_pane_mut().status_message = Some(format!(
                "Registered remote host '{name}' ({destination}) tags={tags:?}"
            ));
            false
        }
        ColonCommand::RemoteList => {
            let names: Vec<String> = wm.fleet.nodes.keys().cloned().collect();
            wm.active_pane_mut().status_message = Some(if names.is_empty() {
                "Fleet nodes: (local only)".to_string()
            } else {
                format!("Fleet nodes ({}): {}", names.len(), names.join(", "))
            });
            false
        }
        ColonCommand::RemoteExec { selector, command } => {
            let sel = FleetSelector::parse(&selector);
            let panes = wm.exec_fleet_command(&sel, &command);
            let nodes = wm.fleet.matching_nodes(&sel);
            wm.active_pane_mut().status_message = Some(format!(
                "🌐 Fleet exec [{}] -> {} pane(s), {} node(s)",
                selector,
                panes.len(),
                nodes.len()
            ));
            false
        }
        ColonCommand::RemoteDisconnect(name) => {
            let removed = wm.fleet.disconnect(&name);
            wm.active_pane_mut().status_message = Some(if removed {
                format!("Disconnected remote fleet node '{name}'")
            } else {
                format!("Remote fleet node '{name}' not found")
            });
            false
        }
        ColonCommand::Glyphs(pack) => {
            wm.config.glyphs = pack;
            false
        }
        ColonCommand::Cursor(style) => {
            wm.config.cursor_style = style;
            false
        }
        ColonCommand::Outline { window, prompt } => {
            if let Some(w) = window {
                wm.config.window_outline = w;
            }
            if let Some(p) = prompt {
                wm.config.prompt_outline = p;
            }
            false
        }
        ColonCommand::CrosshairToggle {
            enabled,
            fullscreen,
        } => {
            wm.config.crosshair_cursor = enabled.unwrap_or(!wm.config.crosshair_cursor);
            if let Some(fs) = fullscreen {
                wm.config.crosshair_fullscreen = fs;
            }
            false
        }
        ColonCommand::Spotlight {
            mode,
            pixelated,
            fullscreen,
            rx,
            ry,
            delta,
            feather_pct,
        } => {
            let no_args = mode.is_none()
                && pixelated.is_none()
                && fullscreen.is_none()
                && rx.is_none()
                && ry.is_none()
                && delta.is_none()
                && feather_pct.is_none();
            if no_args {
                let label = wm.config.cycle_spotlight();
                wm.active_pane_mut().status_message = Some(format!(
                    "🔦 Focal Spotlight: {} (rx={}, ry={}, feather={}%)",
                    label,
                    wm.config.spotlight_rx,
                    wm.config.spotlight_ry,
                    wm.config.spotlight_feather_pct
                ));
                return false;
            }
            if let Some(m) = mode {
                wm.config.spotlight_mode = m;
            } else if wm.config.spotlight_mode == SpotlightMode::Off {
                wm.config.spotlight_mode = SpotlightMode::Ellipse;
            }
            if let Some(px) = pixelated {
                wm.config.spotlight_pixelated = px;
            }
            if let Some(fs) = fullscreen {
                wm.config.spotlight_fullscreen = fs;
            }
            if let Some(rx_val) = rx {
                wm.config.spotlight_rx = rx_val.clamp(4, 120);
                if ry.is_none() && wm.config.spotlight_mode == SpotlightMode::Circle {
                    wm.config.spotlight_ry = (rx_val / 2).clamp(2, 60);
                }
            }
            if let Some(ry_val) = ry {
                wm.config.spotlight_ry = ry_val.clamp(2, 60);
            }
            if let Some((drx, dry)) = delta {
                wm.config.resize_spotlight(drx, dry);
            }
            if let Some(f) = feather_pct {
                wm.config.spotlight_feather_pct = f.clamp(5, 100);
            }
            let label = wm
                .config
                .spotlight_mode
                .label(wm.config.spotlight_pixelated);
            wm.active_pane_mut().status_message = Some(format!(
                "🔦 Focal Spotlight: {} (rx={}, ry={}, feather={}%)",
                label,
                wm.config.spotlight_rx,
                wm.config.spotlight_ry,
                wm.config.spotlight_feather_pct
            ));
            false
        }
        ColonCommand::FocalZoom {
            mode,
            zoom_pct,
            step_pct,
            pixelated,
            fullscreen,
            rx,
            ry,
            delta_size,
            feather_pct,
        } => {
            let no_args = mode.is_none()
                && zoom_pct.is_none()
                && step_pct.is_none()
                && pixelated.is_none()
                && fullscreen.is_none()
                && rx.is_none()
                && ry.is_none()
                && delta_size.is_none()
                && feather_pct.is_none();
            if no_args {
                let label = wm.config.cycle_focal_zoom();
                wm.active_pane_mut().status_message = Some(format!(
                    "🔍 Focal Zoom: {} (rx={}, ry={}, feather={}%)",
                    label,
                    wm.config.focal_zoom_rx,
                    wm.config.focal_zoom_ry,
                    wm.config.focal_zoom_feather_pct
                ));
                return false;
            }
            if let Some(m) = mode {
                wm.config.focal_zoom_mode = m;
            } else if wm.config.focal_zoom_mode == SpotlightMode::Off && step_pct.is_none() {
                wm.config.focal_zoom_mode = SpotlightMode::Ellipse;
            }
            if let Some(pct) = zoom_pct {
                wm.config.focal_zoom_pct = pct.clamp(105, 400);
            }
            if let Some(delta_p) = step_pct {
                wm.config.step_focal_zoom_level(delta_p);
            }
            if let Some(px) = pixelated {
                wm.config.focal_zoom_pixelated = px;
            }
            if let Some(fs) = fullscreen {
                wm.config.focal_zoom_fullscreen = fs;
            }
            if let Some(rx_val) = rx {
                wm.config.focal_zoom_rx = rx_val.clamp(4, 120);
                if ry.is_none() && wm.config.focal_zoom_mode == SpotlightMode::Circle {
                    wm.config.focal_zoom_ry = (rx_val / 2).clamp(2, 60);
                }
            }
            if let Some(ry_val) = ry {
                wm.config.focal_zoom_ry = ry_val.clamp(2, 60);
            }
            if let Some((drx, dry)) = delta_size {
                wm.config.resize_focal_zoom(drx, dry);
            }
            if let Some(f) = feather_pct {
                wm.config.focal_zoom_feather_pct = f.clamp(5, 100);
            }
            let label = wm.config.focal_zoom_label();
            wm.active_pane_mut().status_message = Some(format!(
                "🔍 Focal Zoom: {} (rx={}, ry={}, feather={}%)",
                label,
                wm.config.focal_zoom_rx,
                wm.config.focal_zoom_ry,
                wm.config.focal_zoom_feather_pct
            ));
            false
        }
        ColonCommand::Shadows(style_opt) => {
            match style_opt {
                Some(ShadowStyle::None) => {
                    wm.config.shadow_style = ShadowStyle::None;
                    wm.config.drop_shadows = false;
                }
                Some(style) => {
                    wm.config.shadow_style = style;
                    wm.config.drop_shadows = true;
                }
                None => {
                    wm.config.drop_shadows = !wm.config.drop_shadows;
                    if wm.config.drop_shadows && wm.config.shadow_style == ShadowStyle::None {
                        wm.config.shadow_style = ShadowStyle::Translucent;
                    }
                }
            }
            false
        }
        ColonCommand::FKeyBarToggle(val) => {
            wm.config.show_dos_fkey_bar = val.unwrap_or(!wm.config.show_dos_fkey_bar);
            false
        }
        other => execute_colon_command(wm.active_pane_mut(), other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_executes_colon_commands_without_dispatching_to_pty() {
        let mut pane = CompositePane::new_headless(1, 25, 80);

        let cmd = parse_colon_command(":prompt bottom").unwrap().unwrap();
        assert!(!execute_colon_command(&mut pane, cmd));
        assert_eq!(pane.prompt.anchor, PromptAnchor::Bottom);

        let cmd = parse_colon_command(":prompt float 7").unwrap().unwrap();
        assert!(!execute_colon_command(&mut pane, cmd));
        assert_eq!(pane.prompt.anchor, PromptAnchor::Floating { y_offset: 7 });

        let cmd = parse_colon_command(":prompt height 6").unwrap().unwrap();
        assert!(!execute_colon_command(&mut pane, cmd));
        assert_eq!(pane.prompt.height, 6);

        pane.vt.record_command_submission("cargo test");
        let cmd = parse_colon_command(":yank").unwrap().unwrap();
        assert!(!execute_colon_command(&mut pane, cmd));
        assert!(pane.last_osc52_sequence.is_some());
    }

    #[test]
    fn history_runs_parses_and_reports_segments() {
        assert_eq!(
            parse_colon_command(":history runs").unwrap().unwrap(),
            ColonCommand::HistoryRuns
        );
        assert_eq!(
            parse_colon_command(":hist segments").unwrap().unwrap(),
            ColonCommand::HistoryRuns
        );

        let tmp = tempfile::tempdir().unwrap();
        let mut pane = CompositePane::new_headless(1, 25, 80);
        pane.history_store.base_dir = tmp.path().to_path_buf();
        pane.history_store.enabled = true;
        pane.history_store.session_file_path = tmp.path().join("sessions/default/history.log");
        assert!(!execute_colon_command(&mut pane, ColonCommand::HistoryRuns));
        assert!(pane.status_message.as_deref().unwrap().contains("no recorded runs"));

        pane.history_store.run_id = "run-1".to_string();
        pane.history_store.record_command("ls").unwrap();
        pane.history_store.run_id = "run-2".to_string();
        pane.history_store.record_command("pwd").unwrap();
        pane.history_store.record_command("id").unwrap();
        execute_colon_command(&mut pane, ColonCommand::HistoryRuns);
        let msg = pane.status_message.clone().unwrap();
        assert!(msg.contains("2 runs"), "{msg}");
        assert!(msg.find("run-2").unwrap() < msg.find("run-1").unwrap(), "{msg}");
        assert!(msg.contains("(2 cmds)"), "{msg}");
    }

    #[test]
    fn spotlight_colon_commands_configure_shape_pixelation_and_size() {
        let mut wm = WindowManager::new_headless(25, 80);
        assert_eq!(wm.config.spotlight_mode, SpotlightMode::Off);

        // Cycle via bare :spotlight
        let cmd = parse_colon_command(":spotlight").unwrap().unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config.spotlight_mode, SpotlightMode::Circle);
        assert!(!wm.config.spotlight_pixelated);

        // Configure elliptical pixelated spotlight with custom size and feather
        let cmd = parse_colon_command(":spotlight ellipse pixel size 30 8 feather 60")
            .unwrap()
            .unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config.spotlight_mode, SpotlightMode::Ellipse);
        assert!(wm.config.spotlight_pixelated);
        assert_eq!(wm.config.spotlight_rx, 30);
        assert_eq!(wm.config.spotlight_ry, 8);
        assert_eq!(wm.config.spotlight_feather_pct, 60);

        // Grow and shrink via :spotlight + and :spotlight -
        let cmd = parse_colon_command(":spotlight +").unwrap().unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config.spotlight_rx, 34);
        assert_eq!(wm.config.spotlight_ry, 9);

        let cmd = parse_colon_command(":spotlight off").unwrap().unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config.spotlight_mode, SpotlightMode::Off);

        // Cycle Focal Zoom levels: Off -> 1.25x -> 1.50x -> 2.00x -> 3.00x -> Off
        for expected_pct in [125u16, 150, 200, 300] {
            let cmd = parse_colon_command(":focalzoom").unwrap().unwrap();
            execute_wm_colon_command(&mut wm, cmd);
            assert_eq!(wm.config.focal_zoom_mode, SpotlightMode::Ellipse);
            assert_eq!(wm.config.focal_zoom_pct, expected_pct);
        }
        let cmd = parse_colon_command(":focalzoom").unwrap().unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config.focal_zoom_mode, SpotlightMode::Off);

        // Configure 2.5x circular pixelated focal zoom with custom size and step
        let cmd = parse_colon_command(":focalzoom 2.5x circle pixel size 24 8 feather 55")
            .unwrap()
            .unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config.focal_zoom_mode, SpotlightMode::Circle);
        assert_eq!(wm.config.focal_zoom_pct, 250);
        assert!(wm.config.focal_zoom_pixelated);
        assert_eq!(wm.config.focal_zoom_rx, 24);
        assert_eq!(wm.config.focal_zoom_ry, 8);
        assert_eq!(wm.config.focal_zoom_feather_pct, 55);
    }
}
