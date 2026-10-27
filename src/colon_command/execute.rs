use crate::colon_command::ColonCommand;
use crate::pane::history_store::HistorySegment;
use crate::pane::{CompositePane, PromptAnchor};
use crate::vt::encode_osc52_clipboard;

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

/// Executes a parsed `ColonCommand` against a single `pane`. Returns `true` if the application should quit.
pub fn execute_colon_command(pane: &mut CompositePane, cmd: ColonCommand) -> bool {
    match cmd {
        ColonCommand::PromptTop => {
            pane.set_prompt_anchor(PromptAnchor::Top);
            pane.set_status_message(Some("Prompt pinned to TOP".to_string()));
            false
        }
        ColonCommand::PromptBottom => {
            pane.set_prompt_anchor(PromptAnchor::Bottom);
            pane.set_status_message(Some("Prompt pinned to BOTTOM".to_string()));
            false
        }
        ColonCommand::PromptSplitTop => {
            pane.set_prompt_anchor(PromptAnchor::SplitTop);
            pane.set_status_message(Some("Prompt split at TOP (non-overlapping)".to_string()));
            false
        }
        ColonCommand::PromptSplitBottom => {
            pane.set_prompt_anchor(PromptAnchor::SplitBottom);
            pane.set_status_message(Some("Prompt split at BOTTOM (non-overlapping)".to_string()));
            false
        }
        ColonCommand::PromptSplitToggle => {
            pane.toggle_split_prompt();
            pane.set_status_message(Some("Toggled split/floating prompt mode".to_string()));
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
            pane.set_status_message(Some(format!("Prompt floating at row +{y_offset}")));
            false
        }
        ColonCommand::PromptHeight(rows) => {
            pane.set_prompt_height(rows);
            pane.set_status_message(Some(format!("Prompt height set to {rows} rows")));
            false
        }
        ColonCommand::PromptWidth(cols_opt) => {
            pane.set_prompt_width(cols_opt);
            pane.set_status_message(Some(match cols_opt {
                Some(c) => format!("Prompt width set to {c} cols"),
                None => "Prompt width reset to full width".to_string(),
            }));
            false
        }
        ColonCommand::PromptMoveVertical(delta) => {
            let h = pane.vt().size().0 + 1;
            pane.move_prompt_vertical(delta, h);
            false
        }
        ColonCommand::PromptResizeHeight(delta) => {
            pane.resize_prompt_height(delta);
            false
        }
        ColonCommand::PromptResizeWidth(delta) => {
            pane.resize_prompt_width(delta);
            false
        }
        ColonCommand::SearchPromptPrefill => {
            if pane.is_prompt_visible() {
                pane.prompt_mut().set_text(":search ");
            }
            false
        }
        ColonCommand::HistoryPopupToggle => {
            pane.prompt_mut().toggle_history_popup();
            false
        }
        ColonCommand::HistorySave(path_opt) => {
            let custom = path_opt.as_deref().map(std::path::Path::new);
            match pane.export_history_to_temp_file(custom) {
                Ok(saved_path) => {
                    pane.set_status_message(
                        Some(format!("Saved history to {}", saved_path.display())),
                    );
                }
                Err(err) => {
                    pane.set_status_message(Some(format!("History save error: {err}")));
                }
            }
            false
        }
        ColonCommand::HistoryToggle(val) => {
            let next = val.unwrap_or(!pane.history_store().enabled);
            pane.history_store_mut().enabled = next;
            pane.set_status_message(Some(format!(
                "Command history persistence: {}",
                if next { "ON" } else { "OFF" }
            )));
            false
        }
        ColonCommand::HistoryMaxFiles(max_f) => {
            pane.history_store_mut().max_files = max_f.max(1);
            let _ = pane.history_store().prune_excess_files();
            pane.set_status_message(Some(format!("History max session files: {}", max_f.max(1))));
            false
        }
        ColonCommand::HistoryRuns => {
            let msg = format_history_runs(
                &pane.history_store().session_name,
                &pane.history_store().segments(),
            );
            pane.set_status_message(Some(msg));
            false
        }
        ColonCommand::BlockPrev => {
            let msg = if let Some(block) = pane.vt_mut().jump_to_prev_command_block() {
                format!("Block #{}: {}", block.index + 1, block.command)
            } else {
                "No command blocks in scrollback".to_string()
            };
            pane.set_status_message(Some(msg));
            false
        }
        ColonCommand::BlockNext => {
            let msg = if let Some(block) = pane.vt_mut().jump_to_next_command_block() {
                format!("Block #{}: {}", block.index + 1, block.command)
            } else {
                "Returned to live output".to_string()
            };
            pane.set_status_message(Some(msg));
            false
        }
        ColonCommand::Yank => {
            let text_to_yank = if let Some(block) = pane.vt().selected_command_block() {
                block.command.clone()
            } else if let Some(last) = pane.vt().command_blocks().last() {
                last.command.clone()
            } else {
                pane.vt().contents()
            };
            let seq = encode_osc52_clipboard(&text_to_yank);
            pane.set_last_osc52_sequence(Some(seq));
            pane.set_status_message(Some(format!("Yanked {} bytes via OSC 52", text_to_yank.len())));
            false
        }
        ColonCommand::Clear => {
            pane.process_pty_output(b"\x1b[2J\x1b[H");
            pane.vt_mut().reset_scrollback();
            pane.set_status_message(Some("Canvas cleared".to_string()));
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
            pane.set_status_message(Some("Cleared canvas search".to_string()));
            false
        }
        ColonCommand::CopyModeToggle => {
            if pane.copy_mode().active {
                pane.exit_copy_mode();
            } else {
                pane.enter_copy_mode();
            }
            false
        }
        ColonCommand::LinksYank => {
            let link = pane
                .vt()
                .detect_visible_urls()
                .last()
                .map(|u| u.url.clone())
                .or_else(|| pane.vt().osc8_links().last().map(|l| l.uri.clone()));
            if let Some(url) = link {
                let seq = encode_osc52_clipboard(&url);
                pane.set_last_osc52_sequence(Some(seq));
                pane.prompt_mut().yank_buffer = url.clone();
                pane.set_status_message(Some(format!("Yanked URL: {url}")));
            } else {
                pane.set_status_message(Some("No URLs detected on canvas".to_string()));
            }
            false
        }
        ColonCommand::HintsToggle => {
            if pane.hints_mode().active {
                pane.exit_hints_mode();
            } else {
                pane.enter_hints_mode();
            }
            false
        }
        ColonCommand::LockToggle(val) => {
            let next = val.unwrap_or(!pane.is_read_only());
            pane.set_read_only(next);
            false
        }
        ColonCommand::VersionToggle(val) => {
            let next = val.unwrap_or(!pane.show_version_badge());
            pane.set_show_version_badge(next);
            pane.set_status_message(Some(format!(
                "Bilterm v{} — status bar version badge: {}",
                env!("CARGO_PKG_VERSION"),
                if next { "ON" } else { "OFF" }
            )));
            false
        }
        ColonCommand::NotificationsClear => {
            let count = pane.vt().notifications().len();
            pane.vt_mut().clear_notifications();
            pane.set_status_message(Some(format!("Cleared {count} notification(s)")));
            false
        }
        ColonCommand::RecordStart(path_opt)
        | ColonCommand::RecordWorkspaceStart(path_opt)
        | ColonCommand::RecordPaneStart(path_opt) => {
            if let Some(p) = path_opt {
                let path = std::path::PathBuf::from(p);
                pane.vt_mut().start_recording(path.clone());
                pane.set_status_message(
                    Some(format!("⏺ Recording asciicast v2 to {}", path.display())),
                );
            } else {
                match crate::secure_fs::private_runtime_dir()
                    .and_then(|d| crate::secure_fs::private_dir_in(&d, "recordings"))
                {
                    Ok(dir) => {
                        let path = dir.join(format!(
                            "bilterm-pane-{}-{}.cast",
                            pane.id(),
                            crate::secure_fs::unique_suffix()
                        ));
                        pane.vt_mut().start_recording_private(path.clone());
                        pane.set_status_message(
                            Some(format!("⏺ Recording asciicast v2 to {}", path.display())),
                        );
                    }
                    Err(e) => {
                        pane.set_status_message(
                            Some(format!("⏺ Recording not started: private dir unavailable: {e}")),
                        );
                    }
                }
            }
            false
        }
        ColonCommand::RecordStop => {
            match pane.vt_mut().stop_recording() {
                Ok(Some(path)) => {
                    pane.set_status_message(
                        Some(format!("Saved asciicast v2 recording to {}", path.display())),
                    );
                }
                Ok(None) => {
                    pane.set_status_message(Some("No active recording on this pane".to_string()));
                }
                Err(err) => {
                    pane.set_status_message(Some(format!("Recording save error: {err}")));
                }
            }
            false
        }
        ColonCommand::CanvasCommand(ref subcmd) => {
            let _ = pane.execute_canvas_command(subcmd);
            false
        }
        ColonCommand::Quit => true,
        _ => false,
    }
}

