//! Multi-pane `WindowManager` `:colon` command executor (`execute_wm_colon_command`, `ARCH-R01`).

use crate::ui::typography::PanelTarget;
use crate::colon_command::ColonCommand;
use crate::colon_command::execute::execute_colon_command;
use crate::remote::{FleetSelector, RemoteHostProfile};
use crate::ui::chrome::ShadowStyle;
use crate::ui::cursor::SpotlightMode;
use crate::wm::WindowManager;
use crate::wm::split_tree::SplitDirection;

/// Resolves a user-supplied `:record` path. Under automation confinement only a plain file name
/// is accepted and it is placed inside `confine`; interactive (unconfined) paths pass through.
pub(crate) fn resolve_recording_path(
    confine: Option<&std::path::Path>,
    p: &str,
) -> Result<std::path::PathBuf, String> {
    match confine {
        Some(dir) => crate::secure_fs::validate_plain_file_name(p).map(|()| dir.join(p)),
        None => Ok(std::path::PathBuf::from(p)),
    }
}

/// [`resolve_recording_path`] against `wm`'s confine dir; on rejection records the automation
/// error, shows it on the active pane, and returns `None`.
fn confined_recording_path(wm: &mut WindowManager, p: &str) -> Option<std::path::PathBuf> {
    match resolve_recording_path(wm.automation_confine_dir(), p) {
        Ok(path) => Some(path),
        Err(e) => {
            wm.record_automation_error(e.clone());
            wm.active_pane_mut().set_status_message(Some(e));
            None
        }
    }
}

/// Executes a parsed `ColonCommand` against the multi-pane `WindowManager`.
pub fn execute_wm_colon_command(wm: &mut WindowManager, cmd: ColonCommand) -> bool {
    if wm.script_studio().recording {
        let colon_line = cmd.colon_form_or_summary();
        wm.script_studio_mut().record_colon_command(&colon_line);
    }
    match cmd {
        ColonCommand::SplitHorizontal => {
            if let Err(err) = wm.split_focused(SplitDirection::Horizontal) {
                wm.active_pane_mut()
                    .set_status_message(Some(format!("Split error: {err}")));
            }
            false
        }
        ColonCommand::SplitVertical => {
            if let Err(err) = wm.split_focused(SplitDirection::Vertical) {
                wm.active_pane_mut()
                    .set_status_message(Some(format!("Split error: {err}")));
            }
            false
        }
        // An explicit pane-focus command hands the keyboard to the pane: the drawer and a pinned
        // MCP console are released too (the floating windows already are, FOCUS-01).
        ColonCommand::DeckNext => {
            wm.shuffle_deck_next();
            wm.release_keyboard();
            false
        }
        ColonCommand::DeckPrev => {
            wm.shuffle_deck_prev();
            wm.release_keyboard();
            false
        }
        ColonCommand::FocusPane(id) => {
            if wm.focus_and_raise(id) || wm.pane(id).is_some() {
                wm.release_keyboard();
            }
            false
        }
        ColonCommand::PaneFocusDirection { dx, dy } => {
            let _ = wm.focus_direction(dx, dy);
            wm.release_keyboard();
            false
        }
        ColonCommand::KeyboardFocus(op) => {
            let msg = wm.keyboard_focus_apply(op);
            wm.active_pane_mut().set_status_message(Some(msg));
            false
        }
        ColonCommand::SelectCommand(ref subcmd) => {
            wm.execute_select_or_clipboard_command(subcmd.as_deref());
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
            wm.set_layout_mode(mode);
            false
        }
        ColonCommand::LayoutCycle => {
            wm.toggle_layout_mode();
            false
        }
        ColonCommand::CrosshairCycle => {
            if !wm.config().crosshair_cursor {
                wm.config_mut().crosshair_cursor = true;
                wm.config_mut().crosshair_fullscreen = false;
            } else if !wm.config().crosshair_fullscreen {
                wm.config_mut().crosshair_fullscreen = true;
            } else {
                wm.config_mut().crosshair_cursor = false;
                wm.config_mut().crosshair_fullscreen = false;
            }
            false
        }
        ColonCommand::FocalRadiusResize(drx, dry) => {
            if wm.config().focal_zoom_mode != SpotlightMode::Off {
                wm.config_mut().resize_focal_zoom(drx, dry);
            }
            if wm.config().spotlight_mode == SpotlightMode::Off
                && wm.config().focal_zoom_mode == SpotlightMode::Off
            {
                wm.config_mut().spotlight_mode = SpotlightMode::Ellipse;
            }
            if wm.config().spotlight_mode != SpotlightMode::Off {
                wm.config_mut().resize_spotlight(drx, dry);
            }
            let label = wm
                .config()
                .spotlight_mode
                .label(wm.config().spotlight_pixelated);
            let s_rx = wm.config().spotlight_rx;
            let s_ry = wm.config().spotlight_ry;
            let z_rx = wm.config().focal_zoom_rx;
            let z_ry = wm.config().focal_zoom_ry;
            wm.active_pane_mut().set_status_message(Some(format!(
                "🔦 Focal Radius: spot=({s_rx}, {s_ry}) zoom=({z_rx}, {z_ry}) [{label}]"
            )));
            false
        }
        ColonCommand::RecordStart(path_opt) | ColonCommand::RecordWorkspaceStart(path_opt) => {
            if let Some(p) = path_opt {
                let Some(path) = confined_recording_path(wm, &p) else {
                    return false;
                };
                wm.start_workspace_recording(path.clone(), false);
                wm.active_pane_mut().vt_mut().start_recording(path.clone());
                wm.active_pane_mut().set_status_message(Some(format!(
                    "⏺ Recording workspace asciicast v2 to {}",
                    path.display()
                )));
            } else {
                match crate::secure_fs::private_runtime_dir()
                    .and_then(|d| crate::secure_fs::private_dir_in(&d, "recordings"))
                {
                    Ok(dir) => {
                        let path = dir.join(format!(
                            "bilterm-workspace-{}.cast",
                            crate::secure_fs::unique_suffix()
                        ));
                        wm.start_workspace_recording(path.clone(), true);
                        wm.active_pane_mut()
                            .vt_mut()
                            .start_recording_private(path.clone());
                        wm.active_pane_mut().set_status_message(Some(format!(
                            "⏺ Recording workspace asciicast v2 to {}",
                            path.display()
                        )));
                    }
                    Err(e) => {
                        wm.active_pane_mut().set_status_message(Some(format!(
                            "⏺ Recording not started: private dir unavailable: {e}"
                        )));
                    }
                }
            }
            false
        }
        ColonCommand::RecordPaneStart(Some(p)) => {
            let Some(path) = confined_recording_path(wm, &p) else {
                return false;
            };
            let pane = wm.active_pane_mut();
            pane.vt_mut().start_recording(path.clone());
            pane.set_status_message(Some(format!(
                "⏺ Recording asciicast v2 to {}",
                path.display()
            )));
            false
        }
        ColonCommand::RecordStop => {
            if wm.has_recorded_workspace_frames() {
                wm.active_pane_mut().vt_mut().cancel_recording();
                match wm.stop_workspace_recording() {
                    Ok(Some(path)) => {
                        wm.active_pane_mut().set_status_message(Some(format!(
                            "Saved workspace asciicast v2 recording to {}",
                            path.display()
                        )));
                    }
                    Ok(None) => {
                        wm.active_pane_mut().set_status_message(Some(
                            "No active recording on this workspace".to_string(),
                        ));
                    }
                    Err(err) => {
                        wm.active_pane_mut()
                            .set_status_message(Some(format!("Recording save error: {err}")));
                    }
                }
                false
            } else {
                let _ = wm.stop_workspace_recording();
                execute_colon_command(wm.active_pane_mut(), ColonCommand::RecordStop)
            }
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
            let next = val.unwrap_or(!wm.config().history_enabled);
            wm.config_mut().history_enabled = next;
            wm.sync_history_config();
            wm.active_pane_mut().set_status_message(Some(format!(
                "Command history persistence: {}",
                if next { "ON" } else { "OFF" }
            )));
            false
        }
        ColonCommand::Scrollback { clear } => {
            let note = if clear {
                if wm.active_pane_mut().vt_mut().clear_scrollback() {
                    "History cleared. "
                } else {
                    "Cannot clear while the alternate screen is active. "
                }
            } else {
                ""
            };
            let report = wm.scrollback_report();
            wm.active_pane_mut()
                .set_status_message(Some(format!("{note}{report}")));
            false
        }
        ColonCommand::ScrollModeSet(mode_opt) => {
            let resolved = match mode_opt {
                Some(mode) => {
                    wm.active_pane_mut().set_scroll_mode(mode);
                    mode
                }
                None => wm.active_pane_mut().cycle_scroll_mode(),
            };
            wm.config_mut().scroll_mode = resolved;
            false
        }
        ColonCommand::ScrollModeTogglePage => {
            let resolved = wm.active_pane_mut().toggle_block_page_scroll_mode();
            wm.config_mut().scroll_mode = resolved;
            false
        }
        ColonCommand::ScrollPageAdvance => {
            execute_colon_command(wm.active_pane_mut(), ColonCommand::ScrollPageAdvance)
        }
        ColonCommand::ScrollPageBack => {
            execute_colon_command(wm.active_pane_mut(), ColonCommand::ScrollPageBack)
        }
        ColonCommand::HistoryMaxFiles(max_f) => {
            wm.config_mut().history_max_files = max_f.max(1);
            wm.sync_history_config();
            wm.active_pane_mut()
                .set_status_message(Some(format!("History max session files: {}", max_f.max(1))));
            false
        }
        ColonCommand::Theme(preset) => {
            wm.apply_theme_preset(preset);
            false
        }
        ColonCommand::InspectorToggle(val) => {
            let next = val.unwrap_or(!wm.is_inspector_visible());
            wm.set_inspector_visible(next);
            false
        }
        ColonCommand::BroadcastToggle(val) => {
            let next = val.unwrap_or(!wm.is_broadcast_input());
            wm.set_broadcast_input(next);
            false
        }
        ColonCommand::MenuBarToggle(val) => {
            let next = val.unwrap_or(!wm.config().show_top_menubar);
            wm.config_mut().show_top_menubar = next;
            if !next {
                wm.set_menubar_open_category(None);
            }
            wm.reflow_default_viewport();
            false
        }
        ColonCommand::VersionToggle(val) => {
            let next = val.unwrap_or(!wm.config().show_version_badge);
            wm.config_mut().show_version_badge = next;
            for p in wm.panes_mut().values_mut() {
                p.set_show_version_badge(next);
            }
            wm.quick_terminal_mut().pane.set_show_version_badge(next);
            wm.active_pane_mut().set_status_message(Some(format!(
                "Bilterm v{} — status bar version badge: {}",
                env!("CARGO_PKG_VERSION"),
                if next { "ON" } else { "OFF" }
            )));
            false
        }
        ColonCommand::SessionSave(name) => {
            match wm.save_and_activate_session((!name.trim().is_empty()).then_some(name.as_str())) {
                Ok((saved_name, path)) => {
                    wm.active_pane_mut().set_status_message(
                        Some(format!("Saved session '{saved_name}' to {}", path.display())),
                    );
                }
                Err(err) => {
                    wm.active_pane_mut()
                        .set_status_message(Some(format!("Session save error: {err}")));
                }
            }
            false
        }
        ColonCommand::SessionRestore(name) => {
            let target = if name.trim().is_empty() {
                wm.config().history_session.clone()
            } else {
                name
            };
            match wm.restore_session(&target) {
                Ok(count) => {
                    let note = wm.take_restore_note().map(|n| format!(" — {n}")).unwrap_or_default();
                    wm.active_pane_mut().set_status_message(
                        Some(format!("Restored session '{target}' ({count} pane(s)){note}")),
                    );
                }
                Err(err) => {
                    wm.active_pane_mut()
                        .set_status_message(Some(format!("Session restore error: {err}")));
                }
            }
            false
        }
        ColonCommand::SessionList => {
            let sessions = wm.list_sessions();
            wm.active_pane_mut().set_status_message(Some(if sessions.is_empty() {
                "Saved sessions: (none)".to_string()
            } else {
                format!("Saved sessions: {}", sessions.join(", "))
            }));
            false
        }
        ColonCommand::CrtToggle(val) => {
            let next = val.unwrap_or(!wm.config().crt_scanlines);
            wm.config_mut().crt_scanlines = next;
            wm.active_pane_mut().set_status_message(Some(format!(
                "CRT Phosphor Scanlines: {}",
                if next { "ON" } else { "OFF" }
            )));
            false
        }
        ColonCommand::PaletteToggle => {
            if wm.command_window().pinned && wm.command_window().active {
                wm.command_window_mut().focused = true;
            } else {
                wm.toggle_palette();
            }
            false
        }
        ColonCommand::TriggerAdd(pat) => {
            if !wm.config().triggers.contains(&pat) {
                wm.config_mut().triggers.push(pat.clone());
            }
            let count = wm.config().triggers.len();
            wm.active_pane_mut().set_status_message(Some(format!(
                "Added output highlight trigger '{pat}' ({count} active)"
            )));
            false
        }
        ColonCommand::TriggerClear => {
            wm.config_mut().triggers.clear();
            wm.active_pane_mut()
                .set_status_message(Some("Cleared all output highlight triggers".to_string()));
            false
        }
        ColonCommand::SshConnect { target, port } => {
            match wm.spawn_ssh_pane(&target, port, Vec::new()) {
                Ok(new_id) => {
                    wm.active_pane_mut().set_status_message(
                        Some(format!("🌐 Connected SSH pane #{new_id} -> {target}")),
                    );
                }
                Err(err) => {
                    wm.active_pane_mut()
                        .set_status_message(Some(format!("SSH connect error: {err}")));
                }
            }
            false
        }
        ColonCommand::SshTmuxConnect { target, session } => {
            // The new pane already carries the honest `Connecting to ...` status.
            if let Err(err) = wm.connect_ssh_tmux(&target, &session, Vec::new()) {
                wm.active_pane_mut()
                    .set_status_message(Some(format!("SSH tmux connect error: {err}")));
            }
            false
        }
        ColonCommand::RemoteAdd {
            name,
            destination,
            tags,
        } => {
            if let Err(e) = crate::remote::validate_destination(&destination) {
                wm.active_pane_mut()
                    .set_status_message(Some(format!("Remote not added: {e}")));
                return false;
            }
            let mut profile = RemoteHostProfile::new(name.clone(), destination.clone());
            profile.tags = tags.clone();
            wm.fleet_mut().register_profile(profile);
            wm.active_pane_mut().set_status_message(Some(format!(
                "Registered remote host '{name}' ({destination}) tags={tags:?}"
            )));
            false
        }
        ColonCommand::RemoteList => {
            let names: Vec<String> = wm.fleet().nodes.keys().cloned().collect();
            let ssh_hosts = crate::remote::discover_ssh_config_hosts();
            let host_hint = if ssh_hosts.is_empty() {
                String::new()
            } else {
                format!(" | ~/.ssh/config hosts: {}", ssh_hosts.join(", "))
            };
            wm.active_pane_mut().set_status_message(Some(if names.is_empty() {
                format!("Fleet nodes: (local only){host_hint} — Connect via :ssh <user@host> [port]")
            } else {
                format!("Fleet nodes ({}): {}{host_hint}", names.len(), names.join(", "))
            }));
            false
        }
        ColonCommand::RemoteExec { selector, command } => {
            let sel = FleetSelector::parse(&selector);
            let panes = wm.exec_fleet_command(&sel, &command);
            let nodes = wm.fleet().matching_nodes(&sel);
            wm.active_pane_mut().set_status_message(Some(format!(
                "🌐 Fleet exec [{}] -> {} pane(s), {} node(s)",
                selector,
                panes.len(),
                nodes.len()
            )));
            false
        }
        ColonCommand::RemoteDisconnect(name) => {
            let removed = wm.disconnect_fleet_node(&name);
            wm.active_pane_mut().set_status_message(Some(if removed {
                format!("Disconnected remote fleet node '{name}'")
            } else {
                format!("Remote fleet node '{name}' not found")
            }));
            false
        }
        ColonCommand::Glyphs(pack) => {
            wm.config_mut().glyphs = pack;
            false
        }
        ColonCommand::Appearance(profile) => {
            wm.config_mut().appearance.profile = Some(profile);
            wm.config_mut().appearance.active_custom = None;
            wm.active_pane_mut()
                .set_status_message(Some(format!("Appearance: {}", profile.label())));
            false
        }
        ColonCommand::AppearanceManage(op) => {
            use crate::action::AppearanceOp;
            let res = match op {
                AppearanceOp::List => Ok(wm.config().appearance.list_profiles_text()),
                AppearanceOp::SelectName(name) => wm
                    .config_mut()
                    .appearance
                    .select_by_name(&name)
                    .map(|label| format!("Appearance: {label}")),
                AppearanceOp::Duplicate { src, new_name } => wm
                    .config_mut()
                    .appearance
                    .duplicate_profile(&src, &new_name),
                AppearanceOp::Modify { key, value } => wm
                    .config_mut()
                    .appearance
                    .modify_setting(&key, &value),
                AppearanceOp::Reset => {
                    wm.config_mut().appearance.reset();
                    Ok("Appearance reset to defaults".to_string())
                }
                AppearanceOp::Import(path) => {
                    let expanded = crate::ui::file_browser::expand_user_path(&path);
                    wm.config_mut()
                        .appearance
                        .import_file(&expanded)
                        .map(|label| format!("Imported appearance profile '{label}' from {path}"))
                }
                AppearanceOp::Export(path) => {
                    let expanded = crate::ui::file_browser::expand_user_path(&path);
                    wm.config().appearance.export_file(&expanded)
                }
            };
            match res {
                Ok(msg) => wm.active_pane_mut().set_status_message(Some(msg)),
                Err(err) => {
                    wm.record_automation_error_msg(err.clone());
                    wm.active_pane_mut().set_status_message(Some(err));
                }
            }
            false
        }
        ColonCommand::PanelScale(n) => {
            wm.set_panel_scale_pct(n);
            let note = if wm.gui_surface { "" } else { " (applies in the GUI only)" };
            let shown = wm.scale_setting(PanelTarget::Drawer);
            wm.active_pane_mut()
                .set_status_message(Some(format!("Panel scale: {shown}%{note}")));
            false
        }
        ColonCommand::PanelScaleOf(target, n) => {
            wm.set_scale(target, n);
            let note = if wm.gui_surface { "" } else { " (applies in the GUI only)" };
            let (name, shown) = (panel_name(target), wm.scale_setting(target));
            wm.active_pane_mut()
                .set_status_message(Some(format!("{name} scale: {shown}%{note}")));
            false
        }
        ColonCommand::PanelScaleStepOf(target, step) => {
            let delta = if step.abs() == 1 { step * 10 } else { step };
            let shown = wm.step_scale(target, delta);
            let note = if wm.gui_surface { "" } else { " (applies in the GUI only)" };
            let name = panel_name(target);
            wm.active_pane_mut()
                .set_status_message(Some(format!("{name} scale: {shown}%{note}")));
            false
        }
        ColonCommand::ChromeTitle(policy) => {
            wm.config_mut().appearance.title = policy;
            wm.active_pane_mut()
                .set_status_message(Some(format!("Pane titles: {} (non-Legacy looks)", policy.map_or("auto", |p| match p {
                    crate::ui::design::TitlePolicy::All => "all",
                    crate::ui::design::TitlePolicy::Focused => "focused",
                    crate::ui::design::TitlePolicy::None => "none",
                }))));
            false
        }
        ColonCommand::ChromeRing(on) => {
            wm.config_mut().appearance.focus_ring = on;
            wm.active_pane_mut()
                .set_status_message(Some(format!("Focus ring: {} (non-Legacy looks)", if on { "on" } else { "off" })));
            false
        }
        ColonCommand::Mode(mode) => {
            wm.config_mut().appearance.display_mode = mode;
            let note = if crate::ui::design::AppearanceProfile::resolve(wm.config())
                == crate::ui::design::AppearanceProfile::Legacy
            {
                " (takes effect on non-Legacy looks; try :appearance obsidian)"
            } else {
                ""
            };
            wm.active_pane_mut()
                .set_status_message(Some(if mode == crate::ui::design::DisplayMode::Zen && note.is_empty() {
                    "Zen: :mode standard to leave".to_string()
                } else {
                    format!("Mode: {}{note}", mode.label())
                }));
            false
        }
        ColonCommand::Cursor(style) => {
            let active_id = wm.active_pane_id();
            wm.pin_other_panes_cursor(active_id);
            wm.config_mut().cursor_style = style;
            wm.sync_active_pane_cursor_from_config();
            if wm.mcp_console().active && wm.mcp_console().title.contains("CURSOR") {
                let lines = crate::ui::cursor_cmd::build_cursor_catalog_lines(wm.config());
                wm.open_mcp_popup_window(
                    "CURSOR CUSTOMIZATION (SHAPES, BLINK, COLORS, GLYPHS)",
                    lines,
                );
            }
            let msg = format!(
                "Cursor updated (Window #{active_id}): style={}, blink={}, color={}",
                style.label(),
                wm.config().cursor_blink.label(),
                wm.config().cursor_color.as_deref().unwrap_or("theme")
            );
            wm.active_pane_mut().set_status_message(Some(msg));
            false
        }
        ColonCommand::Outline { window, prompt } => {
            if let Some(w) = window {
                wm.config_mut().window_outline = w;
            }
            if let Some(p) = prompt {
                wm.config_mut().prompt_outline = p;
            }
            false
        }
        ColonCommand::CrosshairToggle {
            enabled,
            fullscreen,
        } => {
            wm.config_mut().crosshair_cursor = enabled.unwrap_or(!wm.config().crosshair_cursor);
            if let Some(fs) = fullscreen {
                wm.config_mut().crosshair_fullscreen = fs;
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
            let label = if no_args {
                wm.config_mut().cycle_spotlight()
            } else {
                let cfg = wm.config_mut();
                if let Some(m) = mode {
                    cfg.spotlight_mode = m;
                } else if cfg.spotlight_mode == SpotlightMode::Off {
                    cfg.spotlight_mode = SpotlightMode::Ellipse;
                }
                if let Some(px) = pixelated {
                    cfg.spotlight_pixelated = px;
                }
                if let Some(fs) = fullscreen {
                    cfg.spotlight_fullscreen = fs;
                }
                if let Some(rx_val) = rx {
                    cfg.spotlight_rx = rx_val.clamp(4, 120);
                    if ry.is_none() && cfg.spotlight_mode == SpotlightMode::Circle {
                        cfg.spotlight_ry = (rx_val / 2).clamp(2, 60);
                    }
                }
                if let Some(ry_val) = ry {
                    cfg.spotlight_ry = ry_val.clamp(2, 60);
                }
                if let Some((drx, dry)) = delta {
                    cfg.resize_spotlight(drx, dry);
                }
                if let Some(f) = feather_pct {
                    cfg.spotlight_feather_pct = f.clamp(5, 100);
                }
                cfg.spotlight_mode.label(cfg.spotlight_pixelated)
            };
            let (rx_v, ry_v, f_v) = (
                wm.config().spotlight_rx,
                wm.config().spotlight_ry,
                wm.config().spotlight_feather_pct,
            );
            wm.active_pane_mut().set_status_message(Some(format!(
                "🔦 Focal Spotlight: {label} (rx={rx_v}, ry={ry_v}, feather={f_v}%)"
            )));
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
            let label = if no_args {
                wm.config_mut().cycle_focal_zoom()
            } else {
                let cfg = wm.config_mut();
                if let Some(m) = mode {
                    cfg.focal_zoom_mode = m;
                } else if cfg.focal_zoom_mode == SpotlightMode::Off && step_pct.is_none() {
                    cfg.focal_zoom_mode = SpotlightMode::Ellipse;
                }
                if let Some(pct) = zoom_pct {
                    cfg.focal_zoom_pct = pct.clamp(105, 400);
                }
                if let Some(delta_p) = step_pct {
                    cfg.step_focal_zoom_level(delta_p);
                }
                if let Some(px) = pixelated {
                    cfg.focal_zoom_pixelated = px;
                }
                if let Some(fs) = fullscreen {
                    cfg.focal_zoom_fullscreen = fs;
                }
                if let Some(rx_val) = rx {
                    cfg.focal_zoom_rx = rx_val.clamp(4, 120);
                    if ry.is_none() && cfg.focal_zoom_mode == SpotlightMode::Circle {
                        cfg.focal_zoom_ry = (rx_val / 2).clamp(2, 60);
                    }
                }
                if let Some(ry_val) = ry {
                    cfg.focal_zoom_ry = ry_val.clamp(2, 60);
                }
                if let Some((drx, dry)) = delta_size {
                    cfg.resize_focal_zoom(drx, dry);
                }
                if let Some(f) = feather_pct {
                    cfg.focal_zoom_feather_pct = f.clamp(5, 100);
                }
                cfg.focal_zoom_label()
            };
            wm.sync_focal_zoom_font_scale();
            let (rx_v, ry_v, f_v, pt_v) = (
                wm.config().focal_zoom_rx,
                wm.config().focal_zoom_ry,
                wm.config().focal_zoom_feather_pct,
                wm.config().scaled_font_size_pt(),
            );
            wm.active_pane_mut().set_status_message(Some(format!(
                "🔍 Focal Zoom: {label} (font={pt_v}pt, rx={rx_v}, ry={ry_v}, feather={f_v}%)"
            )));
            false
        }
        ColonCommand::Shadows(style_opt) => {
            let cfg = wm.config_mut();
            match style_opt {
                Some(ShadowStyle::None) => {
                    cfg.shadow_style = ShadowStyle::None;
                    cfg.drop_shadows = false;
                }
                Some(style) => {
                    cfg.shadow_style = style;
                    cfg.drop_shadows = true;
                }
                None => {
                    cfg.drop_shadows = !cfg.drop_shadows;
                    if cfg.drop_shadows && cfg.shadow_style == ShadowStyle::None {
                        cfg.shadow_style = ShadowStyle::Translucent;
                    }
                }
            }
            false
        }
        ColonCommand::FKeyBarToggle(val) => {
            wm.config_mut().show_dos_fkey_bar = val.unwrap_or(!wm.config().show_dos_fkey_bar);
            wm.reflow_default_viewport();
            false
        }
        ref drawer_act @ (ColonCommand::DrawerToggle
        | ColonCommand::DrawerFocus
        | ColonCommand::DrawerDock(_)
        | ColonCommand::DrawerVisibility(_)
        | ColonCommand::DrawerLayout(_)
        | ColonCommand::DrawerSize(_)) => {
            wm.execute_drawer_action(drawer_act);
            false
        }
        ColonCommand::DrawerStyle(style) => {
            wm.set_drawer_style(style);
            false
        }
        ColonCommand::DrawerRows(n) => {
            wm.set_drawer_row_lines(n);
            false
        }
        ColonCommand::DrawerNav(op) => {
            wm.apply_drawer_nav(op);
            false
        }
        ColonCommand::DrawerPreview(op) => {
            wm.execute_drawer_preview_op(op);
            false
        }
        ref font_act @ (ColonCommand::FontZoomIn
        | ColonCommand::FontZoomOut
        | ColonCommand::FontZoomReset
        | ColonCommand::FontSetSize(_)) => {
            wm.execute_font_action(font_act);
            false
        }
        ColonCommand::PaneFontCommand(ref subcmd) => {
            let res = wm.execute_pane_font_command(subcmd);
            let first_word = subcmd.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
            if matches!(first_word.as_str(), "list" | "presets" | "scan" | "dir") {
                wm.open_fonts_window();
                match &res {
                    Ok(msg) => wm.fonts_window_mut().last_status = Some(msg.clone()),
                    Err(err) => wm.fonts_window_mut().last_status = Some(err.clone()),
                }
            }
            false
        }
        ColonCommand::CanvasCommand(ref subcmd) => {
            wm.execute_canvas_action(subcmd);
            false
        }
        ColonCommand::PaneResize { dw, dh } => {
            wm.resize_focused_pane_or_window(dw, dh);
            wm.maybe_autosave_layout();
            false
        }
        ColonCommand::PaneMove { dx, dy } => {
            wm.move_focused_pane_or_window(dx, dy);
            wm.maybe_autosave_layout();
            false
        }
        ColonCommand::WindowResetGeometry => {
            wm.reset_window_geometries();
            wm.maybe_autosave_layout();
            false
        }
        ColonCommand::ArrangeGrid => {
            wm.execute_layout_profile_command("arrange grid");
            wm.maybe_autosave_layout();
            false
        }
        ColonCommand::ForceQuit => {
            wm.maybe_autosave_layout();
            true
        }
        ColonCommand::DetachClient => wm.detach_client(),
        ColonCommand::ExitProtectionSet(policy) => {
            wm.config_mut().exit_protection = policy;
            wm.active_pane_mut().set_status_message(Some(format!(
                "Exit protection policy set to {}",
                policy.label()
            )));
            false
        }
        ColonCommand::ThemeCommand(ref subcmd) => {
            wm.execute_theme_command(subcmd);
            false
        }
        ColonCommand::WindowStyle(ref subcmd) => {
            wm.execute_window_style_command(subcmd);
            false
        }
        ColonCommand::SessionTreeCommand(ref subcmd) => {
            wm.execute_session_tree_command(subcmd);
            false
        }
        ColonCommand::LayoutProfileCommand(ref subcmd) => {
            wm.execute_layout_profile_command(subcmd);
            false
        }
        ColonCommand::McpCommand(ref subcmd) => wm.execute_mcp_command(subcmd),
        ColonCommand::FkeyModeCommand(ref arg_opt) => {
            use crate::ui::visual_style::{RawFkeyMode, parse_fkey_number};
            use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
            match arg_opt.as_deref() {
                None => {
                    let next = wm.config().raw_fkeys.cycle();
                    wm.config_mut().raw_fkeys = next;
                    wm.active_pane_mut().set_status_message(Some(format!(
                        "RAW Mode F-Keys: [{}] — {}",
                        next.label(),
                        next.description()
                    )));
                }
                Some("status") => {
                    let cur = wm.config().raw_fkeys;
                    wm.active_pane_mut().set_status_message(Some(format!(
                        "RAW Mode F-Keys: [{}] — {}",
                        cur.label(),
                        cur.description()
                    )));
                }
                Some(s) if s.starts_with("send ") => {
                    if let Some(n) = s.strip_prefix("send ").and_then(parse_fkey_number) {
                        let pane = wm.active_pane_mut();
                        let bytes = crate::input::encode_key_to_pty_bytes_full(
                            KeyEvent::new(KeyCode::F(n), KeyModifiers::NONE),
                            pane.vt().is_application_cursor(),
                            pane.vt().is_backarrow_key_67(),
                            pane.vt().kitty_keyboard_flags(),
                        );
                        if !bytes.is_empty() {
                            let _ = pane.write_to_pty(&bytes);
                            pane.set_status_message(Some(format!(
                                "Sent raw F{n} escape sequence ({} bytes) to PTY",
                                bytes.len()
                            )));
                        }
                    }
                }
                Some(tok) => {
                    if let Some(mode) = RawFkeyMode::from_name(tok) {
                        wm.config_mut().raw_fkeys = mode;
                        wm.active_pane_mut().set_status_message(Some(format!(
                            "RAW Mode F-Keys: [{}] — {}",
                            mode.label(),
                            mode.description()
                        )));
                    }
                }
            }
            false
        }
        ColonCommand::VisualStyleCommand(ref mode_opt) => {
            let label = crate::ui::visual_style::apply_visual_style_mode(
                wm.config_mut(),
                mode_opt.as_deref(),
            );
            wm.active_pane_mut().set_status_message(Some(format!(
                "Visual Style: {label} (toggle via F11, :modern, :liquid, :tron, :snazzy, :glass, or :style)"
            )));
            false
        }
        ColonCommand::Quit => {
            wm.maybe_autosave_layout();
            if wm.intercept_prompt_exit_if_needed("exit") {
                wm.is_client_detached()
            } else {
                true
            }
        }
        ColonCommand::ConfigGet(_)
        | ColonCommand::ConfigSet(..)
        | ColonCommand::ConfigList
        | ColonCommand::ConfigSave
        | ColonCommand::AppearanceSave
        | ColonCommand::StateQuery => super::execute_config::execute_config_command(wm, cmd),
        ColonCommand::Agents(op) => super::execute_agents::execute_agents_command(wm, op),
        ColonCommand::AgentsList(op) => super::execute_agents_list::execute_agents_list_command(wm, op),
        ColonCommand::AgentsHud(op) => super::execute_agents_hud::execute_hud(wm, &op),
        ColonCommand::AgentsReplay(op) => super::execute_agents_hud::execute_replay(wm, &op),
        ColonCommand::AgentsWatchdog(_) => super::execute_agents_hud::execute_watchdog(wm),
        ColonCommand::SessionMap(op) => {
            if let Some(msg) = wm.apply_session_map_op(&op) {
                wm.active_pane_mut().set_status_message(Some(msg));
            }
            false
        }
        ColonCommand::AgentMail(op) => super::execute_agent_mail::execute_agent_mail_command(wm, op),
        ColonCommand::FontsWindow(op) => {
            let msg = wm.fonts_window_apply(&op);
            if let Some(msg) = msg {
                wm.active_pane_mut().set_status_message(Some(msg));
            }
            false
        }
        ColonCommand::CommandWindow(op) => super::execute_command_window::execute_command_window(wm, op),
        ColonCommand::ClipboardWindow(op) => {
            wm.clipboard_window_apply(op);
            false
        }
        ColonCommand::Context(op) => super::execute_context::execute_context(wm, op),
        ColonCommand::AgentQueue(op) => super::execute_context::execute_agent_queue(wm, op),
        ColonCommand::Swarm(op) => super::execute_swarm::execute_swarm(wm, op),
        ColonCommand::AgentGate(op) => super::execute_context::execute_agent_gate(wm, op),
        ColonCommand::AgentGuide(op) => super::execute_agent_mail::execute_agent_guide(wm, op),
        ColonCommand::Coords(op) => super::execute_coords::execute_coords(wm, op),
        ColonCommand::TrafficMonitor(op) => {
            if let Err(e) = wm.traffic_monitor_apply(op) {
                wm.active_pane_mut().set_status_message(Some(e));
            }
            false
        }
        ColonCommand::ScriptStudio(op) => {
            if let Some(msg) = wm.script_studio_apply(&op) {
                wm.active_pane_mut().set_status_message(Some(msg));
            }
            false
        }
        ColonCommand::Find(op) => super::execute_find::execute_find_command(wm, op),
        ColonCommand::RespawnPaneInPlace => {
            wm.respawn_active_pane();
            false
        }
        ColonCommand::WindowManage(op) => super::execute_window::execute_window_manage(wm, op),
        ColonCommand::SessionManage(op) => super::execute_window::execute_session_manage(wm, op),
        ColonCommand::RemoteLink(op) => super::execute_link::execute_link_command(wm, op),
        ColonCommand::DaemonSupervisor(ref op) | ColonCommand::Daemon(ref op) => {
            super::execute_daemon::execute_daemon_command(wm, op)
        }
        ColonCommand::Voice(ref op) => {
            super::execute_voice::execute_voice_command(wm, op);
            false
        }
        ColonCommand::Browser(op) => super::execute_media::execute_browser_command(wm, op),
        ColonCommand::Video(op) => super::execute_media::execute_video_command(wm, op),
        ColonCommand::Web(ref op) => super::execute_web::execute_web_action(wm, op),
        ColonCommand::WindowMode(op) => {
            wm.apply_window_mode(op);
            false
        }
        ColonCommand::MenuOpen { category } => {
            wm.open_menu_from_keyboard(category);
            false
        }
        ColonCommand::Keys(ref op) => {
            wm.apply_keys_op(op);
            false
        }
        other => {
            let is_prompt_layout = matches!(
                other,
                ColonCommand::PromptTop
                    | ColonCommand::PromptBottom
                    | ColonCommand::PromptSplitTop
                    | ColonCommand::PromptSplitBottom
                    | ColonCommand::PromptSplitToggle
                    | ColonCommand::PromptModeSet(_)
                    | ColonCommand::PromptFloat(_)
                    | ColonCommand::PromptHeight(_)
                    | ColonCommand::PromptWidth(_)
                    | ColonCommand::PromptMoveVertical(_)
                    | ColonCommand::PromptResizeHeight(_)
                    | ColonCommand::PromptResizeWidth(_)
            );
            let quit = execute_colon_command(wm.active_pane_mut(), other);
            if is_prompt_layout {
                wm.maybe_autosave_layout();
            }
            quit
        }
    }
}

fn panel_name(target: PanelTarget) -> &'static str {
    match target {
        PanelTarget::Drawer => "Drawer",
        PanelTarget::Mcp => "MCP",
        PanelTarget::Agents => "Agents",
        PanelTarget::AgentsList => "Agents list",
        PanelTarget::SessionMap => "Session map",
        PanelTarget::Clipboard => "Clipboard",
        PanelTarget::Traffic => "Traffic",
    }
}
