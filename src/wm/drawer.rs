//! `navigator`-Style Slide-Out Sessions / Windows / Panes & Fleet Drawer (`DRW-01`, `DRW-02`).
//!
//! Provides a dockable (`Left`, `Right`, `Top`, `Bottom`), multi-policy (`OnDemand`, `Fixed`,
//! `AutoHide`) workspace drawer that either floats over panes (`Overlay` with Borland drop shadow
//! and zero `SIGWINCH`) or carves out workspace geometry (`Push` with automatic pane reflow),
//! rendering high-density telemetry status glyphs for every session, layout window, pane, and
//! remote fleet node.

use std::collections::BTreeMap;

use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

use crate::pane::{CompositePane, PaneId};
use crate::remote::FleetNodeKind;
use crate::vt::Osc133Phase;
use crate::wm::{WindowManager, WorkspaceLayoutMode};

pub mod navigator;
pub mod nav;
pub mod menu;
mod ops;
pub(crate) mod navigator_detail;
pub use navigator::{CommsIndicator, DrawerStyle, NavigatorLevel, NavigatorRowMeta};
pub use nav::DrawerNavOp;
pub use menu::{NavigatorMenu, NavigatorMenuEntry};

/// 10-frame Braille activity spinner glyphs (`DRW-02`).
pub const BRAILLE_SPINNER_FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// 6-level output throughput sparkline bar glyphs (`DRW-02`).
pub const SPARKLINE_BARS: [char; 6] = ['▁', '▂', '▃', '▅', '▆', '▇'];

/// Screen edge where the slide-out drawer docks (`DRW-01`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrawerEdge {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl DrawerEdge {
    pub const fn cycle(self) -> Self {
        match self { Self::Left => Self::Right, Self::Right => Self::Top, Self::Top => Self::Bottom, Self::Bottom => Self::Left }
    }
    pub const fn is_vertical(self) -> bool { matches!(self, Self::Left | Self::Right) }
    pub const fn label(self) -> &'static str {
        match self { Self::Left => "Left", Self::Right => "Right", Self::Top => "Top", Self::Bottom => "Bottom" }
    }
}

/// Visibility policy for the slide-out drawer (`DRW-01`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrawerVisibilityMode {
    #[default]
    OnDemand,
    Fixed,
    AutoHide,
}

impl DrawerVisibilityMode {
    pub const fn toggle_pin(self) -> Self {
        match self { Self::Fixed => Self::AutoHide, Self::OnDemand | Self::AutoHide => Self::Fixed }
    }
    pub const fn cycle(self) -> Self {
        match self { Self::OnDemand => Self::Fixed, Self::Fixed => Self::AutoHide, Self::AutoHide => Self::OnDemand }
    }
    pub const fn label(self) -> &'static str {
        match self { Self::OnDemand => "OnDemand", Self::Fixed => "Fixed", Self::AutoHide => "AutoHide" }
    }
}

/// Workspace interaction layout mode when the drawer is visible (`DRW-01`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrawerLayoutMode {
    /// Floats over the active workspace with `occlude_rect` and Borland drop shadow (`zero SIGWINCH`).
    Overlay,
    /// Subtracts `drawer_rect` from `workspace_area` and reflows panes (`Push`).
    #[default]
    Push,
}

impl DrawerLayoutMode {
    pub const fn toggle(self) -> Self {
        match self { Self::Overlay => Self::Push, Self::Push => Self::Overlay }
    }
    pub const fn label(self) -> &'static str {
        match self { Self::Overlay => "Float", Self::Push => "Push" }
    }
}

/// Declarative `[drawer]` configuration in `~/.config/bilterm/config.toml` (`DRW-01`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawerConfig {
    pub edge: DrawerEdge,
    pub visibility: DrawerVisibilityMode,
    pub layout: DrawerLayoutMode,
    pub width_cols: u16,
    pub height_rows: u16,
    pub style: DrawerStyle,
    /// Legacy key `herdr_width_cols` is still accepted when loading configs.
    #[serde(alias = "herdr_width_cols")]
    pub navigator_width_cols: u16,
    /// Navigator style only: `2` (default) draws pane rows over two lines (state/title, then a
    /// shell-prompt style `user@host  path`); `1` restores the single-line layout. Other values are
    /// treated as `2`; read it through [`DrawerConfig::navigator_row_lines`].
    pub row_lines: u8,
}

impl Default for DrawerConfig {
    fn default() -> Self {
        Self {
            edge: DrawerEdge::Left,
            visibility: DrawerVisibilityMode::OnDemand,
            layout: DrawerLayoutMode::Push,
            width_cols: 34,
            height_rows: 6,
            style: DrawerStyle::Classic,
            navigator_width_cols: 28,
            row_lines: 2,
        }
    }
}

/// Semantic kind of a row rendered inside the slide-out drawer (`DRW-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawerRowKind {
    SessionHeader(String),
    LayoutGroup(WorkspaceLayoutMode),
    Pane(PaneId),
    QuickTerminal,
    FleetNode(String),
}

/// Formatted telemetry row in the slide-out drawer (`DRW-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawerRow {
    pub kind: DrawerRowKind,
    pub connector: &'static str,
    pub focus_rank_badge: String,
    pub spinner: char,
    pub sparkline: String,
    pub cmd_badge: String,
    pub mode_badges: Vec<String>,
    pub transport_badges: Vec<String>,
    pub title: String,
    pub is_focused_pane: bool,
    pub navigator: Option<NavigatorRowMeta>,
}

impl DrawerRow {
    /// Formats a rich single-line telemetry string for rendering and snapshot inspection.
    pub fn formatted_line(&self) -> String {
        match &self.kind {
            DrawerRowKind::SessionHeader(name) => {
                let mut line = format!("◈ SESSION [{name}]");
                for b in self.mode_badges.iter().chain(self.transport_badges.iter()) {
                    line.push(' ');
                    line.push_str(b);
                }
                line
            }
            DrawerRowKind::LayoutGroup(mode) => {
                let label = match mode {
                    WorkspaceLayoutMode::Tiled => "WINDOW: TILED SPLITS",
                    WorkspaceLayoutMode::Stacked => "WINDOW: Z-ORDER DECK",
                };
                format!("{} {}", self.connector, label)
            }
            DrawerRowKind::Pane(_) | DrawerRowKind::QuickTerminal => {
                let mut line = format!(
                    "{} {} {} {} {} {}",
                    self.connector,
                    self.focus_rank_badge,
                    self.spinner,
                    self.sparkline,
                    self.cmd_badge,
                    self.title
                );
                for b in self.mode_badges.iter().chain(self.transport_badges.iter()) {
                    line.push(' ');
                    line.push_str(b);
                }
                line
            }
            DrawerRowKind::FleetNode(_) => {
                let mut line = format!(
                    "{} {} {} {}",
                    self.connector, self.focus_rank_badge, self.cmd_badge, self.title
                );
                for t in &self.transport_badges {
                    line.push(' ');
                    line.push_str(t);
                }
                line
            }
        }
    }
}

/// Runtime state of the workspace slide-out drawer (`DRW-01`, `DRW-02`).
#[derive(Clone, Debug, Default)]
pub struct WorkspaceDrawer {
    pub open: bool,
    pub focused: bool,
    pub maximized: bool,
    pub hover_active: bool,
    pub selected_index: usize,
    pub scroll_offset: usize,
    /// Columns the row text is scrolled left by (46-04); 0 = unchanged rendering.
    pub h_scroll: usize,
    /// Wheel-set viewport `(selected_index it was set for, first visible row)`; ignored once the selection moves.
    pub view_scroll: Option<(usize, usize)>,
    pub resizing_drag: bool,
    /// Open navigator right-click menu (Phase 50-15); changed only through `DrawerNavOp::Menu*`.
    nav_menu: Option<NavigatorMenu>,
    pub navigator: navigator::NavigatorDrawerState,
    pane_prev_bytes: BTreeMap<PaneId, usize>,
    pane_sparklines: BTreeMap<PaneId, [u8; 6]>,
    tick_counter: usize,
    /// Saved-session names snapshot used by the Navigator row builder (WR-05). Refilled lazily
    /// after [`Self::invalidate_saved_sessions`], so one frame or event sees one list and the
    /// directory is not rescanned by every layout helper.
    saved_sessions: std::cell::RefCell<Option<Vec<String>>>,
    /// Pane and tick of a pending Navigator close of a Working pane (WR-06).
    close_armed: Option<(PaneId, usize)>,
}

impl WorkspaceDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Saved-session snapshot, filled by `fill` on first use after an invalidation.
    pub(crate) fn cached_saved_sessions(&self, fill: impl FnOnce() -> Vec<String>) -> Vec<String> {
        if let Some(list) = self.saved_sessions.borrow().as_ref() {
            return list.clone();
        }
        let list = fill();
        *self.saved_sessions.borrow_mut() = Some(list.clone());
        list
    }

    /// Drops the saved-session snapshot (any action may create, rename or delete a session).
    pub(crate) fn invalidate_saved_sessions(&self) {
        *self.saved_sessions.borrow_mut() = None;
    }

    /// First visible row: a wheel-set viewport while the selection is unchanged, else pinned to keep `selected` visible.
    pub fn effective_scroll(&self, selected: usize, cap: usize, len: usize) -> usize {
        let max = len.saturating_sub(cap);
        if let Some((sel, v)) = self.view_scroll
            && sel == selected
        {
            return v.min(max);
        }
        let mut scroll = self.scroll_offset;
        if selected < scroll {
            scroll = selected;
        } else if selected >= scroll + cap {
            scroll = selected + 1 - cap;
        }
        scroll.min(max)
    }

    /// Wheel scroll: moves the viewport by `delta` rows without touching the selection.
    pub fn wheel_scroll(&mut self, selected: usize, cap: usize, len: usize, delta: isize) {
        let cur = self.effective_scroll(selected, cap, len) as isize;
        let v = cur.saturating_add(delta).clamp(0, len.saturating_sub(cap) as isize) as usize;
        self.view_scroll = Some((selected, v));
    }

    pub fn sparkline_for(&self, pane_id: PaneId, total_bytes: usize) -> String {
        if let Some(buckets) = self.pane_sparklines.get(&pane_id)
            && buckets.iter().any(|&b| b > 0)
        {
            return buckets
                .iter()
                .map(|&idx| SPARKLINE_BARS[usize::from(idx).min(SPARKLINE_BARS.len() - 1)])
                .collect();
        }
        if total_bytes == 0 {
            "▁▁▁▁▁▁".to_string()
        } else {
            let level = (total_bytes / 16).clamp(1, 5);
            let mut s = String::from("▁▁▁▂▃");
            s.push(SPARKLINE_BARS[level]);
            s
        }
    }

    pub fn spinner_for(&self, pane: &CompositePane) -> char {
        let idx = (pane.vt().total_bytes_processed().wrapping_add(self.tick_counter))
            % BRAILLE_SPINNER_FRAMES.len();
        BRAILLE_SPINNER_FRAMES[idx]
    }
}

fn pane_cmd_badge(pane: &CompositePane) -> String {
    if pane.prompt().running_command.is_some()
        || matches!(pane.vt().osc133_phase(), Some(Osc133Phase::CommandExecuted))
    {
        return "⚙ RUN".to_string();
    }
    if let Some(Osc133Phase::CommandFinished(Some(code))) = pane.vt().osc133_phase() {
        return if code == 0 { "✓ 0".to_string() } else { format!("✗ {code}") };
    }
    if let Some(last_block) = pane.vt().command_blocks().last() {
        return match last_block.exit_code {
            Some(0) => "✓ 0".to_string(),
            Some(code) => format!("✗ {code}"),
            None => "⚙ RUN".to_string(),
        };
    }
    "✓ 0".to_string()
}

fn subtract_dock_rect(area: Rect, edge: DrawerEdge, dr: Rect) -> Rect {
    match edge {
        DrawerEdge::Left => Rect::new(area.x + dr.width, area.y, area.width.saturating_sub(dr.width).max(1), area.height),
        DrawerEdge::Right => Rect::new(area.x, area.y, area.width.saturating_sub(dr.width).max(1), area.height),
        DrawerEdge::Top => Rect::new(area.x, area.y + dr.height, area.width, area.height.saturating_sub(dr.height).max(1)),
        DrawerEdge::Bottom => Rect::new(area.x, area.y, area.width, area.height.saturating_sub(dr.height).max(1)),
    }
}

impl WindowManager {
    pub fn drawer(&self) -> &WorkspaceDrawer { &self.drawer }
    pub fn drawer_mut(&mut self) -> &mut WorkspaceDrawer { &mut self.drawer }
    pub fn drawer_is_open(&self) -> bool { self.drawer.open || self.drawer.focused }
    pub fn drawer_config(&self) -> &DrawerConfig { &self.config.drawer }
    pub fn drawer_config_mut(&mut self) -> &mut DrawerConfig { &mut self.config.drawer }

    /// Returns `true` if any background pane has unread notifications or has exited (`AutoHide` alert trigger).
    pub fn has_background_pane_alert(&self) -> bool {
        let active_id = self.active_pane_id();
        self.panes.iter().any(|(&id, pane)| {
            !pane.vt().notifications().is_empty() || (id != active_id && pane.has_exited())
        })
    }

    /// Returns `true` when the slide-out drawer should be rendered on screen (`DRW-01`).
    pub fn is_drawer_effectively_visible(&self) -> bool {
        if self.drawer.open || self.drawer.focused {
            return true;
        }
        match self.config.drawer.visibility {
            DrawerVisibilityMode::OnDemand => false,
            DrawerVisibilityMode::Fixed => true,
            DrawerVisibilityMode::AutoHide => {
                self.leader_pending || self.drawer.hover_active || self.has_background_pane_alert()
            }
        }
    }

    /// Computes the bounding [`Rect`] of the slide-out drawer within `workspace_area` when visible (`DRW-01`).
    pub fn drawer_rect(&self, workspace_area: Rect) -> Option<Rect> {
        if !self.is_drawer_effectively_visible() || workspace_area.width < 12 || workspace_area.height < 4 {
            return None;
        }
        let cfg = &self.config.drawer;
        let (w, h) = if self.drawer.maximized {
            let mw = ((workspace_area.width * 2) / 3).clamp(44.min(workspace_area.width), workspace_area.width);
            let mh = ((workspace_area.height * 2) / 3).clamp(10.min(workspace_area.height), workspace_area.height);
            (mw, mh)
        } else {
            let max_w = (workspace_area.width / 2).max(12);
            let max_h = (workspace_area.height / 2).max(3);
            (
                cfg.effective_width_cols().clamp(12, max_w).min(workspace_area.width),
                cfg.height_rows.clamp(3, max_h).min(workspace_area.height),
            )
        };
        Some(match cfg.edge {
            DrawerEdge::Left => Rect::new(workspace_area.x, workspace_area.y, w, workspace_area.height),
            DrawerEdge::Right => Rect::new(workspace_area.x + workspace_area.width.saturating_sub(w), workspace_area.y, w, workspace_area.height),
            DrawerEdge::Top => Rect::new(workspace_area.x, workspace_area.y, workspace_area.width, h),
            DrawerEdge::Bottom => Rect::new(workspace_area.x, workspace_area.y + workspace_area.height.saturating_sub(h), workspace_area.width, h),
        })
    }

    /// Returns the effective workspace area available for panes (`Overlay` leaves `workspace_area` unchanged; `Push` subtracts `drawer_rect` and/or `mcp_console` push rect).
    pub fn effective_workspace_area(&self, workspace_area: Rect) -> Rect {
        let mut area = workspace_area;
        if self.config.drawer.layout == DrawerLayoutMode::Push
            && let Some(dr) = self.drawer_rect(area)
        {
            area = subtract_dock_rect(area, self.config.drawer.edge, dr);
        }
        if self.mcp_console.active
            && self.mcp_console.layout_mode == DrawerLayoutMode::Push
            && let Some(mr) = self.mcp_console.panel_rect(area)
        {
            area = subtract_dock_rect(area, self.mcp_console.edge, mr);
        }
        self.subtract_docked_panels(area)
    }

    /// Updates rolling throughput sparklines and spinner ticks across all panes (`DRW-02`).
    pub fn update_drawer_telemetry(&mut self) {
        self.drawer.tick_counter = self.drawer.tick_counter.wrapping_add(1);
        // Pick up sessions saved by other processes about twice a second (WR-05).
        if self.drawer.tick_counter.is_multiple_of(32) {
            self.drawer.invalidate_saved_sessions();
        }
        let ids: Vec<(PaneId, usize)> = self
            .panes
            .iter()
            .map(|(&id, p)| (id, p.vt().total_bytes_processed()))
            .collect();
        for (id, total) in ids {
            let prev = self.drawer.pane_prev_bytes.insert(id, total).unwrap_or(0);
            let delta = total.saturating_sub(prev);
            if delta > 0 || !self.drawer.pane_sparklines.contains_key(&id) {
                let bucket = match delta {
                    0 => 0,
                    1..=32 => 1,
                    33..=128 => 2,
                    129..=512 => 3,
                    513..=2048 => 4,
                    _ => 5,
                };
                let entry = self.drawer.pane_sparklines.entry(id).or_insert([0; 6]);
                if delta > 0 {
                    entry.rotate_left(1);
                    entry[5] = bucket;
                }
            }
        }
    }

    /// Returns all active `PaneId`s in their visual layout (`SplitTree` leaf) order (`DRW-02`).
    pub fn ordered_pane_ids(&self) -> Vec<PaneId> {
        let mut ids = self.split_tree.leaves();
        for &id in self.panes.keys() {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids.retain(|id| self.panes.contains_key(id));
        ids
    }

    /// Builds the complete list of hierarchical [`DrawerRow`]s with live telemetry glyphs (`DRW-02`).
    fn classic_drawer_rows(&self) -> Vec<DrawerRow> {
        let mut rows = Vec::new();
        let mut sess_modes = Vec::new();
        if self.is_workspace_recording() {
            sess_modes.push("⏺ REC".to_string());
        }
        if self.broadcast_input {
            sess_modes.push("📡 BCAST".to_string());
        }
        if let Some(p) = self
            .daemon_state
            .session_tree
            .node(&self.config.history_session)
            .and_then(|n| n.parent.clone())
        {
            sess_modes.push(format!("[↳ {p}]"));
        }
        let mut sess_transports = Vec::new();
        if self.is_tmux_control_attached() {
            sess_transports.push("🔗 TMUX".to_string());
        }
        rows.push(DrawerRow {
            kind: DrawerRowKind::SessionHeader(self.config.history_session.clone()),
            connector: "",
            focus_rank_badge: String::new(),
            spinner: '◈',
            sparkline: String::new(),
            cmd_badge: format!("{}p", self.panes.len()),
            mode_badges: sess_modes,
            transport_badges: sess_transports,
            title: self.config.history_session.clone(),
            is_focused_pane: false,
            navigator: None,
        });

        let has_extra_groups = self.quick_terminal.visible || !self.fleet.nodes.is_empty();
        rows.push(DrawerRow {
            kind: DrawerRowKind::LayoutGroup(self.layout_mode),
            connector: if has_extra_groups { "├─" } else { "╰─" },
            focus_rank_badge: String::new(),
            spinner: '⊞',
            sparkline: String::new(),
            cmd_badge: String::new(),
            mode_badges: Vec::new(),
            transport_badges: Vec::new(),
            title: match self.layout_mode {
                WorkspaceLayoutMode::Tiled => "Tiled SplitTree".to_string(),
                WorkspaceLayoutMode::Stacked => "Stacked Z-Deck".to_string(),
            },
            is_focused_pane: false,
            navigator: None,
        });

        let top_id = self.z_stack.top();
        let qt_visible = self.quick_terminal.visible;
        let ordered_ids = self.ordered_pane_ids();
        let pane_count = ordered_ids.len();

        for (idx, id) in ordered_ids.into_iter().enumerate() {
            let Some(pane) = self.panes.get(&id) else {
                continue;
            };
            let connector = if idx + 1 == pane_count && !has_extra_groups { "╰─" } else { "├─" };
            let is_focused = !qt_visible && top_id == Some(id);
            let (z_pos, z_tot) = self.z_stack.rank_of(id).unwrap_or((idx + 1, pane_count));
            let z_r = z_tot.saturating_sub(z_pos) + 1;
            let focus_marker = if is_focused { "►" } else { " " };
            let focus_rank_badge = format!("{focus_marker} [{id}] Z:{z_r}/{z_tot}");

            let mut mode_badges = Vec::new();
            if pane.vt().is_alternate_screen() { mode_badges.push("🖥 ALT".to_string()); }
            if pane.is_password_prompt() || pane.is_active_canvas_password_prompt() {
                mode_badges.push("🔑 !ECHO".to_string());
            }
            if pane.is_read_only() { mode_badges.push("🔒 PROT".to_string()); }
            if pane.vt().is_recording() || self.is_workspace_recording() {
                mode_badges.push("⏺ REC".to_string());
            }
            if self.broadcast_input { mode_badges.push("📡 BCAST".to_string()); }
            let notif_count = pane.vt().notifications().len();
            if notif_count > 0 { mode_badges.push(format!("🔔 {notif_count}")); }
            let unread_mail = self.agent_mail.unread_count_by_pane(id);
            if unread_mail > 0 { mode_badges.push(format!("✉ {unread_mail}")); }

            let mut transport_badges = Vec::new();
            let is_ssh = pane.remote_target().is_some()
                || self.fleet.nodes.values().any(|n| match &n.kind {
                    FleetNodeKind::SshPane { pane_id, .. } => *pane_id == id,
                    FleetNodeKind::SshTmux { pane_ids, .. } => pane_ids.contains(&id),
                    FleetNodeKind::HostProfile { .. } => false,
                });
            if is_ssh { transport_badges.push("🌐 SSH".to_string()); }
            let is_tmux = pane.title().contains("TMUX")
                || (self.tmux_bridge.bilterm_to_tmux.contains_key(&id) && (id != 1 || pane.title().contains("TMUX")))
                || self.fleet.nodes.values().any(
                    |n| matches!(&n.kind, FleetNodeKind::SshTmux { pane_ids, .. } if pane_ids.contains(&id)),
                );
            if is_tmux { transport_badges.push("🔗 TMUX".to_string()); }

            rows.push(DrawerRow {
                kind: DrawerRowKind::Pane(id),
                connector,
                focus_rank_badge,
                spinner: self.drawer.spinner_for(pane),
                sparkline: self.drawer.sparkline_for(id, pane.vt().total_bytes_processed()),
                cmd_badge: pane_cmd_badge(pane),
                mode_badges,
                transport_badges,
                title: if self.is_window_selected(id) {
                    format!("[✓] {}", pane.title())
                } else {
                    pane.title().to_string()
                },
                is_focused_pane: is_focused,
                navigator: None,
            });
        }

        if qt_visible {
            let qt_pane = &self.quick_terminal.pane;
            rows.push(DrawerRow {
                kind: DrawerRowKind::QuickTerminal,
                connector: if self.fleet.nodes.is_empty() { "╰─" } else { "├─" },
                focus_rank_badge: "► [Q] Z:TOP".to_string(),
                spinner: self.drawer.spinner_for(qt_pane),
                sparkline: self.drawer.sparkline_for(0, qt_pane.vt().total_bytes_processed()),
                cmd_badge: pane_cmd_badge(qt_pane),
                mode_badges: Vec::new(),
                transport_badges: Vec::new(),
                title: "QUAKE QUICK TERM".to_string(),
                is_focused_pane: true,
                navigator: None,
            });
        }

        let fleet_len = self.fleet.nodes.len();
        for (f_idx, (name, node)) in self.fleet.nodes.iter().enumerate() {
            let connector = if f_idx + 1 == fleet_len { "╰─" } else { "├─" };
            let mut transport_badges = vec!["🌐 SSH".to_string()];
            if matches!(node.kind, FleetNodeKind::SshTmux { .. }) {
                transport_badges.push("🔗 TMUX".to_string());
            }
            rows.push(DrawerRow {
                kind: DrawerRowKind::FleetNode(name.clone()),
                connector,
                focus_rank_badge: format!("  [@{name}]"),
                spinner: '🌐',
                sparkline: "▁▁▁▁▁▁".to_string(),
                cmd_badge: if node.connected { "✓ ON".to_string() } else { "✗ OFF".to_string() },
                mode_badges: Vec::new(),
                transport_badges,
                title: name.clone(),
                is_focused_pane: false,
                navigator: None,
            });
        }
        let mut extra_sessions = self.list_sessions();
        for node in &self.daemon_state.session_tree.nodes {
            let is_explicit = node.parent.is_some()
                || !node.children.is_empty()
                || (node.name != "main" && node.name != self.config.history_session);
            if is_explicit && !extra_sessions.iter().any(|s| s == &node.name) {
                extra_sessions.push(node.name.clone());
            }
        }
        for saved in extra_sessions {
            if saved != self.config.history_session {
                let parent_opt = self
                    .daemon_state
                    .session_tree
                    .node(&saved)
                    .and_then(|n| n.parent.clone());
                let mut mode_badges = vec!["[SAVED]".to_string()];
                if let Some(ref p) = parent_opt {
                    mode_badges.push(format!("[↳ {p}]"));
                }
                rows.push(DrawerRow {
                    kind: DrawerRowKind::SessionHeader(saved.clone()),
                    connector: if parent_opt.is_some() { "├─" } else { "" },
                    focus_rank_badge: String::new(),
                    spinner: '◈',
                    sparkline: String::new(),
                    cmd_badge: "SAVED".to_string(),
                    mode_badges,
                    transport_badges: Vec::new(),
                    title: saved,
                    is_focused_pane: false,
                    navigator: None,
                });
            }
        }
        rows
    }
}
