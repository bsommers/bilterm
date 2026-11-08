//! Declarative per-profile key data (`KEY-02`, `KEY-03`, Phase 63). Every row lowers to an
//! [`Action`]; the legacy tables in `keymap_tables.rs` stay the base of every profile.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::action::{Action, WindowModeOp};

/// `Primary + <key>` bindings (63-RESEARCH R2). Resolved to Cmd on macOS GUI and Ctrl+Shift on
/// Windows/Linux GUI and kitty-capable TUIs. No row may need Shift itself (Shift is part of
/// Primary on Windows/Linux), so letters must differ and symbols are base keys.
pub const PRIMARY_BINDINGS: &[(KeyCode, Action)] = &[
    (KeyCode::Char('z'), Action::ZoomToggle),
    (KeyCode::Char('w'), Action::ClosePane),
    (KeyCode::Char('d'), Action::SplitVertical),
    (KeyCode::Char('b'), Action::SplitHorizontal),
    // Replaces Alt+M (compose key on macOS).
    (KeyCode::Char('l'), Action::LayoutCycle),
    (KeyCode::Char('o'), Action::WindowMode(WindowModeOp::Enter)),
    (KeyCode::Char('p'), Action::PaletteToggle),
    (KeyCode::Char(']'), Action::DeckNext),
    (KeyCode::Char('['), Action::DeckPrev),
    (KeyCode::Char('1'), Action::FocusPane(1)),
    (KeyCode::Char('2'), Action::FocusPane(2)),
    (KeyCode::Char('3'), Action::FocusPane(3)),
    (KeyCode::Char('4'), Action::FocusPane(4)),
    (KeyCode::Char('5'), Action::FocusPane(5)),
    (KeyCode::Char('6'), Action::FocusPane(6)),
    (KeyCode::Char('7'), Action::FocusPane(7)),
    (KeyCode::Char('8'), Action::FocusPane(8)),
    (KeyCode::Char('9'), Action::FocusPane(9)),
    (KeyCode::Char(';'), Action::MenuOpen { category: None }),
];

/// `Alt+<key>` chords that stay chords on every profile because they edit the prompt
/// (readline `M-b M-f M-d M-y`, word delete/move). All other Option+key input is composed text.
pub const PROMPT_EDIT_ALT_KEYS: &[KeyCode] = &[
    KeyCode::Char('b'),
    KeyCode::Char('f'),
    KeyCode::Char('d'),
    KeyCode::Char('y'),
    KeyCode::Backspace,
    KeyCode::Delete,
    KeyCode::Left,
    KeyCode::Right,
];

/// macOS-only rows: `Cmd+Option+Arrows` replace the `Alt+Arrow` focus aliases.
pub const MAC_EXTRA: &[(KeyModifiers, KeyCode, Action)] = &[
    (KeyModifiers::SUPER.union(KeyModifiers::ALT), KeyCode::Left, Action::PaneFocusDirection { dx: -1, dy: 0 }),
    (KeyModifiers::SUPER.union(KeyModifiers::ALT), KeyCode::Right, Action::PaneFocusDirection { dx: 1, dy: 0 }),
    (KeyModifiers::SUPER.union(KeyModifiers::ALT), KeyCode::Up, Action::PaneFocusDirection { dx: 0, dy: -1 }),
    (KeyModifiers::SUPER.union(KeyModifiers::ALT), KeyCode::Down, Action::PaneFocusDirection { dx: 0, dy: 1 }),
];

/// Rows of `DEFAULT_GLOBAL_BINDINGS` that exist only on the `legacy` profile: ambiguous after
/// Shift-folding (`Alt+Shift+{H,J,K,L,A,S,W,D}` focus aliases), reserved by every OS
/// (`Alt+Tab`), readline undo (`Ctrl+_`), or IME/desktop owned (`Ctrl+Space`, `Alt+Space`, ``Alt+` ``).
pub const NON_LEGACY_REMOVED: &[(KeyModifiers, KeyCode)] = &[
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('H')),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('J')),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('K')),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('L')),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('A')),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('S')),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('W')),
    (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::Char('D')),
    (KeyModifiers::ALT, KeyCode::Tab),
    (KeyModifiers::ALT, KeyCode::BackTab),
    (KeyModifiers::CONTROL, KeyCode::Char('_')),
    (KeyModifiers::CONTROL, KeyCode::Char('\x1f')),
    (KeyModifiers::CONTROL, KeyCode::Char(' ')),
    (KeyModifiers::ALT, KeyCode::Char(' ')),
    (KeyModifiers::ALT, KeyCode::Char('`')),
];

/// Leader rows that differ outside the `legacy` profile (`w` enters Window mode, `m` opens the
/// menu bar). `W` stays `ClosePane` everywhere; `legacy` keeps `w` = ClosePane, `m` = PromptModeSet.
pub const LEADER_OVERRIDES: &[(KeyCode, Action)] = &[
    (KeyCode::Char('w'), Action::WindowMode(WindowModeOp::Enter)),
    (KeyCode::Char('m'), Action::MenuOpen { category: None }),
];

/// Chords that two different actions share after Shift-folding on purpose. Each entry carries the
/// reason the shadow is tolerated; the integrity test fails on any other duplicate. Chord strings
/// are the folded canonical form (`KeyChord::folded().format_with("Super")`).
pub const ALLOWED_SHADOWS: &[(&str, &str)] = &[
    ("Alt+O", "legacy Alt layer: lowercase Alt+o is the spotlight cycle and Alt+O (Shift) the focal-zoom cycle; the case is the distinction"),
    ("Alt+H", "legacy Alt layer: Alt+h splits horizontally; the Alt+Shift+H focus-left alias exists on `legacy` only"),
    ("Alt+W", "legacy Alt layer: Alt+w closes the pane; the Alt+Shift+W focus-up alias exists on `legacy` only"),
    ("Alt+D", "legacy Alt layer: Alt+D toggles the drawer; the Alt+Shift+D focus-right alias exists on `legacy` only"),
    ("Alt+S", "legacy Alt layer: pane Alt+s toggles the prompt split; the Alt+Shift+S focus-down alias exists on `legacy` only"),
    ("Alt+L", "legacy Alt layer: pane Alt+l toggles the lock; the Alt+Shift+L focus-right alias exists on `legacy` only"),
    ("Alt+P", "Alt layer: global Alt+Shift+P opens the drawer preview while pane Alt+p opens the history popup; the case is the distinction"),
    ("Alt+N", "Alt layer: pane Alt+n is search-next and Alt+Shift+N search-previous; the case is the distinction"),
];
