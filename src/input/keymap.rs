use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::colon_command::{execute_colon_command, parse_colon_command};
use crate::error::Result;
use crate::input::AppAction;
use crate::pane::CompositePane;
use crate::pty::line_discipline::InputRoutingMode;
use crate::wm::WindowManager;

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
    (KeyCode::Char('v'), Action::SplitVertical),
    (KeyCode::Char('V'), Action::SplitVertical),
    (KeyCode::Char('z'), Action::ZoomToggle),
    (KeyCode::Char('Z'), Action::ZoomToggle),
    (KeyCode::F(5), Action::ZoomToggle),
    (KeyCode::Char('o'), Action::DeckNext),
    (KeyCode::Char('O'), Action::DeckNext),
    (KeyCode::Char('n'), Action::DeckNext),
    (KeyCode::Tab, Action::DeckNext),
    (KeyCode::F(6), Action::DeckNext),
    (KeyCode::Char('p'), Action::DeckPrev),
    (KeyCode::BackTab, Action::DeckPrev),
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
    (KeyCode::Char('D'), Action::DrawerToggle),
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
    (KeyCode::Char('f'), Action::HintsToggle),
    (KeyCode::Char('F'), Action::HintsToggle),
    (KeyCode::F(3), Action::HintsToggle),
    (KeyCode::Char('l'), Action::LockToggle(None)),
    (KeyCode::Char('/'), Action::SearchPromptPrefill),
    (KeyCode::Char('N'), Action::SearchPrev),
    (KeyCode::Char('P'), Action::HistoryPopupToggle),
    (KeyCode::Char('H'), Action::HistoryPopupToggle),
    (KeyCode::Char('R'), Action::RecordWorkspaceStart(None)),
    (KeyCode::Up, Action::PromptMoveVertical(-1)),
    (KeyCode::Down, Action::PromptMoveVertical(1)),
    (KeyCode::Char('+'), Action::PromptResizeHeight(1)),
    (KeyCode::Char('='), Action::PromptResizeHeight(1)),
    (KeyCode::Char(']'), Action::PromptResizeWidth(4)),
    (KeyCode::PageUp, Action::BlockPrev),
    (KeyCode::PageDown, Action::BlockNext),
];

/// Declarative global workspace shortcut table (`F1`..`F10`, `Alt+<key>`, `Ctrl+P`, `Ctrl+\\`, `ARCH-R01.2`).
pub const DEFAULT_GLOBAL_BINDINGS: &[(KeyModifiers, KeyCode, Action)] = &[
    // Global Fuzzy Command Palette & Guake/Quake quick terminal
    (KeyModifiers::ALT, KeyCode::Char(' '), Action::PaletteToggle),
    (
        KeyModifiers::CONTROL,
        KeyCode::Char('p'),
        Action::PaletteToggle,
    ),
    (KeyModifiers::ALT, KeyCode::Char('`'), Action::QuakeToggle),
    (
        KeyModifiers::CONTROL,
        KeyCode::Char(' '),
        Action::QuakeToggle,
    ),
    (
        KeyModifiers::CONTROL,
        KeyCode::Char('\\'),
        Action::QuakeToggle,
    ),
    (
        KeyModifiers::CONTROL,
        KeyCode::Char('\x1c'),
        Action::QuakeToggle,
    ),
    // Borland Turbo Vision function keys
    (KeyModifiers::NONE, KeyCode::F(1), Action::HelpToggle),
    (KeyModifiers::NONE, KeyCode::F(2), Action::SplitHorizontal),
    (KeyModifiers::NONE, KeyCode::F(5), Action::ZoomToggle),
    (KeyModifiers::NONE, KeyCode::F(6), Action::DeckNext),
    (
        KeyModifiers::NONE,
        KeyCode::F(9),
        Action::InspectorToggle(None),
    ),
    (KeyModifiers::NONE, KeyCode::F(10), Action::QuakeToggle),
    // Multi-pane Window Manager Alt-chords
    (
        KeyModifiers::ALT,
        KeyCode::Char('o'),
        Action::spotlight_cycle(),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('O'),
        Action::focal_zoom_cycle(),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('}'),
        Action::FocalRadiusResize(4, 1),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('{'),
        Action::FocalRadiusResize(-4, -1),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('x'),
        Action::CrosshairCycle,
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('X'),
        Action::CrosshairCycle,
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('h'),
        Action::SplitHorizontal,
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('H'),
        Action::SplitHorizontal,
    ),
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
];

/// Declarative intra-pane shortcut table (`F3` and `Alt+<key>` in `CommandPrompt` mode, `ARCH-R01.2`).
pub const DEFAULT_PANE_BINDINGS: &[(KeyModifiers, KeyCode, Action)] = &[
    (KeyModifiers::NONE, KeyCode::F(3), Action::HintsToggle),
    (KeyModifiers::ALT, KeyCode::Char('t'), Action::PromptTop),
    (KeyModifiers::ALT, KeyCode::Char('T'), Action::PromptTop),
    (KeyModifiers::ALT, KeyCode::Char('B'), Action::PromptBottom),
    (
        KeyModifiers::ALT,
        KeyCode::Char('s'),
        Action::PromptSplitToggle,
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('S'),
        Action::PromptSplitToggle,
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('c'),
        Action::CopyModeToggle,
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('C'),
        Action::CopyModeToggle,
    ),
    (KeyModifiers::ALT, KeyCode::Char('F'), Action::HintsToggle),
    (
        KeyModifiers::ALT,
        KeyCode::Char('l'),
        Action::LockToggle(None),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('L'),
        Action::LockToggle(None),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('/'),
        Action::SearchPromptPrefill,
    ),
    (KeyModifiers::ALT, KeyCode::Char('n'), Action::SearchNext),
    (KeyModifiers::ALT, KeyCode::Char('N'), Action::SearchPrev),
    (
        KeyModifiers::ALT,
        KeyCode::Up,
        Action::PromptMoveVertical(-1),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Down,
        Action::PromptMoveVertical(1),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('+'),
        Action::PromptResizeHeight(1),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('='),
        Action::PromptResizeHeight(1),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('-'),
        Action::PromptResizeHeight(-1),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char(']'),
        Action::PromptResizeWidth(4),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('['),
        Action::PromptResizeWidth(-4),
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('p'),
        Action::HistoryPopupToggle,
    ),
    (
        KeyModifiers::ALT,
        KeyCode::Char('P'),
        Action::HistoryPopupToggle,
    ),
    (KeyModifiers::ALT, KeyCode::PageUp, Action::BlockPrev),
    (KeyModifiers::ALT, KeyCode::PageDown, Action::BlockNext),
];

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
    DEFAULT_GLOBAL_BINDINGS
        .iter()
        .find(|(mods, code, _)| *mods == lookup_mods && *code == key.code)
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
    let lookup_key = if key.modifiers.contains(KeyModifiers::CONTROL)
        && let KeyCode::Char(c) = key.code
    {
        if c.eq_ignore_ascii_case(&'q') {
            return wm.apply(Action::Quit);
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
        return wm.apply(action);
    }

    Ok(AppAction::Handled)
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
            "⌨ LEADER [{leader_label}]: \"/-=SplitH %/|=SplitV z=Zoom n/p/o=Deck 1-9=Pane q=Quake e/t/b/s=Prompt [=Copy f=Hints ?=Help"
        )));
        return Ok(Some(AppAction::Handled));
    }

    Ok(None)
}

/// Handles global WindowManager shortcuts (Palette, Quake drop-down, F1..F10, and Alt-chords).
pub(super) fn handle_wm_shortcut_key(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
    let fuzzy_active = wm.active_pane().prompt().fuzzy_search.active;
    if let Some(action) = default_global_action(key, fuzzy_active) {
        return Ok(Some(wm.apply(action)?));
    }
    Ok(None)
}

/// Handles `Enter` submissions on the active `WindowManager` pane (in-prompt `:colon` commands and broadcast mode).
pub(super) fn handle_wm_enter_submission(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
    if key.code == KeyCode::Enter
        && !key.modifiers.contains(KeyModifiers::SHIFT)
        && !key.modifiers.contains(KeyModifiers::ALT)
        && wm.active_pane().effective_routing_mode() == InputRoutingMode::CommandPrompt
        && !wm.active_pane().prompt().fuzzy_search.active
        && !wm.active_pane().copy_mode().active
        && !wm.active_pane().hints_mode().active
        && wm.active_pane().pending_unsafe_paste().is_none()
    {
        let text = wm.active_pane().prompt().text();
        if let Some(cmd_res) = parse_colon_command(&text) {
            let _ = wm.active_pane_mut().prompt_mut().take_submission();
            return match cmd_res {
                Ok(action) => Ok(Some(wm.apply(action)?)),
                Err(err) => {
                    wm.active_pane_mut().set_status_message(Some(err));
                    Ok(Some(AppAction::Handled))
                }
            };
        }

        if wm.is_broadcast_input() && !wm.active_pane().is_read_only() {
            let submitted = wm.active_pane_mut().prompt_mut().take_submission();
            wm.broadcast_command(&submitted)?;
            return Ok(Some(AppAction::Handled));
        }
    }
    Ok(None)
}

/// Handles single-pane function keys (`F3` Hints) and Alt-chord shortcuts in `CommandPrompt` mode.
pub(super) fn handle_pane_alt_or_fkey_shortcut(
    pane: &mut CompositePane,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Char('b') => {
                if !pane.prompt().input.is_empty() {
                    pane.prompt_mut().move_word_left();
                } else {
                    let _ = execute_colon_command(pane, Action::PromptBottom);
                }
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('f') => {
                if !pane.prompt().input.is_empty() {
                    pane.prompt_mut().move_word_right();
                } else {
                    let _ = execute_colon_command(pane, Action::HintsToggle);
                }
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                pane.prompt_mut().kill_next_word();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Backspace => {
                pane.prompt_mut().kill_prev_word();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Left => {
                pane.prompt_mut().move_word_left();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Right => {
                pane.prompt_mut().move_word_right();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Enter => {
                pane.prompt_mut().insert_newline();
                return Ok(Some(AppAction::Handled));
            }
            _ => {}
        }
    }

    if let Some(action) = default_pane_action(key) {
        let _ = execute_colon_command(pane, action);
        return Ok(Some(AppAction::Handled));
    }

    Ok(None)
}
