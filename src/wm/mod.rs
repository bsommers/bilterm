pub mod chrome_style;
pub mod drawer;
pub mod journal;
pub mod layout_profile;
pub mod quick_terminal;
pub mod session;
pub mod session_manager;
pub mod session_tree;
pub mod split_tree;
pub mod v2_runtime;
pub mod window_style;
pub mod z_stack;

pub use drawer::{
    DrawerConfig, DrawerEdge, DrawerLayoutMode, DrawerRow, DrawerRowKind, DrawerVisibilityMode,
    WorkspaceDrawer,
};
pub use journal::{PaneJournalRecord, SessionJournalRecord, WorkspaceJournal};
pub use layout_profile::{BUILTIN_LAYOUT_PROFILES, LayoutProfileMeta, sanitize_profile_name};
pub use quick_terminal::{encode_sgr_mouse_local, inset_window_frame, rect_contains};
pub use session::{SavedPaneState, SavedWorkspaceSession};
pub use session_tree::{SessionNodeKind, SessionTree, SessionTreeNode};
pub use z_stack::{FloatingWindowGeometry, WindowResizeEdge, WmMouseDrag};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

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
pub use crate::wm::split_tree::SplitDirection;
use crate::wm::split_tree::SplitTree;
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
    /// Interactive Borland/3270 Exit Guard confirmation modal (`SAFE-01`).
    ExitGuard(crate::pane::ExitGuardState),
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
    pub(crate) default_size: (u16, u16),
    /// Dynamic scrollback budget governor (`[scrollback]`).
    scrollback: ScrollbackGovernor,
    /// Whole-workspace Asciicast v2 (`.cast`) compositor frame recorder (`REC-02`).
    workspace_recorder: Mutex<Option<WorkspaceRecorder>>,
    /// Directory file-writing commands are confined to while an automation action runs (`SEC-R01`).
    automation_confine_dir: Option<PathBuf>,
    /// First error raised by a command whose `apply` arm otherwise reports only via the status bar.
    automation_error: Option<String>,
    /// Optional attached `tmux -CC` Control Mode subprocess (`TMUX-03`, `ARCH-R05`).
    pub(crate) tmux_control: Option<crate::tmux::TmuxControlSubprocess>,
    /// Bi-directional `tmux -CC` Control Mode parser/router (`TMUX-03`, `ARCH-R05`).
    pub(crate) tmux_bridge: crate::tmux::TmuxControlBridge,
    /// `herdr`-style slide-out Sessions / Windows / Panes & Fleet drawer runtime state (`DRW-01`, `DRW-02`).
    pub(crate) drawer: WorkspaceDrawer,
    /// Active mouse drag gesture for moving or resizing workspace windows/splits.
    pub(crate) wm_mouse_drag: Option<WmMouseDrag>,
    /// Scale percentage at which `default_size` viewport was computed (`100` by default).
    pub(crate) viewport_scale_pct: u16,
    /// Persistent background multiplexer daemon & hierarchical `SessionTree` state (`DAEM-01..02`, `TREE-01..02`).
    pub(crate) daemon_state: crate::daemon::DaemonServerState,
    /// TRON Master Control Program (`MCP`) root terminal state (`MCP-01..02`).
    pub(crate) mcp_console: crate::mcp_console::McpConsoleState,
    /// Whether this client detached cleanly without terminating the session (`SAFE-02`).
    pub(crate) client_detached: bool,
    /// In-memory cache of saved/autosaved named layout profiles for instant dynamic switching.
    pub(crate) layout_profiles_mem: BTreeMap<String, SavedWorkspaceSession>,
    /// Timestamp of the most recent software cursor blink toggle (`UI-05`).
    pub(crate) last_blink_toggle: Instant,
    pane_chrome: BTreeMap<PaneId, chrome_style::PaneChromeStyle>,
    /// When `true` (host terminal TUI mode), disables per-pane pixel font scaling so `vt` and `Buffer` stay 1:1.
    pub(crate) tui_cell_grid_mode: bool,
    /// Sub-cell mouse pixel hint `(px, py, cell_width_px, cell_height_px)` from native GUI windows (`--gui`).
    pub(crate) mouse_pixel_hint: Option<(u32, u32, u16, u16)>,
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
            automation_confine_dir: None,
            automation_error: None,
            tmux_control: None,
            tmux_bridge: crate::tmux::TmuxControlBridge::new(),
            drawer: WorkspaceDrawer::new(),
            wm_mouse_drag: None,
            viewport_scale_pct: 100,
            daemon_state: crate::daemon::DaemonServerState::new(),
            mcp_console: crate::mcp_console::McpConsoleState::new(),
            client_detached: false,
            layout_profiles_mem: BTreeMap::new(),
            last_blink_toggle: Instant::now(),
            pane_chrome: BTreeMap::new(),
            tui_cell_grid_mode: false,
            mouse_pixel_hint: None,
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

    /// Executes an [`Action`] on behalf of the automation plane (JSON-RPC / playbooks) through [`Self::apply`].
    ///
    /// With `confine = Some(dir)`, `:canvas export` and `:record start <path>` may only write a plain
    /// file name inside `dir`. Command errors that the interactive path only shows in the status bar
    /// are returned as `Err`, identically for confined and trusted callers.
    pub fn apply_automation(
        &mut self,
        action: Action,
        confine: Option<&Path>,
    ) -> std::result::Result<AppAction, String> {
        self.automation_confine_dir = confine.map(Path::to_path_buf);
        self.automation_error = None;
        let res = self.apply(action);
        self.automation_confine_dir = None;
        if let Some(err) = self.automation_error.take() {
            return Err(err);
        }
        res.map_err(|e| e.to_string())
    }

    /// Directory the running automation action is confined to, if any.
    pub fn automation_confine_dir(&self) -> Option<&Path> {
        self.automation_confine_dir.as_deref()
    }

    /// Records the first error of the running automation action (surfaced by [`Self::apply_automation`]).
    pub fn record_automation_error(&mut self, err: String) {
        self.automation_error.get_or_insert(err);
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
    pub fn set_layout_mode(&mut self, mode: WorkspaceLayoutMode) {
        self.layout_mode = mode;
        self.reflow_default_viewport();
        self.maybe_autosave_layout();
    }
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
        self.reflow_default_viewport();
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
        new_pane.set_scroll_mode_from_config(self.config.scroll_mode);
        new_pane.font_options_mut().scale_pct = Some(100);

        self.panes.insert(new_id, new_pane);
        self.split_tree.split_leaf(target_id, new_id, direction);
        self.z_stack.push(new_id);
        self.zoomed_pane = None;
        self.sync_config_scale_from_active_pane();
        self.reflow_default_viewport();
        self.rebalance_scrollback(Instant::now());
        self.maybe_autosave_layout();
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
        self.pane_chrome.remove(&target_id);
        self.z_stack.remove(target_id);
        if self.zoomed_pane == Some(target_id) {
            self.zoomed_pane = None;
        }
        self.fleet.remove_pane_references(target_id);
        self.reflow_default_viewport();
        self.rebalance_scrollback(Instant::now());
        self.maybe_autosave_layout();
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
    pub fn poll_all_ptys(&mut self) -> bool {
        let mut changed = false;
        let mut exited_ids = Vec::new();
        for (&id, pane) in self.panes.iter_mut() {
            let prev_ld = *pane.line_discipline();
            if pane.poll_pty_events() {
                changed = true;
            }
            if pane.tick_smooth_scroll() {
                changed = true;
            }
            pane.refresh_line_discipline();
            if *pane.line_discipline() != prev_ld {
                changed = true;
            }
            if pane.has_exited() {
                exited_ids.push(id);
            }
        }
        for id in exited_ids {
            if self.panes.len() > 1 && self.remove_pane(id) {
                changed = true;
            }
        }
        let prev_qt_ld = *self.quick_terminal.pane.line_discipline();
        if self.quick_terminal.pane.poll_pty_events() {
            changed = true;
        }
        if self.quick_terminal.pane.tick_smooth_scroll() {
            changed = true;
        }
        self.quick_terminal.pane.refresh_line_discipline();
        if *self.quick_terminal.pane.line_discipline() != prev_qt_ld {
            changed = true;
        }
        if self.poll_tmux_control() > 0 {
            changed = true;
        }
        if !self.headless_mode && self.tick_cursor_blink(Instant::now()) {
            changed = true;
        }
        self.update_drawer_telemetry();
        self.maybe_rebalance_scrollback(Instant::now());
        changed
    }

    /// Returns `true` when every workspace pane has exited.
    pub fn all_panes_exited(&self) -> bool {
        self.panes.values().all(CompositePane::has_exited)
    }

    /// Selectively raises `pane_id` to the top of the z-order stack (`Alt+1..9` or mouse click, `WM-02`).
    pub fn focus_and_raise(&mut self, pane_id: PaneId) -> bool {
        if self.panes.contains_key(&pane_id) {
            self.quick_terminal.visible = false;
            let changed = self.z_stack.raise(pane_id);
            self.sync_config_scale_from_active_pane();
            changed
        } else {
            false
        }
    }

    /// Shuffles the pane deck top-to-bottom (`Alt+Tab` / `F6` / `:deck next`, `WM-02`).
    pub fn shuffle_deck_next(&mut self) -> Option<PaneId> {
        self.quick_terminal.visible = false;
        let top = self.z_stack.shuffle_top_to_bottom();
        self.sync_config_scale_from_active_pane();
        top
    }

    /// Shuffles the pane deck bottom-to-top (`Alt+Shift+Tab` / `:deck prev`, `WM-02`).
    pub fn shuffle_deck_prev(&mut self) -> Option<PaneId> {
        self.quick_terminal.visible = false;
        let top = self.z_stack.shuffle_bottom_to_top();
        self.sync_config_scale_from_active_pane();
        top
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
        self.reflow_default_viewport();
        self.maybe_autosave_layout();
    }

    /// Toggles between `WorkspaceLayoutMode::Tiled` and `WorkspaceLayoutMode::Stacked`.
    pub fn toggle_layout_mode(&mut self) {
        self.layout_mode = match self.layout_mode {
            WorkspaceLayoutMode::Tiled => WorkspaceLayoutMode::Stacked,
            WorkspaceLayoutMode::Stacked => WorkspaceLayoutMode::Tiled,
        };
        self.reflow_default_viewport();
        self.maybe_autosave_layout();
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
                    .filter_map(|id| {
                        rect_map
                            .get(id)
                            .copied()
                            .map(|r| (*id, self.z_stack.apply_tiled_rect(*id, r)))
                    })
                    .collect()
            }
            WorkspaceLayoutMode::Stacked => self.z_stack.compute_cascade_rects(area),
        }
    }

    /// Resizes all visible panes' viewports to match `area`, scaling each pane's `(rows, cols)`
    /// inversely by its per-pane font scale so larger fonts have fewer cols/rows and smaller fonts have more.
    pub fn resize_all(&mut self, area: Rect) -> Result<()> {
        self.default_size = (area.height.max(1), area.width.max(1));
        let top_off = u16::from(self.config.show_top_menubar && area.y == 0 && area.height >= 4);
        let bot_off = u16::from(self.config.show_dos_fkey_bar && area.height >= 4 + top_off);
        let ws_area = Rect::new(
            area.x,
            area.y + top_off,
            area.width,
            area.height.saturating_sub(top_off + bot_off),
        );
        let framed = self.is_framed();
        let rects = self.compute_workspace_rects(ws_area);
        for (id, rect) in rects {
            let inner = if framed {
                inset_window_frame(rect)
            } else {
                rect
            };
            let scaled_inner = self.pane_scaled_inner_rect(id, inner);
            if let Some(pane) = self.panes.get_mut(&id) {
                pane.resize_viewport(scaled_inner)?;
            }
        }
        if self.quick_terminal.visible {
            let qt_rect = inset_window_frame(self.quick_terminal.overlay_rect(ws_area));
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

#[cfg(test)]
mod tests;
