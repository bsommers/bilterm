//! Unified `Action` command vocabulary (`ARCH-R01`, `REC-02`).
//!
//! Every key chord, `:colon` command, playbook step, and JSON-RPC/MCP workspace command lowers
//! into a typed [`Action`] executed by [`crate::wm::WindowManager::apply`].

pub use crate::editor::EditorOp;
pub use crate::pane::{BrowserOp, VideoOp};
use crate::pane::{ExitProtectionPolicy, PaneId, PaneScrollMode, PromptMode};
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::{CaretStyle, SpotlightMode};
use crate::ui::glyphs::GlyphPack;
use crate::ui::theme::ThemePreset;
use crate::wm::{DrawerEdge, DrawerLayoutMode, DrawerVisibilityMode, WorkspaceLayoutMode};
use serde::{Deserialize, Serialize};

pub use crate::action_keys::{KeysOp, WinSub, WindowModeOp};
pub use crate::stt::VoiceOp;

/// Canonical workspace and pane action (`ARCH-R01`, `PANE-03..05`, `WM-01..05`, `UI-01..05`, `REC-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    PromptTop,
    PromptBottom,
    PromptSplitTop,
    PromptSplitBottom,
    PromptSplitToggle,
    PromptModeSet(Option<PromptMode>),
    PromptFloat(u16),
    PromptHeight(u16),
    PromptWidth(Option<u16>),
    PromptMoveVertical(i16),
    PromptResizeHeight(i16),
    PromptResizeWidth(i16),
    BlockPrev,
    BlockNext,
    Yank,
    /// `:paste` / `:p` / `Ctrl+Shift+V` / `Shift+Insert` / `Ctrl+X p` / `Ctrl+X v`: Paste OS clipboard into active pane/PTY.
    PasteClipboard,
    Clear,
    /// `:repaint` / `:redraw` / `Ctrl+X )` / RPC `ui.repaint` / MCP `bilterm_repaint`: invalidate the host-terminal diff so the next frame rewrites every cell (what a window resize does).
    Repaint,
    /// `:scrollback` (report) / `:scrollback clear` (free history).
    Scrollback {
        clear: bool,
    },
    /// `:scroll [smooth|page|block|instant]` (`SCRL-01`, `SCRL-02`).
    ScrollModeSet(Option<PaneScrollMode>),
    /// `Ctrl+X PageDown` / `:scroll toggle`: toggles `BlockPage` (`3270 MORE...`) vs `Smooth` (`SCRL-02`).
    ScrollModeTogglePage,
    /// `:scroll next` / `:more` / `PF8` / `PA2`: advances one full page in `BlockPage` mode (`SCRL-02`).
    ScrollPageAdvance,
    /// `:scroll prev` / `:scroll back` / `PF7`: steps backward one full page in `BlockPage` mode (`SCRL-02`).
    ScrollPageBack,
    RawToggle,
    SplitHorizontal,
    SplitVertical,
    DeckNext,
    DeckPrev,
    FocusPane(usize),
    /// Switches focus to the adjacent window/pane in direction `(dx, dy)` (Arrow / VIM `hjkl` / WASD `wasd`).
    PaneFocusDirection {
        dx: i16,
        dy: i16,
    },
    ZoomToggle,
    ClosePane,
    LayoutMode(WorkspaceLayoutMode),
    LayoutCycle,
    QuakeToggle,
    HelpToggle,
    HistoryPopupToggle,
    HistorySave(Option<String>),
    HistoryToggle(Option<bool>),
    HistoryMaxFiles(usize),
    HistoryRuns,
    Theme(ThemePreset),
    Glyphs(GlyphPack),
    Appearance(crate::ui::design::AppearanceProfile),
    /// `:appearance [list|select|duplicate|modify|reset|import|export ...]` lifecycle operations (Phase 45).
    AppearanceManage(AppearanceOp),
    Mode(crate::ui::design::DisplayMode),
    /// Drawer / `:mcp` console font scale in percent (`75..=200`), GUI only.
    PanelScale(u16),
    /// Font scale of just the drawer or just the `:mcp` console.
    PanelScaleOf(crate::ui::typography::PanelTarget, u16),
    /// Relative step (`+` / `-`) of font scale for a single panel (`:mcp scale +|-`).
    PanelScaleStepOf(crate::ui::typography::PanelTarget, i16),
    /// Pane title policy for non-Legacy looks (`None` = display-mode default).
    ChromeTitle(Option<crate::ui::design::TitlePolicy>),
    /// Accent border on the focused pane (non-Legacy looks).
    ChromeRing(bool),
    /// Window drag drop-zone overlay on/off (`:drag zones on|off`).
    DragZones(bool),
    /// TRON light-cycle border animation: show, on, off or set the speed (`:cycles`).
    LightCycles(crate::ui::lightcycle::LightCycleOp),
    Cursor(CaretStyle),
    Outline {
        window: Option<OutlineStyle>,
        prompt: Option<OutlineStyle>,
    },
    CrosshairToggle {
        enabled: Option<bool>,
        fullscreen: Option<bool>,
    },
    CrosshairCycle,
    Spotlight {
        mode: Option<SpotlightMode>,
        pixelated: Option<bool>,
        fullscreen: Option<bool>,
        rx: Option<u16>,
        ry: Option<u16>,
        delta: Option<(i16, i16)>,
        feather_pct: Option<u8>,
    },
    FocalZoom {
        mode: Option<SpotlightMode>,
        zoom_pct: Option<u16>,
        step_pct: Option<i16>,
        pixelated: Option<bool>,
        fullscreen: Option<bool>,
        rx: Option<u16>,
        ry: Option<u16>,
        delta_size: Option<(i16, i16)>,
        feather_pct: Option<u8>,
    },
    FocalRadiusResize(i16, i16),
    Shadows(Option<ShadowStyle>),
    FKeyBarToggle(Option<bool>),
    SearchQuery(String),
    SearchPromptPrefill,
    /// `:get <key>` - read one config value (Phase 49-04).
    ConfigGet(String),
    /// `:set <key> <value>` - set one config value in the running config.
    ConfigSet(String, String),
    /// `:config list` - list every config key.
    ConfigList,
    /// `:config save` - persist differing config keys to config.toml.
    ConfigSave,
    /// `:appearance save` - persist only `appearance.*` keys.
    AppearanceSave,
    /// `:state` - structured workspace state query.
    StateQuery,
    SearchNext,
    SearchPrev,
    SearchOff,
    CopyModeToggle,
    /// `:rect [subcmd]` / `:select [subcmd]` / `:copy <subcmd>`: Rectangular block selection, highlight, & copy (`VAL-04`).
    SelectCommand(Option<String>),
    LinksYank,
    InspectorToggle(Option<bool>),
    HintsToggle,
    LockToggle(Option<bool>),
    BroadcastToggle(Option<bool>),
    MenuBarToggle(Option<bool>),
    VersionToggle(Option<bool>),
    /// Opens the `:version` details window (version, build, platform).
    VersionInfo,
    SessionSave(String),
    SessionRestore(String),
    SessionList,
    CrtToggle(Option<bool>),
    PaletteToggle,
    NotificationsClear,
    TriggerAdd(String),
    TriggerClear,
    RecordStart(Option<String>),
    RecordWorkspaceStart(Option<String>),
    RecordPaneStart(Option<String>),
    RecordStop,
    SshConnect {
        target: String,
        port: Option<u16>,
    },
    SshTmuxConnect {
        target: String,
        session: String,
    },
    RemoteAdd {
        name: String,
        destination: String,
        tags: Vec<String>,
    },
    RemoteList,
    /// `:remote status [pane]` / `remote.status` RPC / `bilterm pane remote`: per-pane origin (local, ssh, tmux-link, ...).
    RemoteStatus { pane: Option<usize> },
    RemoteExec {
        selector: String,
        command: String,
    },
    RemoteDisconnect(String),
    /// `:ssh-tmux reconnect|disconnect|info|prefill` - the persistent tmux link verbs (Phase 51.1, UNI-R01).
    RemoteLink(crate::wm::link_cmd::RemoteLinkOp),
    DrawerToggle,
    DrawerFocus,
    DrawerDock(Option<DrawerEdge>),
    DrawerVisibility(Option<DrawerVisibilityMode>),
    DrawerLayout(Option<DrawerLayoutMode>),
    DrawerSize(u16),
    DrawerNav(crate::wm::drawer::DrawerNavOp),
    /// `None` toggles the drawer style.
    DrawerStyle(Option<crate::wm::drawer::DrawerStyle>),
    /// `:drawer rows 1|2` - lines per Navigator pane row (`drawer.row_lines`).
    DrawerRows(u8),
    /// `:agents [list]` - per-pane agent state (Phase 50-03, FLT-01).
    Agents(crate::wm::agents_cmd::AgentsOp),
    /// `:agents list <verb>` - the AGENTS list window (Phase 51, UIT-03).
    AgentsList(crate::wm::agents_list_cmd::AgentsListOp),
    /// `:agents hud <verb>` - the Mission Control HUD (Phase 70, HUD-01).
    AgentsHud(crate::wm::agents_hud_cmd::HudOp),
    /// `:agent replay <pane|verb>` - the read-only turn replay view (Phase 70, HUD-02).
    AgentsReplay(crate::wm::agents_hud_cmd::ReplayOp),
    /// `:agents watchdog [status]` - stall / loop watchdog status (Phase 70, HUD-03).
    AgentsWatchdog(crate::wm::agents_hud_cmd::WatchdogOp),
    /// `:sessions map <verb>` / `:packing <verb>` - the SESSION MAP packing viewer (Phase 54, VIEW-02).
    SessionMap(crate::wm::session_map::SessionMapOp),
    /// `:fonts <verb>` - the first-class FONTS utility window (`WIN-01`, Phase 51.2).
    FontsWindow(crate::wm::fonts_window::FontsWindowOp),
    /// `:commands [open|close|toggle|pin|...]` - Pinnable Command Palette window (WIN-02, Phase 51.2).
    CommandWindow(crate::wm::command_window::CommandWindowOp),
    /// `:clipboard [open|close|toggle|pin|dock|...]` - Filterable Clipboard Viewer window (`CLIP-03..04`, Phase 57).
    ClipboardWindow(crate::wm::clipboard_window::ClipboardWindowOp),
    /// `:traffic ...` - TRAFFIC MONITOR window (`TMON-03`, Phase 64).
    TrafficMonitor(crate::wm::traffic_monitor::TrafficMonitorOp),
    /// `:focus cycle [prev]|pane|<window>` (`Ctrl+Alt+O`) - keyboard ownership of the side windows (`FOCUS-01`).
    KeyboardFocus(crate::wm::keyboard_focus::FocusOp),
    /// `:coords ...` - hierarchical `D:(W,S,Y,P)` coordinates: show, badge style, locate, copy (`COORD-04`, Phase 71).
    Coords(crate::wm::coordinates::CoordsOp),
    /// `:context set|get|list|clear` - shared context bus (`UNI-R01`, Phase 68).
    Context(crate::wm::context_queue_ops::ContextOp),
    /// `:agent queue list|add|clear|drop` - per-pane prompt queue (`UNI-R01`, Phase 68).
    AgentQueue(crate::wm::context_queue_ops::AgentQueueOp),
    /// `:agent approve|deny` - answer the active pane's blocked prompt (`UNI-R01`, Phase 68).
    AgentGate(crate::wm::context_queue_ops::AgentGateOp),
    /// `:agent guide [PANE|all|--print]` - hand the bilterm control guide to agent panes (`AGUIDE-01`).
    AgentGuide(crate::wm::agent_guide::AgentGuideOp),
    /// `:swarm status|load|clear` - worktree swarm DAG orchestrator (`UNI-R01`, Phase 69).
    Swarm(crate::wm::swarm_surface::SwarmOp),
    /// `:script <verb>` / `:studio <verb>` - SCRIPT STUDIO floating window (Plan 51.2-03, WIN-03).
    ScriptStudio(crate::wm::script_studio::ScriptStudioOp),
    /// `:find agent|session|history [text]` and the `:finder` modal verbs (Phase 50.1, FND-R01).
    Find(crate::wm::find_cmd::FindOp),
    /// `:find --pid N --pty P --cwd D --cmd C --text T` and `:find-focus|send|beep|flash|select` - locate panes by OS attributes and act (`Ctrl+Alt+D`).
    SessionFind(crate::wm::session_find::SessionFindOp),
    FontZoomIn,
    FontZoomOut,
    FontZoomReset,
    FontSetSize(u16),
    PaneFontCommand(String),
    CanvasCommand(String),
    PaneResize {
        dw: i16,
        dh: i16,
    },
    PaneMove {
        dx: i16,
        dy: i16,
    },
    WindowResetGeometry,
    /// `Ctrl+X #`: Arrange windows into an automatic grid (`51-FOLLOWUPS.md` item 5).
    ArrangeGrid,
    /// `:quit!` / `:q!`: Force immediate application exit bypassing Exit Guard (`SAFE-02`).
    ForceQuit,
    /// `:detach` / `:d`: Detach client while preserving background session (`SAFE-02`, `DAEM-02`).
    DetachClient,
    /// Attach size owner resized the host (`T-A3`): resizes all panes/PTYs to `cols`x`rows`.
    /// Constructed host-side from the attach `Resize` message, never parsed or deserialized.
    AttachResize {
        cols: u16,
        rows: u16,
    },
    /// Raw bytes from an attach client for a pane's PTY (`T-A5`). Constructed host-side from the
    /// attach `Input` message after role, size and rate checks; never parsed or deserialized.
    PaneInputBytes {
        pane: PaneId,
        bytes: Vec<u8>,
    },
    /// `:exit-guard [guard|respawn|detach|off]` (`SAFE-01`).
    ExitProtectionSet(ExitProtectionPolicy),
    /// `:respawn-pane` / `:respawn`: Respawn exited shell in-place inside held pane (`SAFE-03`).
    RespawnPaneInPlace,
    /// `:theme import|load|export|list|<name>` (`THM-01`, `THM-02`).
    ThemeCommand(String),
    /// `:window theme|outline|bold|reset|show ...` per-window chrome overrides and `:outline bold [on|off]`.
    WindowStyle(String),
    /// `:session branch|graft|detach-subtree|tree|checkpoint ...` (`TREE-01..02`, `PERS-01..02`).
    SessionTreeCommand(String),
    /// `:session kill|end|close [name]` (`delete: false`, saved layout kept) and
    /// `:session delete <name>` (`delete: true`). Ending the active session moves to another one;
    /// ending the last session quits.
    SessionEnd { name: Option<String>, delete: bool },
    /// `:session on-empty [end|keep]`: what happens when a session's last window exits or is closed
    /// (`None` reports the current setting). `keep` leaves the session open with zero windows.
    SessionOnEmpty(Option<crate::config::SessionOnEmpty>),
    /// `:profile [list|switch|save|default|autosave|next|prev|delete ...]` / `:layout [save|load|default|autosave|next|prev ...]`.
    LayoutProfileCommand(String),
    /// `:mcp [status|tree|attach|spawn|graft|broadcast|freeze|thaw|derez|checkpoint|shutdown ...]` (`MCP-01..02`).
    McpCommand(String),
    /// `:fkeys [bilterm|app|auto|toggle|send <F1..F12>]`: Function-key routing in RAW mode (`FKEY-01`).
    FkeyModeCommand(Option<String>),
    /// `:style [classic|snazzy|glass|both|cycle]` / `:snazzy [on|off]` / `:glass [on|off]` / `F11`: Visual style (`STYLE-01`).
    VisualStyleCommand(Option<String>),
    /// `:dos run [--name N | --image FILE] [--drive-c DIR] [--backend ID]`: opens a DOS pane (feature `dos`,
    /// PR D2a). Part of the `Dos*` set (D3a) that mirrors `bilterm dos ...`, `dos.*` RPC and `bilterm_dos` MCP.
    DosRun {
        name: Option<String>,
        image: Option<String>,
        drive_c: Option<String>,
        backend: Option<String>,
    },
    /// `:dos setup [--yes] [--full] [--entry ID]...`: download + verify + build the DOS image (long: runs off the UI thread).
    DosSetup { yes: bool, full: bool, entries: Vec<String> },
    /// `:edit` / `:scratch` / `:editor ...`, `Ctrl+Alt+E`, `editor.*` RPC, `bilterm_editor` MCP (`EDIT-01`).
    Editor(EditorOp),
    /// `:dos doctor`: host readiness checks.
    DosDoctor,
    /// `:dos remove [--all] [--yes]`: delete downloaded DOS data.
    DosRemove { all: bool, yes: bool },
    /// `:dos status`: emulator, downloads, images, snapshots, locks, running DOS job.
    DosStatus,
    /// `:dos reset [--name N] [--full] [--yes]` (`args` = the CLI flags, validated by the shared parser).
    DosReset { args: Vec<String> },
    /// `:dos snapshot [list|rm] [--name N] [--tag T]`.
    DosSnapshot { args: Vec<String> },
    /// `:dos restore --tag T [--name N] [--yes]`.
    DosRestore { args: Vec<String> },
    /// `:agent [name|send|broadcast|inbox|ack|list|enable]` (`AGT-01..03`, Phase 52).
    AgentMail(crate::wm::agent_mail::AgentMailOp),
    /// `:mail status|ack` / `pane.mail_state` / `pane.mail_ack`: pane-mail envelope state.
    PaneMail(crate::wm::pane_mail::PaneMailOp),
    /// `:window <move|rename|select|bulk|undo>` (`SES-01..05`).
    WindowManage(crate::wm::window_manage::WindowManageOp),
    /// `:session <rename|move|undo>` (`SES-02`, `SES-04`).
    SessionManage(crate::wm::window_manage::SessionManageOp),
    /// `:daemon [list|status|start|stop|switch|prune]` (`DAEM-05..08`, Phase 55).
    DaemonSupervisor(DaemonSupervisorOp),
    /// Alias for `DaemonSupervisor` matching contract shorthand (`DAEM-05..08`).
    Daemon(DaemonSupervisorOp),
    /// `:drawer preview ...` / `:peek ...`: Live window peek & preview pane (`DRW-PEEK-01..03`, Phase 56).
    DrawerPreview(DrawerPreviewOp),
    /// Speech-to-text dictation operations (`STT-01..03`).
    Voice(VoiceOp),
    /// `:browser [open|back|forward|reload|stop|focus|zoom|close]` - Chrome Engine Web Browser Canvas (`WEB-01..02`).
    Browser(BrowserOp),
    /// `:video [open|play|pause|toggle|seek|volume|mute|loop|speed|close]` - Video Player Canvas (`VID-01..02`).
    Video(VideoOp),
    /// `:web [status|open|port <N>|stop]`: Web UI surface control (`WEB-FE-01..04`, Phase 60).
    Web(WebOp),
    /// `:web external [url]` / `:web open --external [url]`: open `url` (default: the pending offer, else the
    /// active browser canvas URL) in the system default browser in its own window.
    WebOpenExternal {
        url: Option<String>,
    },
    /// `:wmode [on|off|focus|resize|move]`: sticky Window mode (`KEY-01`, Phase 63).
    WindowMode(WindowModeOp),
    /// `:menu open [category]`: open the menu bar from the keyboard (`KEY-01`, Phase 63).
    MenuOpen {
        category: Option<usize>,
    },
    /// `:keys [list|check|explain <chord>|profile [name]]`: key profile inspection (`KEY-02`, Phase 63).
    Keys(KeysOp),
    Quit,
}

/// Web UI surface operations (`WEB-FE-01..04`, `UNI-R01`, Phase 60).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebOp {
    /// Report enabled / port / url / connected clients.
    Status,
    /// Open the web UI URL in the system browser (reports the URL when headless or API-driven).
    OpenBrowser,
    /// Set the port used by the next start (refused while running).
    SetPort(u16),
    /// Stop the running web server from accepting new connections.
    Stop,
}

impl WebOp {
    /// Canonical `:web` colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Status => ":web status".to_string(),
            Self::OpenBrowser => ":web open".to_string(),
            Self::SetPort(p) => format!(":web port {p}"),
            Self::Stop => ":web stop".to_string(),
        }
    }
}

/// Operations for the Drawer Window Peek & Live Preview Pane (`DRW-PEEK-01..03`, Phase 56).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawerPreviewOp {
    /// Open the live preview pane for the currently highlighted drawer row.
    On,
    /// Close the live preview pane without changing active workspace focus.
    Off,
    /// Toggle the live preview pane on/off for the currently highlighted drawer row.
    Toggle,
    /// Commit focus/restore/switch to the currently previewed window or session and close the preview.
    Commit,
    /// Toggle automatic preview when scrolling through drawer rows (`auto_preview_on_scroll`).
    AutoToggle,
    /// Select row `idx` in the drawer and open its live preview.
    Row(usize),
    /// Open live preview directly for pane `id`.
    Pane(PaneId),
    /// Open live preview directly for session `name` (background or saved session).
    Session(String),
}

/// Lifecycle operations for `:appearance` profiles (`DSN-01..03`, Phase 45).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppearanceOp {
    List,
    SelectName(String),
    Duplicate { src: String, new_name: String },
    Modify { key: String, value: String },
    Reset,
    Import(String),
    Export(String),
}

/// Operations supported by the background daemon supervisor (`DAEM-05..08`, Phase 55).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonSupervisorOp {
    List,
    Status { name: Option<String> },
    /// `restart`: supervisor restart policy for a newly spawned host (`None` = `never`).
    /// `limits`: memory limits for the host (`None` = unlimited; P4 / C2).
    /// `host_ptys`: the host owns real PTYs (`--headless --host-ptys`); applies to a new spawn only.
    Start { name: Option<String>, restart: Option<crate::daemon::restart_policy::RestartMode>, #[serde(default)] limits: Option<crate::daemon::limits::MemoryLimits>, #[serde(default)] host_ptys: bool },
    /// `:daemon new [NAME]` / `daemon.new` / `[n] New`: create a NEW daemon. `name: None` picks the
    /// first free `daemon-N`; an existing name (any state) is refused, never overwritten. The chosen
    /// name is returned in the command data.
    New { name: Option<String>, restart: Option<crate::daemon::restart_policy::RestartMode>, #[serde(default)] limits: Option<crate::daemon::limits::MemoryLimits>, #[serde(default)] host_ptys: bool },
    Stop { name: Option<String>, force: bool },
    Switch { name: String },
    Prune,
    /// `:daemon host [name]`: read-only view of a session host's persisted state (P1; restore is P6).
    HostState { name: Option<String> },
    /// `:daemon attach` / `attach.clients`: read-only list of attached clients (P2).
    AttachClients,
    /// `:daemon attach kick <client-id> | --ro` / `attach.kick`: close one attach client (or all
    /// read-only ones) with `Bye{kicked}`. Refused when applied on behalf of an attach client.
    AttachKick { client_id: Option<u64>, all_ro: bool },
    /// `:daemon transfer [<session>...|--all] --to <daemon>` / `session.transfer`: move live sessions
    /// (PTYs, shells and terminal state, no restart) from THIS session host to host `to`. Empty
    /// `sessions` without `all` = the active session. `takeover` targets a `--takeover` host's
    /// socket and replaces its pristine session; `retire` makes this host exit afterwards.
    SessionTransfer { sessions: Vec<String>, all: bool, to: String, #[serde(default)] takeover: bool, #[serde(default)] retire: bool },
    /// `:daemon restart --live [name]` / `daemon.restart`: start a new daemon, transfer every
    /// session to it, retire the old one; no terminal is killed or reset.
    RestartLive { name: Option<String> },
}

pub type DaemonOp = DaemonSupervisorOp;

impl Action {
    /// Default spotlight cycle action (`Alt+O` / `Ctrl+X L` / `:spotlight`).
    pub const fn spotlight_cycle() -> Self {
        Self::Spotlight {
            mode: None,
            pixelated: None,
            fullscreen: None,
            rx: None,
            ry: None,
            delta: None,
            feather_pct: None,
        }
    }

    /// Default focal-zoom cycle action (`Alt+Shift+O` / `:focalzoom`).
    pub const fn focal_zoom_cycle() -> Self {
        Self::FocalZoom {
            mode: None,
            zoom_pct: None,
            step_pct: None,
            pixelated: None,
            fullscreen: None,
            rx: None,
            ry: None,
            delta_size: None,
            feather_pct: None,
        }
    }

    /// Canonical colon form or string summary of this action for script recording and logs.
    pub fn colon_form_or_summary(&self) -> String {
        match self {
            Self::DosRun { name, image, drive_c, backend } => {
                let mut s = String::from(":dos run");
                for (flag, v) in [("--name", name), ("--image", image), ("--drive-c", drive_c), ("--backend", backend)] {
                    if let Some(v) = v {
                        s.push_str(&format!(" {flag} {v}"));
                    }
                }
                s
            }
            Self::DosSetup { yes, full, entries } => {
                let mut s = String::from(":dos setup");
                if *yes { s.push_str(" --yes"); }
                if *full { s.push_str(" --full"); }
                for e in entries { s.push_str(&format!(" --entry {e}")); }
                s
            }
            Self::Editor(op) => op.colon_form(),
            Self::DosDoctor => ":dos doctor".to_string(),
            Self::DosStatus => ":dos status".to_string(),
            Self::DosRemove { all, yes } => format!(":dos remove{}{}", if *all { " --all" } else { "" }, if *yes { " --yes" } else { "" }),
            Self::DosReset { args } => format!(":dos reset {}", args.join(" ")).trim_end().to_string(),
            Self::DosSnapshot { args } => format!(":dos snapshot {}", args.join(" ")).trim_end().to_string(),
            Self::DosRestore { args } => format!(":dos restore {}", args.join(" ")).trim_end().to_string(),
            Self::AgentMail(op) => op.colon_form(),
            Self::PaneMail(op) => op.colon_form(),
            Self::RemoteStatus { pane: Some(p) } => format!(":remote status {p}"),
            Self::RemoteStatus { pane: None } => ":remote status".to_string(),
            Self::AgentsHud(op) => op.colon_form(),
            Self::AgentsReplay(op) => op.colon_form(),
            Self::AgentsWatchdog(_) => ":agents watchdog status".to_string(),
            Self::WindowManage(crate::wm::window_manage::WindowManageOp::Reorder { window_id, to }) => match window_id {
                Some(id) => format!(":window reorder {id} {to}"),
                None => format!(":window reorder {to}"),
            },
            Self::WindowManage(crate::wm::window_manage::WindowManageOp::Place { window_id, target, how }) => {
                let id = window_id.map(|i| format!(" {i}")).unwrap_or_default();
                let to = target.map(|t| format!(" {t}")).unwrap_or_default();
                format!(":window place{id} {}{to}", how.name())
            }
            Self::WindowManage(crate::wm::window_manage::WindowManageOp::List) => ":window list".to_string(),
            Self::WindowManage(_) => ":window undo".to_string(),
            Self::SessionManage(crate::wm::window_manage::SessionManageOp::MoveOrder { name, target_pos }) => match name {
                Some(n) => format!(":session move {n} {target_pos}"),
                None => format!(":session move {target_pos}"),
            },
            Self::SessionManage(crate::wm::window_manage::SessionManageOp::Reorder { names }) => {
                format!(":session reorder {}", names.join(" "))
            }
            Self::SessionManage(_) => ":session undo".to_string(),
            Self::SessionOnEmpty(None) => ":session on-empty".to_string(),
            Self::SessionOnEmpty(Some(p)) => format!(":session on-empty {}", p.label()),
            Self::SessionEnd { name: None, .. } => ":session kill".to_string(),
            Self::SessionEnd { name: Some(n), delete: false } => format!(":session kill {n}"),
            Self::SessionEnd { name: Some(n), delete: true } => format!(":session delete {n}"),
            Self::DaemonSupervisor(_) | Self::Daemon(_) => ":daemon list".to_string(),
            Self::DrawerPreview(DrawerPreviewOp::On) => ":drawer preview on".to_string(),
            Self::DrawerPreview(DrawerPreviewOp::Off) => ":drawer preview off".to_string(),
            Self::DrawerPreview(DrawerPreviewOp::Toggle) => ":drawer preview toggle".to_string(),
            Self::DrawerPreview(DrawerPreviewOp::Commit) => ":drawer preview commit".to_string(),
            Self::DrawerPreview(DrawerPreviewOp::AutoToggle) => ":drawer preview auto".to_string(),
            Self::DrawerPreview(DrawerPreviewOp::Row(idx)) => format!(":drawer preview row {idx}"),
            Self::DrawerPreview(DrawerPreviewOp::Pane(id)) => format!(":drawer preview pane {id}"),
            Self::DrawerPreview(DrawerPreviewOp::Session(name)) => {
                format!(":drawer preview session {name}")
            }
            Self::ScriptStudio(op) => op.colon_form(),
            Self::SessionFind(op) => op.colon_form(),
            Self::Web(op) => op.colon_form(),
            Self::WebOpenExternal { url: None } => ":web external".to_string(),
            Self::WebOpenExternal { url: Some(u) } => format!(":web external {u}"),
            Self::WindowMode(op) => op.colon_form(),
            Self::MenuOpen { category: None } => ":menu open".to_string(),
            Self::MenuOpen { category: Some(c) } => format!(":menu open {c}"),
            Self::Keys(op) => op.colon_form(),
            Self::FontsWindow(op) => op.colon_form(),
            Self::CommandWindow(op) => op.colon_form(),
            Self::ClipboardWindow(op) => op.colon_form(),
            Self::TrafficMonitor(op) => op.colon_form(),
            Self::KeyboardFocus(op) => op.colon_form(),
            Self::Coords(op) => crate::colon_command::coords_colon_form(op),
            Self::Context(op) => op.colon_form(),
            Self::AgentQueue(op) => op.colon_form(),
            Self::AgentGate(op) => op.colon_form(),
            Self::AgentGuide(op) => op.colon_form(),
            Self::Swarm(op) => op.colon_form(),
            Self::PaneFontCommand(sub) => format!(":font {sub}"),
            Self::AgentsList(op) => op.colon_form(),
            Self::SessionMap(op) => op.colon_form(),
            Self::SplitHorizontal => ":split h".to_string(),
            Self::SplitVertical => ":split v".to_string(),
            Self::DeckNext => ":deck next".to_string(),
            Self::DeckPrev => ":deck prev".to_string(),
            Self::FocusPane(id) => format!(":focus {id}"),
            Self::ZoomToggle => ":zoom".to_string(),
            Self::ClosePane => ":close".to_string(),
            Self::Clear => ":clear".to_string(),
            Self::Repaint => ":repaint".to_string(),
            Self::PasteClipboard => ":paste".to_string(),
            Self::Yank => ":yank".to_string(),
            Self::PromptTop => ":top".to_string(),
            Self::PromptBottom => ":bottom".to_string(),
            Self::PromptSplitTop => ":prompt split-top".to_string(),
            Self::PromptSplitBottom => ":prompt split-bottom".to_string(),
            Self::PromptSplitToggle => ":prompt split".to_string(),
            Self::Scrollback { clear: false } => ":scrollback".to_string(),
            Self::Scrollback { clear: true } => ":scrollback clear".to_string(),
            Self::Theme(t) => format!(":theme {}", t.label()),
            Self::Glyphs(g) => format!(":glyphs {:?}", g).to_lowercase(),
            Self::FontZoomIn => ":fontsize +".to_string(),
            Self::FontZoomOut => ":fontsize -".to_string(),
            Self::FontZoomReset => ":fontsize reset".to_string(),
            Self::FontSetSize(pt) => format!(":fontsize {pt}"),
            Self::ArrangeGrid => ":arrange grid".to_string(),
            Self::AppearanceManage(AppearanceOp::List) => ":appearance list".to_string(),
            Self::AppearanceManage(AppearanceOp::Reset) => ":appearance reset".to_string(),
            Self::AppearanceManage(AppearanceOp::SelectName(name)) => {
                format!(":appearance select {name}")
            }
            Self::AppearanceManage(AppearanceOp::Duplicate { src, new_name }) => {
                format!(":appearance duplicate {src} {new_name}")
            }
            Self::AppearanceManage(AppearanceOp::Modify { key, value }) => {
                format!(":appearance modify {key} {value}")
            }
            Self::AppearanceManage(AppearanceOp::Import(path)) => {
                format!(":appearance import {path}")
            }
            Self::AppearanceManage(AppearanceOp::Export(path)) => {
                format!(":appearance export {path}")
            }
            Self::PanelScaleStepOf(crate::ui::typography::PanelTarget::Mcp, d) => {
                format!(":mcp scale {}", if *d >= 0 { "+" } else { "-" })
            }
            Self::PanelScaleStepOf(crate::ui::typography::PanelTarget::Drawer, d) => {
                format!(":drawer scale {}", if *d >= 0 { "110" } else { "90" })
            }
            Self::HelpToggle => ":help".to_string(),
            Self::VersionInfo => ":version".to_string(),
            Self::VersionToggle(Some(true)) => ":version on".to_string(),
            Self::VersionToggle(Some(false)) => ":version off".to_string(),
            Self::VersionToggle(None) => ":version toggle".to_string(),
            other => format!("{other:?}"),
        }
    }
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
