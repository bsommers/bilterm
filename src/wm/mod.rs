pub mod drawer;
pub mod quick_terminal;
pub mod session;
pub mod split_tree;
pub mod z_stack;

pub use drawer::{
    DrawerConfig, DrawerEdge, DrawerLayoutMode, DrawerRow, DrawerRowKind, DrawerVisibilityMode,
    WorkspaceDrawer,
};
pub use session::{SavedPaneState, SavedWorkspaceSession};
pub use z_stack::{FloatingWindowGeometry, WindowResizeEdge, WmMouseDrag};

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

use crate::action::Action;
use crate::colon_command::execute_wm_colon_command;
use crate::config::BiltermConfig;
use crate::error::Result;
use crate::input::AppAction;
use crate::pane::{CompositePane, PaneId};
use crate::pty::PtyConfig;
use crate::remote::FleetRegistry;
use crate::vt::recording::WorkspaceRecorder;
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

/// Exclusive workspace modal state (`ARCH-R02.4`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Modal {
    /// No workspace modal is open.
    #[default]
    None,
    /// Interactive `<F1>` Borland Turbo Vision Help modal.
    Help { scroll: usize },
    /// Interactive `<F9>` / `:inspector` Live 3270 Terminal Inspector modal (`BEL-01`).
    Inspector { scroll: usize },
    /// Interactive Fuzzy Command Palette modal (`Ctrl+P` / `:palette`, `v0.6.0`).
    Palette { query: String, selected: usize },
    /// Active pull-down category (`0..7`) and selected row inside the top Borland Menu Bar (`ECO-03`).
    Menubar { category: usize, item: usize },
}

/// Workspace focus target without `PaneId 0` sentinel collision (`ARCH-R02.4`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Focus {
    /// Persistent Guake/Quake-style drop-down quick terminal overlay.
    Quick,
    /// Standard workspace pane in `SplitTree` / `ZStack`.
    Pane(PaneId),
}

/// Multi-pane Window Manager coordinating tiled H/V splits, Z-order stacking/shuffling,
/// zoom/maximize, SGR 1006 local mouse coordinate translation, and the persistent Guake
/// drop-down `QuickTerminal` (`WM-01..05`).
pub struct WindowManager {
    panes: BTreeMap<PaneId, CompositePane>,
    split_tree: SplitTree,
    z_stack: ZStack,
    layout_mode: WorkspaceLayoutMode,
    zoomed_pane: Option<PaneId>,
    quick_terminal: QuickTerminal,
    config: BiltermConfig,
    /// Active exclusive modal overlay (`Help`, `Inspector`, `Palette`, `Menubar`, or `None`).
    modal: Modal,
    /// Synchronized Multi-Pane Input Broadcast (`:broadcast`, `ECO-04`).
    broadcast_input: bool,
    /// Whether the Prefix/Leader key (`Ctrl+X` by default) is awaiting the next chord (`v1.5.0`).
    leader_pending: bool,
    /// Central `herdr`-style fleet registry managing local and remote SSH / `tmux -CC` sessions (`v1.4.0`).
    fleet: FleetRegistry,
    next_pane_id: PaneId,
    headless_mode: bool,
    default_size: (u16, u16),
    /// Dynamic scrollback budget governor (`[scrollback]`).
    scrollback: ScrollbackGovernor,
    /// Whole-workspace Asciicast v2 (`.cast`) compositor frame recorder (`REC-02`).
    workspace_recorder: Mutex<Option<WorkspaceRecorder>>,
    /// Optional attached `tmux -CC` Control Mode subprocess (`TMUX-03`, `ARCH-R05`).
    pub(crate) tmux_control: Option<crate::tmux::TmuxControlSubprocess>,
    /// Bi-directional `tmux -CC` Control Mode parser/router (`TMUX-03`, `ARCH-R05`).
    pub(crate) tmux_bridge: crate::tmux::TmuxControlBridge,
    /// `herdr`-style slide-out Sessions / Windows / Panes & Fleet drawer runtime state (`DRW-01`, `DRW-02`).
    pub(crate) drawer: WorkspaceDrawer,
    /// Active mouse drag gesture for moving or resizing workspace windows/splits.
    pub(crate) wm_mouse_drag: Option<WmMouseDrag>,
}

impl WindowManager {
    /// Creates a `WindowManager` around an initial `CompositePane`.
    pub fn from_initial_pane(pane: CompositePane, headless_mode: bool) -> Self {
        let initial_id = pane.id();
        let (rows, cols) = pane.vt().size();
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
            modal: Modal::None,
            broadcast_input: false,
            leader_pending: false,
            fleet: FleetRegistry::new(),
            next_pane_id: initial_id + 1,
            headless_mode,
            default_size: (screen_rows, cols),
            scrollback: if headless_mode {
                ScrollbackGovernor::new(Box::new(FixedMemoryProbe::default()))
            } else {
                ScrollbackGovernor::new(Box::new(SystemMemoryProbe))
            },
            workspace_recorder: Mutex::new(None),
            tmux_control: None,
            tmux_bridge: crate::tmux::TmuxControlBridge::new(),
            drawer: WorkspaceDrawer::new(),
            wm_mouse_drag: None,
        }
    }

    /// Executes a unified [`Action`] against the workspace and active pane (`ARCH-R01.1`).
    pub fn apply(&mut self, action: Action) -> Result<AppAction> {
        if execute_wm_colon_command(self, action) {
            Ok(AppAction::Quit)
        } else {
            Ok(AppAction::Handled)
        }
    }

    /// Starts whole-workspace `.cast` compositor frame recording (`REC-02`).
    pub fn start_workspace_recording(&mut self, path: PathBuf, private: bool) {
        let (rows, cols) = self.default_size;
        if let Ok(mut guard) = self.workspace_recorder.lock() {
            *guard = Some(WorkspaceRecorder::new(path, private, cols, rows));
        }
    }

    /// Returns `true` if whole-workspace `.cast` compositor frame recording is active.
    pub fn is_workspace_recording(&self) -> bool {
        self.workspace_recorder
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }

    /// Returns `true` if an active whole-workspace `.cast` recorder has captured at least one rendered frame.
    pub fn has_recorded_workspace_frames(&self) -> bool {
        self.workspace_recorder
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|r| r.frame_count() > 0))
            .unwrap_or(false)
    }

    /// Captures a rendered workspace `Buffer` frame into the active `WorkspaceRecorder` (`REC-02`).
    pub fn record_workspace_frame(&self, buf: &Buffer) {
        if let Ok(mut guard) = self.workspace_recorder.lock()
            && let Some(recorder) = guard.as_mut()
        {
            recorder.capture_buffer(buf);
        }
    }

    /// Stops whole-workspace `.cast` recording and writes the `.cast` file to disk (`REC-02`).
    pub fn stop_workspace_recording(&mut self) -> std::io::Result<Option<PathBuf>> {
        let recorder_opt = self
            .workspace_recorder
            .lock()
            .ok()
            .and_then(|mut g| g.take());
        match recorder_opt {
            Some(rec) => rec.stop().map(Some),
            None => Ok(None),
        }
    }

    pub fn pane_count(&self) -> usize { self.panes.len() }
    pub fn pane(&self, id: PaneId) -> Option<&CompositePane> { self.panes.get(&id) }
    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut CompositePane> { self.panes.get_mut(&id) }
    pub fn panes(&self) -> &BTreeMap<PaneId, CompositePane> { &self.panes }
    pub fn panes_mut(&mut self) -> &mut BTreeMap<PaneId, CompositePane> { &mut self.panes }
    pub fn layout_mode(&self) -> WorkspaceLayoutMode { self.layout_mode }
    pub fn set_layout_mode(&mut self, mode: WorkspaceLayoutMode) { self.layout_mode = mode; }
    pub fn split_tree(&self) -> &SplitTree { &self.split_tree }
    pub fn z_stack(&self) -> &ZStack { &self.z_stack }
    pub fn zoomed_pane(&self) -> Option<PaneId> { self.zoomed_pane }
    pub fn quick_terminal(&self) -> &QuickTerminal { &self.quick_terminal }
    pub fn quick_terminal_mut(&mut self) -> &mut QuickTerminal { &mut self.quick_terminal }
    pub fn is_quick_terminal_visible(&self) -> bool { self.quick_terminal.visible }
    pub fn config(&self) -> &BiltermConfig { &self.config }
    pub fn config_mut(&mut self) -> &mut BiltermConfig { &mut self.config }
    pub fn modal(&self) -> &Modal { &self.modal }
    pub fn modal_mut(&mut self) -> &mut Modal { &mut self.modal }
    pub fn close_modal(&mut self) { self.modal = Modal::None; }
    pub fn is_help_visible(&self) -> bool { matches!(self.modal, Modal::Help { .. }) }

    pub fn help_scroll(&self) -> usize {
        match self.modal {
            Modal::Help { scroll } => scroll,
            _ => 0,
        }
    }

    pub fn is_inspector_visible(&self) -> bool {
        matches!(self.modal, Modal::Inspector { .. })
    }

    pub fn inspector_scroll(&self) -> usize {
        match self.modal {
            Modal::Inspector { scroll } => scroll,
            _ => 0,
        }
    }

    pub fn set_inspector_visible(&mut self, visible: bool) {
        if visible {
            self.modal = Modal::Inspector { scroll: 0 };
        } else if matches!(self.modal, Modal::Inspector { .. }) {
            self.modal = Modal::None;
        }
    }

    pub fn is_palette_visible(&self) -> bool {
        matches!(self.modal, Modal::Palette { .. })
    }

    pub fn palette_state(&self) -> Option<(&str, usize)> {
        match &self.modal {
            Modal::Palette { query, selected } => Some((query.as_str(), *selected)),
            _ => None,
        }
    }

    pub fn open_palette_with_query(&mut self, query: impl Into<String>) {
        self.modal = Modal::Palette {
            query: query.into(),
            selected: 0,
        };
    }

    pub fn menubar_open_category(&self) -> Option<usize> {
        match self.modal {
            Modal::Menubar { category, .. } => Some(category),
            _ => None,
        }
    }

    pub fn menubar_state(&self) -> Option<(usize, usize)> {
        match self.modal {
            Modal::Menubar { category, item } => Some((category, item)),
            _ => None,
        }
    }

    pub fn set_menubar_open_category(&mut self, category: Option<usize>) {
        match category {
            Some(cat) => {
                self.modal = Modal::Menubar {
                    category: cat,
                    item: 0,
                };
            }
            None => {
                if matches!(self.modal, Modal::Menubar { .. }) {
                    self.modal = Modal::None;
                }
            }
        }
    }

    pub fn is_broadcast_input(&self) -> bool {
        self.broadcast_input
    }

    pub fn is_leader_pending(&self) -> bool {
        self.leader_pending
    }

    pub fn set_leader_pending(&mut self, pending: bool) {
        self.leader_pending = pending;
    }

    pub fn fleet(&self) -> &FleetRegistry {
        &self.fleet
    }

    pub fn fleet_mut(&mut self) -> &mut FleetRegistry {
        &mut self.fleet
    }

    pub fn scrollback_mut(&mut self) -> &mut ScrollbackGovernor {
        &mut self.scrollback
    }

    /// Toggles the `<F1>` interactive Help Modal.
    pub fn toggle_help(&mut self) {
        if matches!(self.modal, Modal::Help { .. }) {
            self.modal = Modal::None;
        } else {
            self.modal = Modal::Help { scroll: 0 };
        }
    }

    /// Toggles the `<F9>` / `:inspector` Live 3270 Terminal Inspector modal (`BEL-01`).
    pub fn toggle_inspector(&mut self) {
        let next = !self.is_inspector_visible();
        self.set_inspector_visible(next);
    }

    /// Toggles the top 1-row Borland Turbo Vision Menu Bar (`:menubar`, `ECO-03`).
    pub fn toggle_menubar(&mut self) {
        self.config.show_top_menubar = !self.config.show_top_menubar;
        if !self.config.show_top_menubar && matches!(self.modal, Modal::Menubar { .. }) {
            self.modal = Modal::None;
        }
    }

    /// Sets or toggles Synchronized Multi-Pane Input Broadcast (`:broadcast`, `ECO-04`).
    pub fn set_broadcast_input(&mut self, enabled: bool) {
        self.broadcast_input = enabled;
        self.active_pane_mut().set_status_message(Some(format!(
            "Multi-Pane Input Broadcast: {}",
            if enabled { "ON (📡 BCAST)" } else { "OFF" }
        )));
    }

    /// Toggles the interactive Fuzzy Command Palette modal (`Ctrl+P` / `Alt+Space` / `:palette`, `v0.6.0`).
    pub fn toggle_palette(&mut self) {
        if matches!(self.modal, Modal::Palette { .. }) {
            self.modal = Modal::None;
        } else {
            self.modal = Modal::Palette {
                query: String::new(),
                selected: 0,
            };
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

    /// Returns the typed `Focus` target (`Focus::Quick` when the drop-down quick terminal is
    /// visible, or `Focus::Pane(top_id)` for the topmost workspace pane).
    pub fn focus(&self) -> Focus {
        if self.quick_terminal.visible {
            Focus::Quick
        } else {
            Focus::Pane(self.z_stack.top().unwrap_or(1))
        }
    }

    /// Returns a shared reference to the pane identified by `focus`.
    pub fn pane_by_focus(&self, focus: Focus) -> Option<&CompositePane> {
        match focus {
            Focus::Quick => Some(&self.quick_terminal.pane),
            Focus::Pane(id) => self.panes.get(&id),
        }
    }

    /// Returns a mutable reference to the pane identified by `focus`.
    pub fn pane_by_focus_mut(&mut self, focus: Focus) -> Option<&mut CompositePane> {
        match focus {
            Focus::Quick => Some(&mut self.quick_terminal.pane),
            Focus::Pane(id) => self.panes.get_mut(&id),
        }
    }

    /// Returns the currently active workspace `PaneId` (the top of `ZStack`).
    /// Never returns a `0` sentinel; use [`Self::focus`] to distinguish `Focus::Quick`.
    pub fn active_pane_id(&self) -> PaneId {
        self.z_stack.top().unwrap_or(1)
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

    /// Removes `target_id` from `panes`, `split_tree`, `z_stack`, `zoomed_pane`, and `fleet`
    /// while preserving the invariant that at least one workspace pane remains (`ARCH-R02`).
    pub fn remove_pane(&mut self, target_id: PaneId) -> bool {
        if self.panes.len() <= 1 || !self.panes.contains_key(&target_id) {
            return false;
        }
        let _ = self.split_tree.remove_leaf(target_id);
        self.panes.remove(&target_id);
        self.z_stack.remove(target_id);
        if self.zoomed_pane == Some(target_id) {
            self.zoomed_pane = None;
        }
        self.fleet.remove_pane_references(target_id);
        self.rebalance_scrollback(Instant::now());
        true
    }

    /// Closes the focused pane (if more than 1 workspace pane exists).
    pub fn close_focused_pane(&mut self) -> bool {
        if self.quick_terminal.visible {
            self.quick_terminal.visible = false;
            return true;
        }
        let Some(target_id) = self.z_stack.top() else {
            return false;
        };
        self.remove_pane(target_id)
    }

    /// Polls all workspace panes and the quick terminal for PTY output and reaps exited panes
    /// when more than one workspace pane exists (`ARCH-R02`).
    pub fn poll_all_ptys(&mut self) {
        let mut exited_ids = Vec::new();
        for (&id, pane) in self.panes.iter_mut() {
            pane.poll_pty_events();
            pane.refresh_line_discipline();
            if pane.has_exited() {
                exited_ids.push(id);
            }
        }
        for id in exited_ids {
            if self.panes.len() > 1 {
                let _ = self.remove_pane(id);
            }
        }
        self.quick_terminal.pane.poll_pty_events();
        self.quick_terminal.pane.refresh_line_discipline();
        let _ = self.poll_tmux_control();
        self.update_drawer_telemetry();
        self.maybe_rebalance_scrollback(Instant::now());
    }

    /// Returns `true` when every workspace pane has exited.
    pub fn all_panes_exited(&self) -> bool {
        self.panes.values().all(CompositePane::has_exited)
    }

    /// Drains any outbound OSC 52 clipboard payloads queued across workspace panes and the quick terminal.
    pub fn drain_outbound_osc52(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for pane in self
            .panes
            .values_mut()
            .chain(std::iter::once(&mut self.quick_terminal.pane))
        {
            out.extend(pane.take_all_outbound_osc52());
        }
        out
    }

    /// Submits `cmd` through [`CompositePane::submit_command`] across all writable workspace panes.
    pub fn broadcast_command(&mut self, cmd: &str) -> Result<usize> {
        let mut count = 0;
        for pane in self.panes.values_mut() {
            if !pane.is_read_only() && !pane.is_password_prompt() {
                pane.submit_command(cmd)?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Allocates and registers a new headless pane mapped to a `tmux -CC` `%pane_id`.
    pub fn insert_tmux_pane(&mut self, tmux_pane_id: usize) -> PaneId {
        let new_id = self
            .panes
            .keys()
            .max()
            .copied()
            .unwrap_or(1)
            .max(self.next_pane_id.saturating_sub(1))
            + 1;
        self.next_pane_id = new_id + 1;
        let (rows, cols) = self.active_pane().vt().size();
        let mut pane = CompositePane::new_headless(new_id, rows + 1, cols);
        pane.set_title(format!("TMUX %{tmux_pane_id}"));
        Self::attach_history(&mut pane, &self.config.history_settings());
        let top = self.z_stack.top().unwrap_or(1);
        self.panes.insert(new_id, pane);
        self.split_tree
            .split_leaf(top, new_id, SplitDirection::Vertical);
        self.z_stack.push(new_id);
        self.rebalance_scrollback(Instant::now());
        new_id
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
        let area = self.effective_workspace_area(area);
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
        let framed = self.is_framed();
        let rects = self.compute_workspace_rects(area);
        for (id, rect) in rects {
            let inner = if framed {
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
    /// Returns `(focus_target, pane_outer_rect, pane_inner_rect)`.
    pub fn hit_test_pane(&self, col: u16, row: u16, area: Rect) -> Option<(Focus, Rect, Rect)> {
        if self.quick_terminal.visible {
            let qt_outer = self.quick_terminal.overlay_rect(area);
            if rect_contains(qt_outer, col, row) {
                return Some((Focus::Quick, qt_outer, inset_window_frame(qt_outer)));
            }
        }

        let rects = self.compute_workspace_rects(area);
        let framed = self.is_framed();
        for &(id, outer) in rects.iter().rev() {
            if rect_contains(outer, col, row) {
                let inner = if framed {
                    inset_window_frame(outer)
                } else {
                    outer
                };
                return Some((Focus::Pane(id), outer, inner));
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
mod tests;
