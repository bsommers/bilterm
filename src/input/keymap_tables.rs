use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::wm::SplitDirection;
use crate::wm::agents_cmd::{AgentsOp, KeywordEdit, SnoopFilterKind, SnoopFilterOp};
use crate::wm::agents_list_cmd::AgentsListOp;
use crate::wm::agents_window::GraphDir;
use crate::wm::drawer::DrawerNavOp;
use crate::wm::find_cmd::{FindKind, FindOp};
use crate::wm::link_cmd::RemoteLinkOp;
use crate::wm::session_map::SessionMapOp;

/// Declarative leader-chord table (`<Leader> + key` -> [`Action`], `ARCH-R01.2`).
///
/// Unifies classic Borland/DOS chords, `tmux` prefix chords (`"`, `%`, `z`, `n`, `p`, `o`, `[`, `:`),
/// and `herdr` workspace chords into a single data table.
pub const DEFAULT_LEADER_BINDINGS: &[(KeyCode, Action)] = &[
    // Tmux / Herdr / Borland window & split management
    (KeyCode::Char('"'), Action::SplitHorizontal),
    (KeyCode::Char('-'), Action::SplitHorizontal),
    (KeyCode::Char('h'), Action::SplitHorizontal),
    (KeyCode::F(2), Action::SplitHorizontal),
    (KeyCode::Char('%'), Action::SplitVertical),
    (KeyCode::Char('|'), Action::SplitVertical),
    (KeyCode::Char('\\'), Action::SplitVertical),
    (KeyCode::Char('V'), Action::SplitVertical),
    (KeyCode::Char('!'), Action::DrawerNav(crate::wm::drawer::DrawerNavOp::NextAttention)),
    (KeyCode::Char('a'), Action::Agents(AgentsOp::Zoom(None))),
    // AGENTS list toggle and agent demo / stop (Phase 51-07, D-03)
    (KeyCode::Char('A'), Action::AgentsList(AgentsListOp::Toggle)),
    (KeyCode::Char('S'), Action::SessionMap(SessionMapOp::Toggle)),
    (KeyCode::Char('y'), Action::Agents(AgentsOp::Demo)),
    (KeyCode::Char('Y'), Action::Agents(AgentsOp::DemoStop)),
    // Persistent tmux link: prefill `:ssh-tmux `, reconnect, disconnect (Phase 51.1, P-16)
    (KeyCode::Char('u'), Action::RemoteLink(RemoteLinkOp::Prefill)),
    (KeyCode::Char('U'), Action::RemoteLink(RemoteLinkOp::Reconnect)),
    (KeyCode::Char('Q'), Action::RemoteLink(RemoteLinkOp::Disconnect)),
    (KeyCode::Char('z'), Action::ZoomToggle),
    (KeyCode::Char('Z'), Action::ZoomToggle),
    (KeyCode::F(5), Action::ZoomToggle),
    (KeyCode::Char('o'), Action::DeckNext),
    (KeyCode::Char('O'), Action::DeckNext),
    (KeyCode::Char('n'), Action::DeckNext),
    (KeyCode::Tab, Action::DeckNext),
    (KeyCode::F(6), Action::DeckNext),
    (KeyCode::BackTab, Action::DeckPrev),
    // Direct OS clipboard paste into active pane / Raw PTY / herdr (`Ctrl+X p` / `Ctrl+X v`)
    (KeyCode::Char('p'), Action::PasteClipboard),
    (KeyCode::Char('v'), Action::PasteClipboard),
    (KeyCode::Char(']'), Action::PasteClipboard),
    (KeyCode::Char('1'), Action::FocusPane(1)),
    (KeyCode::Char('2'), Action::FocusPane(2)),
    (KeyCode::Char('3'), Action::FocusPane(3)),
    (KeyCode::Char('4'), Action::FocusPane(4)),
    (KeyCode::Char('5'), Action::FocusPane(5)),
    (KeyCode::Char('6'), Action::FocusPane(6)),
    (KeyCode::Char('7'), Action::FocusPane(7)),
    (KeyCode::Char('8'), Action::FocusPane(8)),
    (KeyCode::Char('9'), Action::FocusPane(9)),
    (KeyCode::Char('w'), Action::ClosePane),
    (KeyCode::Char('W'), Action::ClosePane),
    (KeyCode::Char('d'), Action::DrawerToggle),
    (KeyCode::Char('D'), Action::DrawerLayout(None)),
    (KeyCode::F(7), Action::DrawerToggle),
    (KeyCode::F(11), Action::VisualStyleCommand(None)),
    (KeyCode::Char('M'), Action::LayoutCycle),
    (KeyCode::Char('q'), Action::QuakeToggle),
    (KeyCode::Char('`'), Action::QuakeToggle),
    (KeyCode::Char('~'), Action::QuakeToggle),
    (KeyCode::F(10), Action::QuakeToggle),
    (KeyCode::Char(' '), Action::PaletteToggle),
    (KeyCode::Char(':'), Action::PaletteToggle),
    (KeyCode::Char('?'), Action::HelpToggle),
    (KeyCode::F(1), Action::HelpToggle),
    (KeyCode::Char('i'), Action::InspectorToggle(None)),
    (KeyCode::Char('I'), Action::InspectorToggle(None)),
    (KeyCode::F(9), Action::InspectorToggle(None)),
    (KeyCode::Char('x'), Action::CrosshairCycle),
    (KeyCode::Char('X'), Action::CrosshairCycle),
    (KeyCode::Char('L'), Action::spotlight_cycle()),
    (
        KeyCode::Char('}'),
        Action::Spotlight {
            mode: None,
            pixelated: None,
            fullscreen: None,
            rx: None,
            ry: None,
            delta: Some((4, 1)),
            feather_pct: None,
        },
    ),
    (
        KeyCode::Char('{'),
        Action::Spotlight {
            mode: None,
            pixelated: None,
            fullscreen: None,
            rx: None,
            ry: None,
            delta: Some((-4, -1)),
            feather_pct: None,
        },
    ),
    // Intra-pane prompt, modes & workspace recorder
    (KeyCode::Char('t'), Action::PromptTop),
    (KeyCode::Char('T'), Action::PromptTop),
    (KeyCode::Char('b'), Action::PromptBottom),
    (KeyCode::Char('B'), Action::PromptBottom),
    (KeyCode::Char('s'), Action::PromptSplitToggle),
    (KeyCode::Char('e'), Action::PromptModeSet(None)),
    (KeyCode::Char('E'), Action::PromptModeSet(None)),
    (KeyCode::Char('m'), Action::PromptModeSet(None)),
    (KeyCode::F(4), Action::PromptModeSet(None)),
    (KeyCode::Char('g'), Action::RawToggle),
    (KeyCode::Char('G'), Action::RawToggle),
    (KeyCode::F(12), Action::RawToggle),
    (KeyCode::Char('c'), Action::CopyModeToggle),
    (KeyCode::Char('C'), Action::CopyModeToggle),
    (KeyCode::Char('['), Action::CopyModeToggle),
    (KeyCode::F(8), Action::CopyModeToggle),
    (KeyCode::Char('r'), Action::SelectCommand(None)),
    (KeyCode::Char('f'), Action::HintsToggle),
    (KeyCode::Char('F'), Action::HintsToggle),
    (KeyCode::F(3), Action::HintsToggle),
    (KeyCode::Char('l'), Action::LockToggle(None)),
    (KeyCode::Char('/'), Action::SearchPromptPrefill),
    (KeyCode::Char('N'), Action::SearchPrev),
    (KeyCode::Char('P'), Action::HistoryPopupToggle),
    (KeyCode::Char('H'), Action::HistoryPopupToggle),
    (KeyCode::Char('R'), Action::RecordWorkspaceStart(None)),
    (KeyCode::Left, Action::PaneFocusDirection { dx: -1, dy: 0 }),
    (KeyCode::Right, Action::PaneFocusDirection { dx: 1, dy: 0 }),
    (KeyCode::Up, Action::PaneFocusDirection { dx: 0, dy: -1 }),
    (KeyCode::Down, Action::PaneFocusDirection { dx: 0, dy: 1 }),
    (KeyCode::Char('j'), Action::PaneFocusDirection { dx: 0, dy: 1 }),
    (KeyCode::Char('k'), Action::PaneFocusDirection { dx: 0, dy: -1 }),
    (KeyCode::Char('<'), Action::PaneResize { dw: -4, dh: 0 }),
    (KeyCode::Char('>'), Action::PaneResize { dw: 4, dh: 0 }),
    (KeyCode::Char('J'), Action::PaneMove { dx: 0, dy: 2 }),
    (KeyCode::Char('K'), Action::PaneMove { dx: 0, dy: -2 }),
    (KeyCode::Char('+'), Action::PromptResizeHeight(1)),
    (KeyCode::Char('='), Action::WindowResetGeometry),
    (KeyCode::Char('#'), Action::ArrangeGrid),
    (KeyCode::Char('0'), Action::FontZoomReset),
    (KeyCode::PageUp, Action::BlockPrev),
    (KeyCode::PageDown, Action::ScrollModeTogglePage),
];

/// Multi-key daemon supervisor sub-chord bindings (`Ctrl+X D <key>`, `DAEM-05..08`).
pub fn resolve_daemon_subchord(key: KeyCode) -> Option<Action> {
    match key {
        KeyCode::Char('l') | KeyCode::Char('L') => {
            Some(Action::DaemonSupervisor(crate::action::DaemonSupervisorOp::List))
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            Some(Action::DaemonSupervisor(crate::action::DaemonSupervisorOp::Status { name: None }))
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            Some(Action::DaemonSupervisor(crate::action::DaemonSupervisorOp::Start { name: None }))
        }
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            Some(Action::DaemonSupervisor(crate::action::DaemonSupervisorOp::Stop {
                name: None,
                force: false,
            }))
        }
        KeyCode::Char('w') | KeyCode::Char('W') => {
            Some(Action::DaemonSupervisor(crate::action::DaemonSupervisorOp::Switch {
                name: "default".to_string(),
            }))
        }
        _ => None,
    }
}

/// Declarative global workspace shortcut table (`F1`..`F10`, `Alt+<key>`, `Ctrl+P`, `Ctrl+\\`, `ARCH-R01.2`).
pub const DEFAULT_GLOBAL_BINDINGS: &[(KeyModifiers, KeyCode, Action)] = &[
    // Direct OS clipboard paste (`Ctrl+Shift+V` / `Shift+Insert`) across CommandPrompt and Raw PTY / herdr
    (KeyModifiers::CONTROL.union(KeyModifiers::SHIFT), KeyCode::Char('v'), Action::PasteClipboard),
    (KeyModifiers::CONTROL.union(KeyModifiers::SHIFT), KeyCode::Char('V'), Action::PasteClipboard),
    (KeyModifiers::SHIFT, KeyCode::Insert, Action::PasteClipboard),
    // Global Fuzzy Command Palette & Guake/Quake quick terminal
    (KeyModifiers::ALT, KeyCode::Char(' '), Action::PaletteToggle),
    (KeyModifiers::CONTROL, KeyCode::Char('p'), Action::PaletteToggle),
    (KeyModifiers::ALT, KeyCode::Char('`'), Action::QuakeToggle),
    (KeyModifiers::CONTROL, KeyCode::Char(' '), Action::QuakeToggle),
    (KeyModifiers::CONTROL, KeyCode::Char('\\'), Action::QuakeToggle),
    (KeyModifiers::CONTROL, KeyCode::Char('\x1c'), Action::QuakeToggle),
    // Borland Turbo Vision function keys
    (KeyModifiers::NONE, KeyCode::F(1), Action::HelpToggle),
    (KeyModifiers::NONE, KeyCode::F(2), Action::SplitHorizontal),
    (KeyModifiers::NONE, KeyCode::F(5), Action::ZoomToggle),
    (KeyModifiers::NONE, KeyCode::F(6), Action::DeckNext),
    (KeyModifiers::NONE, KeyCode::F(9), Action::InspectorToggle(None)),
    (KeyModifiers::NONE, KeyCode::F(10), Action::QuakeToggle),
    // Multi-pane Window Manager Alt-chords
    (KeyModifiers::ALT, KeyCode::Char('o'), Action::spotlight_cycle()),
    (KeyModifiers::ALT, KeyCode::Char('O'), Action::focal_zoom_cycle()),
    (KeyModifiers::ALT, KeyCode::Char('}'), Action::FocalRadiusResize(4, 1)),
    (KeyModifiers::ALT, KeyCode::Char('{'), Action::FocalRadiusResize(-4, -1)),
    (KeyModifiers::ALT, KeyCode::Char('x'), Action::CrosshairCycle),
    (KeyModifiers::ALT, KeyCode::Char('X'), Action::CrosshairCycle),
    (KeyModifiers::ALT, KeyCode::Char('h'), Action::SplitHorizontal),
    (KeyModifiers::ALT, KeyCode::Char('H'), Action::SplitHorizontal),
    (KeyModifiers::ALT, KeyCode::Char('v'), Action::SplitVertical),
    (KeyModifiers::ALT, KeyCode::Char('V'), Action::SplitVertical),
    (KeyModifiers::ALT, KeyCode::Char('w'), Action::ClosePane),
    (KeyModifiers::ALT, KeyCode::Char('W'), Action::ClosePane),
    (KeyModifiers::ALT, KeyCode::Char('z'), Action::ZoomToggle),
    (KeyModifiers::ALT, KeyCode::Char('Z'), Action::ZoomToggle),
    (KeyModifiers::ALT, KeyCode::Char('m'), Action::LayoutCycle),
    (KeyModifiers::ALT, KeyCode::Char('M'), Action::LayoutCycle),
    (KeyModifiers::ALT, KeyCode::Char('D'), Action::DrawerToggle),
    // Agent traffic window toggle (every F1..F12 is taken, so a Ctrl+Alt chord; also menu View, drawer footer `@`)
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('a'), Action::Agents(AgentsOp::Toggle(None))),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('A'), Action::Agents(AgentsOp::Toggle(None))),
    // AGENTS list toggle: Ctrl+Alt+N (verified free; D-03).
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('n'), Action::AgentsList(AgentsListOp::Toggle)),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('N'), Action::AgentsList(AgentsListOp::Toggle)),
    // SESSION MAP toggle: Ctrl+Alt+M (Phase 54, VIEW-02)
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('m'), Action::SessionMap(SessionMapOp::Toggle)),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('M'), Action::SessionMap(SessionMapOp::Toggle)),
    // Find window (`:find agent`): Ctrl+Alt+F. Leader `?` is HelpToggle, so Find has no leader chord (P-06).
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('f'), Action::Find(FindOp::Open { kind: FindKind::Agent, query: String::new() })),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('F'), Action::Find(FindOp::Open { kind: FindKind::Agent, query: String::new() })),
    // Reconnect the tmux link (`:ssh-tmux reconnect`): Ctrl+Alt+R, also reserved in raw passthrough.
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('r'), Action::RemoteLink(RemoteLinkOp::Reconnect)),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('R'), Action::RemoteLink(RemoteLinkOp::Reconnect)),
    (KeyModifiers::NONE, KeyCode::F(7), Action::DrawerToggle),
    (KeyModifiers::NONE, KeyCode::F(11), Action::VisualStyleCommand(None)),
    (KeyModifiers::ALT, KeyCode::F(12), Action::FkeyModeCommand(None)),
    (KeyModifiers::ALT, KeyCode::Tab, Action::DeckNext),
    (KeyModifiers::ALT, KeyCode::BackTab, Action::DeckPrev),
    (KeyModifiers::ALT, KeyCode::Char('1'), Action::FocusPane(1)),
    (KeyModifiers::ALT, KeyCode::Char('2'), Action::FocusPane(2)),
    (KeyModifiers::ALT, KeyCode::Char('3'), Action::FocusPane(3)),
    (KeyModifiers::ALT, KeyCode::Char('4'), Action::FocusPane(4)),
    (KeyModifiers::ALT, KeyCode::Char('5'), Action::FocusPane(5)),
    (KeyModifiers::ALT, KeyCode::Char('6'), Action::FocusPane(6)),
    (KeyModifiers::ALT, KeyCode::Char('7'), Action::FocusPane(7)),
    (KeyModifiers::ALT, KeyCode::Char('8'), Action::FocusPane(8)),
    (KeyModifiers::ALT, KeyCode::Char('9'), Action::FocusPane(9)),
    (KeyModifiers::ALT, KeyCode::Char('j'), Action::PaneFocusDirection { dx: 0, dy: 1 }),
    (KeyModifiers::ALT, KeyCode::Char('k'), Action::PaneFocusDirection { dx: 0, dy: -1 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('H'), Action::PaneFocusDirection { dx: -1, dy: 0 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('L'), Action::PaneFocusDirection { dx: 1, dy: 0 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('K'), Action::PaneFocusDirection { dx: 0, dy: -1 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('J'), Action::PaneFocusDirection { dx: 0, dy: 1 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('A'), Action::PaneFocusDirection { dx: -1, dy: 0 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('D'), Action::PaneFocusDirection { dx: 1, dy: 0 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('W'), Action::PaneFocusDirection { dx: 0, dy: -1 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('S'), Action::PaneFocusDirection { dx: 0, dy: 1 }),
    // Dynamic terminal font & text scaling (FONT-01)
    (KeyModifiers::CONTROL, KeyCode::Char('+'), Action::FontZoomIn),
    (KeyModifiers::CONTROL, KeyCode::Char('='), Action::FontZoomIn),
    (KeyModifiers::CONTROL, KeyCode::Char('-'), Action::FontZoomOut),
    (KeyModifiers::CONTROL, KeyCode::Char('_'), Action::FontZoomOut),
    (KeyModifiers::CONTROL, KeyCode::Char('\x1f'), Action::FontZoomOut),
    (KeyModifiers::CONTROL, KeyCode::Char('0'), Action::FontZoomReset),
    (KeyModifiers::CONTROL, KeyCode::Char(')'), Action::FontZoomReset),
    // Window / Pane resizing (Z-Stack floating geometry & Tiled split ratios)
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Left, Action::PaneResize { dw: -4, dh: 0 }),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Right, Action::PaneResize { dw: 4, dh: 0 }),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Up, Action::PaneResize { dw: 0, dh: -2 }),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Down, Action::PaneResize { dw: 0, dh: 2 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Left, Action::PaneResize { dw: -4, dh: 0 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Right, Action::PaneResize { dw: 4, dh: 0 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Up, Action::PaneResize { dw: 0, dh: -2 }),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Down, Action::PaneResize { dw: 0, dh: 2 }),
    // Window / Pane moving (Z-Stack floating position & Tiled pane swap)
    (KeyModifiers::CONTROL.union(KeyModifiers::SHIFT), KeyCode::Left, Action::PaneMove { dx: -4, dy: 0 }),
    (KeyModifiers::CONTROL.union(KeyModifiers::SHIFT), KeyCode::Right, Action::PaneMove { dx: 4, dy: 0 }),
    (KeyModifiers::CONTROL.union(KeyModifiers::SHIFT), KeyCode::Up, Action::PaneMove { dx: 0, dy: -2 }),
    (KeyModifiers::CONTROL.union(KeyModifiers::SHIFT), KeyCode::Down, Action::PaneMove { dx: 0, dy: 2 }),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('h'), Action::PaneMove { dx: -4, dy: 0 }),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('l'), Action::PaneMove { dx: 4, dy: 0 }),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), KeyCode::Char('k'), Action::PaneMove { dx: 0, dy: -2 }),
    (
        KeyModifiers::CONTROL.union(KeyModifiers::ALT),
        KeyCode::Char('j'),
        Action::PaneMove { dx: 0, dy: 2 },
    ),
];

/// Declarative intra-pane shortcut table (`F3` and `Alt+<key>` in `CommandPrompt` mode, `ARCH-R01.2`).
pub const DEFAULT_PANE_BINDINGS: &[(KeyModifiers, KeyCode, Action)] = &[
    (KeyModifiers::NONE, KeyCode::F(3), Action::HintsToggle),
    (KeyModifiers::NONE, KeyCode::F(8), Action::CopyModeToggle),
    (KeyModifiers::ALT, KeyCode::Char('t'), Action::PromptTop),
    (KeyModifiers::ALT, KeyCode::Char('T'), Action::PromptTop),
    (KeyModifiers::ALT, KeyCode::Char('B'), Action::PromptBottom),
    (KeyModifiers::ALT, KeyCode::Char('s'), Action::PromptSplitToggle),
    (KeyModifiers::ALT, KeyCode::Char('S'), Action::PromptSplitToggle),
    (KeyModifiers::ALT, KeyCode::Char('c'), Action::CopyModeToggle),
    (KeyModifiers::ALT, KeyCode::Char('C'), Action::CopyModeToggle),
    (KeyModifiers::ALT, KeyCode::Char('r'), Action::SelectCommand(None)),
    (KeyModifiers::ALT, KeyCode::Char('R'), Action::SelectCommand(None)),
    (KeyModifiers::ALT, KeyCode::Char('F'), Action::HintsToggle),
    (KeyModifiers::ALT, KeyCode::Char('l'), Action::LockToggle(None)),
    (KeyModifiers::ALT, KeyCode::Char('L'), Action::LockToggle(None)),
    (KeyModifiers::ALT, KeyCode::Char('/'), Action::SearchPromptPrefill),
    (KeyModifiers::ALT, KeyCode::Char('n'), Action::SearchNext),
    (KeyModifiers::ALT, KeyCode::Char('N'), Action::SearchPrev),
    (KeyModifiers::ALT, KeyCode::Up, Action::PromptMoveVertical(-1)),
    (KeyModifiers::ALT, KeyCode::Down, Action::PromptMoveVertical(1)),
    (KeyModifiers::ALT, KeyCode::Char('+'), Action::PromptResizeHeight(1)),
    (KeyModifiers::ALT, KeyCode::Char('='), Action::PromptResizeHeight(1)),
    (KeyModifiers::ALT, KeyCode::Char('-'), Action::PromptResizeHeight(-1)),
    (KeyModifiers::ALT, KeyCode::Char(']'), Action::PromptResizeWidth(4)),
    (KeyModifiers::ALT, KeyCode::Char('['), Action::PromptResizeWidth(-4)),
    (KeyModifiers::ALT, KeyCode::Char('p'), Action::HistoryPopupToggle),
    (KeyModifiers::ALT, KeyCode::Char('P'), Action::HistoryPopupToggle),
    (KeyModifiers::ALT, KeyCode::PageUp, Action::BlockPrev),
    (KeyModifiers::ALT, KeyCode::PageDown, Action::BlockNext),
];

/// Which part of the focused navigator drawer a key row applies to (Phase 50-06).
///
/// Scopes are tried in the order given to [`navigator_drawer_action`] (most specific first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawerKeyScope {
    /// The workspace > session > pane tree.
    Navigator,
    /// The comms glyph of the selected pane row has keyboard focus (most specific).
    NavigatorComms,
}

/// Declarative key table of the focused navigator drawer (`FLT-01`, `ARCH-R01.2`).
///
/// Consulted only while the navigator drawer holds keyboard focus; keys it does not claim fall through
/// to the unchanged classic drawer handler. Every row lowers to [`Action::DrawerNav`] and runs through
/// `WindowManager::apply`; each operation has a `:drawer` colon twin.
pub const NAVIGATOR_DRAWER_BINDINGS: &[(DrawerKeyScope, KeyModifiers, KeyCode, Action)] = &[
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('j'), Action::DrawerNav(DrawerNavOp::Next)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Down, Action::DrawerNav(DrawerNavOp::Next)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('k'), Action::DrawerNav(DrawerNavOp::Prev)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Up, Action::DrawerNav(DrawerNavOp::Prev)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Enter, Action::DrawerNav(DrawerNavOp::Activate)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char(' '), Action::DrawerNav(DrawerNavOp::Activate)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('!'), Action::DrawerNav(DrawerNavOp::NextAttention)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('e'), Action::DrawerNav(DrawerNavOp::Fold)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('E'), Action::DrawerNav(DrawerNavOp::ExpandAll)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('C'), Action::DrawerNav(DrawerNavOp::CollapseAll)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('N'), Action::DrawerNav(DrawerNavOp::NewSession)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('a'), Action::DrawerNav(DrawerNavOp::AddWindow(None))),
    (
        DrawerKeyScope::Navigator,
        KeyModifiers::NONE,
        KeyCode::Char('A'),
        Action::DrawerNav(DrawerNavOp::AddPane(SplitDirection::Horizontal)),
    ),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('n'), Action::DrawerNav(DrawerNavOp::NewPane)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('c'), Action::DrawerNav(DrawerNavOp::NewPane)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('+'), Action::DrawerNav(DrawerNavOp::NewPane)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('w'), Action::DrawerNav(DrawerNavOp::NewPane)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('r'), Action::DrawerNav(DrawerNavOp::RenamePrompt)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('R'), Action::DrawerNav(DrawerNavOp::RenamePrompt)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('x'), Action::DrawerNav(DrawerNavOp::Close)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Delete, Action::DrawerNav(DrawerNavOp::Close)),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::PageUp, Action::DrawerNav(DrawerNavOp::Scroll(-5))),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::PageDown, Action::DrawerNav(DrawerNavOp::Scroll(5))),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('{'), Action::DrawerNav(DrawerNavOp::Widen(-2))),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('}'), Action::DrawerNav(DrawerNavOp::Widen(2))),
    // Comms glyph (Phase 50-12). Right is claimed only on a row that shows one (guard in the handler).
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Right, Action::DrawerNav(DrawerNavOp::CommsFocus(true))),
    (DrawerKeyScope::Navigator, KeyModifiers::NONE, KeyCode::Char('o'), Action::DrawerNav(DrawerNavOp::OpenComms)),
    (DrawerKeyScope::NavigatorComms, KeyModifiers::NONE, KeyCode::Enter, Action::DrawerNav(DrawerNavOp::OpenComms)),
    (DrawerKeyScope::NavigatorComms, KeyModifiers::NONE, KeyCode::Left, Action::DrawerNav(DrawerNavOp::CommsFocus(false))),
    (DrawerKeyScope::NavigatorComms, KeyModifiers::NONE, KeyCode::Esc, Action::DrawerNav(DrawerNavOp::CommsFocus(false))),
];

/// Looks up `key` in [`NAVIGATOR_DRAWER_BINDINGS`] for the active `scopes` (first scope wins).
///
/// Modifiers are compared with SHIFT removed for `KeyCode::Char` (like [`super::default_pane_action`]) and
/// exactly for every other code, so Shift/Alt+Up/Down never match the plain arrow rows.
pub fn navigator_drawer_action(scopes: &[DrawerKeyScope], key: KeyEvent) -> Option<Action> {
    let lookup_mods = match key.code {
        KeyCode::Char(_) => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    scopes.iter().find_map(|scope| {
        NAVIGATOR_DRAWER_BINDINGS
            .iter()
            .find(|(s, mods, code, _)| s == scope && *mods == lookup_mods && *code == key.code)
            .map(|(_, _, _, action)| action.clone())
    })
}

/// Which part of the AGENT TRAFFIC window a key row applies to (Phase 50-08).
///
/// Scopes are tried in the order given to [`agents_window_action`] (most specific first); `Window`
/// rows apply in every window pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentsKeyScope {
    /// Applies whatever part of the window is active.
    Window,
    /// The node/edge graph cursor.
    Graph,
    /// Node grab mode (most specific): arrows nudge the selected node; `g`, Enter and Esc drop it.
    /// Its Esc row is the one allowed override of the `Window` Esc row.
    Grab,
    /// The conversation (snoop) pane has the keyboard.
    Snoop,
    /// The keyword entry line is open; printable characters edit it (`Push`). Its Esc row is the
    /// second allowed override of the `Window` Esc row.
    KeywordEntry,
    /// The `send as me` compose line has the keyboard.
    Compose,
}

/// Declarative key table of the AGENT TRAFFIC window (`FLT-01`, `ARCH-R01.2`).
///
/// Consulted only while the agents window captures input (open and focused; the window is not modal, so a click on a pane or `:agents blur` releases the keyboard).
/// Every row lowers to [`Action::Agents`] and runs through `WindowManager::apply`; each operation has
/// an `:agents` colon twin. Keys the table does not claim are swallowed while the window has the
/// keyboard; the leader key is never consumed.
pub const AGENTS_WINDOW_BINDINGS: &[(AgentsKeyScope, KeyModifiers, KeyCode, Action)] = &[
    (AgentsKeyScope::Window, KeyModifiers::NONE, KeyCode::Esc, Action::Agents(AgentsOp::Dismiss)),
    (AgentsKeyScope::Graph, KeyModifiers::NONE, KeyCode::Left, Action::Agents(AgentsOp::Select(GraphDir::Left))),
    (AgentsKeyScope::Graph, KeyModifiers::NONE, KeyCode::Right, Action::Agents(AgentsOp::Select(GraphDir::Right))),
    (AgentsKeyScope::Graph, KeyModifiers::NONE, KeyCode::Up, Action::Agents(AgentsOp::Select(GraphDir::Up))),
    (AgentsKeyScope::Graph, KeyModifiers::NONE, KeyCode::Down, Action::Agents(AgentsOp::Select(GraphDir::Down))),
    (AgentsKeyScope::Graph, KeyModifiers::NONE, KeyCode::Tab, Action::Agents(AgentsOp::CycleEdge(true))),
    (AgentsKeyScope::Graph, KeyModifiers::NONE, KeyCode::BackTab, Action::Agents(AgentsOp::CycleEdge(false))),
    (AgentsKeyScope::Graph, KeyModifiers::NONE, KeyCode::Enter, Action::Agents(AgentsOp::Activate)),
    (AgentsKeyScope::Graph, KeyModifiers::NONE, KeyCode::Char('g'), Action::Agents(AgentsOp::Grab(None))),
    (AgentsKeyScope::Graph, KeyModifiers::NONE, KeyCode::Char('u'), Action::Agents(AgentsOp::UnpinNode(None))),
    (AgentsKeyScope::Window, KeyModifiers::SHIFT, KeyCode::Left, Action::Agents(AgentsOp::Move { dx: -1, dy: 0 })),
    (AgentsKeyScope::Window, KeyModifiers::SHIFT, KeyCode::Right, Action::Agents(AgentsOp::Move { dx: 1, dy: 0 })),
    (AgentsKeyScope::Window, KeyModifiers::SHIFT, KeyCode::Up, Action::Agents(AgentsOp::Move { dx: 0, dy: -1 })),
    (AgentsKeyScope::Window, KeyModifiers::SHIFT, KeyCode::Down, Action::Agents(AgentsOp::Move { dx: 0, dy: 1 })),
    (AgentsKeyScope::Window, KeyModifiers::ALT, KeyCode::Left, Action::Agents(AgentsOp::Resize { dw: -1, dh: 0 })),
    (AgentsKeyScope::Window, KeyModifiers::ALT, KeyCode::Right, Action::Agents(AgentsOp::Resize { dw: 1, dh: 0 })),
    (AgentsKeyScope::Window, KeyModifiers::ALT, KeyCode::Up, Action::Agents(AgentsOp::Resize { dw: 0, dh: -1 })),
    (AgentsKeyScope::Window, KeyModifiers::ALT, KeyCode::Down, Action::Agents(AgentsOp::Resize { dw: 0, dh: 1 })),
    (AgentsKeyScope::Window, KeyModifiers::NONE, KeyCode::Char('*'), Action::Agents(AgentsOp::Pin(None))),
    (AgentsKeyScope::Window, KeyModifiers::NONE, KeyCode::Char('['), Action::Agents(AgentsOp::ScaleStep(-10))),
    (AgentsKeyScope::Window, KeyModifiers::NONE, KeyCode::Char(']'), Action::Agents(AgentsOp::ScaleStep(10))),
    (AgentsKeyScope::Window, KeyModifiers::NONE, KeyCode::Char('m'), Action::Agents(AgentsOp::Maximize)),
    (AgentsKeyScope::Window, KeyModifiers::NONE, KeyCode::Char('<'), Action::Agents(AgentsOp::SplitStep(-5))),
    (AgentsKeyScope::Window, KeyModifiers::NONE, KeyCode::Char('>'), Action::Agents(AgentsOp::SplitStep(5))),
    // Ctrl+O cycles graph -> conversation -> compose; F6 stays the global `DeckNext`.
    (AgentsKeyScope::Window, KeyModifiers::CONTROL, KeyCode::Char('o'), Action::Agents(AgentsOp::FocusPane(None))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('j'), Action::Agents(AgentsOp::SnoopStep(1))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Down, Action::Agents(AgentsOp::SnoopStep(1))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('k'), Action::Agents(AgentsOp::SnoopStep(-1))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Up, Action::Agents(AgentsOp::SnoopStep(-1))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('y'), Action::Agents(AgentsOp::Copy)),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::PageUp, Action::Agents(AgentsOp::SnoopScroll(-10))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::PageDown, Action::Agents(AgentsOp::SnoopScroll(10))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Home, Action::Agents(AgentsOp::SnoopScroll(i16::MIN))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::End, Action::Agents(AgentsOp::Follow(Some(true)))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('f'), Action::Agents(AgentsOp::Follow(None))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('/'), Action::Agents(AgentsOp::KeywordEdit(KeywordEdit::Begin))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('a'), Action::Agents(AgentsOp::Filter(SnoopFilterOp::AgentFromSelection))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('p'), Action::Agents(AgentsOp::Filter(SnoopFilterOp::PairFromSelection))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('c'), Action::Agents(AgentsOp::Filter(SnoopFilterOp::Clear))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('A'), Action::Agents(AgentsOp::Filter(SnoopFilterOp::Remove(SnoopFilterKind::Agent)))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('P'), Action::Agents(AgentsOp::Filter(SnoopFilterOp::Remove(SnoopFilterKind::Pair)))),
    (AgentsKeyScope::Snoop, KeyModifiers::NONE, KeyCode::Char('T'), Action::Agents(AgentsOp::Filter(SnoopFilterOp::Remove(SnoopFilterKind::Text)))),
    (AgentsKeyScope::Compose, KeyModifiers::NONE, KeyCode::Enter, Action::Agents(AgentsOp::ComposeSubmit)),
    (AgentsKeyScope::KeywordEntry, KeyModifiers::NONE, KeyCode::Enter, Action::Agents(AgentsOp::KeywordEdit(KeywordEdit::Commit))),
    (AgentsKeyScope::KeywordEntry, KeyModifiers::NONE, KeyCode::Esc, Action::Agents(AgentsOp::KeywordEdit(KeywordEdit::Cancel))),
    (AgentsKeyScope::KeywordEntry, KeyModifiers::NONE, KeyCode::Backspace, Action::Agents(AgentsOp::KeywordEdit(KeywordEdit::Pop))),
    (AgentsKeyScope::Grab, KeyModifiers::NONE, KeyCode::Left, Action::Agents(AgentsOp::NudgeNode(GraphDir::Left))),
    (AgentsKeyScope::Grab, KeyModifiers::NONE, KeyCode::Right, Action::Agents(AgentsOp::NudgeNode(GraphDir::Right))),
    (AgentsKeyScope::Grab, KeyModifiers::NONE, KeyCode::Up, Action::Agents(AgentsOp::NudgeNode(GraphDir::Up))),
    (AgentsKeyScope::Grab, KeyModifiers::NONE, KeyCode::Down, Action::Agents(AgentsOp::NudgeNode(GraphDir::Down))),
    (AgentsKeyScope::Grab, KeyModifiers::NONE, KeyCode::Char('g'), Action::Agents(AgentsOp::Grab(Some(false)))),
    (AgentsKeyScope::Grab, KeyModifiers::NONE, KeyCode::Enter, Action::Agents(AgentsOp::Grab(Some(false)))),
    (AgentsKeyScope::Grab, KeyModifiers::NONE, KeyCode::Esc, Action::Agents(AgentsOp::Grab(Some(false)))),
];

/// Looks up `key` in [`AGENTS_WINDOW_BINDINGS`] for the active `scopes` (first scope wins).
///
/// SHIFT is removed from the modifiers for `KeyCode::Char` and `KeyCode::BackTab` (crossterm reports
/// Shift+Tab as `BackTab` with SHIFT); every other code is compared with exact modifiers.
pub fn agents_window_action(scopes: &[AgentsKeyScope], key: KeyEvent) -> Option<Action> {
    let lookup_mods = match key.code {
        KeyCode::Char(_) | KeyCode::BackTab => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    scopes.iter().find_map(|scope| {
        AGENTS_WINDOW_BINDINGS
            .iter()
            .find(|(s, mods, code, _)| s == scope && *mods == lookup_mods && *code == key.code)
            .map(|(_, _, _, action)| action.clone())
    })
}

/// Declarative key table of the Find window, consulted while `Modal::Find` is open (Phase 50.1).
///
/// Printable characters without CONTROL/ALT are not listed: [`find_modal_key_action`] maps them to
/// `FindOp::Push`. Every row lowers to [`Action::Find`] and runs through `WindowManager::apply`
/// (`ARCH-R01`); each operation has a `:finder` colon twin. The global entry chord (`Ctrl+Alt+F`)
/// lives in [`DEFAULT_GLOBAL_BINDINGS`] and closes the window while it is open.
pub const FIND_MODAL_BINDINGS: &[(KeyModifiers, KeyCode, Action)] = &[
    (KeyModifiers::NONE, KeyCode::Esc, Action::Find(FindOp::Close)),
    (KeyModifiers::NONE, KeyCode::Enter, Action::Find(FindOp::Activate)),
    (KeyModifiers::NONE, KeyCode::Up, Action::Find(FindOp::Move(-1))),
    (KeyModifiers::NONE, KeyCode::Down, Action::Find(FindOp::Move(1))),
    (KeyModifiers::NONE, KeyCode::PageUp, Action::Find(FindOp::Move(-10))),
    (KeyModifiers::NONE, KeyCode::PageDown, Action::Find(FindOp::Move(10))),
    (KeyModifiers::NONE, KeyCode::Home, Action::Find(FindOp::Move(i16::MIN))),
    (KeyModifiers::NONE, KeyCode::End, Action::Find(FindOp::Move(i16::MAX))),
    (KeyModifiers::NONE, KeyCode::Tab, Action::Find(FindOp::CycleKind(true))),
    (KeyModifiers::NONE, KeyCode::BackTab, Action::Find(FindOp::CycleKind(false))),
    (KeyModifiers::NONE, KeyCode::Backspace, Action::Find(FindOp::Pop)),
    (KeyModifiers::CONTROL, KeyCode::Char('u'), Action::Find(FindOp::ClearQuery)),
    (KeyModifiers::CONTROL, KeyCode::Char('y'), Action::Find(FindOp::Prefill)),
];

/// Looks up `key` in [`FIND_MODAL_BINDINGS`]; a printable character without CONTROL/ALT becomes
/// `FindOp::Push`. SHIFT is dropped from the lookup for `Char` and `BackTab` (as for the agents window).
pub fn find_modal_key_action(key: KeyEvent) -> Option<Action> {
    let lookup_mods = match key.code {
        KeyCode::Char(_) | KeyCode::BackTab => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    if let Some((_, _, action)) = FIND_MODAL_BINDINGS
        .iter()
        .find(|(mods, code, _)| *mods == lookup_mods && *code == key.code)
    {
        return Some(action.clone());
    }
    match key.code {
        KeyCode::Char(c)
            if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) && !c.is_control() =>
        {
            Some(Action::Find(FindOp::Push(c)))
        }
        _ => None,
    }
}
