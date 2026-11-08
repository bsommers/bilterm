//! Unified `Action` command vocabulary (`ARCH-R01`, `REC-02`).
//!
//! Every key chord, `:colon` command, playbook step, and JSON-RPC/MCP workspace command lowers
//! into a typed [`Action`] executed by [`crate::wm::WindowManager::apply`].

use crate::pane::{ExitProtectionPolicy, PaneId, PaneScrollMode, PromptMode};
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::{CaretStyle, SpotlightMode};
use crate::ui::glyphs::GlyphPack;
use crate::ui::theme::ThemePreset;
use crate::wm::{DrawerEdge, DrawerLayoutMode, DrawerVisibilityMode, WorkspaceLayoutMode};
use serde::{Deserialize, Serialize};

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
    /// `:scrollback` (report) / `:scrollback clear` (free history).
    Scrollback { clear: bool },
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
    /// `:sessions map <verb>` / `:packing <verb>` - the SESSION MAP packing viewer (Phase 54, VIEW-02).
    SessionMap(crate::wm::session_map::SessionMapOp),
    /// `:fonts <verb>` - the first-class FONTS utility window (`WIN-01`, Phase 51.2).
    FontsWindow(crate::wm::fonts_window::FontsWindowOp),
    /// `:commands [open|close|toggle|pin|...]` - Pinnable Command Palette window (WIN-02, Phase 51.2).
    CommandWindow(crate::wm::command_window::CommandWindowOp),
    /// `:script <verb>` / `:studio <verb>` - SCRIPT STUDIO floating window (Plan 51.2-03, WIN-03).
    ScriptStudio(crate::wm::script_studio::ScriptStudioOp),
    /// `:find agent|session|history [text]` and the `:finder` modal verbs (Phase 50.1, FND-R01).
    Find(crate::wm::find_cmd::FindOp),
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
    /// `:profile [list|switch|save|default|autosave|next|prev|delete ...]` / `:layout [save|load|default|autosave|next|prev ...]`.
    LayoutProfileCommand(String),
    /// `:mcp [status|tree|attach|spawn|graft|broadcast|freeze|thaw|derez|checkpoint|shutdown ...]` (`MCP-01..02`).
    McpCommand(String),
    /// `:fkeys [bilterm|app|auto|toggle|send <F1..F12>]`: Function-key routing in RAW mode (`FKEY-01`).
    FkeyModeCommand(Option<String>),
    /// `:style [classic|snazzy|glass|both|cycle]` / `:snazzy [on|off]` / `:glass [on|off]` / `F11`: Visual style (`STYLE-01`).
    VisualStyleCommand(Option<String>),
    /// `:agent [name|send|broadcast|inbox|ack|list|enable]` (`AGT-01..03`, Phase 52).
    AgentMail(crate::wm::agent_mail::AgentMailOp),
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
    Quit,
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
    Start { name: Option<String> },
    Stop { name: Option<String>, force: bool },
    Switch { name: String },
    Prune,
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
            Self::AgentMail(op) => op.colon_form(),
            Self::WindowManage(_) => ":window undo".to_string(),
            Self::SessionManage(_) => ":session undo".to_string(),
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
            Self::FontsWindow(op) => op.colon_form(),
            Self::CommandWindow(op) => op.colon_form(),
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
            other => format!("{other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AutomationControlPlane, JsonValue};
    use crate::colon_command::parse_colon_command;
    use crate::input::{
        DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS,
        handle_wm_key_event, is_leader_key_with_tmux,
    };
    use crate::pty::line_discipline::InputRoutingMode;
    use crate::ui::WorkspaceWidget;
    use crate::wm::WindowManager;
    use crate::wm::find_cmd::{FindKind, FindOp};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::Widget;

    fn assert_action_reachable(action: &Action) {
        let in_keymap = DEFAULT_LEADER_BINDINGS.iter().any(|(_, a)| a == action)
            || DEFAULT_GLOBAL_BINDINGS.iter().any(|(_, _, a)| a == action)
            || DEFAULT_PANE_BINDINGS.iter().any(|(_, _, a)| a == action);

        let colon_expr: Option<&str> = match action {
            Action::PromptTop => Some(":top"),
            Action::PromptBottom => Some(":bottom"),
            Action::PromptSplitTop => Some(":prompt split-top"),
            Action::PromptSplitBottom => Some(":prompt split-bottom"),
            Action::PromptSplitToggle => Some(":prompt split"),
            Action::PromptModeSet(None) => Some(":mode"),
            Action::PromptModeSet(Some(PromptMode::Floating)) => Some(":mode float"),
            Action::PromptModeSet(Some(PromptMode::Fixed)) => Some(":mode fixed"),
            Action::PromptModeSet(Some(PromptMode::RegularTerminal)) => Some(":mode terminal"),
            Action::PromptFloat(_) => Some(":prompt float 5"),
            Action::PromptHeight(_) => Some(":prompt height 5"),
            Action::PromptWidth(_) => Some(":prompt width 60"),
            Action::PromptMoveVertical(_) => Some(":prompt move up"),
            Action::PromptResizeHeight(_) => Some(":prompt resize-height +1"),
            Action::PromptResizeWidth(_) => Some(":prompt resize-width +4"),
            Action::BlockPrev => Some(":prev"),
            Action::BlockNext => Some(":next"),
            Action::Yank => Some(":yank"),
            Action::PasteClipboard => Some(":paste"),
            Action::Clear => Some(":clear"),
            Action::Scrollback { clear: false } => Some(":scrollback"),
            Action::Scrollback { clear: true } => Some(":scrollback clear"),
            Action::ScrollModeSet(None) => Some(":scroll"),
            Action::ScrollModeSet(Some(PaneScrollMode::Smooth)) => Some(":scroll smooth"),
            Action::ScrollModeSet(Some(PaneScrollMode::BlockPage)) => Some(":scroll page"),
            Action::ScrollModeSet(Some(PaneScrollMode::Instant)) => Some(":scroll instant"),
            Action::ScrollModeTogglePage => Some(":scroll toggle"),
            Action::ScrollPageAdvance => Some(":scroll next"),
            Action::ScrollPageBack => Some(":scroll prev"),
            Action::RawToggle => Some(":raw"),
            Action::SplitHorizontal => Some(":split h"),
            Action::SplitVertical => Some(":split v"),
            Action::DeckNext => Some(":deck next"),
            Action::DeckPrev => Some(":deck prev"),
            Action::FocusPane(_) => Some(":focus 1"),
            Action::PaneFocusDirection { .. } => Some(":focus right"),
            Action::ZoomToggle => Some(":zoom"),
            Action::ClosePane => Some(":close"),
            Action::LayoutMode(_) => Some(":layout tiled"),
            Action::LayoutCycle => Some(":layout cycle"),
            Action::QuakeToggle => Some(":quake"),
            Action::HelpToggle => Some(":help"),
            Action::HistoryPopupToggle => Some(":history"),
            Action::HistorySave(_) => Some(":history save"),
            Action::HistoryToggle(_) => Some(":history toggle"),
            Action::HistoryMaxFiles(_) => Some(":history max-files 10"),
            Action::HistoryRuns => Some(":history runs"),
            Action::Theme(_) => Some(":theme borland"),
            Action::Glyphs(_) => Some(":glyphs unicode"),
            Action::Appearance(_) => Some(":appearance obsidian"),
            Action::AppearanceManage(_) => Some(":appearance list"),
            Action::Mode(_) => Some(":mode zen"),
            Action::PanelScale(_) => Some(":panel scale 125"),
            Action::PanelScaleOf(..) => Some(":drawer scale 125"),
            Action::PanelScaleStepOf(..) => Some(":mcp scale +"),
            Action::ChromeTitle(_) => Some(":chrome title focused"),
            Action::ChromeRing(_) => Some(":chrome ring on"),
            Action::Cursor(_) => Some(":cursor block"),
            Action::Outline { .. } => Some(":outline double"),
            Action::CrosshairToggle { .. } => Some(":crosshair"),
            Action::CrosshairCycle => Some(":crosshair cycle"),
            Action::Spotlight { .. } => Some(":spotlight"),
            Action::FocalZoom { .. } => Some(":focalzoom"),
            Action::FocalRadiusResize(_, _) => Some(":focal radius +4 +1"),
            Action::Shadows(_) => Some(":shadows"),
            Action::FKeyBarToggle(_) => Some(":fkeybar"),
            Action::SearchQuery(_) => Some(":search foo"),
            Action::SearchPromptPrefill => Some(":search prefill"),
            Action::ConfigGet(_) => Some(":get appearance.profile"),
            Action::ConfigSet(..) => Some(":set theme turbo-blue"),
            Action::ConfigList => Some(":config list"),
            Action::ConfigSave => Some(":config save"),
            Action::AppearanceSave => Some(":appearance save"),
            Action::StateQuery => Some(":state"),
            Action::SearchNext => Some(":search next"),
            Action::SearchPrev => Some(":search prev"),
            Action::SearchOff => Some(":search off"),
            Action::CopyModeToggle => Some(":copy"),
            Action::SelectCommand(_) => Some(":rect"),
            Action::LinksYank => Some(":links"),
            Action::InspectorToggle(_) => Some(":inspector"),
            Action::HintsToggle => Some(":hints"),
            Action::LockToggle(_) => Some(":lock"),
            Action::BroadcastToggle(_) => Some(":broadcast"),
            Action::MenuBarToggle(_) => Some(":menubar"),
            Action::VersionToggle(_) => Some(":version"),
            Action::SessionSave(_) => Some(":session save demo"),
            Action::SessionRestore(_) => Some(":session restore demo"),
            Action::SessionList => Some(":session list"),
            Action::CrtToggle(_) => Some(":crt"),
            Action::PaletteToggle => Some(":palette"),
            Action::NotificationsClear => Some(":notifications clear"),
            Action::TriggerAdd(_) => Some(":trigger add ERROR"),
            Action::TriggerClear => Some(":trigger clear"),
            Action::RecordStart(_) => Some(":record start"),
            Action::RecordWorkspaceStart(_) => Some(":record workspace"),
            Action::RecordPaneStart(_) => Some(":record pane"),
            Action::RecordStop => Some(":record stop"),
            Action::SshConnect { .. } => Some(":ssh user@host"),
            Action::SshTmuxConnect { .. } => Some(":ssh-tmux user@host main"),
            Action::RemoteAdd { .. } => Some(":remote add web1 user@host prod"),
            Action::RemoteList => Some(":remote list"),
            Action::RemoteExec { .. } => Some(":remote exec @all uptime"),
            Action::RemoteDisconnect(_) => Some(":remote disconnect web1"),
            Action::RemoteLink(_) => Some(":ssh-tmux info"),
            Action::DrawerToggle => Some(":drawer"),
            Action::DrawerFocus => Some(":drawer focus"),
            Action::DrawerDock(_) => Some(":drawer left"),
            Action::DrawerVisibility(_) => Some(":drawer fixed"),
            Action::DrawerLayout(_) => Some(":drawer push"),
            Action::DrawerSize(_) => Some(":drawer size 32"),
            Action::DrawerNav(_) => Some(":drawer attention"),
            Action::DrawerStyle(_) => Some(":drawer style navigator"),
            Action::DrawerRows(_) => Some(":drawer rows 2"),
            Action::Agents(_) => Some(":agents list"),
            Action::AgentsList(_) => Some(":agents list toggle"),
            Action::SessionMap(_) => Some(":sessions map toggle"),
            Action::FontsWindow(_) => Some(":fonts toggle"),
            Action::CommandWindow(_) => Some(":commands toggle"),
            Action::ScriptStudio(_) => Some(":script toggle"),
            Action::Find(_) => Some(":find agent"),
            Action::FontZoomIn => Some(":fontsize +"),
            Action::FontZoomOut => Some(":fontsize -"),
            Action::FontZoomReset => Some(":fontsize reset"),
            Action::FontSetSize(_) => Some(":fontsize 18"),
            Action::PaneFontCommand(_) => Some(":font ibm3270 16pt"),
            Action::CanvasCommand(_) => Some(":canvas tek demo"),
            Action::PaneResize { .. } => Some(":resize +w"),
            Action::PaneMove { .. } => Some(":move right"),
            Action::WindowResetGeometry => Some(":cascade"),
            Action::ArrangeGrid => Some(":arrange grid"),
            Action::ForceQuit => Some(":quit!"),
            Action::DetachClient => Some(":detach"),
            Action::ExitProtectionSet(_) => Some(":exit-guard guard"),
            Action::RespawnPaneInPlace => Some(":respawn-pane"),
            Action::ThemeCommand(_) => Some(":theme list"),
            Action::WindowStyle(_) => Some(":window outline dashed"),
            Action::SessionTreeCommand(_) => Some(":session tree"),
            Action::LayoutProfileCommand(_) => Some(":profile list"),
            Action::McpCommand(_) => Some(":mcp status"),
            Action::FkeyModeCommand(_) => Some(":fkeys bilterm"),
            Action::VisualStyleCommand(_) => Some(":style snazzy"),
            Action::AgentMail(_) => Some(":agent list"),
            Action::WindowManage(_) => Some(":window undo"),
            Action::SessionManage(_) => Some(":session undo"),
            Action::DaemonSupervisor(_) | Action::Daemon(_) => Some(":daemon list"),
            Action::DrawerPreview(DrawerPreviewOp::On) => Some(":drawer preview on"),
            Action::DrawerPreview(DrawerPreviewOp::Off) => Some(":drawer preview off"),
            Action::DrawerPreview(DrawerPreviewOp::Toggle) => Some(":drawer preview toggle"),
            Action::DrawerPreview(DrawerPreviewOp::Commit) => Some(":drawer preview commit"),
            Action::DrawerPreview(DrawerPreviewOp::AutoToggle) => Some(":drawer preview auto"),
            Action::DrawerPreview(DrawerPreviewOp::Row(_)) => Some(":drawer preview row 0"),
            Action::DrawerPreview(DrawerPreviewOp::Pane(_)) => Some(":drawer preview pane 1"),
            Action::DrawerPreview(DrawerPreviewOp::Session(_)) => {
                Some(":drawer preview session default")
            }
            Action::Quit => Some(":quit"),
        };

        let in_colon = colon_expr
            .and_then(parse_colon_command)
            .is_some_and(|r| r.is_ok());

        if let Some(first) = colon_expr.filter(|_| in_colon).and_then(|e| e.split_whitespace().next()) {
            assert!(
                crate::pane::completion::colon_command_names().contains(&first),
                "{action:?}: `{first}` parses but is missing from the completion list (COLON_COMMANDS)"
            );
        }

        assert!(
            in_keymap || in_colon,
            "Action variant {action:?} must be reachable from keymap or colon/RPC parser"
        );
    }

    #[test]
    fn key_only_actions_round_trip_through_colon_strings() {
        use crate::input::keymap::{DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS};
        let colon_for = |a: &Action| -> Option<String> {
            Some(match a {
                Action::CrosshairCycle => ":crosshair cycle".into(),
                Action::LayoutCycle => ":layout cycle".into(),
                Action::SearchPromptPrefill => ":search prefill".into(),
                Action::FocalRadiusResize(x, y) => format!(":focal radius {x:+} {y:+}"),
                Action::PromptMoveVertical(d) => {
                    format!(":prompt move {}", if *d < 0 { "up" } else { "down" })
                }
                Action::PromptResizeHeight(d) => format!(":prompt resize-height {d:+}"),
                Action::PromptResizeWidth(d) => format!(":prompt resize-width {d:+}"),
                _ => return None,
            })
        };
        let all: Vec<&Action> = DEFAULT_LEADER_BINDINGS
            .iter()
            .map(|(_, a)| a)
            .chain(DEFAULT_GLOBAL_BINDINGS.iter().map(|(_, _, a)| a))
            .chain(DEFAULT_PANE_BINDINGS.iter().map(|(_, _, a)| a))
            .collect();
        let mut seen = 0;
        for a in all {
            if let Some(cmd) = colon_for(a) {
                seen += 1;
                assert_eq!(parse_colon_command(&cmd).and_then(|r| r.ok()).as_ref(), Some(a), "{cmd}");
            }
        }
        assert!(seen >= 10);
    }

    #[test]
    fn every_completion_entry_is_a_command_the_parser_recognises() {
        for name in crate::pane::completion::colon_command_names() {
            assert!(parse_colon_command(name).is_some(), "`{name}` is offered by completion but the parser does not know it");
        }
    }

    #[test]
    fn every_action_is_reachable_from_keymap_colon_or_rpc() {
        let samples = vec![
            Action::PromptTop,
            Action::PromptBottom,
            Action::PromptSplitTop,
            Action::PromptSplitBottom,
            Action::PromptSplitToggle,
            Action::PromptModeSet(None),
            Action::PromptFloat(5),
            Action::PromptHeight(5),
            Action::PromptWidth(Some(60)),
            Action::PromptMoveVertical(-1),
            Action::PromptResizeHeight(1),
            Action::PromptResizeWidth(4),
            Action::BlockPrev,
            Action::BlockNext,
            Action::Yank,
            Action::PasteClipboard,
            Action::Clear,
            Action::Scrollback { clear: false },
            Action::Scrollback { clear: true },
            Action::ScrollModeSet(None),
            Action::ScrollModeSet(Some(PaneScrollMode::Smooth)),
            Action::ScrollModeSet(Some(PaneScrollMode::BlockPage)),
            Action::ScrollModeSet(Some(PaneScrollMode::Instant)),
            Action::ScrollModeTogglePage,
            Action::ScrollPageAdvance,
            Action::ScrollPageBack,
            Action::RawToggle,
            Action::SplitHorizontal,
            Action::SplitVertical,
            Action::DeckNext,
            Action::DeckPrev,
            Action::FocusPane(1),
            Action::PaneFocusDirection { dx: 1, dy: 0 },
            Action::ZoomToggle,
            Action::ClosePane,
            Action::LayoutMode(WorkspaceLayoutMode::Tiled),
            Action::LayoutCycle,
            Action::QuakeToggle,
            Action::HelpToggle,
            Action::HistoryPopupToggle,
            Action::HistorySave(None),
            Action::HistoryToggle(None),
            Action::HistoryMaxFiles(10),
            Action::HistoryRuns,
            Action::Theme(ThemePreset::default()),
            Action::Glyphs(GlyphPack::Unicode),
            Action::Appearance(crate::ui::design::AppearanceProfile::Obsidian),
            Action::AppearanceManage(AppearanceOp::List),
            Action::Mode(crate::ui::design::DisplayMode::Zen),
            Action::PanelScale(125),
            Action::PanelScaleOf(crate::ui::typography::PanelTarget::Mcp, 125),
            Action::PanelScaleStepOf(crate::ui::typography::PanelTarget::Mcp, 10),
            Action::ChromeTitle(Some(crate::ui::design::TitlePolicy::Focused)),
            Action::ChromeRing(true),
            Action::Cursor(CaretStyle::SolidBlock),
            Action::Outline {
                window: Some(OutlineStyle::Double),
                prompt: Some(OutlineStyle::Double),
            },
            Action::CrosshairToggle {
                enabled: None,
                fullscreen: None,
            },
            Action::CrosshairCycle,
            Action::spotlight_cycle(),
            Action::focal_zoom_cycle(),
            Action::FocalRadiusResize(4, 1),
            Action::Shadows(None),
            Action::FKeyBarToggle(None),
            Action::SearchQuery("foo".to_string()),
            Action::SearchPromptPrefill,
            Action::ConfigGet("theme".to_string()),
            Action::ConfigSet("theme".to_string(), "turbo-blue".to_string()),
            Action::ConfigList,
            Action::ConfigSave,
            Action::AppearanceSave,
            Action::StateQuery,
            Action::SearchNext,
            Action::SearchPrev,
            Action::SearchOff,
            Action::CopyModeToggle,
            Action::SelectCommand(None),
            Action::LinksYank,
            Action::InspectorToggle(None),
            Action::HintsToggle,
            Action::LockToggle(None),
            Action::BroadcastToggle(None),
            Action::MenuBarToggle(None),
            Action::VersionToggle(None),
            Action::SessionSave("demo".to_string()),
            Action::SessionRestore("demo".to_string()),
            Action::SessionList,
            Action::CrtToggle(None),
            Action::PaletteToggle,
            Action::NotificationsClear,
            Action::TriggerAdd("ERROR".to_string()),
            Action::TriggerClear,
            Action::RecordStart(None),
            Action::RecordWorkspaceStart(None),
            Action::RecordPaneStart(None),
            Action::RecordStop,
            Action::SshConnect {
                target: "user@host".to_string(),
                port: None,
            },
            Action::SshTmuxConnect {
                target: "user@host".to_string(),
                session: "main".to_string(),
            },
            Action::RemoteAdd {
                name: "web1".to_string(),
                destination: "user@host".to_string(),
                tags: vec!["prod".to_string()],
            },
            Action::RemoteList,
            Action::RemoteExec {
                selector: "@all".to_string(),
                command: "uptime".to_string(),
            },
            Action::RemoteDisconnect("web1".to_string()),
            Action::RemoteLink(crate::wm::link_cmd::RemoteLinkOp::Prefill),
            Action::RemoteLink(crate::wm::link_cmd::RemoteLinkOp::Reconnect),
            Action::RemoteLink(crate::wm::link_cmd::RemoteLinkOp::Disconnect),
            Action::RemoteLink(crate::wm::link_cmd::RemoteLinkOp::Info),
            Action::DrawerToggle,
            Action::DrawerFocus,
            Action::DrawerDock(Some(DrawerEdge::Left)),
            Action::DrawerVisibility(Some(DrawerVisibilityMode::Fixed)),
            Action::DrawerLayout(Some(DrawerLayoutMode::Push)),
            Action::DrawerSize(32),
            Action::DrawerNav(crate::wm::drawer::DrawerNavOp::NextAttention),
            Action::DrawerNav(crate::wm::drawer::DrawerNavOp::ExpandAll),
            Action::DrawerNav(crate::wm::drawer::DrawerNavOp::CollapseAll),
            Action::DrawerStyle(Some(crate::wm::drawer::DrawerStyle::Navigator)),
            Action::DrawerRows(2),
            Action::Agents(crate::wm::agents_cmd::AgentsOp::List),
            Action::AgentsList(crate::wm::agents_list_cmd::AgentsListOp::Toggle),
            Action::SessionMap(crate::wm::session_map::SessionMapOp::Toggle),
            Action::FontsWindow(crate::wm::fonts_window::FontsWindowOp::Toggle),
            Action::CommandWindow(crate::wm::command_window::CommandWindowOp::Toggle),
            Action::ScriptStudio(crate::wm::script_studio::ScriptStudioOp::Toggle),
            Action::Find(FindOp::Open { kind: FindKind::Agent, query: String::new() }),
            Action::Find(FindOp::Open { kind: FindKind::Session, query: "x".to_string() }),
            Action::Find(FindOp::Open { kind: FindKind::History, query: String::new() }),
            Action::Find(FindOp::SetKind(FindKind::Session)),
            Action::Find(FindOp::CycleKind(true)),
            Action::Find(FindOp::SetQuery("x".to_string())),
            Action::Find(FindOp::Push('a')),
            Action::Find(FindOp::Pop),
            Action::Find(FindOp::ClearQuery),
            Action::Find(FindOp::Move(1)),
            Action::Find(FindOp::Select(0)),
            Action::Find(FindOp::Activate),
            Action::Find(FindOp::Prefill),
            Action::Find(FindOp::Close),
            Action::FontZoomIn,
            Action::FontZoomOut,
            Action::FontZoomReset,
            Action::FontSetSize(18),
            Action::PaneFontCommand("ibm3270 16pt".to_string()),
            Action::CanvasCommand("tek demo".to_string()),
            Action::PaneResize { dw: 4, dh: 0 },
            Action::PaneMove { dx: 4, dy: 0 },
            Action::WindowResetGeometry,
            Action::ArrangeGrid,
            Action::ForceQuit,
            Action::DetachClient,
            Action::ExitProtectionSet(ExitProtectionPolicy::Guard),
            Action::RespawnPaneInPlace,
            Action::ThemeCommand("list".to_string()),
            Action::WindowStyle("outline dashed".to_string()),
            Action::SessionTreeCommand("tree".to_string()),
            Action::LayoutProfileCommand("list".to_string()),
            Action::McpCommand("status".to_string()),
            Action::FkeyModeCommand(None),
            Action::VisualStyleCommand(None),
            Action::AgentMail(crate::wm::agent_mail::AgentMailOp::List),
            Action::WindowManage(crate::wm::window_manage::WindowManageOp::Undo),
            Action::SessionManage(crate::wm::window_manage::SessionManageOp::Undo),
            Action::DaemonSupervisor(DaemonSupervisorOp::List),
            Action::Daemon(DaemonSupervisorOp::List),
            Action::DrawerPreview(DrawerPreviewOp::On),
            Action::DrawerPreview(DrawerPreviewOp::Off),
            Action::DrawerPreview(DrawerPreviewOp::Toggle),
            Action::DrawerPreview(DrawerPreviewOp::Commit),
            Action::DrawerPreview(DrawerPreviewOp::AutoToggle),
            Action::DrawerPreview(DrawerPreviewOp::Row(0)),
            Action::DrawerPreview(DrawerPreviewOp::Pane(1)),
            Action::DrawerPreview(DrawerPreviewOp::Session("default".to_string())),
            Action::Quit,
        ];

        for action in &samples {
            assert_action_reachable(action);
        }

        // Also verify RPC wm.command lowers through WindowManager::apply(Action)
        let mut cp = AutomationControlPlane::new_headless(24, 80);
        let res = cp
            .dispatch_method(
                "wm.command",
                &JsonValue::obj([("command", JsonValue::String(":split v".to_string()))]),
            )
            .expect("wm.command split v");
        assert_eq!(res.get("pane_count").and_then(JsonValue::as_usize), Some(2));
    }

    #[test]
    fn dual_leader_ctrl_b_and_ctrl_x_with_tmux_fallback_and_held_ctrl_chords_work() {
        let ctrl_b = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL);
        let ctrl_x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);

        // Standalone (!inside_tmux): both Ctrl+B and Ctrl+X trigger leader mode
        assert!(is_leader_key_with_tmux(ctrl_b, "Ctrl+X", false));
        assert!(is_leader_key_with_tmux(ctrl_x, "Ctrl+X", false));

        // Inside outer tmux (inside_tmux == true): Ctrl+B passes through to outer tmux; Ctrl+X is Bilterm leader
        assert!(!is_leader_key_with_tmux(ctrl_b, "Ctrl+X", true));
        assert!(is_leader_key_with_tmux(ctrl_x, "Ctrl+X", true));

        let mut wm = WindowManager::new_headless(24, 80);
        // Ctrl+B % splits vertically (tmux chord parity)
        handle_wm_key_event(&mut wm, ctrl_b).unwrap();
        assert!(wm.is_leader_pending());
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('%'), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(wm.pane_count(), 2);

        // Ctrl+B " splits horizontally (tmux chord parity)
        handle_wm_key_event(&mut wm, ctrl_b).unwrap();
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('"'), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(wm.pane_count(), 3);

        // Held-Ctrl leader chords: Ctrl+X Ctrl+Z toggles zoom, Ctrl+B Ctrl+P / Ctrl+B Ctrl+N cycle deck
        handle_wm_key_event(&mut wm, ctrl_x).unwrap();
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert!(wm.zoomed_pane().is_some());

        handle_wm_key_event(&mut wm, ctrl_b).unwrap();
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert!(wm.zoomed_pane().is_none());

        let before = wm.active_pane_id();
        handle_wm_key_event(&mut wm, ctrl_b).unwrap();
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert_ne!(wm.active_pane_id(), before);
    }

    #[test]
    fn asciinema_pty_wrapper_override_restores_floating_prompt_without_breaking_sudo_lock() {
        let mut wm = WindowManager::new_headless(24, 80);
        // Simulate asciinema rec / script PTY wrapper disabling ECHO on outer PTY without ALT_SCREEN or password prompt
        wm.active_pane_mut().line_discipline_mut().echo_enabled = false;
        assert_eq!(
            wm.active_pane().effective_routing_mode(),
            InputRoutingMode::RawPtyPassthrough
        );

        // Pressing Ctrl+X Ctrl+E (or F4 / F12) explicitly restores CommandPrompt over the asciinema wrapper
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        )
        .unwrap();
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert!(wm.active_pane().is_prompt_visible());
        assert_eq!(
            wm.active_pane().effective_routing_mode(),
            InputRoutingMode::CommandPrompt
        );

        // When an actual [sudo] password prompt appears on the canvas while a command is running,
        // is_active_canvas_password_prompt() still enforces RawPtyPassthrough so secrets never leak!
        wm.active_pane_mut().prompt_mut().running_command = Some("sudo ls".to_string());
        wm.active_pane_mut()
            .process_pty_output(b"[sudo] password for user: ");
        assert_eq!(
            wm.active_pane().effective_routing_mode(),
            InputRoutingMode::RawPtyPassthrough
        );

        // Regression test (WSL `sudo apt install ...` from 3270 prompt after F4 mode cycle):
        // Even if the shell was at PromptStart (OSC 133;A) and force_command_prompt was true from F4,
        // submitting `sudo apt install htop` from the 3270 prompt and ticking refresh_line_discipline
        // before `[sudo] password for user: ` arrives must switch to RawPtyPassthrough and route
        // password keystrokes to the PTY instead of the 3270 prompt editor.
        let mut wsl_wm = WindowManager::new_headless(24, 80);
        wsl_wm
            .active_pane_mut()
            .process_pty_output(b"\x1b]133;A\x07bills@wsl:~$ ");
        // User cycles F4 back to Floating Prompt (setting force_command_prompt = true)
        for _ in 0..3 {
            handle_wm_key_event(&mut wsl_wm, KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE))
                .unwrap();
        }
        assert!(wsl_wm.active_pane().is_prompt_visible());

        for ch in "sudo apt install htop".chars() {
            handle_wm_key_event(
                &mut wsl_wm,
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            )
            .unwrap();
        }
        handle_wm_key_event(
            &mut wsl_wm,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        )
        .unwrap();
        assert!(!wsl_wm.active_pane().prompt().force_command_prompt);

        // Simulate 16ms tick before sudo prints password prompt
        wsl_wm.active_pane_mut().refresh_line_discipline();
        assert_eq!(
            wsl_wm.active_pane().prompt().running_command.as_deref(),
            Some("sudo apt install htop")
        );

        // sudo prints password prompt on canvas
        wsl_wm
            .active_pane_mut()
            .process_pty_output(b"\r\n[sudo] password for bills: ");
        assert!(!wsl_wm.active_pane().is_prompt_visible());
        assert_eq!(
            wsl_wm.active_pane().effective_routing_mode(),
            InputRoutingMode::RawPtyPassthrough
        );

        for ch in "my_wsl_pw".chars() {
            handle_wm_key_event(
                &mut wsl_wm,
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            )
            .unwrap();
        }
        assert_eq!(wsl_wm.active_pane().prompt().input, "");
    }

    #[test]
    fn whole_workspace_asciicast_v2_recorder_captures_multi_pane_buffer_frames() {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.apply(Action::SplitVertical).unwrap();
        wm.pane_mut(1)
            .unwrap()
            .process_pty_output(b"\r\n\r\n\r\n\r\nLEFT_PANE_3270\r\n");
        wm.pane_mut(2)
            .unwrap()
            .process_pty_output(b"\r\n\r\n\r\n\r\nRIGHT_PANE_BORLAND\r\n");

        let dir = tempfile::tempdir().expect("tempdir");
        let cast_path = dir.path().join("workspace-demo.cast");
        wm.apply(Action::RecordWorkspaceStart(Some(
            cast_path.to_string_lossy().into_owned(),
        )))
        .unwrap();
        assert!(wm.is_workspace_recording());

        let viewport = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(viewport);
        WorkspaceWidget::new(&wm).render(viewport, &mut buf);
        assert!(wm.has_recorded_workspace_frames());

        wm.apply(Action::RecordStop).unwrap();
        assert!(!wm.is_workspace_recording());

        let content = std::fs::read_to_string(&cast_path).expect("read workspace .cast");
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines.len() >= 2, "expected header + frame event");
        assert!(lines[0].contains("\"version\":2"));
        assert!(lines[0].contains("\"width\":80"));
        assert!(lines[0].contains("\"height\":24"));
        assert!(content.contains("LEFT_PANE_3270"));
        assert!(content.contains("RIGHT_PANE_BORLAND"));
    }
}
