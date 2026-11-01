use crate::colon_command::ColonCommand;
use crate::pane::history_store::HistorySegment;
use crate::pane::{CompositePane, PromptAnchor};

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
            if pane.copy_mode().has_selection() {
                let _ = pane.yank_copy_selection();
                return false;
            }
            if pane.prompt().has_selection()
                && let Some(copied) = pane.prompt_mut().copy_selection()
            {
                pane.record_yank(&copied);
                pane.prompt_mut().clear_selection();
                pane.set_status_message(Some(format!(
                    "Yanked {} chars from prompt to clipboard",
                    copied.chars().count()
                )));
                return false;
            }
            let text_to_yank = if let Some(block) = pane.vt().selected_command_block() {
                block.command.clone()
            } else if let Some(last) = pane.vt().command_blocks().last() {
                last.command.clone()
            } else {
                pane.vt().contents()
            };
            pane.record_yank(&text_to_yank);
            pane.set_status_message(Some(format!("Yanked {} bytes via OSC 52", text_to_yank.len())));
            false
        }
        ColonCommand::PasteClipboard => {
            let _ = crate::input::paste_clipboard_into_pane(pane);
            false
        }
        ColonCommand::Clear => {
            pane.process_pty_output(b"\x1b[2J\x1b[H");
            pane.vt_mut().reset_scrollback();
            pane.reset_page_anchor();
            pane.set_status_message(Some("Canvas cleared".to_string()));
            false
        }
        ColonCommand::ScrollModeSet(mode_opt) => {
            match mode_opt {
                Some(mode) => pane.set_scroll_mode(mode),
                None => {
                    pane.cycle_scroll_mode();
                }
            }
            false
        }
        ColonCommand::ScrollModeTogglePage => {
            pane.toggle_block_page_scroll_mode();
            false
        }
        ColonCommand::ScrollPageAdvance => {
            if !pane.scroll_page_advance() {
                pane.set_status_message(Some("Already at live output tail".to_string()));
            }
            false
        }
        ColonCommand::ScrollPageBack => {
            let _ = pane.scroll_page_back();
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
        ColonCommand::SelectCommand(ref subcmd) => {
            pane.execute_select_command(subcmd.as_deref().unwrap_or(""));
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
                pane.record_yank(&url);
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
        ColonCommand::RespawnPaneInPlace => {
            let _ = pane.respawn_shell_in_place();
            false
        }
        ColonCommand::Quit => true,
        _ => false,
    }
}

impl crate::wm::WindowManager {
    /// Executes `:rect`, `:select`, `:copy [auto on|off]`, `:clipboard [...]`, or `:paste [...]` commands.
    pub fn execute_select_or_clipboard_command(&mut self, subcmd_opt: Option<&str>) {
        let raw = subcmd_opt.unwrap_or("").trim();
        let parts: Vec<&str> = raw.split_whitespace().collect();
        let first = parts.first().map(|s| s.to_ascii_lowercase()).unwrap_or_default();

        if matches!(first.as_str(), "auto" | "copy-on-select" | "explicit") {
            let next = match parts.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
                Some("on" | "true" | "enable" | "auto") => first != "explicit",
                Some("off" | "false" | "disable" | "explicit") => first == "explicit",
                _ => !self.config().copy_on_select,
            };
            self.config_mut().copy_on_select = next;
            self.active_pane_mut().set_status_message(Some(format!(
                "Copy Mode: {} (copy_on_select={})",
                if next {
                    "AUTO-COPY ON MOUSE SELECT"
                } else {
                    "EXPLICIT COPY (Enter/y/Ctrl+C required to copy selection)"
                },
                next
            )));
            return;
        }

        if first == "clipboard" {
            let sub = parts.get(1).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            match sub.as_str() {
                "persist" | "save" => {
                    let next = match parts.get(2).map(|s| s.to_ascii_lowercase()).as_deref() {
                        Some("on" | "true" | "enable") => true,
                        Some("off" | "false" | "disable") => false,
                        _ => !self.config().clipboard_history_persist,
                    };
                    self.config_mut().clipboard_history_persist = next;
                    self.sync_history_config();
                    if next {
                        let ring = self.active_pane().prompt().yank_history.clone();
                        for entry in ring.iter().rev() {
                            let _ = self
                                .active_pane_mut()
                                .history_store_mut()
                                .record_clipboard_entry(entry);
                        }
                    }
                    let sess = self.config().history_session.clone();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Clipboard history persistence for session '{sess}': {}",
                        if next { "ON" } else { "OFF" }
                    )));
                }
                "clear" | "reset" => {
                    self.clear_session_clipboard_history();
                    self.active_pane_mut().set_status_message(Some(
                        "Cleared cut-and-paste buffer history for active session".to_string(),
                    ));
                }
                "copy" | "add" | "push" => {
                    let text = parts.get(2..).map(|s| s.join(" ")).unwrap_or_default();
                    if !text.is_empty() {
                        self.active_pane_mut().record_yank(&text);
                        self.active_pane_mut().set_status_message(Some(format!(
                            "Added {} chars to session clipboard history (#1)",
                            text.chars().count()
                        )));
                    }
                }
                "pop" | "cycle" | "next" => {
                    if let Some(entry) = self.active_pane_mut().prompt_mut().yank_pop() {
                        let idx = self.active_pane().prompt().yank_ring_index + 1;
                        let total = self.active_pane().prompt().yank_history.len();
                        self.active_pane_mut().set_status_message(Some(format!(
                            "Clipboard Ring [{idx}/{total}]: {}",
                            entry.replace('\n', " ↵ ")
                        )));
                    } else {
                        self.active_pane_mut()
                            .set_status_message(Some("Clipboard history is empty".to_string()));
                    }
                }
                "paste" => {
                    let idx_1 = parts
                        .get(2)
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    self.paste_clipboard_history_index(idx_1 - 1);
                }
                other if other.parse::<usize>().is_ok() => {
                    let idx_1 = other.parse::<usize>().unwrap_or(1).max(1);
                    self.paste_clipboard_history_index(idx_1 - 1);
                }
                _ => {
                    let sess = self.config().history_session.clone();
                    let persist = if self.config().clipboard_history_persist {
                        "ON"
                    } else {
                        "OFF"
                    };
                    let ring = self.active_pane().prompt().yank_history.clone();
                    let mut lines = vec![
                        format!(
                            "◈ SESSION '{sess}' CUT & PASTE BUFFER HISTORY ({} item(s), PERSIST: {persist})",
                            ring.len()
                        ),
                        "  Up/Down + Enter (or click) to paste entry | :clipboard persist [on|off] | :clipboard clear"
                            .to_string(),
                    ];
                    if ring.is_empty() {
                        lines.push("  (No cut/copy entries recorded yet in this session)".to_string());
                    } else {
                        for (i, item) in ring.iter().enumerate() {
                            let preview: String = item.replace('\n', " ↵ ").chars().take(64).collect();
                            lines.push(format!(
                                "  • clipboard paste {} — [{} chars] {}",
                                i + 1,
                                item.chars().count(),
                                preview
                            ));
                        }
                    }
                    self.open_mcp_popup_window("CUT & PASTE BUFFER HISTORY", lines);
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Session '{sess}' clipboard history: {} entry(s) [persist={persist}]",
                        ring.len()
                    )));
                }
            }
            return;
        }

        self.active_pane_mut().execute_select_command(raw);
    }

    fn paste_clipboard_history_index(&mut self, idx: usize) {
        let mut ring = self.active_pane().prompt().yank_history.clone();
        if ring.is_empty() {
            ring = self.active_pane().history_store().load_recent_clipboard(50);
        }
        let Some(entry) = ring.get(idx).cloned() else {
            self.active_pane_mut().set_status_message(Some(format!(
                "Clipboard history entry #{} not found ({} available)",
                idx + 1,
                ring.len()
            )));
            return;
        };
        let pane = self.active_pane_mut();
        pane.prompt_mut().yank_buffer = entry.clone();
        crate::clipboard::set_system_clipboard(&entry);
        let _ = crate::input::handle_paste_event(pane, &entry);
        pane.set_status_message(Some(format!(
            "Pasted clipboard history #{} ({} chars)",
            idx + 1,
            entry.chars().count()
        )));
    }
}

