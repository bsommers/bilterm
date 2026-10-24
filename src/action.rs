//! Unified `Action` command vocabulary (`ARCH-R01`, `REC-02`).
//!
//! Every key chord, `:colon` command, playbook step, and JSON-RPC/MCP workspace command lowers
//! into a typed [`Action`] executed by [`crate::wm::WindowManager::apply`].

use crate::pane::PromptMode;
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::{CaretStyle, SpotlightMode};
use crate::ui::glyphs::GlyphPack;
use crate::ui::theme::ThemePreset;
use crate::wm::WorkspaceLayoutMode;

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
    Clear,
    /// `:scrollback` (report) / `:scrollback clear` (free history).
    Scrollback { clear: bool },
    RawToggle,
    SplitHorizontal,
    SplitVertical,
    DeckNext,
    DeckPrev,
    FocusPane(usize),
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
    SearchNext,
    SearchPrev,
    SearchOff,
    CopyModeToggle,
    LinksYank,
    InspectorToggle(Option<bool>),
    HintsToggle,
    LockToggle(Option<bool>),
    BroadcastToggle(Option<bool>),
    MenuBarToggle(Option<bool>),
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
    Quit,
}

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
}
