//! Multi-pane `WindowManager` `:colon` command executor (`execute_wm_colon_command`, `ARCH-R01`).

use crate::colon_command::ColonCommand;
use crate::colon_command::execute::execute_colon_command;
use crate::remote::{FleetSelector, RemoteHostProfile};
use crate::ui::chrome::ShadowStyle;
use crate::ui::cursor::SpotlightMode;
use crate::wm::WindowManager;
use crate::wm::split_tree::SplitDirection;

/// Executes a parsed `ColonCommand` against the multi-pane `WindowManager`.
pub fn execute_wm_colon_command(wm: &mut WindowManager, cmd: ColonCommand) -> bool {
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
                let path = match wm.automation_confine_dir().map(std::path::Path::to_path_buf) {
                    Some(dir) => match crate::secure_fs::validate_plain_file_name(&p) {
                        Ok(()) => dir.join(&p),
                        Err(e) => {
                            wm.record_automation_error(e.clone());
                            wm.active_pane_mut().set_status_message(Some(e));
                            return false;
                        }
                    },
                    None => std::path::PathBuf::from(p),
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
        ColonCommand::HistoryMaxFiles(max_f) => {
            wm.config_mut().history_max_files = max_f.max(1);
            wm.sync_history_config();
            wm.active_pane_mut()
                .set_status_message(Some(format!("History max session files: {}", max_f.max(1))));
            false
        }
        ColonCommand::Theme(preset) => {
            wm.config_mut().theme = preset;
            wm.config_mut().custom_palette_override = None;
            wm.notify_theme_changed();
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
            false
        }
        ColonCommand::SessionSave(name) => {
            match wm.save_session(&name) {
                Ok(path) => {
                    wm.active_pane_mut().set_status_message(
                        Some(format!("Saved session '{}' to {}", name, path.display())),
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
            match wm.restore_session(&name) {
                Ok(count) => {
                    wm.active_pane_mut().set_status_message(
                        Some(format!("Restored session '{name}' ({count} pane(s))")),
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
            wm.toggle_palette();
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
            match wm.connect_ssh_tmux(&target, &session, Vec::new()) {
                Ok(new_id) => {
                    wm.active_pane_mut().set_status_message(Some(format!(
                        "🌐 Attached remote tmux -CC '{session}' on {target} (pane #{new_id})"
                    )));
                }
                Err(err) => {
                    wm.active_pane_mut()
                        .set_status_message(Some(format!("SSH tmux connect error: {err}")));
                }
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
            wm.active_pane_mut().set_status_message(Some(if names.is_empty() {
                "Fleet nodes: (local only)".to_string()
            } else {
                format!("Fleet nodes ({}): {}", names.len(), names.join(", "))
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
        ColonCommand::Cursor(style) => {
            wm.config_mut().cursor_style = style;
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
        ref font_act @ (ColonCommand::FontZoomIn
        | ColonCommand::FontZoomOut
        | ColonCommand::FontZoomReset
        | ColonCommand::FontSetSize(_)) => {
            wm.execute_font_action(font_act);
            false
        }
        ColonCommand::PaneFontCommand(ref subcmd) => {
            let _ = wm.execute_pane_font_command(subcmd);
            false
        }
        ColonCommand::CanvasCommand(ref subcmd) => {
            wm.execute_canvas_action(subcmd);
            false
        }
        ColonCommand::PaneResize { dw, dh } => {
            wm.resize_focused_pane_or_window(dw, dh);
            false
        }
        ColonCommand::PaneMove { dx, dy } => {
            wm.move_focused_pane_or_window(dx, dy);
            false
        }
        ColonCommand::WindowResetGeometry => {
            wm.reset_window_geometries();
            false
        }
        other => execute_colon_command(wm.active_pane_mut(), other),
    }
}
