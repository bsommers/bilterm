use crate::pane::{CompositePane, PromptAnchor};
use crate::vt::encode_osc52_clipboard;
use crate::wm::split_tree::SplitDirection;
use crate::wm::{WindowManager, WorkspaceLayoutMode};

/// Parsed in-prompt `:command` (`PANE-03`, `PANE-05`, `PTY-04`, `WM-01..05`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColonCommand {
    PromptTop,
    PromptBottom,
    PromptFloat(u16),
    PromptHeight(u16),
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
            _ => Some(Err(
                "Usage: :prompt top | bottom | float [row] | height <3..12>".to_string(),
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
