use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::colon_command::{
    execute_colon_command, execute_wm_colon_command, parse_colon_command,
};
use crate::error::Result;
use crate::input::AppAction;
use crate::pane::{CompositePane, PromptAnchor};
use crate::pty::line_discipline::InputRoutingMode;
use crate::wm::WindowManager;
use crate::wm::split_tree::SplitDirection;

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
        // Option + a..z
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
        // Option + Shift + A..Z
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
        // Option + 1..9, 0
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
        // Option + Space & Punctuation
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

/// Checks whether `key` matches the configured Prefix/Leader key chord (e.g. `"Ctrl+X"`, `"Ctrl+A"`).
pub fn is_leader_key(key: KeyEvent, leader_cfg: &str) -> bool {
    let target = leader_cfg.trim();
    if target.is_empty() {
        return false;
    }
    if let Some(chord) = format_key_chord(key) {
        chord.eq_ignore_ascii_case(target)
    } else {
        false
    }
}

pub(super) fn format_key_chord(key: KeyEvent) -> Option<String> {
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

pub(super) fn handle_pane_leader_key(
    pane: &mut CompositePane,
    key: KeyEvent,
) -> Result<AppAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && let KeyCode::Char(c) = key.code
    {
        if c.eq_ignore_ascii_case(&'q') {
            return Ok(AppAction::Quit);
        }
        // Double-tap leader (e.g. Ctrl+X Ctrl+X or Ctrl+A Ctrl+A)
        if pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough {
            let byte = (c.to_ascii_lowercase() as u8)
                .wrapping_sub(b'a')
                .wrapping_add(1);
            pane.write_to_pty(&[byte])?;
        } else if c.eq_ignore_ascii_case(&'a') {
            pane.prompt_mut().move_cursor_home();
        }
        return Ok(AppAction::Handled);
    }

    match key.code {
        KeyCode::Esc => {}
        KeyCode::Char('t') | KeyCode::Char('T') => {
            pane.set_prompt_anchor(PromptAnchor::Top);
        }
        KeyCode::Char('b') | KeyCode::Char('B') => {
            pane.set_prompt_anchor(PromptAnchor::Bottom);
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            pane.toggle_split_prompt();
        }
        KeyCode::Char('e') | KeyCode::Char('m') | KeyCode::F(4) => {
            pane.cycle_prompt_mode();
        }
        KeyCode::Char('g') | KeyCode::F(12) => {
            pane.toggle_manual_raw_override();
        }
        KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Char('[') => {
            if pane.copy_mode().active {
                pane.exit_copy_mode();
            } else {
                pane.enter_copy_mode();
            }
        }
        KeyCode::Char('f') | KeyCode::Char('F') | KeyCode::F(3) => {
            if pane.hints_mode().active {
                pane.exit_hints_mode();
            } else {
                pane.enter_hints_mode();
            }
        }
        KeyCode::Char('l') | KeyCode::Char('L') => {
            pane.toggle_read_only();
        }
        KeyCode::Char('/') => {
            if pane.is_prompt_visible() {
                pane.prompt_mut().set_text(":search ");
            }
        }
        KeyCode::Char('n') => {
            pane.next_canvas_search_match();
        }
        KeyCode::Char('N') => {
            pane.prev_canvas_search_match();
        }
        KeyCode::Char('p') | KeyCode::Char('P') | KeyCode::Char('H') => {
            pane.prompt_mut().toggle_history_popup();
        }
        KeyCode::Up => {
            let h = pane.vt().size().0 + 1;
            pane.move_prompt_vertical(-1, h);
        }
        KeyCode::Down => {
            let h = pane.vt().size().0 + 1;
            pane.move_prompt_vertical(1, h);
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            pane.resize_prompt_height(1);
        }
        KeyCode::Char('-') => {
            pane.resize_prompt_height(-1);
        }
        KeyCode::Char(']') | KeyCode::Char('}') => {
            pane.resize_prompt_width(4);
        }
        KeyCode::Char('{') => {
            pane.resize_prompt_width(-4);
        }
        KeyCode::PageUp => {
            pane.vt_mut().jump_to_prev_command_block();
        }
        KeyCode::PageDown => {
            pane.vt_mut().jump_to_next_command_block();
        }
        _ => {}
    }
    Ok(AppAction::Handled)
}

pub(super) fn handle_wm_leader_key(wm: &mut WindowManager, key: KeyEvent) -> Result<AppAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && let KeyCode::Char(c) = key.code
    {
        if c.eq_ignore_ascii_case(&'q') {
            return Ok(AppAction::Quit);
        }
        return handle_pane_leader_key(wm.active_pane_mut(), key);
    }

    match key.code {
        KeyCode::Esc => Ok(AppAction::Handled),
        KeyCode::Char('-') | KeyCode::Char('h') | KeyCode::F(2) => {
            wm.split_focused(SplitDirection::Horizontal)?;
            Ok(AppAction::Handled)
        }
        KeyCode::Char('|') | KeyCode::Char('\\') | KeyCode::Char('v') | KeyCode::Char('V') => {
            wm.split_focused(SplitDirection::Vertical)?;
            Ok(AppAction::Handled)
        }
        KeyCode::Char('z') | KeyCode::Char('Z') | KeyCode::F(5) => {
            wm.toggle_zoom();
            Ok(AppAction::Handled)
        }
        KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Tab | KeyCode::F(6) => {
            wm.shuffle_deck_next();
            Ok(AppAction::Handled)
        }
        KeyCode::BackTab => {
            wm.shuffle_deck_prev();
            Ok(AppAction::Handled)
        }
        KeyCode::Char(d @ '1'..='9') => {
            let id = (d as u8 - b'0') as usize;
            wm.focus_and_raise(id);
            Ok(AppAction::Handled)
        }
        KeyCode::Char('w') | KeyCode::Char('W') => {
            wm.close_focused_pane();
            Ok(AppAction::Handled)
        }
        KeyCode::Char('M') => {
            wm.toggle_layout_mode();
            Ok(AppAction::Handled)
        }
        KeyCode::Char('q') | KeyCode::Char('`') | KeyCode::Char('~') | KeyCode::F(10) => {
            wm.toggle_quick_terminal();
            Ok(AppAction::Handled)
        }
        KeyCode::Char(' ') | KeyCode::Char(':') => {
            wm.toggle_palette();
            Ok(AppAction::Handled)
        }
        KeyCode::Char('?') | KeyCode::F(1) => {
            wm.toggle_help();
            Ok(AppAction::Handled)
        }
        KeyCode::Char('i') | KeyCode::Char('I') | KeyCode::F(9) => {
            wm.toggle_inspector();
            Ok(AppAction::Handled)
        }
        KeyCode::Char('x') | KeyCode::Char('X') => {
            if !wm.config.crosshair_cursor {
                wm.config.crosshair_cursor = true;
                wm.config.crosshair_fullscreen = false;
            } else if !wm.config.crosshair_fullscreen {
                wm.config.crosshair_fullscreen = true;
            } else {
                wm.config.crosshair_cursor = false;
                wm.config.crosshair_fullscreen = false;
            }
            Ok(AppAction::Handled)
        }
        KeyCode::Char('L') => {
            let label = wm.config.cycle_spotlight();
            let rx = wm.config.spotlight_rx;
            let ry = wm.config.spotlight_ry;
            let feather = wm.config.spotlight_feather_pct;
            wm.active_pane_mut().set_status_message(Some(format!(
                "🔦 Focal Spotlight: {label} (rx={rx}, ry={ry}, feather={feather}%)"
            )));
            Ok(AppAction::Handled)
        }
        KeyCode::Char('}') => {
            if wm.config.spotlight_mode == crate::ui::cursor::SpotlightMode::Off {
                wm.config.spotlight_mode = crate::ui::cursor::SpotlightMode::Ellipse;
            }
            wm.config.resize_spotlight(4, 1);
            Ok(AppAction::Handled)
        }
        KeyCode::Char('{') => {
            if wm.config.spotlight_mode == crate::ui::cursor::SpotlightMode::Off {
                wm.config.spotlight_mode = crate::ui::cursor::SpotlightMode::Ellipse;
            }
            wm.config.resize_spotlight(-4, -1);
            Ok(AppAction::Handled)
        }
        _ => handle_pane_leader_key(wm.active_pane_mut(), key),
    }
}

/// Handles user-configured `[keybindings]` and Workspace Leader mode entry.
pub(super) fn handle_wm_user_binding_or_leader_entry(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Option<AppAction> {
    if !wm.config.keybindings.is_empty()
        && let Some(chord) = format_key_chord(key)
    {
        let bound_opt = wm
            .config
            .keybindings
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&chord))
            .map(|(_, v)| v.clone());
        if let Some(bound_action) = bound_opt {
            if let Some(Ok(cmd)) = parse_colon_command(&bound_action) {
                if execute_wm_colon_command(wm, cmd.clone()) {
                    return Some(AppAction::Quit);
                }
                if execute_colon_command(wm.active_pane_mut(), cmd) {
                    return Some(AppAction::Quit);
                }
            } else {
                wm.active_pane_mut().prompt_mut().set_text(&bound_action);
            }
            return Some(AppAction::Handled);
        }
    }

    if is_leader_key(key, &wm.config.leader_key) {
        wm.leader_pending = true;
        let leader_label = wm.config.leader_key.clone();
        wm.active_pane_mut().set_status_message(Some(format!(
            "⌨ LEADER [{leader_label}]: -/h=SplitH |/v=SplitV z=Zoom o/n/p=Deck 1-9=Pane q=Quake e/t/b/s=Prompt [=Copy f=Hints ?=Help"
        )));
        return Some(AppAction::Handled);
    }

    None
}

/// Handles global WindowManager shortcuts (Palette, Quake drop-down, F1..F10, and Alt-chords).
pub(super) fn handle_wm_shortcut_key(
    wm: &mut WindowManager,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
    //Global Fuzzy Command Palette hotkey: Ctrl+P (when not in fuzzy history search) or Alt+Space (v0.6.0)
    if (key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char(' '))
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('p')
            && !wm.active_pane().prompt().fuzzy_search.active)
    {
        wm.toggle_palette();
        return Ok(Some(AppAction::Handled));
    }

    // Global Guake/Quake-style quick terminal drop-down hotkey: Ctrl+\ (macOS-safe), Alt+`, or Ctrl+Space (WM-05)
    if (key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('`'))
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(
                key.code,
                KeyCode::Char(' ') | KeyCode::Char('\\') | KeyCode::Char('\x1c')
            ))
    {
        wm.toggle_quick_terminal();
        return Ok(Some(AppAction::Handled));
    }

    // Borland Turbo Vision function keys: F1 = Help, F2 = Split, F5 = Zoom, F6 = Next Pane, F9 = Inspector, F10 = Quake
    match key.code {
        KeyCode::F(1) => {
            wm.toggle_help();
            return Ok(Some(AppAction::Handled));
        }
        KeyCode::F(2) => {
            wm.split_focused(SplitDirection::Horizontal)?;
            return Ok(Some(AppAction::Handled));
        }
        KeyCode::F(5) => {
            wm.toggle_zoom();
            return Ok(Some(AppAction::Handled));
        }
        KeyCode::F(6) => {
            wm.shuffle_deck_next();
            return Ok(Some(AppAction::Handled));
        }
        KeyCode::F(9) => {
            wm.toggle_inspector();
            return Ok(Some(AppAction::Handled));
        }
        KeyCode::F(10) => {
            wm.toggle_quick_terminal();
            return Ok(Some(AppAction::Handled));
        }
        _ => {}
    }

    // Multi-pane Window Manager Alt-chords
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Char('o') => {
                let label = wm.config.cycle_spotlight();
                let rx = wm.config.spotlight_rx;
                let ry = wm.config.spotlight_ry;
                let feather = wm.config.spotlight_feather_pct;
                wm.active_pane_mut().set_status_message(Some(format!(
                    "🔦 Focal Spotlight: {label} (rx={rx}, ry={ry}, feather={feather}%)"
                )));
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('O') => {
                let label = wm.config.cycle_focal_zoom();
                let rx = wm.config.focal_zoom_rx;
                let ry = wm.config.focal_zoom_ry;
                let feather = wm.config.focal_zoom_feather_pct;
                wm.active_pane_mut().set_status_message(Some(format!(
                    "🔍 Focal Zoom: {label} (rx={rx}, ry={ry}, feather={feather}%)"
                )));
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('}') => {
                if wm.config.focal_zoom_mode != crate::ui::cursor::SpotlightMode::Off {
                    wm.config.resize_focal_zoom(4, 1);
                }
                if wm.config.spotlight_mode == crate::ui::cursor::SpotlightMode::Off
                    && wm.config.focal_zoom_mode == crate::ui::cursor::SpotlightMode::Off
                {
                    wm.config.spotlight_mode = crate::ui::cursor::SpotlightMode::Ellipse;
                }
                if wm.config.spotlight_mode != crate::ui::cursor::SpotlightMode::Off {
                    wm.config.resize_spotlight(4, 1);
                }
                let label = wm
                    .config
                    .spotlight_mode
                    .label(wm.config.spotlight_pixelated);
                let s_rx = wm.config.spotlight_rx;
                let s_ry = wm.config.spotlight_ry;
                let z_rx = wm.config.focal_zoom_rx;
                let z_ry = wm.config.focal_zoom_ry;
                wm.active_pane_mut().set_status_message(Some(format!(
                    "🔦 Focal Radius: spot=({s_rx}, {s_ry}) zoom=({z_rx}, {z_ry}) [{label}]"
                )));
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('{') => {
                if wm.config.focal_zoom_mode != crate::ui::cursor::SpotlightMode::Off {
                    wm.config.resize_focal_zoom(-4, -1);
                }
                if wm.config.spotlight_mode == crate::ui::cursor::SpotlightMode::Off
                    && wm.config.focal_zoom_mode == crate::ui::cursor::SpotlightMode::Off
                {
                    wm.config.spotlight_mode = crate::ui::cursor::SpotlightMode::Ellipse;
                }
                if wm.config.spotlight_mode != crate::ui::cursor::SpotlightMode::Off {
                    wm.config.resize_spotlight(-4, -1);
                }
                let label = wm
                    .config
                    .spotlight_mode
                    .label(wm.config.spotlight_pixelated);
                let s_rx = wm.config.spotlight_rx;
                let s_ry = wm.config.spotlight_ry;
                let z_rx = wm.config.focal_zoom_rx;
                let z_ry = wm.config.focal_zoom_ry;
                wm.active_pane_mut().set_status_message(Some(format!(
                    "🔦 Focal Radius: spot=({s_rx}, {s_ry}) zoom=({z_rx}, {z_ry}) [{label}]"
                )));
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                if !wm.config.crosshair_cursor {
                    wm.config.crosshair_cursor = true;
                    wm.config.crosshair_fullscreen = false;
                } else if !wm.config.crosshair_fullscreen {
                    wm.config.crosshair_fullscreen = true;
                } else {
                    wm.config.crosshair_cursor = false;
                    wm.config.crosshair_fullscreen = false;
                }
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                wm.split_focused(SplitDirection::Horizontal)?;
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                wm.split_focused(SplitDirection::Vertical)?;
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                wm.close_focused_pane();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('z') | KeyCode::Char('Z') => {
                wm.toggle_zoom();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                wm.toggle_layout_mode();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Tab => {
                wm.shuffle_deck_next();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::BackTab => {
                wm.shuffle_deck_prev();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char(d @ '1'..='9') => {
                let id = (d as u8 - b'0') as usize;
                wm.focus_and_raise(id);
                return Ok(Some(AppAction::Handled));
            }
            _ => {}
        }
    }

    Ok(None)
}

/// Handles single-pane function keys (F3 Hints) and Alt-chord shortcuts in CommandPrompt mode.
pub(super) fn handle_pane_alt_or_fkey_shortcut(
    pane: &mut CompositePane,
    key: KeyEvent,
) -> Result<Option<AppAction>> {
    // F3: QuickSelect 3270 Field Hints (non-colliding function key in CommandPrompt mode)
    if key.code == KeyCode::F(3) {
        if pane.hints_mode().active {
            pane.exit_hints_mode();
        } else {
            pane.enter_hints_mode();
        }
        return Ok(Some(AppAction::Handled));
    }

    // Intra-pane prompt positioning, height resize, search, copy mode, Readline word navigation, and OSC 133 command-block jump hotkeys
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Char('t') | KeyCode::Char('T') => {
                pane.set_prompt_anchor(PromptAnchor::Top);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('B') => {
                pane.set_prompt_anchor(PromptAnchor::Bottom);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('b') => {
                // On macOS, Option+Left emits ESC b (`Alt+b`), and in Readline `Alt+b` is backward-word.
                // When editing text in the command prompt, move word left; when empty, pin prompt to bottom.
                if !pane.prompt().input.is_empty() {
                    pane.prompt_mut().move_word_left();
                } else {
                    pane.set_prompt_anchor(PromptAnchor::Bottom);
                }
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                pane.toggle_split_prompt();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if pane.copy_mode().active {
                    pane.exit_copy_mode();
                } else {
                    pane.enter_copy_mode();
                }
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('F') => {
                if pane.hints_mode().active {
                    pane.exit_hints_mode();
                } else {
                    pane.enter_hints_mode();
                }
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('f') => {
                // On macOS, Option+Right emits ESC f (`Alt+f`), and in Readline `Alt+f` is forward-word.
                // When editing text in the command prompt, move word right; when empty, open 3270 Field Hints.
                if !pane.prompt().input.is_empty() {
                    pane.prompt_mut().move_word_right();
                } else if pane.hints_mode().active {
                    pane.exit_hints_mode();
                } else {
                    pane.enter_hints_mode();
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
            KeyCode::Char('l') | KeyCode::Char('L') => {
                pane.toggle_read_only();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('/') => {
                if pane.is_prompt_visible() {
                    pane.prompt_mut().set_text(":search ");
                }
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('n') => {
                pane.next_canvas_search_match();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('N') => {
                pane.prev_canvas_search_match();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Up => {
                let h = pane.vt().size().0 + 1;
                pane.move_prompt_vertical(-1, h);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Down => {
                let h = pane.vt().size().0 + 1;
                pane.move_prompt_vertical(1, h);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                pane.resize_prompt_height(1);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('-') => {
                pane.resize_prompt_height(-1);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char(']') => {
                pane.resize_prompt_width(4);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('[') => {
                pane.resize_prompt_width(-4);
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                pane.prompt_mut().toggle_history_popup();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::PageUp => {
                pane.vt_mut().jump_to_prev_command_block();
                return Ok(Some(AppAction::Handled));
            }
            KeyCode::PageDown => {
                pane.vt_mut().jump_to_next_command_block();
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

    Ok(None)
}
