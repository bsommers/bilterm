pub mod quick_terminal;
pub mod session;
pub mod split_tree;
pub mod z_stack;

pub use session::{SavedPaneState, SavedWorkspaceSession};

use std::collections::BTreeMap;
use std::time::Instant;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::config::BiltermConfig;
use crate::error::Result;
use crate::pane::history_store::HistorySettings;
use crate::pane::{CompositePane, PaneId};
use crate::pty::PtyConfig;
use crate::remote::FleetRegistry;
use crate::vt::scrollback::{FixedMemoryProbe, ScrollbackGovernor, SystemMemoryProbe};
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
    pub(super) next_pane_id: PaneId,
    pub(super) headless_mode: bool,
    pub(super) default_size: (u16, u16),
    /// Dynamic scrollback budget governor (`[scrollback]`).
    pub scrollback: ScrollbackGovernor,
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
            scrollback: if headless_mode {
                ScrollbackGovernor::new(Box::new(FixedMemoryProbe::default()))
            } else {
                ScrollbackGovernor::new(Box::new(SystemMemoryProbe))
            },
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

    /// Replaces the active configuration (theme + history persistence settings) at startup or reload.
    pub fn apply_config(&mut self, config: BiltermConfig) {
        self.config = config;
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
        if pane.history_store.apply_settings(settings) {
            pane.restore_persisted_history();
        }
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
        let mut new_pane = if self.headless_mode {
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
        Self::attach_history(&mut new_pane, &self.config.history_settings());

        self.panes.insert(new_id, new_pane);
        self.split_tree.split_leaf(target_id, new_id, direction);
        self.z_stack.push(new_id);
        self.zoomed_pane = None;
        self.rebalance_scrollback(Instant::now());
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
            self.rebalance_scrollback(Instant::now());
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
            && self.panes.contains_key(&zoomed_id)
        {
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
            let inner =
                if self.panes.len() > 1 || self.layout_mode == WorkspaceLayoutMode::Stacked {
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
    fn apply_config_restores_session_history_into_existing_and_split_panes() {
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("sessions/proj/history.log");
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(
            &log,
            "[2026-01-01T00:00:00Z | session=proj | run=r1 | pane=1] ls\n\
             [2026-01-01T00:00:01Z | session=proj | run=r1 | pane=1] ls\n\
             [2026-01-01T00:00:02Z | session=proj | run=r2 | pane=2] echo a\\necho b\n",
        )
        .unwrap();

        let mut wm = WindowManager::new_headless(25, 80);
        assert!(wm.active_pane().prompt.history.is_empty());
        let cfg = BiltermConfig {
            history_dir: Some(tmp.path().to_string_lossy().into_owned()),
            history_session: "proj".to_string(),
            ..BiltermConfig::default()
        };
        wm.apply_config(cfg);
        let expected = vec!["ls".to_string(), "echo a\necho b".to_string()];
        assert_eq!(wm.active_pane().prompt.history, expected);

        wm.split_focused(SplitDirection::Vertical).unwrap();
        assert_eq!(wm.active_pane().prompt.history, expected);
        assert_eq!(wm.active_pane().history_store.session_file_path, log);
    }

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
