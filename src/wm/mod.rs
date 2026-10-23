pub mod quick_terminal;
pub mod split_tree;
pub mod z_stack;

use std::collections::BTreeMap;
use std::path::PathBuf;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::config::BiltermConfig;
use crate::error::Result;
use crate::pane::{CompositePane, PaneId, PromptAnchor};
use crate::pty::PtyConfig;
use crate::remote::{
    FleetNodeKind, FleetRegistry, FleetSelector, RemoteHostProfile,
    remote_osc133_bootstrap_command,
};
use crate::wm::quick_terminal::QuickTerminal;
use crate::wm::split_tree::{SplitDirection, SplitTree};
use crate::wm::z_stack::ZStack;

/// Workspace window arrangement mode (`WM-01`, `WM-02`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceLayoutMode {
    /// Non-overlapping H/V binary splits (`SplitTree`).
    Tiled,
    /// Overlapping Borland Turbo Vision z-order stacked/cascaded windows (`ZStack`).
    Stacked,
}

/// Serialized per-pane metadata inside a `SavedWorkspaceSession` (`ECO-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPaneState {
    pub id: PaneId,
    pub title: String,
    pub cwd: String,
    pub prompt_anchor: PromptAnchor,
    pub prompt_height: u16,
    pub prompt_width_cols: Option<u16>,
    pub read_only: bool,
    pub history: Vec<String>,
}

/// Declarative TOML snapshot of a multi-pane Bilterm workspace (`ECO-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedWorkspaceSession {
    pub name: String,
    pub layout_mode: WorkspaceLayoutMode,
    pub split_tree: SplitTree,
    pub z_order: Vec<PaneId>,
    pub zoomed_pane: Option<PaneId>,
    pub panes: Vec<SavedPaneState>,
}

/// Multi-pane Window Manager coordinating tiled H/V splits, Z-order stacking/shuffling,
/// zoom/maximize, SGR 1006 local mouse coordinate translation, and the persistent Guake
/// drop-down `QuickTerminal` (`WM-01..05`).
pub struct WindowManager {
    pub panes: BTreeMap<PaneId, CompositePane>,
    pub split_tree: SplitTree,
    pub z_stack: ZStack,
    pub layout_mode: WorkspaceLayoutMode,
    pub zoomed_pane: Option<PaneId>,
    pub quick_terminal: QuickTerminal,
    pub config: BiltermConfig,
    /// Whether the interactive `<F1>` Borland Turbo Vision Help Modal is visible.
    pub help_visible: bool,
    /// Vertical scroll offset inside the `<F1>` Help Modal.
    pub help_scroll: usize,
    /// Whether the `<F9>` / `:inspector` Live 3270 Terminal Inspector modal is visible (`BEL-01`).
    pub inspector_visible: bool,
    /// Vertical scroll offset inside the `<F9>` Terminal Inspector modal (`BEL-01`).
    pub inspector_scroll: usize,
    /// Active pull-down category index (`0..7`) when the top Borland Menu Bar is open (`ECO-03`).
    pub menubar_open_category: Option<usize>,
    /// Selected menu item row inside the open pull-down menu (`ECO-03`).
    pub menubar_selected_item: usize,
    /// Synchronized Multi-Pane Input Broadcast (`:broadcast`, `ECO-04`).
    pub broadcast_input: bool,
    /// Whether the Prefix/Leader key (`Ctrl+X` by default) is awaiting the next chord (`v1.5.0`).
    pub leader_pending: bool,
    /// Whether the interactive Fuzzy Command Palette modal (`Ctrl+P` / `:palette`) is visible (`v0.6.0`).
    pub palette_visible: bool,
    /// Current search query inside the Fuzzy Command Palette modal (`v0.6.0`).
    pub palette_query: String,
    /// Selected item index inside the Fuzzy Command Palette modal (`v0.6.0`).
    pub palette_selected: usize,
    /// Central `herdr`-style fleet registry managing local and remote SSH / `tmux -CC` sessions (`v1.4.0`).
    pub fleet: FleetRegistry,
    next_pane_id: PaneId,
    headless_mode: bool,
    default_size: (u16, u16),
}

impl WindowManager {
    /// Creates a `WindowManager` around an initial `CompositePane`.
    pub fn from_initial_pane(pane: CompositePane, headless_mode: bool) -> Self {
        let initial_id = pane.id;
        let (rows, cols) = pane.vt.size();
        let screen_rows = rows + 1;
        let quick_terminal = if headless_mode {
            QuickTerminal::new_headless(screen_rows, cols)
        } else {
            QuickTerminal::spawn(PtyConfig {
                rows: screen_rows,
                cols,
                ..Default::default()
            })
            .unwrap_or_else(|_| QuickTerminal::new_headless(screen_rows, cols))
        };

        let mut panes = BTreeMap::new();
        panes.insert(initial_id, pane);

        Self {
            panes,
            split_tree: SplitTree::new(initial_id),
            z_stack: ZStack::new(initial_id),
            layout_mode: WorkspaceLayoutMode::Tiled,
            zoomed_pane: None,
            quick_terminal,
            config: BiltermConfig::default(),
            help_visible: false,
            help_scroll: 0,
            inspector_visible: false,
            inspector_scroll: 0,
            menubar_open_category: None,
            menubar_selected_item: 0,
            broadcast_input: false,
            leader_pending: false,
            palette_visible: false,
            palette_query: String::new(),
            palette_selected: 0,
            fleet: FleetRegistry::new(),
            next_pane_id: initial_id + 1,
            headless_mode,
            default_size: (screen_rows, cols),
        }
    }

    /// Toggles the `<F1>` interactive Help Modal.
    pub fn toggle_help(&mut self) {
        self.help_visible = !self.help_visible;
        if self.help_visible {
            self.help_scroll = 0;
        }
    }

    /// Toggles the `<F9>` / `:inspector` Live 3270 Terminal Inspector modal (`BEL-01`).
    pub fn toggle_inspector(&mut self) {
        self.inspector_visible = !self.inspector_visible;
        if self.inspector_visible {
            self.inspector_scroll = 0;
        }
    }

    /// Toggles the top 1-row Borland Turbo Vision Menu Bar (`:menubar`, `ECO-03`).
    pub fn toggle_menubar(&mut self) {
        self.config.show_top_menubar = !self.config.show_top_menubar;
        if !self.config.show_top_menubar {
            self.menubar_open_category = None;
        }
    }

    /// Sets or toggles Synchronized Multi-Pane Input Broadcast (`:broadcast`, `ECO-04`).
    pub fn set_broadcast_input(&mut self, enabled: bool) {
        self.broadcast_input = enabled;
        self.active_pane_mut().status_message = Some(format!(
            "Multi-Pane Input Broadcast: {}",
            if enabled { "ON (📡 BCAST)" } else { "OFF" }
        ));
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

        let new_id = self.split_focused(SplitDirection::Vertical)?;
        let args = profile.build_ssh_args();
        let dest = profile.destination.clone();
        if !self.headless_mode {
            let (rows, cols) = self.default_size;
            let pty_cfg = PtyConfig {
                program: "ssh".to_string(),
                args,
                rows,
                cols,
                cwd: None,
                env: BTreeMap::new(),
            };
            if let Ok(mut live_pane) = CompositePane::spawn(new_id, pty_cfg) {
                if profile.remote_osc133_bootstrap {
                    let _ = live_pane.write_to_pty(remote_osc133_bootstrap_command().as_bytes());
                }
                live_pane.title = format!("SSH:{dest} [#{new_id}]");
                live_pane.remote_target = Some(dest.clone());
                self.panes.insert(new_id, live_pane);
            }
        }
        if let Some(pane) = self.panes.get_mut(&new_id) {
            pane.title = format!("SSH:{dest} [#{new_id}]");
            pane.remote_target = Some(dest.clone());
            pane.status_message = Some(format!("🌐 Connected SSH pane #{new_id} -> {dest}"));
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
        let profile = self
            .fleet
            .profiles
            .get(target)
            .cloned()
            .or_else(|| self.config.remotes.get(target).cloned())
            .unwrap_or_else(|| RemoteHostProfile::new(target, target));
        let dest = profile.destination.clone();
        let new_id = self.split_focused(SplitDirection::Vertical)?;
        let badge = format!("{dest}/{session_name}");
        if let Some(pane) = self.panes.get_mut(&new_id) {
            pane.title = format!("TMUX-SSH:{badge} [#{new_id}]");
            pane.remote_target = Some(badge.clone());
            pane.status_message = Some(format!(
                "🌐 Attached remote tmux -CC session '{session_name}' on {dest} (pane #{new_id})"
            ));
        }
        let mut merged_tags = profile.tags.clone();
        for t in tags {
            if !merged_tags.contains(&t) {
                merged_tags.push(t);
            }
        }
        let node_name = format!("tmux-{dest}-{session_name}");
        self.fleet
            .register_ssh_tmux(node_name, dest, session_name, vec![new_id], merged_tags);
        Ok(new_id)
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
                        .filter(|(_, p)| p.remote_target.is_none())
                        .map(|(&id, _)| id),
                );
            }
            FleetSelector::RemoteOnly => {
                target_pane_ids.extend(
                    self.panes
                        .iter()
                        .filter(|(_, p)| p.remote_target.is_some())
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
                && !pane.read_only
                && !pane.line_discipline.is_password_prompt()
            {
                pane.prompt.history.push(command.to_string());
                let _ = pane.history_store.record_command(command);
                pane.prompt.running_command = Some(command.to_string());
                pane.vt.record_command_submission(command);
                let mut payload = command.as_bytes().to_vec();
                payload.push(b'\n');
                let _ = pane.write_to_pty(&payload);
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

    /// Toggles the interactive Fuzzy Command Palette modal (`Ctrl+P` / `Alt+Space` / `:palette`, `v0.6.0`).
    pub fn toggle_palette(&mut self) {
        self.palette_visible = !self.palette_visible;
        if self.palette_visible {
            self.palette_query.clear();
            self.palette_selected = 0;
        }
    }

    /// Returns `(colon_cmd, shortcut, description)` entries matching `palette_query` ranked by `nucleo-matcher` (`v0.6.0`).
    pub fn filtered_palette_items(&self) -> Vec<(&'static str, &'static str, &'static str)> {
        use nucleo_matcher::{Config, Matcher, Utf32Str};
        const CATALOG: &[(&str, &str, &str)] = &[
            (":split h", "Ctrl+X - / F2", "Split focused pane horizontally (Top/Bottom)"),
            (":split v", "Ctrl+X | / Alt+V", "Split focused pane vertically (Left/Right)"),
            (":zoom", "Ctrl+X z / F5", "Toggle zoom/maximize on focused pane"),
            (":layout stack", "Ctrl+X M / Alt+M", "Switch workspace to Z-Order Stacked Cascade"),
            (":layout tile", "Ctrl+X M / Alt+M", "Switch workspace to Tiled Binary Split Tree"),
            (":deck next", "Ctrl+X o / F6", "Shuffle Z-order window deck top-to-bottom"),
            (":quake", "Ctrl+X q / Ctrl+\\", "Toggle persistent Guake drop-down quick terminal"),
            (":inspector", "Ctrl+X i / F9", "Open Live 3270 Terminal Inspector modal"),
            (":hints", "Ctrl+X f / F3", "Activate QuickSelect 3270 Field Hints ([a]..[z])"),
            (":copy", "Ctrl+X [ / Alt+C", "Enter Vi Copy Mode & Rectangular Block Selection"),
            (":lock", "Ctrl+X l / Alt+L", "Toggle Read-Only Protected 3270 Field Lock"),
            (":broadcast", "—", "Toggle Synchronized Multi-Pane Input Broadcast"),
            (":menubar", "—", "Toggle Top Borland Turbo Vision Menu Bar"),
            (":crt", "—", "Toggle Retro CRT Phosphor Scanline Effect"),
            (":mode cycle", "Ctrl+X e / F4", "Cycle Floating Prompt -> Fixed Prompt -> Regular Terminal"),
            (":mode fixed", "Ctrl+X s / Alt+S", "Switch to Fixed (Non-Overlapping Docked) Command Prompt"),
            (":mode float", "Ctrl+X t / Alt+T", "Switch to Floating Command Prompt over Terminal Canvas"),
            (":mode terminal", "F12 / Ctrl+G", "Switch to Regular Terminal (Hide Command Prompt Box)"),
            (":prompt split-top", "Ctrl+X s / Alt+S", "Non-overlapping Split Command Prompt at Top"),
            (":prompt top", "Ctrl+X t / Alt+T", "Pin Floating Command Prompt to Top"),
            (":prompt bottom", "Ctrl+X b / Alt+B", "Pin Floating Command Prompt to Bottom"),
            (":crosshair full", "Ctrl+X x / Alt+X", "Enable Full-Screen IBM 3270 Cross-Hair Cursor"),
            (":spotlight", "Alt+O", "Cycle Focal Spotlight (Circle/Ellipse × Fuzzy/Pixelated)"),
            (":spotlight ellipse fuzzy", "Alt+O", "Enable Elliptical Fuzzy Focal Spotlight"),
            (":spotlight ellipse pixel", "Alt+O", "Enable Elliptical Pixelated Character-Block Spotlight"),
            (":spotlight circle pixel", "Alt+O", "Enable Circular Pixelated Character-Block Spotlight"),
            (":spotlight +", "Alt+Shift+]", "Grow Focal Spotlight Radius (+4 cols, +1 row)"),
            (":spotlight -", "Alt+Shift+[", "Shrink Focal Spotlight Radius (-4 cols, -1 row)"),
            (":spotlight off", "—", "Disable Focal Spotlight"),
            (":focalzoom", "Alt+Shift+O", "Cycle Focal Zoom Lens (1.25x -> 1.50x -> 2.00x -> 3.00x -> Off)"),
            (":focalzoom 1.5x fuzzy", "Alt+Shift+O", "Enable 1.50x Smooth Fuzzy Focal Zoom Lens"),
            (":focalzoom 2x fuzzy", "Alt+Shift+O", "Enable 2.00x High Magnification Focal Zoom Lens"),
            (":focalzoom 2x pixel", "—", "Enable 2.00x Pixelated Character-Block Focal Zoom Lens"),
            (":focalzoom +", "—", "Step Focal Zoom Magnification Up (+0.25x)"),
            (":focalzoom -", "—", "Step Focal Zoom Magnification Down (-0.25x)"),
            (":focalzoom off", "—", "Disable Focal Zoom Lens"),
            (":theme turbo-blue", "—", "Switch palette to Borland Turbo Vision Blue"),
            (":theme 3278", "—", "Switch palette to IBM 3278 Phosphor Green"),
            (":theme 3279", "—", "Switch palette to IBM 3279 Amber CRT"),
            (":session save default", "—", "Save multi-pane workspace session to TOML"),
            (":session restore default", "—", "Restore saved multi-pane workspace session"),
            (":record start", "—", "Start Asciicast v2 (.cast) terminal recording"),
            (":record stop", "—", "Stop Asciicast v2 (.cast) terminal recording"),
            (":notifications clear", "—", "Clear OSC 9 / OSC 777 desktop notifications"),
            (":remote list", "—", "List local and remote SSH / tmux-CC fleet sessions"),
            (":help", "Ctrl+X ? / F1", "Open Borland Turbo Vision Quick Reference Help"),
            (":quit", "Ctrl+Q", "Cleanly exit Bilterm"),
        ];
        let q = self.palette_query.trim();
        if q.is_empty() {
            return CATALOG.to_vec();
        }
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut needle_buf = Vec::new();
        let needle = Utf32Str::new(q, &mut needle_buf);
        let mut scored: Vec<(u16, (&'static str, &'static str, &'static str))> = CATALOG
            .iter()
            .filter_map(|&item| {
                let combined = format!("{} {} {}", item.0, item.1, item.2);
                let mut hay_buf = Vec::new();
                let hay = Utf32Str::new(&combined, &mut hay_buf);
                matcher.fuzzy_match(hay, needle).map(|score| (score, item))
            })
            .collect();
        scored.sort_by_key(|a| std::cmp::Reverse(a.0));
        scored.into_iter().map(|(_, item)| item).collect()
    }

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
            pane.vt.notify_color_scheme_changed(fg, bg, cur);
            if let Some(pty) = &pane.pty {
                for resp in pane.vt.take_query_responses() {
                    let _ = pty.write_all(&resp);
                }
            }
        }
        self.quick_terminal
            .pane
            .vt
            .notify_color_scheme_changed(fg, bg, cur);
    }

    fn resolve_session_dir(&self) -> PathBuf {
        if let Some(custom) = &self.config.session_dir {
            return PathBuf::from(custom);
        }
        if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
            return PathBuf::from(xdg).join("bilterm").join("sessions");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("state")
                .join("bilterm")
                .join("sessions");
        }
        std::env::temp_dir().join("bilterm-sessions")
    }

    /// Captures a `SavedWorkspaceSession` of the current multi-pane workspace (`ECO-02`).
    pub fn snapshot_session(&self, name: &str) -> SavedWorkspaceSession {
        let panes = self
            .panes
            .values()
            .map(|p| SavedPaneState {
                id: p.id,
                title: p.title.clone(),
                cwd: p.resolve_cwd().to_string_lossy().into_owned(),
                prompt_anchor: p.prompt.anchor,
                prompt_height: p.prompt.height,
                prompt_width_cols: p.prompt.width_cols,
                read_only: p.read_only,
                history: p.prompt.history.clone(),
            })
            .collect();
        SavedWorkspaceSession {
            name: name.to_string(),
            layout_mode: self.layout_mode,
            split_tree: self.split_tree.clone(),
            z_order: self.z_stack.order().to_vec(),
            zoomed_pane: self.zoomed_pane,
            panes,
        }
    }

    /// Saves the current workspace layout and pane states to `<session_dir>/<name>.toml` (`:session save <name>`, `ECO-02`).
    pub fn save_session(&self, name: &str) -> Result<PathBuf> {
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
        let dir = self.resolve_session_dir();
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
        let path = self.resolve_session_dir().join(format!("{file_stem}.toml"));
        let raw = std::fs::read_to_string(&path)?;
        let saved: SavedWorkspaceSession = toml::from_str(&raw)
            .map_err(|e| crate::error::BiltermError::Config(e.to_string()))?;

        let (rows, cols) = self.default_size;
        let mut new_panes = BTreeMap::new();
        let mut max_id = 1usize;
        for sp in &saved.panes {
            max_id = max_id.max(sp.id);
            let mut pane = if self.headless_mode {
                CompositePane::new_headless(sp.id, rows, cols)
            } else {
                CompositePane::spawn(
                    sp.id,
                    PtyConfig {
                        rows,
                        cols,
                        cwd: Some(PathBuf::from(&sp.cwd)),
                        ..Default::default()
                    },
                )
                .unwrap_or_else(|_| CompositePane::new_headless(sp.id, rows, cols))
            };
            pane.title = sp.title.clone();
            pane.prompt.anchor = sp.prompt_anchor;
            pane.prompt.height = sp.prompt_height.clamp(3, 12);
            pane.prompt.width_cols = sp.prompt_width_cols;
            pane.read_only = sp.read_only;
            pane.prompt.history = sp.history.clone();
            new_panes.insert(sp.id, pane);
        }

        if new_panes.is_empty() {
            new_panes.insert(1, CompositePane::new_headless(1, rows, cols));
        }

        let pane_count = new_panes.len();
        self.panes = new_panes;
        self.split_tree = saved.split_tree;
        self.z_stack = ZStack::from_order(saved.z_order);
        self.layout_mode = saved.layout_mode;
        self.zoomed_pane = saved.zoomed_pane;
        self.next_pane_id = max_id + 1;
        self.sync_history_config();
        Ok(pane_count)
    }

    /// Lists all saved workspace session names found in `<session_dir>` (`:session list`, `ECO-02`).
    pub fn list_sessions(&self) -> Vec<String> {
        let dir = self.resolve_session_dir();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("toml") {
                    p.file_stem()
                        .and_then(|s| s.to_str())
                        .map(ToOwned::to_owned)
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        names
    }

    /// Installs a loaded `BiltermConfig` and propagates history and theme settings to every pane.
    pub fn apply_config(&mut self, config: BiltermConfig) {
        self.config = config;
        self.sync_history_config();
        self.notify_theme_changed();
    }

    /// Propagates `config.history_enabled`, `config.history_max_files`, and `config.history_dir` to all panes.
    pub fn sync_history_config(&mut self) {
        let enabled = self.config.history_enabled;
        let max_files = self.config.history_max_files;
        let dir = self.config.history_dir.clone();
        for pane in self.panes.values_mut() {
            pane.history_store
                .configure(enabled, max_files, dir.as_deref());
        }
        self.quick_terminal
            .pane
            .history_store
            .configure(enabled, max_files, dir.as_deref());
    }

    pub fn new_headless(rows: u16, cols: u16) -> Self {
        let pane = CompositePane::new_headless(1, rows, cols);
        Self::from_initial_pane(pane, true)
    }

    pub fn spawn_default(rows: u16, cols: u16) -> Result<Self> {
        let cfg = PtyConfig {
            rows,
            cols,
            ..Default::default()
        };
        let pane = CompositePane::spawn(1, cfg)?;
        Ok(Self::from_initial_pane(pane, false))
    }

    /// Returns the currently active `PaneId` (`0` when the Guake drop-down terminal is visible,
    /// otherwise the top of `ZStack`).
    pub fn active_pane_id(&self) -> PaneId {
        if self.quick_terminal.visible {
            0
        } else {
            self.z_stack.top().unwrap_or(1)
        }
    }

    /// Returns a shared reference to the currently focused `CompositePane` (either the Guake
    /// quick terminal if visible, or the topmost pane in `ZStack`).
    pub fn active_pane(&self) -> &CompositePane {
        if self.quick_terminal.visible {
            &self.quick_terminal.pane
        } else {
            let id = self.z_stack.top().unwrap_or(1);
            self.panes
                .get(&id)
                .unwrap_or_else(|| self.panes.values().next().expect("at least one pane"))
        }
    }

    /// Returns a mutable reference to the currently focused `CompositePane`.
    pub fn active_pane_mut(&mut self) -> &mut CompositePane {
        if self.quick_terminal.visible {
            &mut self.quick_terminal.pane
        } else {
            let id = self.z_stack.top().unwrap_or(1);
            if self.panes.contains_key(&id) {
                self.panes.get_mut(&id).unwrap()
            } else {
                self.panes.values_mut().next().expect("at least one pane")
            }
        }
    }

    /// Splits the currently focused workspace pane in `direction` (`Horizontal` or `Vertical`),
    /// spawning a new independent `CompositePane` and raising it to the top of `ZStack` (`WM-01`).
    pub fn split_focused(&mut self, direction: SplitDirection) -> Result<PaneId> {
        let target_id = self.z_stack.top().unwrap_or(1);
        let new_id = self.next_pane_id;
        self.next_pane_id += 1;

        let (rows, cols) = self.default_size;
        let new_pane = if self.headless_mode {
            CompositePane::new_headless(new_id, rows, cols)
        } else {
            CompositePane::spawn(
                new_id,
                PtyConfig {
                    rows,
                    cols,
                    ..Default::default()
                },
            )?
        };

        self.panes.insert(new_id, new_pane);
        self.split_tree.split_leaf(target_id, new_id, direction);
        self.z_stack.push(new_id);
        self.zoomed_pane = None;
        Ok(new_id)
    }

    /// Closes the focused pane (if more than 1 workspace pane exists).
    pub fn close_focused_pane(&mut self) -> bool {
        if self.quick_terminal.visible {
            self.quick_terminal.visible = false;
            return true;
        }
        if self.panes.len() <= 1 {
            return false;
        }
        let Some(target_id) = self.z_stack.top() else {
            return false;
        };
        if self.split_tree.remove_leaf(target_id) {
            self.panes.remove(&target_id);
            self.z_stack.remove(target_id);
            if self.zoomed_pane == Some(target_id) {
                self.zoomed_pane = None;
            }
            true
        } else {
            false
        }
    }

    /// Selectively raises `pane_id` to the top of the z-order stack (`Alt+1..9` or mouse click, `WM-02`).
    pub fn focus_and_raise(&mut self, pane_id: PaneId) -> bool {
        if self.panes.contains_key(&pane_id) {
            self.quick_terminal.visible = false;
            self.z_stack.raise(pane_id)
        } else {
            false
        }
    }

    /// Shuffles the pane deck top-to-bottom (`Alt+Tab` / `F6` / `:deck next`, `WM-02`).
    pub fn shuffle_deck_next(&mut self) -> Option<PaneId> {
        self.quick_terminal.visible = false;
        self.z_stack.shuffle_top_to_bottom()
    }

    /// Shuffles the pane deck bottom-to-top (`Alt+Shift+Tab` / `:deck prev`, `WM-02`).
    pub fn shuffle_deck_prev(&mut self) -> Option<PaneId> {
        self.quick_terminal.visible = false;
        self.z_stack.shuffle_bottom_to_top()
    }

    /// Toggles zoom/maximize on the currently focused workspace pane (`F5` / `Alt+Z` / `:zoom`, `WM-03`).
    pub fn toggle_zoom(&mut self) {
        let Some(top_id) = self.z_stack.top() else {
            return;
        };
        if self.zoomed_pane == Some(top_id) {
            self.zoomed_pane = None;
        } else {
            self.zoomed_pane = Some(top_id);
        }
    }

    /// Toggles between `WorkspaceLayoutMode::Tiled` and `WorkspaceLayoutMode::Stacked`.
    pub fn toggle_layout_mode(&mut self) {
        self.layout_mode = match self.layout_mode {
            WorkspaceLayoutMode::Tiled => WorkspaceLayoutMode::Stacked,
            WorkspaceLayoutMode::Stacked => WorkspaceLayoutMode::Tiled,
        };
    }

    /// Toggles the persistent Guake/Quake-style drop-down quick terminal (`Alt+\`` / `Ctrl+Space` / `:quake`, `WM-05`).
    pub fn toggle_quick_terminal(&mut self) {
        self.quick_terminal.toggle();
    }

    /// Computes `(PaneId, Rect)` pairs in ascending render order (painter's algorithm:
    /// bottom-most pane first, topmost focused pane last).
    pub fn compute_workspace_rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        if let Some(zoomed_id) = self.zoomed_pane
            && self.panes.contains_key(&zoomed_id) {
                return vec![(zoomed_id, area)];
            }
        match self.layout_mode {
            WorkspaceLayoutMode::Tiled => {
                let raw = self.split_tree.compute_rects(area);
                let rect_map: BTreeMap<PaneId, Rect> = raw.into_iter().collect();
                // Return in ZStack ascending order so active pane is rendered last
                self.z_stack
                    .order()
                    .iter()
                    .filter_map(|id| rect_map.get(id).copied().map(|r| (*id, r)))
                    .collect()
            }
            WorkspaceLayoutMode::Stacked => self.z_stack.compute_cascade_rects(area),
        }
    }

    /// Resizes all visible panes' viewports to match `area`.
    pub fn resize_all(&mut self, area: Rect) -> Result<()> {
        self.default_size = (area.height.max(1), area.width.max(1));
        let rects = self.compute_workspace_rects(area);
        for (id, rect) in rects {
            let inner = if self.panes.len() > 1 || self.layout_mode == WorkspaceLayoutMode::Stacked {
                inset_window_frame(rect)
            } else {
                rect
            };
            if let Some(pane) = self.panes.get_mut(&id) {
                pane.resize_viewport(inner)?;
            }
        }
        if self.quick_terminal.visible {
            let qt_rect = inset_window_frame(self.quick_terminal.overlay_rect(area));
            self.quick_terminal.pane.resize_viewport(qt_rect)?;
        }
        Ok(())
    }

    /// Hit-tests global `(col, row)` in reverse z-order (topmost first).
    /// Returns `(pane_id, pane_outer_rect, pane_inner_rect)`.
    pub fn hit_test_pane(&self, col: u16, row: u16, area: Rect) -> Option<(PaneId, Rect, Rect)> {
        if self.quick_terminal.visible {
            let qt_outer = self.quick_terminal.overlay_rect(area);
            if rect_contains(qt_outer, col, row) {
                return Some((0, qt_outer, inset_window_frame(qt_outer)));
            }
        }

        let rects = self.compute_workspace_rects(area);
        let framed = self.panes.len() > 1 || self.layout_mode == WorkspaceLayoutMode::Stacked;
        for &(id, outer) in rects.iter().rev() {
            if rect_contains(outer, col, row) {
                let inner = if framed {
                    inset_window_frame(outer)
                } else {
                    outer
                };
                return Some((id, outer, inner));
            }
        }
        None
    }
}

/// Insets a window rectangle by 1 cell on all sides for the Borland Turbo Vision window frame (`╔═╗` / `┌─┐`).
pub fn inset_window_frame(outer: Rect) -> Rect {
    if outer.width <= 2 || outer.height <= 2 {
        return outer;
    }
    Rect {
        x: outer.x + 1,
        y: outer.y + 1,
        width: outer.width.saturating_sub(2),
        height: outer.height.saturating_sub(2),
    }
}

pub fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

/// Translates a global `MouseEvent` into pane-local coordinates and encodes an SGR (1006)
/// mouse sequence (`\x1b[<btn;local_col;local_rowM` or `m`) (`WM-03`).
pub fn encode_sgr_mouse_local(mouse: MouseEvent, pane_inner: Rect) -> Option<Vec<u8>> {
    if !rect_contains(pane_inner, mouse.column, mouse.row) {
        return None;
    }
    let local_col = mouse.column.saturating_sub(pane_inner.x) + 1;
    let local_row = mouse.row.saturating_sub(pane_inner.y) + 1;

    let (btn_code, suffix) = match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => (0, 'M'),
        MouseEventKind::Down(MouseButton::Middle) => (1, 'M'),
        MouseEventKind::Down(MouseButton::Right) => (2, 'M'),
        MouseEventKind::Up(MouseButton::Left) => (0, 'm'),
        MouseEventKind::Up(MouseButton::Middle) => (1, 'm'),
        MouseEventKind::Up(MouseButton::Right) => (2, 'm'),
        MouseEventKind::Drag(MouseButton::Left) => (32, 'M'),
        MouseEventKind::ScrollUp => (64, 'M'),
        MouseEventKind::ScrollDown => (65, 'M'),
        _ => return None,
    };

    Some(format!("\x1b[<{btn_code};{local_col};{local_row}{suffix}").into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    #[test]
    fn window_manager_splits_stacks_shuffles_raises_and_translates_sgr_mouse() {
        let mut wm = WindowManager::new_headless(25, 80);
        let screen = Rect::new(0, 0, 80, 25);

        let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
        assert_eq!(p2, 2);
        assert_eq!(wm.active_pane_id(), 2);

        let p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();
        assert_eq!(p3, 3);
        assert_eq!(wm.active_pane_id(), 3);

        // Shuffle deck top-to-bottom: 3 -> bottom, 2 becomes active
        assert_eq!(wm.shuffle_deck_next(), Some(2));
        assert_eq!(wm.active_pane_id(), 2);

        // Selectively raise pane #1
        assert!(wm.focus_and_raise(1));
        assert_eq!(wm.active_pane_id(), 1);

        // Hit-test pane #2 on the right half of the screen (col=60, row=5)
        let (hit_id, _outer, inner) = wm.hit_test_pane(60, 5, screen).expect("hit right pane");
        assert_eq!(hit_id, 2);
        let sgr = encode_sgr_mouse_local(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 60,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
            inner,
        )
        .expect("sgr bytes");
        // Pane #2 outer starts at x=40, y=0; inner starts at x=41, y=1 -> local_col = 60 - 41 + 1 = 20, local_row = 5 - 1 + 1 = 5
        assert_eq!(String::from_utf8(sgr).unwrap(), "\x1b[<0;20;5M");
    }
}
