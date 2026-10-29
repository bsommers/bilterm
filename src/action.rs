//! Unified `Action` command vocabulary (`ARCH-R01`, `REC-02`).
//!
//! Every key chord, `:colon` command, playbook step, and JSON-RPC/MCP workspace command lowers
//! into a typed [`Action`] executed by [`crate::wm::WindowManager::apply`].

use crate::pane::{ExitProtectionPolicy, PaneScrollMode, PromptMode};
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::{CaretStyle, SpotlightMode};
use crate::ui::glyphs::GlyphPack;
use crate::ui::theme::ThemePreset;
use crate::wm::{DrawerEdge, DrawerLayoutMode, DrawerVisibilityMode, WorkspaceLayoutMode};

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
    DrawerToggle,
    DrawerFocus,
    DrawerDock(Option<DrawerEdge>),
    DrawerVisibility(Option<DrawerVisibilityMode>),
    DrawerLayout(Option<DrawerLayoutMode>),
    DrawerSize(u16),
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
    /// `:session branch|graft|detach-subtree|tree|checkpoint ...` (`TREE-01..02`, `PERS-01..02`).
    SessionTreeCommand(String),
    /// `:profile [list|switch|save|default|autosave|next|prev|delete ...]` / `:layout [save|load|default|autosave|next|prev ...]`.
    LayoutProfileCommand(String),
    /// `:mcp [status|tree|attach|spawn|graft|broadcast|freeze|thaw|derez|checkpoint|shutdown ...]` (`MCP-01..02`).
    McpCommand(String),
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
            Action::PromptMoveVertical(_) => None,
            Action::PromptResizeHeight(_) => None,
            Action::PromptResizeWidth(_) => None,
            Action::BlockPrev => Some(":prev"),
            Action::BlockNext => Some(":next"),
            Action::Yank => Some(":yank"),
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
            Action::ZoomToggle => Some(":zoom"),
            Action::ClosePane => Some(":close"),
            Action::LayoutMode(_) => Some(":layout tiled"),
            Action::LayoutCycle => None,
            Action::QuakeToggle => Some(":quake"),
            Action::HelpToggle => Some(":help"),
            Action::HistoryPopupToggle => Some(":history"),
            Action::HistorySave(_) => Some(":history save"),
            Action::HistoryToggle(_) => Some(":history toggle"),
            Action::HistoryMaxFiles(_) => Some(":history max-files 10"),
            Action::HistoryRuns => Some(":history runs"),
            Action::Theme(_) => Some(":theme borland"),
            Action::Glyphs(_) => Some(":glyphs unicode"),
            Action::Cursor(_) => Some(":cursor block"),
            Action::Outline { .. } => Some(":outline double"),
            Action::CrosshairToggle { .. } => Some(":crosshair"),
            Action::CrosshairCycle => None,
            Action::Spotlight { .. } => Some(":spotlight"),
            Action::FocalZoom { .. } => Some(":focalzoom"),
            Action::FocalRadiusResize(_, _) => None,
            Action::Shadows(_) => Some(":shadows"),
            Action::FKeyBarToggle(_) => Some(":fkeybar"),
            Action::SearchQuery(_) => Some(":search foo"),
            Action::SearchPromptPrefill => None,
            Action::SearchNext => Some(":search next"),
            Action::SearchPrev => Some(":search prev"),
            Action::SearchOff => Some(":search off"),
            Action::CopyModeToggle => Some(":copy"),
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
            Action::DrawerToggle => Some(":drawer"),
            Action::DrawerFocus => Some(":drawer focus"),
            Action::DrawerDock(_) => Some(":drawer left"),
            Action::DrawerVisibility(_) => Some(":drawer fixed"),
            Action::DrawerLayout(_) => Some(":drawer push"),
            Action::DrawerSize(_) => Some(":drawer size 32"),
            Action::FontZoomIn => Some(":fontsize +"),
            Action::FontZoomOut => Some(":fontsize -"),
            Action::FontZoomReset => Some(":fontsize reset"),
            Action::FontSetSize(_) => Some(":fontsize 18"),
            Action::PaneFontCommand(_) => Some(":font ibm3270 16pt"),
            Action::CanvasCommand(_) => Some(":canvas tek demo"),
            Action::PaneResize { .. } => Some(":resize +w"),
            Action::PaneMove { .. } => Some(":move right"),
            Action::WindowResetGeometry => Some(":cascade"),
            Action::ForceQuit => Some(":quit!"),
            Action::DetachClient => Some(":detach"),
            Action::ExitProtectionSet(_) => Some(":exit-guard guard"),
            Action::RespawnPaneInPlace => Some(":respawn-pane"),
            Action::ThemeCommand(_) => Some(":theme list"),
            Action::SessionTreeCommand(_) => Some(":session tree"),
            Action::LayoutProfileCommand(_) => Some(":profile list"),
            Action::McpCommand(_) => Some(":mcp status"),
            Action::Quit => Some(":quit"),
        };

        let in_colon = colon_expr
            .and_then(parse_colon_command)
            .is_some_and(|r| r.is_ok());

        assert!(
            in_keymap || in_colon,
            "Action variant {action:?} must be reachable from keymap or colon/RPC parser"
        );
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
            Action::SearchNext,
            Action::SearchPrev,
            Action::SearchOff,
            Action::CopyModeToggle,
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
            Action::DrawerToggle,
            Action::DrawerFocus,
            Action::DrawerDock(Some(DrawerEdge::Left)),
            Action::DrawerVisibility(Some(DrawerVisibilityMode::Fixed)),
            Action::DrawerLayout(Some(DrawerLayoutMode::Push)),
            Action::DrawerSize(32),
            Action::FontZoomIn,
            Action::FontZoomOut,
            Action::FontZoomReset,
            Action::FontSetSize(18),
            Action::PaneFontCommand("ibm3270 16pt".to_string()),
            Action::CanvasCommand("tek demo".to_string()),
            Action::PaneResize { dw: 4, dh: 0 },
            Action::PaneMove { dx: 4, dy: 0 },
            Action::WindowResetGeometry,
            Action::ForceQuit,
            Action::DetachClient,
            Action::ExitProtectionSet(ExitProtectionPolicy::Guard),
            Action::RespawnPaneInPlace,
            Action::ThemeCommand("list".to_string()),
            Action::SessionTreeCommand("tree".to_string()),
            Action::LayoutProfileCommand("list".to_string()),
            Action::McpCommand("status".to_string()),
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
