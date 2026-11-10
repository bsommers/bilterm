use super::*;
use crate::api::{AutomationControlPlane, JsonValue};
use crate::colon_command::parse_colon_command;
use crate::input::{
    DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS, handle_wm_key_event,
    is_leader_key_with_tmux,
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
        Action::ClipboardWindow(_) => Some(":clipboard toggle"),
        Action::TrafficMonitor(_) => Some(":traffic toggle"),
        Action::Coords(_) => Some(":coords compact"),
        Action::Context(_) => Some(":context list"),
        Action::AgentQueue(_) => Some(":agent queue list"),
        Action::AgentGate(_) => Some(":agent approve"),
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
        Action::Web(WebOp::Status) => Some(":web status"),
        Action::Web(WebOp::OpenBrowser) => Some(":web open"),
        Action::Web(WebOp::SetPort(_)) => Some(":web port 9000"),
        Action::Web(WebOp::Stop) => Some(":web stop"),
        Action::WindowMode(_) => Some(":wmode"),
        Action::MenuOpen { .. } => Some(":menu open"),
        Action::Keys(_) => Some(":keys check"),
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
        Action::Voice(VoiceOp::Toggle) => Some(":voice toggle"),
        Action::Voice(VoiceOp::Start { pane_id: None }) => Some(":voice start"),
        Action::Voice(VoiceOp::Start { pane_id: Some(_) }) => None,
        Action::Voice(VoiceOp::Stop) => Some(":voice stop"),
        Action::Voice(VoiceOp::Cancel) => Some(":voice cancel"),
        Action::Voice(VoiceOp::Status) => Some(":voice status"),
        Action::Voice(VoiceOp::SetProvider { .. }) => Some(":voice provider openai"),
        Action::Voice(VoiceOp::ApplyTranscript { .. }) => None,
        Action::Voice(VoiceOp::UpdateTelemetry { .. }) => None,
        Action::Voice(VoiceOp::InjectWav { .. }) => None,
        Action::Browser(_) => Some(":browser"),
        Action::Video(_) => Some(":video"),
        Action::Quit => Some(":quit"),
    };

    let in_colon = colon_expr
        .and_then(parse_colon_command)
        .is_some_and(|r| r.is_ok());

    if let Some(first) = colon_expr
        .filter(|_| in_colon)
        .and_then(|e| e.split_whitespace().next())
    {
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
    use crate::input::keymap::{
        DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS,
    };
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
            assert_eq!(
                parse_colon_command(&cmd).and_then(|r| r.ok()).as_ref(),
                Some(a),
                "{cmd}"
            );
        }
    }
    assert!(seen >= 10);
}

#[test]
fn every_completion_entry_is_a_command_the_parser_recognises() {
    for name in crate::pane::completion::colon_command_names() {
        assert!(
            parse_colon_command(name).is_some(),
            "`{name}` is offered by completion but the parser does not know it"
        );
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
        Action::ClipboardWindow(crate::wm::clipboard_window::ClipboardWindowOp::Toggle),
        Action::TrafficMonitor(crate::wm::traffic_monitor::TrafficMonitorOp::Toggle),
        Action::Coords(crate::wm::coordinates::CoordsOp::Show),
        Action::Context(crate::wm::context_queue_ops::ContextOp::List { prefix: None }),
        Action::AgentQueue(crate::wm::context_queue_ops::AgentQueueOp::List),
        Action::AgentGate(crate::wm::context_queue_ops::AgentGateOp::Approve),
        Action::ScriptStudio(crate::wm::script_studio::ScriptStudioOp::Toggle),
        Action::Find(FindOp::Open {
            kind: FindKind::Agent,
            query: String::new(),
        }),
        Action::Find(FindOp::Open {
            kind: FindKind::Session,
            query: "x".to_string(),
        }),
        Action::Find(FindOp::Open {
            kind: FindKind::History,
            query: String::new(),
        }),
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
        Action::Web(WebOp::Status),
        Action::Web(WebOp::OpenBrowser),
        Action::Web(WebOp::SetPort(9000)),
        Action::Web(WebOp::Stop),
        Action::WindowMode(WindowModeOp::Enter),
        Action::WindowMode(WindowModeOp::Exit),
        Action::WindowMode(WindowModeOp::Toggle),
        Action::WindowMode(WindowModeOp::Sub(WinSub::Resize)),
        Action::MenuOpen { category: None },
        Action::MenuOpen { category: Some(2) },
        Action::Keys(KeysOp::List),
        Action::Keys(KeysOp::Check),
        Action::Keys(KeysOp::Explain("Ctrl+X".to_string())),
        Action::Keys(KeysOp::Profile(None)),
        Action::Keys(KeysOp::Profile(Some("mac".to_string()))),
        Action::DrawerPreview(DrawerPreviewOp::On),
        Action::DrawerPreview(DrawerPreviewOp::Off),
        Action::DrawerPreview(DrawerPreviewOp::Toggle),
        Action::DrawerPreview(DrawerPreviewOp::Commit),
        Action::DrawerPreview(DrawerPreviewOp::AutoToggle),
        Action::DrawerPreview(DrawerPreviewOp::Row(0)),
        Action::DrawerPreview(DrawerPreviewOp::Pane(1)),
        Action::DrawerPreview(DrawerPreviewOp::Session("default".to_string())),
        Action::Voice(VoiceOp::Toggle),
        Action::Browser(BrowserOp::Toggle),
        Action::Browser(BrowserOp::Open {
            url: "https://example.com".to_string(),
            pane_id: None,
        }),
        Action::Browser(BrowserOp::Back { pane_id: None }),
        Action::Browser(BrowserOp::Forward { pane_id: None }),
        Action::Browser(BrowserOp::Reload { pane_id: None }),
        Action::Browser(BrowserOp::Stop { pane_id: None }),
        Action::Browser(BrowserOp::FocusViewport {
            pane_id: None,
            focused: Some(true),
        }),
        Action::Browser(BrowserOp::Zoom {
            delta_pct: 10,
            pane_id: None,
        }),
        Action::Browser(BrowserOp::Click {
            x_px: 10,
            y_px: 10,
            button: 0,
            pane_id: None,
        }),
        Action::Browser(BrowserOp::Scroll {
            delta_x: 0,
            delta_y: 10,
            pane_id: None,
        }),
        Action::Browser(BrowserOp::Close { pane_id: None }),
        Action::Video(VideoOp::Toggle),
        Action::Video(VideoOp::Open {
            source: "demo://smpte".to_string(),
            pane_id: None,
        }),
        Action::Video(VideoOp::Play { pane_id: None }),
        Action::Video(VideoOp::Pause { pane_id: None }),
        Action::Video(VideoOp::TogglePlayPause { pane_id: None }),
        Action::Video(VideoOp::SeekRelative {
            delta_ms: 5000,
            pane_id: None,
        }),
        Action::Video(VideoOp::SeekPercent {
            pct: 50,
            pane_id: None,
        }),
        Action::Video(VideoOp::SeekAbsolute {
            position_ms: 10000,
            pane_id: None,
        }),
        Action::Video(VideoOp::VolumeDelta {
            delta_pct: 5,
            pane_id: None,
        }),
        Action::Video(VideoOp::ToggleMute { pane_id: None }),
        Action::Video(VideoOp::ToggleLoop { pane_id: None }),
        Action::Video(VideoOp::SetSpeed {
            speed_x100: 100,
            pane_id: None,
        }),
        Action::Video(VideoOp::Close { pane_id: None }),
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
        handle_wm_key_event(
            &mut wsl_wm,
            KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE),
        )
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
