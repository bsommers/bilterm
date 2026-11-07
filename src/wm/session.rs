//! Workspace session persistence (`SavedWorkspaceSession`), SSH/fleet pane spawning, and scrollback rebalancing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use super::{WindowManager, WorkspaceLayoutMode};
use crate::config::BiltermConfig;
use crate::error::Result;
use crate::pane::history_store::HistorySettings;
use crate::pane::{CompositePane, PaneId, PromptAnchor};
use crate::pty::PtyConfig;
use crate::remote::{FleetNodeKind, FleetSelector, RemoteHostProfile};
use crate::vt::scrollback::{PaneUsage, format_mib, plan_capacities};
use crate::wm::split_tree::{SplitDirection, SplitTree};

/// Serialized per-pane metadata inside a `SavedWorkspaceSession` (`ECO-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPaneState {
    pub id: PaneId,
    pub title: String,
    pub cwd: String,
    /// Shell executable running in this pane (`$SHELL`, `/bin/zsh`, `fish`, `pwsh`, `ssh`, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    /// Arguments passed to the pane's shell executable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shell_args: Vec<String>,
    /// Active software / foreground command running in this pane (`htop`, `vim`, `cargo watch`, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_command: Option<String>,
    /// Optional remote SSH / `tmux -CC` target associated with this pane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_target: Option<String>,
    pub prompt_anchor: PromptAnchor,
    pub prompt_height: u16,
    pub prompt_width_cols: Option<u16>,
    #[serde(default)]
    pub prompt_x_offset: Option<u16>,
    #[serde(default)]
    pub prompt_input: String,
    #[serde(default)]
    pub font_scale_pct: Option<u16>,
    #[serde(default)]
    pub scroll_mode: Option<crate::pane::PaneScrollMode>,
    #[serde(default)]
    pub scrollback_lines: Vec<String>,
    pub read_only: bool,
    pub history: Vec<String>,
    /// Per-session cut-and-paste yank buffer history ring when `clipboard_history_persist` is enabled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub yank_history: Vec<String>,
    /// Custom window name (`SES-02`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
    /// Per-window theme name (re-resolved via `resolve_named_palette`); `None` = global.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Per-window outline style; `None` = global `window_outline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<crate::ui::chrome::OutlineStyle>,
    /// Per-window bold border; `None` = global `window_outline_bold`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    /// Per-window individual element color overrides (`font-color`, `cursor-color`, `frame-color`, `controls-color`, `status-color`, etc.).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, String>,
}

/// Declarative TOML snapshot of a multi-pane Bilterm workspace (`ECO-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedWorkspaceSession {
    pub name: String,
    pub layout_mode: WorkspaceLayoutMode,
    pub split_tree: SplitTree,
    pub z_order: Vec<PaneId>,
    #[serde(default)]
    pub z_geometries: BTreeMap<String, crate::wm::FloatingWindowGeometry>,
    pub zoomed_pane: Option<PaneId>,
    #[serde(default)]
    pub detached: bool,
    pub panes: Vec<SavedPaneState>,
    /// Multi-selected window IDs (`SES-03`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub selected_windows: BTreeSet<PaneId>,
}

impl WindowManager {
    /// Returns `(colon_cmd, shortcut, description)` entries matching `palette_query` ranked by `nucleo-matcher` (`v0.6.0`).
    pub fn filtered_palette_items(&self) -> Vec<(&'static str, &'static str, &'static str)> {
        use crate::ui::modals::PALETTE_CATALOG;
        use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
        use nucleo_matcher::{Config, Matcher, Utf32Str};
        let q = self.palette_state().map(|(q, _)| q).unwrap_or("").trim();
        if q.is_empty() {
            return PALETTE_CATALOG.to_vec();
        }
        let q_lower = q.to_lowercase();
        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(&q_lower, CaseMatching::Ignore, Normalization::Smart);
        let mut scored: Vec<(u32, (&'static str, &'static str, &'static str))> = PALETTE_CATALOG
            .iter()
            .filter_map(|&item| {
                let combined = format!("{} {} {}", item.0, item.1, item.2);
                let score_opt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut hay_buf = Vec::new();
                    let hay = Utf32Str::new(&combined, &mut hay_buf);
                    pattern.score(hay, &mut matcher)
                }))
                .ok()
                .flatten()
                .or_else(|| fallback_subsequence_score(&combined, &q_lower));
                score_opt.map(|score| (score, item))
            })
            .collect();
        scored.sort_by_key(|a| std::cmp::Reverse(a.0));
        scored.into_iter().map(|(_, item)| item).collect()
    }
}

fn fallback_subsequence_score(haystack: &str, needle_lower: &str) -> Option<u32> {
    let hay_lower = haystack.to_lowercase();
    if hay_lower.contains(needle_lower) {
        return Some(100);
    }
    let mut hay_chars = hay_lower.chars();
    needle_lower.chars().all(|nc| hay_chars.any(|hc| hc == nc)).then_some(10)
}

impl WindowManager {
    /// Propagates the active `ThemePalette` RGB values to all panes and queues unsolicited
    /// `CSI ? 997 ; 1|2 n` notifications on any pane with `DECSET ?2031` active (`BEL-03`).
    pub fn notify_theme_changed(&mut self) {
        let pal = self.config.palette();
        let to_rgb = |c: Color, fallback: (u8, u8, u8)| match c {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::Black => (0x00, 0x00, 0x00),
            Color::White => (0xFF, 0xFF, 0xFF),
            _ => fallback,
        };
        let fg = to_rgb(pal.title_fg, (0xFF, 0xFF, 0xFF));
        let bg = to_rgb(pal.window_frame_bg, (0x00, 0x00, 0x84));
        let cur = to_rgb(pal.active_border_fg, (0xFF, 0xFF, 0x55));
        for pane in self.panes.values_mut() {
            pane.vt_mut().notify_color_scheme_changed(fg, bg, cur);
            pane.flush_query_responses();
        }
        self.quick_terminal
            .pane
            .vt_mut()
            .notify_color_scheme_changed(fg, bg, cur);
    }

    /// Replaces the active configuration (theme + history persistence settings) at startup or reload.
    pub fn apply_config(&mut self, config: BiltermConfig) {
        let scroll_mode = config.scroll_mode;
        self.config = config;
        if self.config.drawer.visibility == crate::wm::DrawerVisibilityMode::Fixed {
            self.drawer.open = true;
        }
        for pane in self.panes.values_mut() {
            pane.set_scroll_mode_from_config(scroll_mode);
        }
        self.quick_terminal.pane.set_scroll_mode_from_config(scroll_mode);
        self.notify_theme_changed();
        self.sync_history_config();
        self.rebalance_scrollback(Instant::now());
    }

    /// Propagates the `history_*` config settings to all panes, reloading prompt history for
    /// panes whose session file location (dir / session name) changed.
    pub fn sync_history_config(&mut self) {
        let settings = self.config.history_settings();
        for pane in self.panes.values_mut() {
            Self::attach_history(pane, &settings);
        }
        Self::attach_history(&mut self.quick_terminal.pane, &settings);
    }

    pub(super) fn attach_history(pane: &mut CompositePane, settings: &HistorySettings) {
        if pane.history_store_mut().apply_settings(settings) {
            pane.restore_persisted_history();
        }
    }

    /// Spawns a new `CompositePane` connected to a remote SSH host or configured `[remotes.<name>]` profile (`v1.4.0`).
    pub fn spawn_ssh_pane(
        &mut self,
        target: &str,
        port_override: Option<u16>,
        tags: Vec<String>,
    ) -> Result<PaneId> {
        let mut profile = self
            .fleet
            .profiles
            .get(target)
            .cloned()
            .or_else(|| self.config.remotes.get(target).cloned())
            .unwrap_or_else(|| RemoteHostProfile::new(target, target));
        if let Some(p) = port_override {
            profile.port = Some(p);
        }
        let mut merged_tags = profile.tags.clone();
        for t in tags {
            if !merged_tags.contains(&t) {
                merged_tags.push(t);
            }
        }
        profile.tags = merged_tags.clone();

        let control_dir = crate::secure_fs::private_runtime_dir()
            .map_err(|e| {
                tracing::warn!(error = %e, "private runtime dir unavailable; ssh ControlMaster disabled")
            })
            .ok();
        let args = profile.build_ssh_args(control_dir.as_deref())?;
        let new_id = self.split_focused(SplitDirection::Vertical)?;
        let dest = profile.destination.clone();
        if !self.headless_mode {
            let (rows, cols) = self
                .panes
                .get(&new_id)
                .map(|p| p.vt().size())
                .unwrap_or(self.default_size);
            let pty_cfg = PtyConfig {
                program: "ssh".to_string(),
                args,
                rows,
                cols,
                cwd: None,
                env: BTreeMap::new(),
            };
            if let Ok(mut live_pane) = CompositePane::spawn(new_id, pty_cfg) {
                live_pane.set_title(format!("SSH:{dest} [#{new_id}]"));
                live_pane.set_remote_target(Some(dest.clone()));
                Self::attach_history(&mut live_pane, &self.config.history_settings());
                self.panes.insert(new_id, live_pane);
                self.reflow_default_viewport();
            }
        }
        if let Some(pane) = self.panes.get_mut(&new_id) {
            pane.set_title(format!("SSH:{dest} [#{new_id}]"));
            pane.set_remote_target(Some(dest.clone()));
            pane.set_status_message(Some(format!("🌐 Connected SSH pane #{new_id} -> {dest}")));
        }
        let node_name = format!("{}-#{}", profile.name, new_id);
        self.fleet
            .register_ssh_pane(node_name, new_id, dest, merged_tags);
        Ok(new_id)
    }

    /// Connects to a persistent remote `tmux -CC` session over SSH and hydrates a local `CompositePane` (`v1.4.0`).
    pub fn connect_ssh_tmux(
        &mut self,
        target: &str,
        session_name: &str,
        tags: Vec<String>,
    ) -> Result<PaneId> {
        self.connect_ssh_tmux_body(target, session_name, tags)
    }

    /// Executes `command` across all local/remote panes and fleet nodes matching `selector` (`herdr`-style fan-out, `v1.4.0`).
    pub fn exec_fleet_command(&mut self, selector: &FleetSelector, command: &str) -> Vec<PaneId> {
        let matching_node_names = self.fleet.matching_nodes(selector);
        let mut target_pane_ids: Vec<PaneId> = Vec::new();

        match selector {
            FleetSelector::All => {
                target_pane_ids.extend(self.panes.keys().copied());
            }
            FleetSelector::LocalOnly => {
                target_pane_ids.extend(
                    self.panes
                        .iter()
                        .filter(|(_, p)| p.remote_target().is_none())
                        .map(|(&id, _)| id),
                );
            }
            FleetSelector::RemoteOnly => {
                target_pane_ids.extend(
                    self.panes
                        .iter()
                        .filter(|(_, p)| p.remote_target().is_some())
                        .map(|(&id, _)| id),
                );
            }
            FleetSelector::Tag(_) | FleetSelector::Node(_) => {
                for node_name in &matching_node_names {
                    if let Some(node) = self.fleet.nodes.get(node_name) {
                        match &node.kind {
                            FleetNodeKind::SshPane { pane_id, .. } => {
                                if !target_pane_ids.contains(pane_id) {
                                    target_pane_ids.push(*pane_id);
                                }
                            }
                            FleetNodeKind::SshTmux { pane_ids, .. } => {
                                for &pid in pane_ids {
                                    if !target_pane_ids.contains(&pid) {
                                        target_pane_ids.push(pid);
                                    }
                                }
                            }
                            FleetNodeKind::HostProfile { .. } => {}
                        }
                    }
                }
            }
        }

        let mut executed_panes = Vec::new();
        for pid in target_pane_ids {
            if let Some(pane) = self.panes.get_mut(&pid)
                && !pane.is_read_only()
                && !pane.is_password_prompt()
            {
                let _ = pane.submit_command(command);
                executed_panes.push(pid);
            }
        }

        for node_name in matching_node_names {
            if let Some(node) = self.fleet.nodes.get_mut(&node_name) {
                node.last_dispatched_command = Some(command.to_string());
            }
        }
        executed_panes
    }

    /// Disconnects a fleet node by name and removes any associated SSH/tmux panes via [`Self::remove_pane`] (`ARCH-R02`).
    pub fn disconnect_fleet_node(&mut self, name: &str) -> bool {
        if let Some(node) = self.fleet.nodes.remove(name) {
            match node.kind {
                FleetNodeKind::SshPane { pane_id, .. } => {
                    let _ = self.remove_pane(pane_id);
                }
                FleetNodeKind::SshTmux { pane_ids, .. } => {
                    for pid in pane_ids {
                        let _ = self.remove_pane(pid);
                    }
                }
                FleetNodeKind::HostProfile { .. } => {}
            }
            true
        } else {
            false
        }
    }

    pub fn session_dir_fallback_in(base: &Path) -> Result<PathBuf> {
        crate::secure_fs::private_dir_in(base, "sessions").map_err(Into::into)
    }

    pub(super) fn resolve_session_dir(&self) -> Result<PathBuf> {
        if let Some(custom) = &self.config.session_dir {
            return Ok(PathBuf::from(custom));
        }
        if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
            return Ok(PathBuf::from(xdg).join("bilterm").join("sessions"));
        }
        if let Ok(home) = std::env::var("HOME") {
            return Ok(PathBuf::from(home)
                .join(".local")
                .join("state")
                .join("bilterm")
                .join("sessions"));
        }
        let runtime = crate::secure_fs::private_runtime_dir()?;
        Self::session_dir_fallback_in(&runtime)
    }

    /// Captures a `SavedWorkspaceSession` of the current multi-pane workspace (`ECO-02`).
    pub fn snapshot_session(&self, name: &str) -> SavedWorkspaceSession {
        let panes = self
            .panes
            .values()
            .map(|p| {
                let (rows, _cols) = p.vt().size();
                let mut scrollback_lines = Vec::new();
                for r in 0..rows {
                    let line = p.vt().row_text(r);
                    let trimmed = line.trim_end();
                    if !trimmed.is_empty() {
                        scrollback_lines.push(trimmed.to_string());
                    }
                }
                let chrome = self.pane_chrome(p.id());
                SavedPaneState {
                    id: p.id(),
                    title: p.title().to_string(),
                    cwd: p.resolve_cwd().to_string_lossy().into_owned(),
                    shell: Some(p.shell_program().to_string()),
                    shell_args: p.shell_args().to_vec(),
                    running_command: p.resolve_running_command(),
                    remote_target: p.remote_target().map(ToOwned::to_owned),
                    prompt_anchor: p.prompt().anchor,
                    prompt_height: p.prompt().height,
                    prompt_width_cols: p.prompt().width_cols,
                    prompt_x_offset: p.prompt().x_offset,
                    prompt_input: p.prompt().text(),
                    font_scale_pct: p.font_options().scale_pct,
                    scroll_mode: Some(p.scroll_mode()),
                    scrollback_lines,
                    read_only: p.is_read_only(),
                    history: p.prompt().history.clone(),
                    yank_history: if self.config.clipboard_history_persist {
                        p.prompt().yank_history.clone()
                    } else {
                        Vec::new()
                    },
                    custom_name: (p.title() != format!("PANE #{}", p.id()))
                        .then(|| p.title().to_string()),
                    theme: chrome.theme_name,
                    outline: chrome.outline,
                    bold: chrome.bold,
                    colors: chrome.colors,
                }
            })
            .collect();
        SavedWorkspaceSession {
            name: name.to_string(),
            layout_mode: self.layout_mode,
            split_tree: self.split_tree.clone(),
            z_order: self.z_stack.order().to_vec(),
            z_geometries: self
                .z_stack
                .geometries()
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
            zoomed_pane: self.zoomed_pane,
            detached: self.client_detached,
            panes,
            selected_windows: self.selected_windows().clone(),
        }
    }

    /// Returns `true` if a detached workspace session `<name>.toml` is waiting to be resumed (`DAEM-02`).
    pub fn has_detached_session(&self, name: &str) -> bool {
        let safe_name: String = name
            .trim()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .collect();
        let file_stem = if safe_name.is_empty() {
            "default"
        } else {
            &safe_name
        };
        let Ok(dir) = self.resolve_session_dir() else {
            return false;
        };
        let path = dir.join(format!("{file_stem}.toml"));
        let Ok(raw) = std::fs::read_to_string(path) else {
            return false;
        };
        toml::from_str::<SavedWorkspaceSession>(&raw)
            .map(|s| s.detached)
            .unwrap_or(false)
    }

    /// Saves the current workspace under `name_opt` (or the active `history_session` when `None`),
    /// activates that session name, and registers it in memory, disk, `session_order`, and `session_tree`.
    pub fn save_and_activate_session(
        &mut self,
        name_opt: Option<&str>,
    ) -> Result<(String, PathBuf)> {
        let raw = name_opt
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.config.history_session);
        let target = crate::wm::layout_profile::sanitize_profile_name(raw);
        let snap = self.snapshot_session(&target);
        self.layout_profiles_mem.insert(target.clone(), snap);
        let path = self.save_session(&target)?;
        self.config.history_session = target.clone();
        self.config.active_layout_profile = target.clone();
        let settings = self.config.history_settings();
        for pane in self.panes.values_mut() {
            let _ = pane.history_store_mut().apply_settings(&settings);
        }
        let mut order = self.ordered_sessions_with_active();
        if !order.contains(&target) {
            order.push(target.clone());
        }
        self.save_session_order(order);
        self.daemon_state.session_tree.upsert_session(
            &target,
            crate::wm::session_tree::SessionNodeKind::Rooted,
            None,
        );
        self.daemon_state
            .session_panes
            .insert(target.clone(), self.panes.len());
        Ok((target, path))
    }

    /// Saves the current workspace layout and pane states to `<session_dir>/<name>.toml` (`:session save <name>`, `ECO-02`).
    pub fn save_session(&self, name: &str) -> Result<PathBuf> {
        let fallback = if self.config.history_session.trim().is_empty() {
            "default"
        } else {
            self.config.history_session.trim()
        };
        let raw = if name.trim().is_empty() { fallback } else { name.trim() };
        let safe_name: String = raw
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .collect();
        let file_stem = if safe_name.is_empty() {
            "default"
        } else {
            &safe_name
        };
        let dir = self.resolve_session_dir()?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{file_stem}.toml"));
        let snap = self.snapshot_session(file_stem);
        let toml_str = toml::to_string_pretty(&snap)
            .map_err(|e| crate::error::BiltermError::Config(e.to_string()))?;
        std::fs::write(&path, toml_str)?;
        Ok(path)
    }

    /// Restores a previously saved workspace session from `<session_dir>/<name>.toml` (`:session restore <name>`, `ECO-02`).
    pub fn restore_session(&mut self, name: &str) -> Result<usize> {
        let fallback = if self.config.history_session.trim().is_empty() {
            "default"
        } else {
            self.config.history_session.trim()
        };
        let raw_name = if name.trim().is_empty() { fallback } else { name.trim() };
        let safe_name: String = raw_name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .collect();
        let file_stem = if safe_name.is_empty() {
            "default".to_string()
        } else {
            safe_name
        };
        let path = self.resolve_session_dir()?.join(format!("{file_stem}.toml"));
        let raw = std::fs::read_to_string(&path)?;
        let mut saved: SavedWorkspaceSession = toml::from_str(&raw)
            .map_err(|e| crate::error::BiltermError::Config(e.to_string()))?;
        let was_detached = saved.detached;
        self.config.history_session = file_stem.clone();
        self.config.active_layout_profile = file_stem.clone();
        let pane_count = self.apply_saved_session_snapshot(&saved);
        self.client_detached = false;
        self.layout_profiles_mem.insert(file_stem.clone(), saved.clone());
        self.daemon_state
            .session_panes
            .insert(file_stem.clone(), pane_count);
        let mut order = self.ordered_sessions_with_active();
        if !order.contains(&file_stem) {
            order.push(file_stem);
            self.save_session_order(order);
        }
        if was_detached {
            saved.detached = false;
            if let Ok(toml_str) = toml::to_string_pretty(&saved) {
                let _ = std::fs::write(&path, toml_str);
            }
        }
        Ok(pane_count)
    }

    /// Runs [`Self::rebalance_scrollback`] when the periodic interval has elapsed (app tick).
    pub fn maybe_rebalance_scrollback(&mut self, now: Instant) {
        if self.scrollback.due(now) {
            self.rebalance_scrollback(now);
        }
    }

    /// Grows nearly-full panes and contracts oversized ones against the global scrollback
    /// budget and host memory pressure. Panes on the alternate screen are deferred.
    pub fn rebalance_scrollback(&mut self, now: Instant) {
        let policy = self.scrollback.begin(&self.config.scrollback, now);
        let live: BTreeSet<PaneId> = self.panes.keys().copied().collect();
        self.scrollback.retain_seen(&live);
        for (&id, pane) in self.panes.iter_mut() {
            let vt = pane.vt_mut();
            if vt.is_alternate_screen() || !self.scrollback.mark_seen(id) {
                continue;
            }
            let initial = policy.initial_capacity(vt.size().1);
            if vt.scrollback_capacity() != initial && vt.scrollback_used_rows() <= initial {
                vt.set_scrollback_capacity(initial);
            }
        }
        {
            let qvt = self.quick_terminal.pane.vt_mut();
            if !qvt.is_alternate_screen() && self.scrollback.mark_quick_seen() {
                let initial = policy.initial_capacity(qvt.size().1);
                if qvt.scrollback_capacity() != initial && qvt.scrollback_used_rows() <= initial {
                    qvt.set_scrollback_capacity(initial);
                }
            }
        }
        let mut vts: Vec<&mut crate::vt::VtParser> = self
            .panes
            .values_mut()
            .map(|pane| pane.vt_mut())
            .chain(std::iter::once(self.quick_terminal.pane.vt_mut()))
            .collect();
        let usages: Vec<PaneUsage> = vts
            .iter_mut()
            .map(|vt| PaneUsage {
                used_rows: vt.scrollback_used_rows(),
                capacity: vt.scrollback_capacity(),
                cols: vt.size().1,
            })
            .collect();
        for (vt, target) in vts.iter_mut().zip(plan_capacities(&usages, &policy)) {
            if target != vt.scrollback_capacity() {
                vt.set_scrollback_capacity(target);
            }
        }
    }

    /// One-line `:scrollback` report for the active pane and the global budget.
    pub fn scrollback_report(&mut self) -> String {
        let policy = self
            .scrollback
            .last_policy()
            .unwrap_or_else(|| self.scrollback.begin(&self.config.scrollback, Instant::now()));
        let total: u64 = self
            .panes
            .values_mut()
            .chain(std::iter::once(&mut self.quick_terminal.pane))
            .map(|p| p.vt_mut().estimated_scrollback_bytes())
            .sum();
        let vt = self.active_pane_mut().vt_mut();
        let used = vt.scrollback_used_rows();
        let cap = vt.scrollback_capacity();
        let bytes = vt.estimated_scrollback_bytes();
        format!(
            "Scrollback: {used}/{cap} lines ({}) | all panes {} of {} budget{}",
            format_mib(bytes),
            format_mib(total),
            format_mib(policy.budget_bytes),
            if policy.memory_pressure {
                " | LOW MEMORY"
            } else {
                ""
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_lines(pane: &mut CompositePane, n: usize) {
        let mut bytes = Vec::new();
        for i in 0..n {
            bytes.extend_from_slice(format!("row {i}\r\n").as_bytes());
        }
        pane.process_pty_output(&bytes);
    }

    #[test]
    fn scrollback_rebalance_sizes_new_panes_and_doubles_full_ones() {
        use crate::vt::scrollback::LineLimit;
        let mut wm = WindowManager::new_headless(25, 80);
        wm.config.scrollback.initial_lines = 1_000;
        wm.config.scrollback.min_lines = 500;
        wm.config.scrollback.max_lines = LineLimit::Fixed(3_000);
        let now = std::time::Instant::now();
        wm.rebalance_scrollback(now);
        assert_eq!(wm.active_pane().vt().scrollback_capacity(), 1_000);
        assert_eq!(wm.quick_terminal.pane.vt().scrollback_capacity(), 1_000);

        fill_lines(wm.active_pane_mut(), 1_200);
        assert!(!wm.scrollback.due(now), "throttled within the interval");
        wm.rebalance_scrollback(now);
        assert_eq!(wm.active_pane().vt().scrollback_capacity(), 2_000);
        assert_eq!(
            wm.quick_terminal.pane.vt().scrollback_capacity(),
            1_000,
            "idle pane untouched"
        );
        fill_lines(wm.active_pane_mut(), 2_000);
        wm.rebalance_scrollback(now);
        assert_eq!(
            wm.active_pane().vt().scrollback_capacity(),
            3_000,
            "clamped to max_lines"
        );
        assert!(wm.active_pane_mut().vt_mut().scrollback_used_rows() >= 1_900);
    }

    #[test]
    fn scrollback_contracts_under_low_available_memory() {
        use crate::vt::scrollback::{FixedMemoryProbe, MemoryBudget, MemorySnapshot};
        let mut wm = WindowManager::new_headless(25, 80);
        wm.config.scrollback.initial_lines = 1_500;
        wm.config.scrollback.min_lines = 100;
        wm.config.scrollback.memory_budget = MemoryBudget::Bytes(4 * 1024 * 1024);
        let now = std::time::Instant::now();
        wm.rebalance_scrollback(now);
        fill_lines(wm.active_pane_mut(), 1_000);
        wm.rebalance_scrollback(now);
        assert_eq!(
            wm.active_pane().vt().scrollback_capacity(),
            1_500,
            "no pressure, no shrink"
        );

        wm.scrollback.set_probe(Box::new(FixedMemoryProbe(MemorySnapshot {
            total_bytes: Some(16 << 30),
            available_bytes: Some(512 << 20),
        })));
        wm.rebalance_scrollback(now);
        // Fair share: 4 MiB / 2 panes / (80*32+64) bytes per row = 799 rows.
        let cap = wm.active_pane().vt().scrollback_capacity();
        assert_eq!(cap, 799);
        assert!(wm.active_pane_mut().vt_mut().scrollback_used_rows() <= cap);
        assert!(wm.active_pane().vt().contents().contains("row 999"));
    }

    #[test]
    fn scrollback_colon_command_reports_and_clears() {
        use crate::colon_command::{execute_wm_colon_command, parse_colon_command};
        let mut wm = WindowManager::new_headless(25, 80);
        fill_lines(wm.active_pane_mut(), 300);
        let cmd = parse_colon_command(":scrollback").unwrap().unwrap();
        assert!(!execute_wm_colon_command(&mut wm, cmd));
        let msg = wm.active_pane().status_message().unwrap().to_string();
        assert!(
            msg.contains("lines") && msg.contains("MiB") && msg.contains("budget"),
            "{msg}"
        );

        let cmd = parse_colon_command(":scrollback clear").unwrap().unwrap();
        assert!(!execute_wm_colon_command(&mut wm, cmd));
        assert_eq!(wm.active_pane_mut().vt_mut().scrollback_used_rows(), 0);
        assert!(parse_colon_command(":scrollback bogus").unwrap().is_err());
    }

    #[test]
    fn spawn_ssh_pane_rejects_option_destination_without_new_pane() {
        let mut wm = WindowManager::new_headless(24, 80);
        let initial_panes = wm.panes.len();
        let res = wm.spawn_ssh_pane("-oProxyCommand=evil", None, vec![]);
        assert!(res.is_err());
        assert_eq!(wm.panes.len(), initial_panes);
    }

    #[test]
    fn connect_ssh_tmux_rejects_option_destination_without_new_pane() {
        let mut wm = WindowManager::new_headless(24, 80);
        let initial_panes = wm.panes.len();
        let res = wm.connect_ssh_tmux("-oProxyCommand=evil", "main", vec![]);
        assert!(res.is_err());
        assert_eq!(wm.panes.len(), initial_panes);
    }

    #[test]
    fn spawn_ssh_pane_accepts_valid_destination() {
        let mut wm = WindowManager::new_headless(24, 80);
        let initial_panes = wm.panes.len();
        let res = wm.spawn_ssh_pane("ubuntu@10.0.0.42", None, vec![]);
        assert!(res.is_ok());
        assert_eq!(wm.panes.len(), initial_panes + 1);
    }

    #[test]
    fn list_sessions_returns_empty_when_session_dir_unavailable() {
        let tmp = tempfile::tempdir().unwrap();
        // Point fallback helper at an unwritable or invalid location
        let non_dir = tmp.path().join("file_not_dir");
        std::fs::write(&non_dir, b"foo").unwrap();
        assert!(WindowManager::session_dir_fallback_in(&non_dir).is_err());

        // A window manager with an invalid session_dir config returns empty list without panicking
        let mut wm = WindowManager::new_headless(24, 80);
        wm.config.session_dir = Some(non_dir.to_str().unwrap().to_string());
        assert!(wm.list_sessions().is_empty());
    }
}
