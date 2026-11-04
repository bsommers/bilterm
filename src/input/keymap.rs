use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::colon_command::parse_colon_command;
use crate::error::Result;
use crate::input::AppAction;
use crate::pty::line_discipline::InputRoutingMode;
use crate::wm::WindowManager;
use crate::wm::SplitDirection;
use crate::wm::agents_cmd::AgentsOp;
use crate::wm::agents_window::GraphDir;
use crate::wm::drawer::DrawerNavOp;

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
    (KeyCode::Char('S'), Action::PromptSplitToggle),
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
    (KeyCode::Char('0'), Action::FontZoomReset),
    (KeyCode::PageUp, Action::BlockPrev),
    (KeyCode::PageDown, Action::ScrollModeTogglePage),
];

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
];

/// Looks up `key` in [`NAVIGATOR_DRAWER_BINDINGS`] for the active `scopes` (first scope wins).
///
/// Modifiers are compared with SHIFT removed for `KeyCode::Char` (like [`default_pane_action`]) and
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
}

/// Declarative key table of the AGENT TRAFFIC window (`FLT-01`, `ARCH-R01.2`).
///
/// Consulted only while the agents window captures input (open and unpinned, or pinned and focused).
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

/// Looks up `key` in [`DEFAULT_LEADER_BINDINGS`].
pub fn default_leader_action(key: KeyEvent) -> Option<Action> {
    DEFAULT_LEADER_BINDINGS
        .iter()
        .find(|(code, _)| *code == key.code)
        .map(|(_, action)| action.clone())
}

/// Looks up `key` in [`DEFAULT_GLOBAL_BINDINGS`].
pub fn default_global_action(key: KeyEvent, fuzzy_search_active: bool) -> Option<Action> {
    if fuzzy_search_active
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('p')
    {
        return None;
    }
    let lookup_mods = match key.code {
        KeyCode::Char(_) => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    if let Some((_, _, action)) = DEFAULT_GLOBAL_BINDINGS
        .iter()
        .find(|(mods, code, _)| *code == key.code && *mods == key.modifiers)
    {
        return Some(action.clone());
    }
    DEFAULT_GLOBAL_BINDINGS
        .iter()
        .find(|(mods, code, _)| {
            *code == key.code && *mods == lookup_mods && !mods.contains(KeyModifiers::SHIFT)
        })
        .map(|(_, _, action)| action.clone())
}

/// Looks up `key` in [`DEFAULT_PANE_BINDINGS`].
pub fn default_pane_action(key: KeyEvent) -> Option<Action> {
    let lookup_mods = match key.code {
        KeyCode::Char(_) => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    DEFAULT_PANE_BINDINGS
        .iter()
        .find(|(mods, code, _)| *mods == lookup_mods && *code == key.code)
        .map(|(_, _, action)| action.clone())
}

/// Normalizes macOS `Option+<key>` and `Option+Shift+<key>` Unicode dead-key / symbol characters
/// (`˙`, `√`, `∫`, `ƒ`, `†`, `ß`, `ç`, `¬`, `π`, `∑`, `Ω`, `µ`, `≈`, `\u{00a0}`, `¡`..`ª`, etc.)
/// into canonical `KeyModifiers::ALT` (and `KeyModifiers::SHIFT`) `KeyEvent`s so hotkeys and Readline
/// word bindings work out-of-the-box on macOS even when the host terminal has not enabled "Use Option as Meta".
pub fn normalize_macos_option_key(key: KeyEvent, enabled: bool) -> KeyEvent {
    if !enabled
        || key.modifiers.contains(KeyModifiers::CONTROL)
        || key.modifiers.contains(KeyModifiers::ALT)
    {
        return key;
    }
    let KeyCode::Char(c) = key.code else {
        return key;
    };
    let mapped: Option<(char, KeyModifiers)> = match c {
        'å' => Some(('a', KeyModifiers::ALT)),
        '∫' => Some(('b', KeyModifiers::ALT)),
        'ç' => Some(('c', KeyModifiers::ALT)),
        '∂' => Some(('d', KeyModifiers::ALT)),
        '´' => Some(('e', KeyModifiers::ALT)),
        'ƒ' => Some(('f', KeyModifiers::ALT)),
        '©' => Some(('g', KeyModifiers::ALT)),
        '˙' => Some(('h', KeyModifiers::ALT)),
        'ˆ' => Some(('i', KeyModifiers::ALT)),
        '∆' => Some(('j', KeyModifiers::ALT)),
        '˚' => Some(('k', KeyModifiers::ALT)),
        '¬' => Some(('l', KeyModifiers::ALT)),
        'µ' => Some(('m', KeyModifiers::ALT)),
        '˜' => Some(('n', KeyModifiers::ALT)),
        'ø' => Some(('o', KeyModifiers::ALT)),
        'π' => Some(('p', KeyModifiers::ALT)),
        'œ' => Some(('q', KeyModifiers::ALT)),
        '®' => Some(('r', KeyModifiers::ALT)),
        'ß' => Some(('s', KeyModifiers::ALT)),
        '†' => Some(('t', KeyModifiers::ALT)),
        '¨' => Some(('u', KeyModifiers::ALT)),
        '√' => Some(('v', KeyModifiers::ALT)),
        '∑' => Some(('w', KeyModifiers::ALT)),
        '≈' => Some(('x', KeyModifiers::ALT)),
        '¥' => Some(('y', KeyModifiers::ALT)),
        'Ω' => Some(('z', KeyModifiers::ALT)),
        'Å' => Some(('A', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'ı' => Some(('B', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Ç' => Some(('C', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Î' => Some(('D', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Ï' => Some(('F', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '˝' => Some(('G', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Ó' => Some(('H', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Ô' => Some(('J', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '\u{f8ff}' => Some(('K', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Ò' => Some(('L', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Â' => Some(('M', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '¯' => Some(('N', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Ø' => Some(('O', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '∏' => Some(('P', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Œ' => Some(('Q', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '‰' => Some(('R', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Í' => Some(('S', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'ˇ' => Some(('T', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '◊' => Some(('V', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '„' => Some(('W', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '˛' => Some(('X', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        'Á' => Some(('Y', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '¸' => Some(('Z', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '¡' => Some(('1', KeyModifiers::ALT)),
        '™' => Some(('2', KeyModifiers::ALT)),
        '£' => Some(('3', KeyModifiers::ALT)),
        '¢' => Some(('4', KeyModifiers::ALT)),
        '∞' => Some(('5', KeyModifiers::ALT)),
        '§' => Some(('6', KeyModifiers::ALT)),
        '¶' => Some(('7', KeyModifiers::ALT)),
        '•' => Some(('8', KeyModifiers::ALT)),
        'ª' => Some(('9', KeyModifiers::ALT)),
        'º' => Some(('0', KeyModifiers::ALT)),
        '\u{00a0}' => Some((' ', KeyModifiers::ALT)),
        '÷' => Some(('/', KeyModifiers::ALT)),
        '≠' => Some(('=', KeyModifiers::ALT)),
        '±' => Some(('+', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '–' => Some(('-', KeyModifiers::ALT)),
        '—' => Some(('-', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '“' => Some(('[', KeyModifiers::ALT)),
        '”' => Some(('[', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '‘' => Some((']', KeyModifiers::ALT)),
        '’' => Some((']', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        '¿' => Some(('?', KeyModifiers::ALT | KeyModifiers::SHIFT)),
        _ => None,
    };
    if let Some((ascii_ch, extra_mods)) = mapped {
        KeyEvent::new(KeyCode::Char(ascii_ch), key.modifiers | extra_mods)
    } else {
        key
    }
}

/// Checks whether `key` matches the configured Prefix/Leader key chord (e.g. `"Ctrl+X"`, `"Ctrl+B"`).
pub fn is_leader_key(key: KeyEvent, leader_cfg: &str) -> bool {
    is_leader_key_with_tmux(key, leader_cfg, crate::tmux::TmuxEnv::detect().is_some())
}

/// Checks whether `key` matches the configured Prefix/Leader key chord with explicit outer-`tmux` awareness (`ARCH-R01.2`, `REC-02`).
///
/// When `leader_cfg` is `"Ctrl+X"` or `"Ctrl+B"`:
/// - Standalone (`!inside_tmux`): accepts both `Ctrl+B` (`tmux`/`herdr`) and `Ctrl+X` (Borland/DOS).
/// - Inside an outer `tmux` session (`inside_tmux == true`): falls back to `Ctrl+X` so `Ctrl+B` passes cleanly to outer `tmux`.
pub fn is_leader_key_with_tmux(key: KeyEvent, leader_cfg: &str, inside_tmux: bool) -> bool {
    let target = leader_cfg.trim();
    if target.is_empty() {
        return false;
    }
    let Some(chord) = format_key_chord(key) else {
        return false;
    };
    if target.eq_ignore_ascii_case("Ctrl+X") || target.eq_ignore_ascii_case("Ctrl+B") {
        if inside_tmux {
            chord.eq_ignore_ascii_case("Ctrl+X")
        } else {
            chord.eq_ignore_ascii_case("Ctrl+X") || chord.eq_ignore_ascii_case("Ctrl+B")
        }
    } else {
        chord.eq_ignore_ascii_case(target)
    }
}

pub(crate) fn format_key_chord(key: KeyEvent) -> Option<String> {
    let mut parts = Vec::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl".to_string());
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt".to_string());
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("Shift".to_string());
    }
    match key.code {
        KeyCode::Char(' ') => parts.push("Space".to_string()),
        KeyCode::Char(c) => parts.push(c.to_ascii_uppercase().to_string()),
        KeyCode::F(n) => parts.push(format!("F{n}")),
        KeyCode::Tab => parts.push("Tab".to_string()),
        KeyCode::BackTab => parts.push("BackTab".to_string()),
        KeyCode::Enter => parts.push("Enter".to_string()),
        KeyCode::Esc => parts.push("Esc".to_string()),
        _ => return None,
    }
    Some(parts.join("+"))
}

pub(super) fn handle_wm_leader_key(wm: &mut WindowManager, key: KeyEvent) -> Result<AppAction> {
    let drawer_was_focused = wm.drawer().focused;
    let lookup_key = if key.modifiers.contains(KeyModifiers::CONTROL)
        && let KeyCode::Char(c) = key.code
    {
        if c.eq_ignore_ascii_case(&'q') {
            return wm.apply(Action::Quit);
        }
        match c {
            '+' | '=' => return wm.apply(Action::FontZoomIn),
            '-' | '_' => return wm.apply(Action::FontZoomOut),
            '0' => return wm.apply(Action::FontZoomReset),
            _ => {}
        }
        if drawer_was_focused && matches!(c.to_ascii_lowercase(), 'x' | 'b') {
            wm.drawer_mut().focused = false;
            return Ok(AppAction::Handled);
        }
        if wm.active_pane().effective_routing_mode() == InputRoutingMode::RawPtyPassthrough
            && matches!(c.to_ascii_lowercase(), 'x' | 'b' | 'a')
        {
            let byte = (c.to_ascii_lowercase() as u8)
                .wrapping_sub(b'a')
                .wrapping_add(1);
            wm.active_pane_mut().write_to_pty(&[byte])?;
            return Ok(AppAction::Handled);
        }
        if c.eq_ignore_ascii_case(&'a') {
            wm.drawer_mut().focused = false;
            wm.active_pane_mut().prompt_mut().move_cursor_home();
            return Ok(AppAction::Handled);
        }
        KeyEvent::new(key.code, key.modifiers - KeyModifiers::CONTROL)
    } else {
        key
    };

    if lookup_key.code == KeyCode::Esc {
        return Ok(AppAction::Handled);
    }

    if let Some(action) = default_leader_action(lookup_key) {
        if drawer_was_focused && !matches!(action, Action::DrawerToggle | Action::DrawerFocus) {
            wm.drawer_mut().focused = false;
        }
        return wm.apply(action);
    }

    Ok(AppAction::Handled)
}

fn bilterm_action_for_fkey(wm: &WindowManager, n: u8) -> Option<Action> {
    let block_holding = wm.active_pane().scroll_mode() == crate::pane::PaneScrollMode::BlockPage
        && (wm.active_pane().is_page_holding() || wm.active_pane().is_holding_inspected());
    match n {
        1 => Some(Action::HelpToggle),
        2 => Some(Action::SplitHorizontal),
        3 => Some(Action::HintsToggle),
        4 => Some(Action::PromptModeSet(None)),
        5 => Some(Action::ZoomToggle),
        6 => Some(Action::DeckNext),
        7 => Some(if block_holding {
            Action::ScrollPageBack
        } else {
            Action::DrawerToggle
        }),
        8 => Some(if block_holding {
            Action::ScrollPageAdvance
        } else {
            Action::CopyModeToggle
        }),
        9 => Some(Action::InspectorToggle(None)),
        10 => Some(Action::QuakeToggle),
        11 => Some(Action::VisualStyleCommand(None)),
        12 => Some(Action::RawToggle),
        _ => None,
    }
}

/// Handles user-configured `[keybindings]` and Workspace Leader mode entry.
pub(super) fn handle_wm_user_binding_or_leader_entry(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
    if !wm.config().keybindings.is_empty()
        && let Some(chord) = format_key_chord(key)
    {
        let bound_opt = wm
            .config()
            .keybindings
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&chord))
            .map(|(_, v)| v.clone());
        if let Some(bound_action) = bound_opt {
            if let Some(Ok(action)) = parse_colon_command(&bound_action) {
                return Ok(Some(wm.apply(action)?));
            }
            wm.active_pane_mut().prompt_mut().set_text(&bound_action);
            return Ok(Some(AppAction::Handled));
        }
    }

    let inside_tmux = crate::tmux::TmuxEnv::detect().is_some();
    if is_leader_key_with_tmux(key, &wm.config().leader_key, inside_tmux) {
        wm.set_leader_pending(true);
        let leader_label = if inside_tmux {
            "Ctrl+X".to_string()
        } else {
            format!("{} / Ctrl+B", wm.config().leader_key)
        };
        wm.active_pane_mut().set_status_message(Some(format!(
            "⌨ LEADER [{leader_label}]: \"/-=SplitH %/|=SplitV z=Zoom n/p/o=Deck 1-9=Pane d=Drawer q=Quake e/t/b/s=Prompt [=Copy f=Hints ?=Help"
        )));
        return Ok(Some(AppAction::Handled));
    }

    // Function key routing across CommandPrompt and RawPtyPassthrough modes (F1..F12, Shift+F1..F12, Alt+F1..F12)
    if let KeyCode::F(n @ 1..=12) = key.code {
        if key.modifiers == KeyModifiers::ALT {
            if n == 12 {
                return Ok(Some(wm.apply(Action::FkeyModeCommand(None))?));
            }
            if let Some(act) = bilterm_action_for_fkey(wm, n) {
                return Ok(Some(wm.apply(act)?));
            }
        }
        wm.active_pane_mut().refresh_line_discipline();
        let in_raw_passthrough = wm.active_pane().effective_routing_mode()
            == InputRoutingMode::RawPtyPassthrough
            && wm.active_pane().pending_unsafe_paste().is_none()
            && !wm.active_pane().copy_mode().active
            && !wm.active_pane().hints_mode().active;
        if key.modifiers == KeyModifiers::SHIFT {
            let pane = wm.active_pane_mut();
            if !pane.is_read_only() {
                let bytes = crate::input::encode_key_to_pty_bytes_full(
                    KeyEvent::new(KeyCode::F(n), KeyModifiers::NONE),
                    pane.vt().is_application_cursor(),
                    pane.vt().is_backarrow_key_67(),
                    pane.vt().kitty_keyboard_flags(),
                );
                if !bytes.is_empty() {
                    pane.write_to_pty(&bytes)?;
                }
            }
            return Ok(Some(AppAction::Handled));
        }
        if in_raw_passthrough && key.modifiers.is_empty() {
            if n == 12 {
                return Ok(Some(wm.apply(Action::RawToggle)?));
            }
            if n == 7 && wm.drawer_is_open() {
                return Ok(Some(wm.apply(Action::DrawerToggle)?));
            }
            if wm
                .config()
                .raw_fkeys
                .intercept_plain_fkeys_in_raw(wm.active_pane().vt().is_alternate_screen())
                && !wm.active_pane().is_active_canvas_password_prompt()
                && let Some(act) = bilterm_action_for_fkey(wm, n)
            {
                return Ok(Some(wm.apply(act)?));
            }
        }
    }

    // Always route per-pane font zoom (`Ctrl++`/`Ctrl+-`/`Ctrl+0`), clipboard paste (`Ctrl+Shift+V`/`Shift+Insert`),
    // Command Palette (`Ctrl+P`/`Alt+Space`), and WM pane focus/resize chords even when the active pane is running an interactive RawPtyPassthrough TUI app (`nmon`, `htop`, `vim`, `herdr`).
    if let Some(action) = default_global_action(key, wm.active_pane().prompt().fuzzy_search.active)
        && matches!(
            action,
            Action::PasteClipboard
                | Action::PaletteToggle
                | Action::FontZoomIn
                | Action::FontZoomOut
                | Action::FontZoomReset
                | Action::PaneResize { .. }
                | Action::PaneMove { .. }
                | Action::FocusPane(_)
                | Action::PaneFocusDirection { .. }
                | Action::DeckNext
                | Action::DeckPrev
        )
    {
        return Ok(Some(wm.apply(action)?));
    }

    if wm.pane_count() > 1
        && key.modifiers == KeyModifiers::ALT
        && !wm.active_pane().vt().is_alternate_screen()
        && (wm.active_pane().effective_routing_mode() == InputRoutingMode::RawPtyPassthrough
            || wm.active_pane().prompt().input.is_empty())
    {
        let dir = match key.code {
            KeyCode::Left => Some((-1, 0)),
            KeyCode::Right => Some((1, 0)),
            KeyCode::Up => Some((0, -1)),
            KeyCode::Down => Some((0, 1)),
            _ => None,
        };
        if let Some((dx, dy)) = dir {
            return Ok(Some(wm.apply(Action::PaneFocusDirection { dx, dy })?));
        }
    }

    if !wm.active_pane().vt().is_alternate_screen()
        && !wm.active_pane().is_active_canvas_password_prompt()
        && !(wm.active_pane().scroll_mode() == crate::pane::PaneScrollMode::BlockPage
            && (wm.active_pane().is_page_holding() || wm.active_pane().is_holding_inspected()))
        && let Some(action) = default_pane_action(key)
        && matches!(
            action,
            Action::SelectCommand(_) | Action::CopyModeToggle | Action::HintsToggle
        )
    {
        return Ok(Some(wm.apply(action)?));
    }

    Ok(None)
}

/// Handles global WindowManager shortcuts (Palette, Quake drop-down, F1..F10, and Alt-chords).
pub(super) fn handle_wm_shortcut_key(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
    if wm.active_pane().scroll_mode() == crate::pane::PaneScrollMode::BlockPage
        && (wm.active_pane().is_page_holding() || wm.active_pane().is_holding_inspected())
        && key.modifiers.is_empty()
    {
        match key.code {
            KeyCode::F(7) => return Ok(Some(wm.apply(Action::ScrollPageBack)?)),
            KeyCode::F(8) => return Ok(Some(wm.apply(Action::ScrollPageAdvance)?)),
            _ => {}
        }
    }
    let fuzzy_active = wm.active_pane().prompt().fuzzy_search.active;
    if wm.pane_count() > 1
        && key.modifiers == KeyModifiers::ALT
        && (wm.active_pane().effective_routing_mode() == InputRoutingMode::RawPtyPassthrough
            || wm.active_pane().prompt().input.is_empty())
    {
        let dir = match key.code {
            KeyCode::Left => Some((-1, 0)),
            KeyCode::Right => Some((1, 0)),
            KeyCode::Up => Some((0, -1)),
            KeyCode::Down => Some((0, 1)),
            _ => None,
        };
        if let Some((dx, dy)) = dir {
            return Ok(Some(wm.apply(Action::PaneFocusDirection { dx, dy })?));
        }
    }
    if key.modifiers == KeyModifiers::ALT && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
        return Ok(Some(wm.apply(Action::SelectCommand(Some("clipboard pop".to_string())))?));
    }
    if let Some(action) = default_global_action(key, fuzzy_active) {
        return Ok(Some(wm.apply(action)?));
    }
    Ok(None)
}
