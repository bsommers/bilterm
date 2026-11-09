//! Declarative key tables of sticky Window mode (`KEY-01`, Phase 63-02).
//!
//! Same shape as [`NAVIGATOR_DRAWER_BINDINGS`](super::keymap::NAVIGATOR_DRAWER_BINDINGS): every
//! row is `(scope, modifiers, key, Action)` and lowers to an [`Action`] that runs through
//! `WindowManager::apply`. Modifiers are always `NONE`: after entering the mode no chord is needed.
//!
//! Scope rules (see [`window_mode_action`](super::window_mode::window_mode_action)): a row applies
//! in its own scope; `Focus`-scope rows whose action is [`Action::WindowMode`] are mode-control rows
//! (exit, sub-mode switch) and apply in every scope.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::action::{Action, WinSub, WindowModeOp};
use crate::wm::WorkspaceLayoutMode;
use crate::wm::agents_list_cmd::AgentsListOp;
use crate::wm::find_cmd::{FindKind, FindOp};

/// Which Window-mode sub-mode a row belongs to (mirrors [`WinSub`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WinScope {
    Focus,
    Resize,
    Move,
}

impl WinScope {
    /// The scope of a sub-mode.
    pub const fn of(sub: WinSub) -> Self {
        match sub {
            WinSub::Focus => Self::Focus,
            WinSub::Resize => Self::Resize,
            WinSub::Move => Self::Move,
        }
    }
}

const N: KeyModifiers = KeyModifiers::NONE;

const FOCUS_LEFT: Action = Action::PaneFocusDirection { dx: -1, dy: 0 };
const FOCUS_RIGHT: Action = Action::PaneFocusDirection { dx: 1, dy: 0 };
const FOCUS_UP: Action = Action::PaneFocusDirection { dx: 0, dy: -1 };
const FOCUS_DOWN: Action = Action::PaneFocusDirection { dx: 0, dy: 1 };

/// Resize steps reuse the leader `<` / `>` and `Ctrl+Alt+Arrow` deltas.
const RESIZE_LEFT: Action = Action::PaneResize { dw: -4, dh: 0 };
const RESIZE_RIGHT: Action = Action::PaneResize { dw: 4, dh: 0 };
const RESIZE_UP: Action = Action::PaneResize { dw: 0, dh: -2 };
const RESIZE_DOWN: Action = Action::PaneResize { dw: 0, dh: 2 };

/// Move steps reuse the leader `J` / `K` and `Ctrl+Shift+Arrow` deltas.
const MOVE_LEFT: Action = Action::PaneMove { dx: -4, dy: 0 };
const MOVE_RIGHT: Action = Action::PaneMove { dx: 4, dy: 0 };
const MOVE_UP: Action = Action::PaneMove { dx: 0, dy: -2 };
const MOVE_DOWN: Action = Action::PaneMove { dx: 0, dy: 2 };

/// Window-mode bindings. The first row listed for an action is its canonical key (hints, footer).
pub static WINDOW_MODE_BINDINGS: &[(WinScope, KeyModifiers, KeyCode, Action)] = &[
    // Mode control (every scope).
    (WinScope::Focus, N, KeyCode::Esc, Action::WindowMode(WindowModeOp::Exit)),
    (WinScope::Focus, N, KeyCode::Enter, Action::WindowMode(WindowModeOp::Exit)),
    (WinScope::Focus, N, KeyCode::Char('q'), Action::WindowMode(WindowModeOp::Exit)),
    (WinScope::Focus, N, KeyCode::Char('r'), Action::WindowMode(WindowModeOp::Sub(WinSub::Resize))),
    (WinScope::Focus, N, KeyCode::Char('m'), Action::WindowMode(WindowModeOp::Sub(WinSub::Move))),
    (WinScope::Focus, N, KeyCode::Char('f'), Action::WindowMode(WindowModeOp::Sub(WinSub::Focus))),
    // Focus: direction.
    (WinScope::Focus, N, KeyCode::Char('h'), FOCUS_LEFT),
    (WinScope::Focus, N, KeyCode::Left, FOCUS_LEFT),
    (WinScope::Focus, N, KeyCode::Char('j'), FOCUS_DOWN),
    (WinScope::Focus, N, KeyCode::Down, FOCUS_DOWN),
    (WinScope::Focus, N, KeyCode::Char('k'), FOCUS_UP),
    (WinScope::Focus, N, KeyCode::Up, FOCUS_UP),
    (WinScope::Focus, N, KeyCode::Char('l'), FOCUS_RIGHT),
    (WinScope::Focus, N, KeyCode::Right, FOCUS_RIGHT),
    // Focus: jump by number.
    (WinScope::Focus, N, KeyCode::Char('1'), Action::FocusPane(1)),
    (WinScope::Focus, N, KeyCode::Char('2'), Action::FocusPane(2)),
    (WinScope::Focus, N, KeyCode::Char('3'), Action::FocusPane(3)),
    (WinScope::Focus, N, KeyCode::Char('4'), Action::FocusPane(4)),
    (WinScope::Focus, N, KeyCode::Char('5'), Action::FocusPane(5)),
    (WinScope::Focus, N, KeyCode::Char('6'), Action::FocusPane(6)),
    (WinScope::Focus, N, KeyCode::Char('7'), Action::FocusPane(7)),
    (WinScope::Focus, N, KeyCode::Char('8'), Action::FocusPane(8)),
    (WinScope::Focus, N, KeyCode::Char('9'), Action::FocusPane(9)),
    // Focus: deck.
    (WinScope::Focus, N, KeyCode::Tab, Action::DeckNext),
    (WinScope::Focus, N, KeyCode::Char('n'), Action::DeckNext),
    (WinScope::Focus, N, KeyCode::BackTab, Action::DeckPrev),
    (WinScope::Focus, N, KeyCode::Char('p'), Action::DeckPrev),
    // Focus: zoom and layout.
    (WinScope::Focus, N, KeyCode::Char('z'), Action::ZoomToggle),
    (WinScope::Focus, N, KeyCode::Char('s'), Action::LayoutMode(WorkspaceLayoutMode::Stacked)),
    (WinScope::Focus, N, KeyCode::Char('t'), Action::LayoutMode(WorkspaceLayoutMode::Tiled)),
    (WinScope::Focus, N, KeyCode::Char(' '), Action::LayoutCycle),
    // Focus: split / close.
    (WinScope::Focus, N, KeyCode::Char('-'), Action::SplitHorizontal),
    (WinScope::Focus, N, KeyCode::Char('"'), Action::SplitHorizontal),
    (WinScope::Focus, N, KeyCode::Char('|'), Action::SplitVertical),
    (WinScope::Focus, N, KeyCode::Char('%'), Action::SplitVertical),
    (WinScope::Focus, N, KeyCode::Char('x'), Action::ClosePane),
    // Focus: drawer, copy mode, tools.
    (WinScope::Focus, N, KeyCode::Char('d'), Action::DrawerToggle),
    (WinScope::Focus, N, KeyCode::Char('D'), Action::DrawerFocus),
    (WinScope::Focus, N, KeyCode::Char('c'), Action::CopyModeToggle),
    (WinScope::Focus, N, KeyCode::Char('['), Action::CopyModeToggle),
    (WinScope::Focus, N, KeyCode::Char('a'), Action::AgentsList(AgentsListOp::Toggle)),
    (
        WinScope::Focus,
        N,
        KeyCode::Char('/'),
        Action::Find(FindOp::Open { kind: FindKind::Agent, query: String::new() }),
    ),
    (WinScope::Focus, N, KeyCode::Char(':'), Action::PaletteToggle),
    // Focus: one-key resize / move steps (leader `<` `>` `J` `K` parity).
    (WinScope::Focus, N, KeyCode::Char('<'), RESIZE_LEFT),
    (WinScope::Focus, N, KeyCode::Char('>'), RESIZE_RIGHT),
    (WinScope::Focus, N, KeyCode::Char('J'), MOVE_DOWN),
    (WinScope::Focus, N, KeyCode::Char('K'), MOVE_UP),
    // Resize sub-mode.
    (WinScope::Resize, N, KeyCode::Char('h'), RESIZE_LEFT),
    (WinScope::Resize, N, KeyCode::Left, RESIZE_LEFT),
    (WinScope::Resize, N, KeyCode::Char('j'), RESIZE_DOWN),
    (WinScope::Resize, N, KeyCode::Down, RESIZE_DOWN),
    (WinScope::Resize, N, KeyCode::Char('k'), RESIZE_UP),
    (WinScope::Resize, N, KeyCode::Up, RESIZE_UP),
    (WinScope::Resize, N, KeyCode::Char('l'), RESIZE_RIGHT),
    (WinScope::Resize, N, KeyCode::Right, RESIZE_RIGHT),
    // Move sub-mode.
    (WinScope::Move, N, KeyCode::Char('h'), MOVE_LEFT),
    (WinScope::Move, N, KeyCode::Left, MOVE_LEFT),
    (WinScope::Move, N, KeyCode::Char('j'), MOVE_DOWN),
    (WinScope::Move, N, KeyCode::Down, MOVE_DOWN),
    (WinScope::Move, N, KeyCode::Char('k'), MOVE_UP),
    (WinScope::Move, N, KeyCode::Up, MOVE_UP),
    (WinScope::Move, N, KeyCode::Char('l'), MOVE_RIGHT),
    (WinScope::Move, N, KeyCode::Right, MOVE_RIGHT),
];

/// The key that pins the full which-key panel. It is view state, not a command, so it has no row.
pub const WHICH_KEY_PIN: char = '?';

/// Footer entries of the Focus sub-mode, in display order. Runs of directions or of `FocusPane`
/// collapse into one `hjkl` / `1-9` chip when the footer is built.
pub static WINDOW_MODE_FOOTER: &[Action] = &[
    FOCUS_LEFT,
    FOCUS_DOWN,
    FOCUS_UP,
    FOCUS_RIGHT,
    Action::FocusPane(1),
    Action::FocusPane(9),
    Action::ZoomToggle,
    Action::LayoutMode(WorkspaceLayoutMode::Stacked),
    Action::LayoutMode(WorkspaceLayoutMode::Tiled),
    Action::ClosePane,
];

/// Footer entries of the Resize sub-mode.
pub static WINDOW_RESIZE_FOOTER: &[Action] = &[RESIZE_LEFT, RESIZE_DOWN, RESIZE_UP, RESIZE_RIGHT];

/// Footer entries of the Move sub-mode.
pub static WINDOW_MOVE_FOOTER: &[Action] = &[MOVE_LEFT, MOVE_DOWN, MOVE_UP, MOVE_RIGHT];

/// Footer actions for `scope`.
pub fn footer_actions(scope: WinScope) -> &'static [Action] {
    match scope {
        WinScope::Focus => WINDOW_MODE_FOOTER,
        WinScope::Resize => WINDOW_RESIZE_FOOTER,
        WinScope::Move => WINDOW_MOVE_FOOTER,
    }
}

/// `true` for rows that control the mode itself and therefore apply in every scope.
pub fn is_mode_control(action: &Action) -> bool {
    matches!(action, Action::WindowMode(_))
}

/// Rows that apply while `scope` is active, in table order.
pub fn rows_for_scope(scope: WinScope) -> impl Iterator<Item = &'static (WinScope, KeyModifiers, KeyCode, Action)> {
    WINDOW_MODE_BINDINGS
        .iter()
        .filter(move |(s, _, _, a)| *s == scope || (*s == WinScope::Focus && is_mode_control(a)))
}
