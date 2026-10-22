use crate::pane::{CompositePane, PromptAnchor};
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::CaretStyle;
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
    PromptFloat(u16),
    PromptHeight(u16),
    PromptWidth(Option<u16>),
    BlockPrev,
    BlockNext,
    Yank,
    Clear,
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
            Some("split-top") | Some("splittop") => Some(Ok(ColonCommand::PromptSplitTop)),
            Some("split-bottom") | Some("split-bot") | Some("splitbottom") => {
                Some(Ok(ColonCommand::PromptSplitBottom))
            }
            Some("split") => Some(Ok(ColonCommand::PromptSplitToggle)),
            Some("float") => {
                let row = parts
                    .get(2)
                    .and_then(|s| s.parse::<u16>().ok())
                    .unwrap_or(4);
                Some(Ok(ColonCommand::PromptFloat(row)))
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
                "Usage: :prompt top | bottom | split-top | split-bottom | split | float [row] | height <3..12> | width <cols|full>"
                    .to_string(),
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
            Some("max-files" | "files") => match parts.get(2).and_then(|s| s.parse::<usize>().ok()) {
                Some(n) => Some(Ok(ColonCommand::HistoryMaxFiles(n.max(1)))),
                None => Some(Err("Usage: :history max-files <count>".to_string())),
            },
            _ => Some(Err(
                "Usage: :history [popup | save [path] | on | off | max-files <N>]".to_string(),
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
        "yank" => Some(Ok(ColonCommand::Yank)),
        "clear" => Some(Ok(ColonCommand::Clear)),
        "raw" => Some(Ok(ColonCommand::RawToggle)),
        "quit" | "q" => Some(Ok(ColonCommand::Quit)),
        _ => None,
    }
}

/// Executes a parsed `ColonCommand` against a single `pane`. Returns `true` if the application should quit.
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
}
