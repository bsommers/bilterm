pub mod agent_guide;
pub mod agent_mail;
pub mod resilience;
pub mod swarm_cmd;
mod agent_mail_apply;
pub mod pane_mail;
pub mod pane_origin;
#[cfg(test)]
mod pane_mail_tests;
#[cfg(test)]
mod pane_origin_tests;
mod session_order;
mod recording_cap;
pub mod agent_state;
pub mod agents_topology;
pub mod agents_window;
pub mod agents_window_ops;
pub mod agents_cmd;
pub mod agents_demo;
pub mod agents_hud;
pub mod agents_hud_cmd;
mod agents_hud_feed;
pub mod agents_list;
pub mod agents_list_cmd;
pub mod agents_geometry;
pub mod agents_list_geometry;
mod host_mode;
#[cfg(unix)]
pub mod session_transfer;
pub mod host_view;
mod attach_resize;
mod pane_input;
mod wm_accessors;
mod wm_toggles;
pub mod command_window;
pub mod context_bus;
pub mod context_queue_ops;
pub mod prompt_queue;
pub mod swarm_surface;
pub mod mission_control_hud;
pub mod turn_replay;
pub mod agent_watchdog;
pub mod clipboard_registry;
pub mod clipboard_image_ops;
pub mod clipboard_window;
pub mod coordinates;
pub mod daemon_window;
pub mod fonts_window;
pub mod script_studio;
pub mod script_studio_cmd;
pub mod chrome_style;
pub mod drawer;
pub mod find_cmd;
pub mod journal;
pub mod keyboard_focus;
pub mod session_find;
pub mod key_mode;
pub mod which_key_state;
pub mod layout_key;
pub mod layout_profile;
pub mod lightcycle;
pub mod dos_jobs;
pub mod link_cmd;
pub mod pane_hold;
pub mod panel_dock;
pub mod pty_drain;
pub mod quick_terminal;
pub mod registry;
pub mod rename_dialog;
pub mod session;
#[cfg(feature = "dos")]
mod dos_pane;
mod editor_menu;
mod editor_pane;
pub mod session_empty;
pub mod session_end;
pub mod session_manager;
pub mod session_map;
pub mod session_pack_layout;
pub mod session_tree;
pub mod split_tree;
pub mod window_drag;
pub mod window_drag_place;
pub mod stt_ops;
pub mod tmux_flow;
pub mod tmux_io;
pub mod tmux_link;
pub mod tmux_remote;
pub mod traffic;
pub mod traffic_monitor;
pub mod traffic_monitor_geometry;
mod traffic_view;
pub mod v2_runtime;
mod media_tick;
pub mod window_manage;
pub mod window_style;
pub mod z_stack;

pub use window_manage::{
    BulkWindowOp, InlineRenameState, SelectSpec, SessionManageOp, TopologyUndoEntry,
    WindowBulkOp, WindowManageOp, WindowManageState, WindowSelectTarget,
    WorkspaceTopologyUndo,
};
pub use key_mode::KeyMode;
pub use rename_dialog::{RenameDialogKind, RenameDialogState};
pub use drawer::{
    DrawerConfig, DrawerEdge, DrawerLayoutMode, DrawerPreviewState, DrawerPreviewTarget, DrawerRow,
    DrawerRowKind, DrawerVisibilityMode, WorkspaceDrawer,
};
pub use journal::{PaneJournalRecord, SessionJournalRecord, WorkspaceJournal};
pub use layout_profile::{BUILTIN_LAYOUT_PROFILES, LayoutProfileMeta, sanitize_profile_name};
pub use quick_terminal::{encode_sgr_mouse_local, inset_window_frame, rect_contains};
pub use session::{SavedEditor, SavedPaneState, SavedWorkspaceSession};
pub use session_map::{SessionMapDrag, SessionMapOp, SessionMapState};
pub use session_pack_layout::{
    compute_inner_rect, pack_rects, pack_sessions_treemap, SessionPackInput, SessionPackRect,
    WindowPackInput, WindowPackRect,
};
pub use session_tree::{SessionNodeKind, SessionTree, SessionTreeNode};
pub use z_stack::{FloatingWindowGeometry, WindowResizeEdge, WmMouseDrag};
pub use command_window::{
    CommandLogEntry, CommandWindowBadge, CommandWindowHit, CommandWindowOp, CommandWindowState,
    PaletteCommandItem, PALETTE_ITEMS,
};
pub use clipboard_registry::{
    ClipboardEntry, ClipboardFilterScope, ClipboardMime, ClipboardRegistry, ClipboardSource,
};
pub use clipboard_window::{
    ClipboardActionChip, ClipboardWindowBadge, ClipboardWindowHit, ClipboardWindowOp,
    ClipboardWindowState,
};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

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
    /// `:version` build/platform details window.
    Version { scroll: usize },
    /// Interactive `<F9>` / `:inspector` Live 3270 Terminal Inspector modal (`BEL-01`).
    Inspector { scroll: usize },
    /// Interactive Fuzzy Command Palette modal (`Ctrl+P` / `:palette`, `v0.6.0`).
    Palette { query: String, selected: usize },
    /// Active pull-down category (`0..7`) and selected row inside the top Borland Menu Bar (`ECO-03`).
    Menubar { category: usize, item: usize },
    /// Interactive Borland/3270 Exit Guard confirmation modal (`SAFE-01`).
    ExitGuard(crate::pane::ExitGuardState),
    /// Unified registry Find window (`:find agent|session|history`, Phase 50.1).
    Find { kind: find_cmd::FindKind, query: String, selected: usize },
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
    /// Leader-pending / sticky Window mode state (`v1.5.0`, Phase 63); see `key_mode.rs`.
    key_mode: key_mode::KeyMode,
    /// Which-key popup idle timer and pin (`KEY-01`, Phase 63-02); see `which_key_state.rs`.
    which_key: which_key_state::WhichKeyState,
    /// Keyboard environment (OS, surface, kitty protocol) driving `Primary` and the key profile.
    key_env: crate::input::keyenv::KeyEnv,
    /// Central `herdr`-style fleet registry managing local and remote SSH / `tmux -CC` sessions (`v1.4.0`).
    fleet: FleetRegistry,
    next_pane_id: PaneId,
    headless_mode: bool,
    /// Session-host mode: headless but panes own real PTYs (`host_mode.rs`); default off.
    host_ptys: bool,
    /// Daemon name this host runs under (set by the daemon; refuses transfers to itself).
    host_name: Option<String>,
    /// Set after a transfer that retires this host: the daemon loop exits once it passes.
    retire_at: Option<std::time::Instant>,
    /// Attach client registry when the host runs an attach listener (`attach::host`); default none.
    attach_registry: Option<crate::attach::registry::SharedRegistry>,
    pub(crate) default_size: (u16, u16),
    /// Dynamic scrollback budget governor (`[scrollback]`).
    scrollback: ScrollbackGovernor,
    /// Whole-workspace Asciicast v2 (`.cast`) compositor frame recorder (`REC-02`).
    workspace_recorder: Mutex<Option<WorkspaceRecorder>>,
    /// Directory file-writing commands are confined to while an automation action runs (`SEC-R01`).
    automation_confine_dir: Option<PathBuf>,
    /// First error raised by a command whose `apply` arm otherwise reports only via the status bar.
    automation_error: Option<String>,
    /// Structured JSON result of the last query command (`:get`, `:config list`, `:state`).
    command_data: Option<String>,
    /// `true` while an API-originated action (RPC/MCP/playbook-confined/send_keys) runs.
    api_origin: bool,
    /// True while applying an `Act` that came from an attach client (never allowed to kick).
    attach_origin: bool,
    /// Optional attached `tmux -CC` Control Mode subprocess (`TMUX-03`, `ARCH-R05`).
    pub(crate) tmux_control: Option<crate::tmux::TmuxControlSubprocess>,
    /// Bi-directional `tmux -CC` Control Mode parser/router (`TMUX-03`, `ARCH-R05`).
    pub(crate) tmux_bridge: crate::tmux::TmuxControlBridge,
    /// Persistent SSH + tmux link record: target, state machine, transport (Phase 51.1).
    pub(crate) tmux_link: crate::tmux::link::TmuxLinkState,
    /// Navigator-style slide-out Sessions / Windows / Panes & Fleet drawer runtime state (`DRW-01`, `DRW-02`).
    pub(crate) drawer: WorkspaceDrawer,
    /// Per-pane agent state tracker (`FLT-01`, Phase 50-03).
    pub(crate) agents: agent_state::AgentTracker,
    /// TRON light-cycle border animation clock (`:cycles`).
    pub(crate) lightcycle: lightcycle::LightCycleClock,
    /// In-memory agent traffic bus read by the zoom graph and snoop view (`FLT-02`, Phase 50-05).
    pub(crate) traffic: traffic::TrafficBus,
    /// Unified agent / session / command-history registry behind `:find` (Phase 50.1).
    pub(crate) registry: registry::RegistryState,
    /// Floating AGENT TRAFFIC zoom window state (`FLT-02`, Phase 50-07).
    pub(crate) agents_window: agents_window::AgentsWindowState,
    /// Floating AGENTS list window state (`UIT-03`, Phase 51).
    pub(crate) agents_list: agents_list::AgentsListState,
    /// Mission Control HUD, turn replay and watchdog state (`HUD-01..03`, Phase 70).
    agents_hud: agents_hud::AgentsHudState,
    /// Keyboard owner at the last settle (`keyboard_focus::settle_keyboard_owner`).
    keyboard_owner_seen: keyboard_focus::KeyboardOwner,
    /// Session find state (`:find --pid ...`): last matches, selection, flashes.
    session_find: session_find::SessionFindState,
    /// Floating SESSION MAP packing viewer state (`VIEW-02`, Phase 54).
    pub(crate) session_map: session_map::SessionMapState,
    /// Floating FONTS window state (`WIN-01`, Phase 51.2).
    daemon_window: daemon_window::DaemonWindowState,
    pub(crate) fonts_window: fonts_window::FontsWindowState,
    pub(crate) fonts_window_dirty: bool,
    /// Set by `Action::Repaint`; the host draw loop consumes it and invalidates the terminal diff.
    repaint_pending: bool,
    /// Pinnable COMMAND PALETTE first-class window (`WIN-02`, Phase 51.2).
    pub(crate) command_window: command_window::CommandWindowState,
    /// First-class workspace clipboard registry (`CLIP-02`, Phase 57).
    pub(crate) clipboard_registry: clipboard_registry::ClipboardRegistry,
    /// Floating, pinnable, dockable `CLIPBOARD` viewer window (`CLIP-03`, Phase 57).
    pub(crate) clipboard_window: clipboard_window::ClipboardWindowState,
    /// Terminal traffic meter (`TMON-01`, Phase 64); one per window manager, no globals.
    traffic_meter: crate::traffic::TrafficMeter,
    /// Title-bar `D:(W,S,Y,P)` coordinate badge style (`COORD-03`, Phase 71).
    coord_style: coordinates::CoordDisplayStyle,
    /// Floating TRAFFIC MONITOR window state (`TMON-03`, Plan 64-03 fills the body).
    pub(crate) traffic_monitor: traffic_monitor::TrafficMonitorState,
    /// Floating SCRIPT STUDIO workbench window state (`WIN-03`, Plan 51.2-03).
    pub(crate) script_studio: script_studio::ScriptStudioState,
    /// In-memory agent mailbox & coordination registry (`AGT-01..03`, Phase 52).
    pub(crate) agent_mail: agent_mail::AgentMailRegistry,
    /// Pane-mail envelope state (unread/sender per pane); private per `ARCH-R02`.
    pane_mail: pane_mail::PaneMailTracker,
    /// Shared artifact scratchpad (`BEYOND-02`, Phase 68).
    context_bus: context_bus::ContextBus,
    /// Per-pane prompt queues (`BEYOND-03`, Phase 68).
    prompt_queue: prompt_queue::PromptQueue,
    swarm_surface: swarm_surface::SwarmSurface,
    agents_demo: agents_demo::AgentsDemoState,
    /// Active mouse drag gesture for moving or resizing workspace windows/splits.
    pub(crate) wm_mouse_drag: Option<WmMouseDrag>,
    /// Tiled window drag-and-drop in progress (drop-zone overlay; `wm/window_drag.rs`).
    window_drag: Option<window_drag::WindowDragState>,
    /// Pane whose title-bar drag grip the pointer rests on (hover highlight).
    grip_hover: Option<PaneId>,
    /// Scale percentage at which `default_size` viewport was computed (`100` by default).
    pub(crate) viewport_scale_pct: u16,
    /// Font scale percentage for the side drawer and `:mcp` console (`100` = unchanged). GUI only (46-04d).
    pub(crate) drawer_scale_pct: u16,
    /// Same for the `:mcp` console; independent of the drawer's.
    pub(crate) mcp_scale_pct: u16,
    /// Same for the AGENT TRAFFIC window; independent of the drawer's and the console's (50-16).
    pub(crate) agents_scale_pct: u16,
    /// Same for the AGENTS list window (Phase 51).
    pub(crate) agents_list_scale_pct: u16,
    /// Same for the SESSION MAP packing viewer (Phase 54).
    pub(crate) session_map_scale_pct: u16,
    /// Same for the CLIPBOARD viewer window (Phase 57).
    pub(crate) clipboard_scale_pct: u16,
    /// Same for the TRAFFIC MONITOR window (Phase 64).
    #[allow(dead_code)] // read by the Plan 64-03 GUI renderer
    pub(crate) traffic_scale_pct: u16,
    /// Set by the GUI renderer path; the TUI cannot scale one region of its single cell grid.
    pub(crate) gui_surface: bool,
    /// Persistent background multiplexer daemon & hierarchical `SessionTree` state (`DAEM-01..02`, `TREE-01..02`).
    pub(crate) daemon_state: crate::daemon::DaemonServerState,
    /// Shared web surface handle (`WEB-FE-01..04`, Phase 60); clones share state with the HTTP/WS server.
    pub(crate) web_surface: crate::api::web_surface::WebSurfaceState,
    /// URL of the pending non-blocking "Open in system browser?" offer (`:web external` accepts it).
    pub(crate) pending_external_url: Option<String>,
    /// TRON Master Control Program (`MCP`) root terminal state (`MCP-01..02`).
    pub(crate) mcp_console: crate::mcp_console::McpConsoleState,
    /// Whether this client detached cleanly without terminating the session (`SAFE-02`).
    pub(crate) client_detached: bool,
    pub(crate) resilience: resilience::ResilienceState,
    /// In-memory cache of saved/autosaved named layout profiles for instant dynamic switching.
    pub(crate) layout_profiles_mem: BTreeMap<String, SavedWorkspaceSession>,
    /// Timestamp of the most recent software cursor blink toggle (`UI-05`).
    pub(crate) last_blink_toggle: Instant,
    pane_chrome: BTreeMap<PaneId, chrome_style::PaneChromeStyle>,
    /// Window and session management state (`SES-01..05`).
    pub(crate) window_manage: window_manage::WindowManageState,
    /// When `true` (host terminal TUI mode), disables per-pane pixel font scaling so `vt` and `Buffer` stay 1:1.
    pub(crate) tui_cell_grid_mode: bool,
    /// Sub-cell mouse pixel hint `(px, py, cell_width_px, cell_height_px)` from native GUI windows (`--gui`).
    pub(crate) mouse_pixel_hint: Option<(u32, u32, u16, u16)>,
    /// Live speech-to-text status and recording telemetry (`STT-02`, `STT-03`).
    pub(crate) stt_status: crate::stt::SttUiStatus,
    /// Handle to the background speech-to-text worker thread.
    pub(crate) stt_handle: Option<crate::stt::worker::SttHandle>,
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
            config: {
                let mut cfg = BiltermConfig::default();
                if headless_mode {
                    // Headless workspaces (tests, daemons) keep the pre-Phase-63 tables bit-for-bit.
                    cfg.keys.profile = crate::config::keys::KeyProfileName::Legacy;
                    // Headless runs quit when their last window exits; only interactive UIs keep it.
                    cfg.session_on_empty = crate::config::SessionOnEmpty::End;
                }
                cfg
            },
            modal: Modal::None,
            broadcast_input: false,
            key_mode: key_mode::KeyMode::Normal,
            which_key: which_key_state::WhichKeyState::default(),
            key_env: if headless_mode {
                crate::input::keyenv::KeyEnv::headless()
            } else {
                crate::input::keyenv::KeyEnv::detect()
            },
            fleet: FleetRegistry::new(),
            next_pane_id: initial_id + 1,
            headless_mode,
            host_ptys: false,
            host_name: None,
            retire_at: None,
            attach_registry: None,
            default_size: (screen_rows, cols),
            scrollback: if headless_mode {
                ScrollbackGovernor::new(Box::new(FixedMemoryProbe::default()))
            } else {
                ScrollbackGovernor::new(Box::new(SystemMemoryProbe))
            },
            workspace_recorder: Mutex::new(None),
            automation_confine_dir: None,
            automation_error: None,
            command_data: None,
            api_origin: false,
            attach_origin: false,
            tmux_control: None,
            tmux_bridge: crate::tmux::TmuxControlBridge::new(),
            tmux_link: crate::tmux::link::TmuxLinkState::default(),
            drawer: WorkspaceDrawer::new(),
            agents: agent_state::AgentTracker::new(),
            lightcycle: lightcycle::LightCycleClock::default(),
            traffic: traffic::TrafficBus::new(),
            registry: registry::RegistryState::new(),
            agents_window: agents_window::AgentsWindowState::default(),
            agents_list: agents_list::AgentsListState::default(),
            agents_hud: agents_hud::AgentsHudState::default(),
            keyboard_owner_seen: keyboard_focus::KeyboardOwner::Pane,
            session_find: session_find::SessionFindState::default(),
            session_map: session_map::SessionMapState::default(),
            daemon_window: daemon_window::DaemonWindowState::default(),
            fonts_window: fonts_window::FontsWindowState::default(),
            fonts_window_dirty: false,
            repaint_pending: false,
            command_window: command_window::CommandWindowState::default(),
            clipboard_registry: clipboard_registry::ClipboardRegistry::new(),
            clipboard_window: clipboard_window::ClipboardWindowState::default(),
            traffic_meter: crate::traffic::TrafficMeter::new(),
            coord_style: coordinates::CoordDisplayStyle::Off,
            traffic_monitor: traffic_monitor::TrafficMonitorState::default(),
            script_studio: script_studio::ScriptStudioState::default(),
            agent_mail: agent_mail::AgentMailRegistry::new(false),
            pane_mail: pane_mail::PaneMailTracker::default(),
            context_bus: context_bus::ContextBus::new(),
            prompt_queue: prompt_queue::PromptQueue::new(),
            swarm_surface: swarm_surface::SwarmSurface::default(),
            agents_demo: agents_demo::AgentsDemoState::default(),
            wm_mouse_drag: None,
            window_drag: None,
            grip_hover: None,
            viewport_scale_pct: 100,
            drawer_scale_pct: 100,
            mcp_scale_pct: 100,
            agents_scale_pct: 100,
            agents_list_scale_pct: 100,
            session_map_scale_pct: 100,
            clipboard_scale_pct: 100,
            traffic_scale_pct: 100,
            gui_surface: false,
            daemon_state: crate::daemon::DaemonServerState::new(),
            web_surface: crate::api::web_surface::WebSurfaceState::new(),
            pending_external_url: None,
            mcp_console: crate::mcp_console::McpConsoleState::new(),
            client_detached: false,
            resilience: Default::default(),
            layout_profiles_mem: BTreeMap::new(),
            last_blink_toggle: Instant::now(),
            pane_chrome: BTreeMap::new(),
            window_manage: window_manage::WindowManageState::default(),
            tui_cell_grid_mode: false,
            mouse_pixel_hint: None,
            stt_status: crate::stt::SttUiStatus::default(),
            stt_handle: None,
        }
    }

    /// Executes a unified [`Action`] against the workspace and active pane (`ARCH-R01.1`).
    pub fn apply(&mut self, action: Action) -> Result<AppAction> {
        // Any action may create, rename or delete a saved session: drop the Navigator's snapshot
        // before and after so rows rebuilt during or right after it are fresh (WR-05).
        self.drawer.invalidate_saved_sessions();
        self.freeze_session_order();
        let quit = execute_wm_colon_command(self, action);
        self.drawer.invalidate_saved_sessions();
        self.settle_keyboard_owner();
        if quit {
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
        if confine.is_some()
            && let Some(err) = self.config_write_denied(&action)
        {
            return Err(err);
        }
        self.automation_confine_dir = confine.map(Path::to_path_buf);
        self.automation_error = None;
        self.command_data = None;
        let prev_origin = self.api_origin;
        self.api_origin = prev_origin || confine.is_some();
        let res = self.apply(action);
        self.api_origin = prev_origin;
        self.automation_confine_dir = None;
        if let Some(err) = self.automation_error.take() {
            return Err(err);
        }
        res.map_err(|e| e.to_string())
    }


    pub fn pane_count(&self) -> usize { self.panes.len() }
    pub fn pane(&self, id: PaneId) -> Option<&CompositePane> { self.panes.get(&id) }
    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut CompositePane> { self.panes.get_mut(&id) }
    pub fn panes(&self) -> &BTreeMap<PaneId, CompositePane> { &self.panes }
    pub fn panes_mut(&mut self) -> &mut BTreeMap<PaneId, CompositePane> { &mut self.panes }
    pub fn stt_status(&self) -> &crate::stt::SttUiStatus { &self.stt_status }
    pub fn stt_status_mut(&mut self) -> &mut crate::stt::SttUiStatus { &mut self.stt_status }
    pub fn drain_stt_events(&mut self) { stt_ops::drain_stt_events(self); }
    pub fn sync_stt_badges(&mut self) { stt_ops::sync_stt_badges(self); }
    pub fn ensure_stt_worker(&mut self) -> Option<&mut crate::stt::worker::SttHandle> { stt_ops::ensure_stt_worker(self) }
    pub fn layout_mode(&self) -> WorkspaceLayoutMode { self.layout_mode }
    pub fn set_layout_mode(&mut self, mode: WorkspaceLayoutMode) {
        self.layout_mode = mode;
        self.reflow_default_viewport();
        self.maybe_autosave_layout();
    }
    pub fn split_tree(&self) -> &SplitTree { &self.split_tree }
    pub fn z_stack(&self) -> &ZStack { &self.z_stack }
    fn effective_zoomed(&self) -> Option<PaneId> {
        self.zoomed_pane
            .and(self.z_stack.top())
            .filter(|id| self.panes.contains_key(id))
    }
    pub fn zoomed_pane(&self) -> Option<PaneId> { self.effective_zoomed() }
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

    pub fn fleet(&self) -> &FleetRegistry {
        &self.fleet
    }

    pub fn fleet_mut(&mut self) -> &mut FleetRegistry {
        &mut self.fleet
    }

    pub fn scrollback_mut(&mut self) -> &mut ScrollbackGovernor {
        &mut self.scrollback
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
        self.split_focused_in(direction, None)
    }

    /// [`Self::split_focused`] with the new pane's shell rooted in `cwd` (swarm worktrees).
    pub fn split_focused_in(&mut self, direction: SplitDirection, cwd: Option<std::path::PathBuf>) -> Result<PaneId> {
        self.split_focused_with(direction, cwd, false)
    }

    /// [`Self::split_focused_in`]; `no_pty` creates a model-only window (editor windows).
    pub(super) fn split_focused_with(&mut self, direction: SplitDirection, cwd: Option<std::path::PathBuf>, no_pty: bool) -> Result<PaneId> {
        let target_id = self.z_stack.top().unwrap_or(1);
        let new_id = self.next_pane_id;
        self.next_pane_id += 1;

        let (rows, cols) = self.default_size;
        let mut new_pane = if no_pty || self.model_only_panes() {
            CompositePane::new_headless(new_id, rows, cols)
        } else {
            CompositePane::spawn(
                new_id,
                PtyConfig {
                    rows,
                    cols,
                    cwd,
                    ..Default::default()
                },
            )?
        };
        Self::attach_history(&mut new_pane, &self.config.history_settings());
        new_pane.set_scroll_mode_from_config(self.config.scroll_mode);
        new_pane.font_options_mut().scale_pct = Some(100);

        if self.is_session_empty() {
            // The first window of an empty session replaces its placeholder.
            self.install_sole_pane(new_id, new_pane);
            self.drawer.invalidate_saved_sessions();
        } else {
            self.panes.insert(new_id, new_pane);
            self.split_tree.split_leaf(target_id, new_id, direction);
            self.z_stack.push(new_id);
            self.window_order_insert_after(target_id, new_id);
        }
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
        self.editor_flush(target_id);
        let _ = self.split_tree.remove_leaf(target_id);
        self.panes.remove(&target_id);
        self.window_manage.window_order.retain(|&i| i != target_id);
        self.pane_chrome.remove(&target_id);
        self.z_stack.remove(target_id);
        if self.zoomed_pane == Some(target_id) {
            self.zoomed_pane = None;
        }
        self.fleet.remove_pane_references(target_id);
        self.agent_mail.remove_pane(target_id);
        self.pane_mail.remove(target_id);
        self.prompt_queue.remove_pane(target_id);
        self.reflow_default_viewport();
        self.rebalance_scrollback(Instant::now());
        self.maybe_autosave_layout();
        true
    }

    /// Closes the focused pane; the last one ends the session if another session exists.
    pub fn close_focused_pane(&mut self) -> bool {
        if self.quick_terminal.visible {
            self.quick_terminal.visible = false;
            return true;
        }
        let Some(target_id) = self.z_stack.top() else {
            return false;
        };
        if !self.editor_allow_close(target_id) {
            return false;
        }
        if self.panes.len() == 1 && self.panes.contains_key(&target_id) {
            // The session's last window: closing it ends the session when another one exists.
            return self.close_last_window_of_session();
        }
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
        if self.reap_or_hold_exited(exited_ids) {
            changed = true;
        }
        self.poll_background_ptys();
        changed |= self.fall_back_to_parked_session();
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
        let link_ms = self.link_now_ms();
        if self.tick_tmux_link(link_ms) {
            changed = true;
        }
        if !self.headless_mode && self.tick_cursor_blink(Instant::now()) {
            changed = true;
        }
        if !self.headless_mode && self.tick_lightcycle(Instant::now()) {
            changed = true;
        }
        self.update_drawer_telemetry();
        let tm_ms = self.link_now_ms();
        if self.traffic_tick(tm_ms) {
            changed = true;
        }
        if self.tick_agent_ui(Instant::now()) {
            changed = true;
        }
        changed |= self.tick_media_canvases(tm_ms);
        changed |= self.editor_tick();
        registry::capture::tick(self, Instant::now());
        self.sync_clipboard_records();
        self.maybe_rebalance_scrollback(Instant::now());
        self.poll_recording_caps();
        changed |= self.resilience_tick(Instant::now());
        changed
    }

    /// The single quit decision shared by the TUI and GUI loops: every pane of the active session
    /// has exited and nothing (another session, or an empty kept session) took its place.
    pub fn should_quit(&self) -> bool {
        self.all_panes_exited()
    }

    /// Returns `true` when every workspace pane has exited.
    pub fn all_panes_exited(&self) -> bool {
        self.panes.values().all(CompositePane::has_exited)
    }

    /// Selectively raises `pane_id` to the top of the z-order stack (`Alt+1..9` or mouse click, `WM-02`).
    pub fn focus_and_raise(&mut self, pane_id: PaneId) -> bool {
        if self.panes.contains_key(&pane_id) {
            self.quick_terminal.visible = false;
            self.blur_floating_windows();
            let changed = self.z_stack.raise(pane_id);
            // NOTE(phase-61): preserve per-pane copy_mode across focus changes (tests/phase61_window_state_preserved.rs)
            self.sync_config_scale_from_active_pane();
            if self.zoomed_pane.is_some() {
                self.reflow_default_viewport();
            }
            changed
        } else {
            false
        }
    }

    /// Shuffles the pane deck top-to-bottom (`Alt+Tab` / `F6` / `:deck next`, `WM-02`).
    pub fn shuffle_deck_next(&mut self) -> Option<PaneId> {
        self.quick_terminal.visible = false;
        self.blur_floating_windows();
        let top = self.z_stack.shuffle_top_to_bottom();
        // NOTE(phase-61): preserve per-pane copy_mode across focus changes (tests/phase61_window_state_preserved.rs)
        self.sync_config_scale_from_active_pane();
        if self.zoomed_pane.is_some() {
            self.reflow_default_viewport();
        }
        top
    }

    /// Shuffles the pane deck bottom-to-top (`Alt+Shift+Tab` / `:deck prev`, `WM-02`).
    pub fn shuffle_deck_prev(&mut self) -> Option<PaneId> {
        self.quick_terminal.visible = false;
        self.blur_floating_windows();
        let top = self.z_stack.shuffle_bottom_to_top();
        // NOTE(phase-61): preserve per-pane copy_mode across focus changes (tests/phase61_window_state_preserved.rs)
        self.sync_config_scale_from_active_pane();
        if self.zoomed_pane.is_some() {
            self.reflow_default_viewport();
        }
        top
    }

    /// Toggles zoom/maximize on the currently focused workspace pane (`F5` / `Alt+Z` / `:zoom`, `WM-03`).
    pub fn toggle_zoom(&mut self) {
        let Some(top_id) = self.z_stack.top() else {
            return;
        };
        self.zoomed_pane = if self.zoomed_pane.is_some() {
            None
        } else {
            Some(top_id)
        };
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
        // Zen (non-Legacy only): the focused pane fills the workspace; others keep running unseen.
        if crate::ui::shell::effective_display_mode(&self.config) == crate::ui::design::DisplayMode::Zen {
            return vec![(self.active_pane_id(), area)];
        }
        if let Some(zoomed_id) = self.effective_zoomed() {
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
        let mut top_off = u16::from(self.config.show_top_menubar && area.y == 0 && area.height >= 4);
        if top_off == 0 && crate::ui::shell::wants_pill_bar(&self.config, self.panes.len(), area.height) {
            top_off = 1; // navigator pills (non-Legacy only; never true for Legacy)
        }
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
