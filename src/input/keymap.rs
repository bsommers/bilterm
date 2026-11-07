use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::colon_command::parse_colon_command;
use crate::error::Result;
use crate::input::AppAction;
use crate::pty::line_discipline::InputRoutingMode;
use crate::wm::WindowManager;
use crate::wm::agents_cmd::AgentsOp;
use crate::wm::agents_list_cmd::AgentsListOp;
use crate::wm::find_cmd::FindOp;
use crate::wm::link_cmd::RemoteLinkOp;
use crate::wm::session_map::SessionMapOp;

#[path = "keymap_tables.rs"]
mod keymap_tables;
pub use keymap_tables::*;

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

/// The global chords that must reach the window manager even while the active pane is in raw PTY
/// passthrough (an alternate-screen app such as an agent TUI, `vim` or `less`), where every other
/// global shortcut is forwarded to the child. Today that is the agent-traffic toggle (`Ctrl+Alt+A`)
/// the Find window (`Ctrl+Alt+F`) and the tmux link reconnect (`Ctrl+Alt+R`): the launchers the user needs
/// while an agent or a remote ssh prompt owns the pane.
pub(super) fn reserved_global_action(key: KeyEvent) -> Option<Action> {
    match default_global_action(key, false) {
        Some(a @ (Action::Agents(AgentsOp::Toggle(_)) | Action::AgentsList(AgentsListOp::Toggle) | Action::SessionMap(SessionMapOp::Toggle) | Action::Find(FindOp::Open { .. }) | Action::RemoteLink(RemoteLinkOp::Reconnect))) => Some(a),
        _ => None,
    }
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
        7 => Some(
            if block_holding && !wm.drawer_is_open() && !wm.is_drawer_effectively_visible() {
                Action::ScrollPageBack
            } else {
                Action::DrawerToggle
            },
        ),
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
            if n == 7 {
                return Ok(Some(wm.apply(Action::DrawerToggle)?));
            }
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
            if n == 7
                && (wm.drawer_is_open()
                    || wm.is_drawer_effectively_visible()
                    || wm.config().raw_fkeys != crate::ui::RawFkeyMode::App)
            {
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
            KeyCode::F(7) if !wm.drawer_is_open() && !wm.is_drawer_effectively_visible() => {
                return Ok(Some(wm.apply(Action::ScrollPageBack)?));
            }
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
