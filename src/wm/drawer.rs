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
        area
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

            let mut transport_badges = Vec::new();
            let is_ssh = pane.remote_target().is_some()
                || self.fleet.nodes.values().any(|n| match &n.kind {
                    FleetNodeKind::SshPane { pane_id, .. } => *pane_id == id,
                    FleetNodeKind::SshTmux { pane_ids, .. } => pane_ids.contains(&id),
                    FleetNodeKind::HostProfile { .. } => false,
                });
            if is_ssh { transport_badges.push("🌐 SSH".to_string()); }
            let is_tmux = pane.title().contains("TMUX")
                || self.is_tmux_control_attached()
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
                title: pane.title().to_string(),
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

    pub fn reflow_default_viewport(&mut self) {
        let area = Rect::new(0, 0, self.default_size.1, self.default_size.0);
        let _ = self.resize_all(area);
    }

    pub fn toggle_drawer(&mut self) {
        if self.is_drawer_effectively_visible() && (self.drawer.open || self.drawer.focused) {
            self.drawer.open = false;
            self.drawer.focused = false;
            self.drawer.nav_menu = None;
            self.drawer.hover_active = false;
            self.drawer.maximized = false;
            if self.config.drawer.visibility == DrawerVisibilityMode::Fixed {
                self.config.drawer.visibility = DrawerVisibilityMode::OnDemand;
            }
        } else {
            self.drawer.open = true;
            self.drawer.focused = true;
            self.sync_drawer_selection_to_active_pane();
        }
        self.reflow_default_viewport();
    }

    pub fn focus_drawer(&mut self) {
        self.drawer.open = true;
        self.drawer.focused = true;
        self.sync_drawer_selection_to_active_pane();
        self.reflow_default_viewport();
    }

    pub fn close_drawer(&mut self) {
        self.drawer.focused = false;
        self.drawer.nav_menu = None;
        self.drawer.maximized = false;
        if self.config.drawer.visibility != DrawerVisibilityMode::Fixed {
            self.drawer.open = false;
            self.drawer.hover_active = false;
        }
        self.reflow_default_viewport();
    }

    pub fn toggle_maximize_drawer(&mut self) -> bool {
        self.drawer.open = true;
        self.drawer.maximized = !self.drawer.maximized;
        self.reflow_default_viewport();
        self.drawer.maximized
    }

    pub fn sync_drawer_selection_to_active_pane(&mut self) {
        let rows = self.drawer_rows();
        if let Some(pos) = rows.iter().position(|r| r.is_focused_pane) {
            self.drawer.selected_index = pos;
        } else if self.drawer.selected_index >= rows.len() {
            self.drawer.selected_index = rows.len().saturating_sub(1);
        }
    }

    pub fn drawer_select_next(&mut self) {
        let len = self.drawer_rows().len();
        if len > 0 {
            self.drawer.selected_index = (self.drawer.selected_index + 1) % len;
        }
    }

    pub fn drawer_select_prev(&mut self) {
        let len = self.drawer_rows().len();
        if len > 0 {
            self.drawer.selected_index = (self.drawer.selected_index + len - 1) % len;
        }
    }

    /// Scrolls drawer row text horizontally; clamped so the longest row's tail stays reachable.
    pub fn drawer_hscroll_by(&mut self, delta: isize) -> bool {
        let max = self
            .drawer_rows()
            .iter()
            .map(|r| r.formatted_line().chars().count())
            .max()
            .unwrap_or(0)
            .saturating_sub(12);
        let cur = self.drawer.h_scroll as isize;
        self.drawer.h_scroll = cur.saturating_add(delta).clamp(0, max as isize) as usize;
        self.drawer.h_scroll as isize != cur
    }

    pub fn drawer_scroll_by(&mut self, delta: isize) {
        let len = self.drawer_rows().len();
        if len > 0 {
            let cur = self.drawer.selected_index as isize;
            let max_idx = (len - 1) as isize;
            self.drawer.selected_index = (cur.saturating_add(delta)).clamp(0, max_idx) as usize;
        }
    }

    pub fn selected_drawer_pane_id(&self) -> Option<PaneId> {
        let rows = self.drawer_rows();
        let row = rows.get(self.drawer.selected_index)?;
        match &row.kind {
            DrawerRowKind::Pane(id) => Some(*id),
            DrawerRowKind::FleetNode(name) => self.fleet.nodes.get(name).and_then(|n| match &n.kind {
                FleetNodeKind::SshPane { pane_id, .. } => Some(*pane_id),
                FleetNodeKind::SshTmux { pane_ids, .. } => pane_ids.first().copied(),
                FleetNodeKind::HostProfile { .. } => None,
            }),
            _ => None,
        }
    }

    pub fn drawer_activate_selected(&mut self, unfocus_after: bool) {
        let rows = self.drawer_rows();
        if let Some(row) = rows.get(self.drawer.selected_index) {
            match &row.kind {
                DrawerRowKind::Pane(id) => {
                    self.focus_and_raise(*id);
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Selected Pane #{id} (Tab/Esc to type in pane, ↑/↓ to keep browsing drawer)"
                    )));
                }
                DrawerRowKind::QuickTerminal => { self.quick_terminal.visible = true; }
                DrawerRowKind::LayoutGroup(_) => { self.toggle_layout_mode(); }
                DrawerRowKind::FleetNode(name) => {
                    if let Some(pid) = self.selected_drawer_pane_id() {
                        self.focus_and_raise(pid);
                    } else {
                        let name_clone = name.clone();
                        self.active_pane_mut().set_status_message(Some(format!("Selected fleet node @{name_clone}")));
                    }
                }
                DrawerRowKind::SessionHeader(name) => {
                    if name != &self.config.history_session {
                        let name_clone = name.clone();
                        if let Ok((sess, count, _)) = self.switch_or_create_session(&name_clone) {
                            self.active_pane_mut().set_status_message(Some(format!(
                                "Switched to session '{sess}' ({count} pane(s))"
                            )));
                        }
                    } else if let Some(top_id) = self.z_stack.top() {
                        self.focus_and_raise(top_id);
                    }
                }
            }
        }
        if unfocus_after {
            self.drawer.focused = false;
            self.reflow_default_viewport();
        }
    }

    pub fn drawer_close_selected_pane(&mut self) -> bool {
        if let Some(id) = self.selected_drawer_pane_id() {
            let removed = self.remove_pane(id);
            let len = self.drawer_rows().len();
            if self.drawer.selected_index >= len && len > 0 {
                self.drawer.selected_index = len - 1;
            }
            return removed;
        }
        false
    }

    pub fn drawer_zoom_selected_pane(&mut self) {
        if let Some(id) = self.selected_drawer_pane_id() {
            self.focus_and_raise(id);
            self.toggle_zoom();
        }
    }

    pub fn selected_drawer_session_name(&self) -> String {
        let rows = self.drawer_rows();
        if let Some(DrawerRow {
            kind: DrawerRowKind::SessionHeader(s),
            ..
        }) = rows.get(self.drawer.selected_index)
        {
            return s.clone();
        }
        self.config.history_session.clone()
    }

    pub fn drawer_move_selected_session(&mut self, up: bool) {
        let rows = self.drawer_rows();
        if let Some(DrawerRow {
            kind: DrawerRowKind::Pane(id),
            ..
        }) = rows.get(self.drawer.selected_index)
        {
            let pane_id = *id;
            let ordered = self.ordered_pane_ids();
            if let Some(pos) = ordered.iter().position(|&p| p == pane_id) {
                let partner_opt = if up {
                    pos.checked_sub(1).and_then(|i| ordered.get(i).copied())
                } else {
                    ordered.get(pos + 1).copied()
                };
                if let Some(partner_id) = partner_opt
                    && self.split_tree.swap_leaves(pane_id, partner_id)
                {
                    self.reflow_default_viewport();
                    if let Some(new_row) = self
                        .drawer_rows()
                        .iter()
                        .position(|r| r.kind == DrawerRowKind::Pane(pane_id))
                    {
                        self.drawer.selected_index = new_row;
                    }
                    let dir = if up { "up" } else { "down" };
                    let order_lbl: Vec<String> = self
                        .ordered_pane_ids()
                        .into_iter()
                        .map(|p| format!("#{p}"))
                        .collect();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Moved Pane #{pane_id} {dir} (Window order: {})",
                        order_lbl.join(", ")
                    )));
                    return;
                }
            }
        }

        let target_sess = rows.get(self.drawer.selected_index).and_then(|r| match &r.kind {
            DrawerRowKind::SessionHeader(s) => Some(s.clone()),
            _ => None,
        });
        let name = target_sess
            .clone()
            .unwrap_or_else(|| self.config.history_session.clone());
        let _ = self.daemon_state.session_tree.move_node(&name, up);
        let dir = if up { "up" } else { "down" };
        if let Ok(order) = self.move_session_in_order(target_sess.as_deref(), dir) {
            if let Some(ref s) = target_sess
                && let Some(pos) = self
                    .drawer_rows()
                    .iter()
                    .position(|r| r.kind == DrawerRowKind::SessionHeader(s.clone()))
            {
                self.drawer.selected_index = pos;
            }
            self.active_pane_mut()
                .set_status_message(Some(format!("Session order: {}", order.join(", "))));
        }
    }

    /// Branches a new child session under the selected session in the drawer (`TREE-01`).
    pub fn drawer_branch_selected_session(&mut self) -> Option<String> {
        let parent = self.selected_drawer_session_name();
        self.sync_daemon_state_from_workspace();
        let child = format!(
            "{parent}/sub-{}",
            self.daemon_state.session_tree.nodes.len() + 1
        );
        if self.daemon_state.session_tree.branch(&parent, &child).is_ok() {
            if let Some(pos) = self
                .drawer_rows()
                .iter()
                .position(|r| r.kind == DrawerRowKind::SessionHeader(child.clone()))
            {
                self.drawer.selected_index = pos;
            }
            self.active_pane_mut().set_status_message(Some(format!(
                "Branched child session '{child}' under '{parent}'"
            )));
            Some(child)
        } else {
            None
        }
    }

    /// Reparents (`indent == true`, grafts under preceding session) or promotes to root (`indent == false`)
    /// the selected session in the drawer (`TREE-01`).
    pub fn drawer_reparent_selected_session(&mut self, indent: bool) -> bool {
        let rows = self.drawer_rows();
        let target = self.selected_drawer_session_name();
        self.sync_daemon_state_from_workspace();
        for r in &rows {
            if let DrawerRowKind::SessionHeader(s) = &r.kind
                && self.daemon_state.session_tree.node(s).is_none()
            {
                self.daemon_state.session_tree.upsert_session(
                    s,
                    crate::wm::session_tree::SessionNodeKind::Independent,
                    None,
                );
            }
        }
        let ok = if indent {
            let sess_names: Vec<String> = rows
                .iter()
                .filter_map(|r| match &r.kind {
                    DrawerRowKind::SessionHeader(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();
            let pos = sess_names.iter().position(|s| s == &target).unwrap_or(0);
            let parent_cand = if pos > 0 {
                sess_names.get(pos - 1).cloned()
            } else {
                sess_names.get(pos + 1).cloned()
            };
            if let Some(parent) = parent_cand
                && parent != target
            {
                match self.daemon_state.session_tree.graft(&target, &parent) {
                    Ok(()) => {
                        self.active_pane_mut().set_status_message(Some(format!(
                            "Reparented session '{target}' under '{parent}'"
                        )));
                        true
                    }
                    Err(e) => {
                        self.active_pane_mut().set_status_message(Some(e));
                        false
                    }
                }
            } else {
                false
            }
        } else {
            match self.daemon_state.session_tree.detach_subtree(&target) {
                Ok(()) => {
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Promoted session '{target}' to root [IND]"
                    )));
                    true
                }
                Err(e) => {
                    self.active_pane_mut().set_status_message(Some(e));
                    false
                }
            }
        };
        if let Some(pos) = self
            .drawer_rows()
            .iter()
            .position(|r| r.kind == DrawerRowKind::SessionHeader(target.clone()))
        {
            self.drawer.selected_index = pos;
        }
        ok
    }

    pub fn set_drawer_edge(&mut self, edge: Option<DrawerEdge>) -> DrawerEdge {
        self.config.drawer.edge = edge.unwrap_or_else(|| self.config.drawer.edge.cycle());
        self.reflow_default_viewport();
        self.config.drawer.edge
    }

    pub fn set_drawer_layout(&mut self, mode: Option<DrawerLayoutMode>) -> DrawerLayoutMode {
        self.config.drawer.layout = mode.unwrap_or_else(|| self.config.drawer.layout.toggle());
        self.reflow_default_viewport();
        self.config.drawer.layout
    }

    pub fn set_drawer_visibility(&mut self, mode: Option<DrawerVisibilityMode>) -> DrawerVisibilityMode {
        let next = mode.unwrap_or_else(|| self.config.drawer.visibility.toggle_pin());
        self.config.drawer.visibility = next;
        if next == DrawerVisibilityMode::Fixed {
            self.drawer.open = true;
        } else if next == DrawerVisibilityMode::OnDemand && !self.drawer.focused {
            self.drawer.open = false;
        }
        self.reflow_default_viewport();
        next
    }

    pub fn resize_drawer(&mut self, size: u16) {
        self.drawer.maximized = false;
        if self.config.drawer.edge.is_vertical() {
            self.config.drawer.set_width_cols_for_style(size.clamp(16, 100));
        } else {
            self.config.drawer.height_rows = size.clamp(3, 14);
        }
        self.reflow_default_viewport();
    }

    pub fn update_drawer_hover(&mut self, col: u16, row: u16, workspace_area: Rect) -> bool {
        let prev = self.drawer.hover_active;
        let at_edge = match self.config.drawer.edge {
            DrawerEdge::Left => col <= workspace_area.x + 1,
            DrawerEdge::Right => col >= workspace_area.x.saturating_add(workspace_area.width.saturating_sub(2)),
            DrawerEdge::Top => row <= workspace_area.y + 1,
            DrawerEdge::Bottom => row >= workspace_area.y.saturating_add(workspace_area.height.saturating_sub(2)),
        };
        let inside_open = self.drawer_rect(workspace_area).is_some_and(|r| crate::wm::rect_contains(r, col, row));
        self.drawer.hover_active = at_edge || (prev && inside_open);
        if prev != self.drawer.hover_active
            && self.config.drawer.visibility == DrawerVisibilityMode::AutoHide
            && self.config.drawer.layout == DrawerLayoutMode::Push
        {
            let _ = self.resize_all(workspace_area);
        }
        prev != self.drawer.hover_active
    }

    /// Executes any `Action::Drawer*` variant (`ARCH-R01`, `DRW-01`).
    pub fn execute_drawer_action(&mut self, action: &crate::action::Action) -> bool {
        use crate::action::Action;
        let msg = match action {
            Action::DrawerToggle => {
                self.toggle_drawer();
                format!("Slide-Out Drawer: {}", if self.is_drawer_effectively_visible() { "OPEN" } else { "HIDDEN" })
            }
            Action::DrawerFocus => {
                self.focus_drawer();
                "Slide-Out Drawer: FOCUSED".to_string()
            }
            Action::DrawerDock(edge_opt) => format!("Slide-Out Drawer Dock Edge: {}", self.set_drawer_edge(*edge_opt).label()),
            Action::DrawerVisibility(vis_opt) => format!("Slide-Out Drawer Visibility: {}", self.set_drawer_visibility(*vis_opt).label()),
            Action::DrawerLayout(layout_opt) => format!("Slide-Out Drawer Layout Mode: {}", self.set_drawer_layout(*layout_opt).label()),
            Action::DrawerSize(0) => {
                if self.drawer.maximized {
                    self.drawer.maximized = false;
                    self.reflow_default_viewport();
                    "Slide-Out Drawer: RESTORED [_]".to_string()
                } else {
                    self.config.drawer.visibility = DrawerVisibilityMode::OnDemand;
                    self.close_drawer();
                    "Slide-Out Drawer: MINIMIZED [_]".to_string()
                }
            }
            Action::DrawerSize(u16::MAX) => {
                let max_now = self.toggle_maximize_drawer();
                format!("Slide-Out Drawer: {}", if max_now { "MAXIMIZED [□]" } else { "RESTORED" })
            }
            Action::DrawerSize(size) => {
                self.resize_drawer(*size);
                format!("Slide-Out Drawer Size: {size}")
            }
            _ => return false,
        };
        self.active_pane_mut().set_status_message(Some(msg));
        true
    }
}
