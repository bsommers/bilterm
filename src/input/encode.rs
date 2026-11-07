use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};

use crate::pane::CompositePane;

/// Computes `DECSET ?1007` Mouse Alternate Scroll arrow-key bytes when scrolling inside an
/// Alternate Screen (`?1049`) program that has not enabled mouse tracking (`less`, `man`, `git log`, `FND-02`).
pub fn encode_alternate_scroll_wheel(pane: &CompositePane, kind: MouseEventKind) -> Option<Vec<u8>> {
    if !pane.vt().is_alternate_screen()
        || pane.vt().is_mouse_sgr_or_tracking()
        || !pane.vt().is_mouse_alternate_scroll()
    {
        return None;
    }
    let unit: &[u8] = match kind {
        MouseEventKind::ScrollUp => {
            if pane.vt().is_application_cursor() {
                b"\x1bOA"
            } else {
                b"\x1b[A"
            }
        }
        MouseEventKind::ScrollDown => {
            if pane.vt().is_application_cursor() {
                b"\x1bOB"
            } else {
                b"\x1b[B"
            }
        }
        _ => return None,
    };
    let mut out = Vec::with_capacity(unit.len() * 3);
    for _ in 0..3 {
        out.extend_from_slice(unit);
    }
    Some(out)
}

/// Encodes a `crossterm::event::KeyEvent` into standard VT100/xterm escape sequences for raw PTY passthrough.
pub fn encode_key_to_pty_bytes(key: KeyEvent) -> Vec<u8> {
    encode_key_to_pty_bytes_with_modes(key, false)
}

/// Encodes a `crossterm::event::KeyEvent` into VT100/VT220/xterm escape sequences, respecting `DECCKM` (`?1h`) application cursor mode.
pub fn encode_key_to_pty_bytes_with_modes(key: KeyEvent, application_cursor: bool) -> Vec<u8> {
    encode_key_to_pty_bytes_full(key, application_cursor, false, 0)
}

/// Encodes a `KeyEvent` into PTY bytes, respecting `DECCKM` (`?1h`), `DECBKM` (`?67h`), and
/// the Kitty Keyboard Protocol (`CSI u`, `FND-04`).
pub fn encode_key_to_pty_bytes_full(
    key: KeyEvent,
    application_cursor: bool,
    backarrow_bs: bool,
    kitty_flags: u8,
) -> Vec<u8> {
    // Kitty Keyboard Protocol disambiguate mode (flag bit 0x01 or higher)
    if kitty_flags > 0 {
        let mut mod_bits = 0u8;
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            mod_bits |= 1;
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            mod_bits |= 2;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            mod_bits |= 4;
        }
        let mod_param = mod_bits + 1;
        match key.code {
            KeyCode::Esc => {
                return if mod_bits == 0 {
                    b"\x1b[27u".to_vec()
                } else {
                    format!("\x1b[27;{mod_param}u").into_bytes()
                };
            }
            KeyCode::Enter if mod_bits > 0 || (kitty_flags & 0x08) != 0 => {
                return format!("\x1b[13;{mod_param}u").into_bytes();
            }
            KeyCode::Tab if mod_bits > 0 || (kitty_flags & 0x08) != 0 => {
                return format!("\x1b[9;{mod_param}u").into_bytes();
            }
            KeyCode::Backspace if mod_bits > 0 || (kitty_flags & 0x08) != 0 => {
                return format!("\x1b[127;{mod_param}u").into_bytes();
            }
            KeyCode::Char(c) if (mod_bits & !1) > 0 || (kitty_flags & 0x08) != 0 => {
                let codepoint = c.to_ascii_lowercase() as u32;
                return format!("\x1b[{codepoint};{mod_param}u").into_bytes();
            }
            _ => {}
        }
    }

    if key.modifiers.contains(KeyModifiers::CONTROL)
        && let KeyCode::Char(c) = key.code
    {
        let lower = c.to_ascii_lowercase();
        if lower.is_ascii_lowercase() {
            return vec![(lower as u8) - b'a' + 1];
        }
    }

    if let Some(m) = xterm_modifier_param(key.modifiers)
        && let Some(bytes) = encode_modified_special_key(key.code, m)
    {
        return bytes;
    }

    match key.code {
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            if key.modifiers.contains(KeyModifiers::ALT) {
                let mut out = vec![0x1b];
                out.extend_from_slice(s.as_bytes());
                out
            } else {
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => {
            if backarrow_bs {
                vec![0x08]
            } else {
                vec![0x7f]
            }
        }
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOA".to_vec()
            } else {
                b"\x1b[A".to_vec()
            }
        }
        KeyCode::Down => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOB".to_vec()
            } else {
                b"\x1b[B".to_vec()
            }
        }
        KeyCode::Right => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOC".to_vec()
            } else {
                b"\x1b[C".to_vec()
            }
        }
        KeyCode::Left => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOD".to_vec()
            } else {
                b"\x1b[D".to_vec()
            }
        }
        KeyCode::Home => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOH".to_vec()
            } else {
                b"\x1b[H".to_vec()
            }
        }
        KeyCode::End => {
            if application_cursor && key.modifiers.is_empty() {
                b"\x1bOF".to_vec()
            } else {
                b"\x1b[F".to_vec()
            }
        }
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::F(1) => b"\x1bOP".to_vec(),
        KeyCode::F(2) => b"\x1bOQ".to_vec(),
        KeyCode::F(3) => b"\x1bOR".to_vec(),
        KeyCode::F(4) => b"\x1bOS".to_vec(),
        KeyCode::F(5) => b"\x1b[15~".to_vec(),
        KeyCode::F(6) => b"\x1b[17~".to_vec(),
        KeyCode::F(7) => b"\x1b[18~".to_vec(),
        KeyCode::F(8) => b"\x1b[19~".to_vec(),
        KeyCode::F(9) => b"\x1b[20~".to_vec(),
        KeyCode::F(10) => b"\x1b[21~".to_vec(),
        KeyCode::F(11) => b"\x1b[23~".to_vec(),
        KeyCode::F(12) => b"\x1b[24~".to_vec(),
        _ => Vec::new(),
    }
}

/// xterm modifier parameter `m = 1 + Shift(1) + Alt(2) + Ctrl(4)`; `None` when none of them is held.
fn xterm_modifier_param(mods: KeyModifiers) -> Option<u8> {
    let mut m = 1u8;
    if mods.contains(KeyModifiers::SHIFT) {
        m += 1;
    }
    if mods.contains(KeyModifiers::ALT) {
        m += 2;
    }
    if mods.contains(KeyModifiers::CONTROL) {
        m += 4;
    }
    (m > 1).then_some(m)
}

/// Encodes a modified cursor / editing / function key the way xterm does by default:
/// `CSI 1;m {A,B,C,D,H,F,P,Q,R,S}` or `CSI n;m ~`. Always the `CSI` form, even under `DECCKM`
/// application cursor mode, because `ESC O` cannot carry a modifier parameter.
fn encode_modified_special_key(code: KeyCode, m: u8) -> Option<Vec<u8>> {
    let (n, final_byte) = match code {
        KeyCode::Up => (1, 'A'),
        KeyCode::Down => (1, 'B'),
        KeyCode::Right => (1, 'C'),
        KeyCode::Left => (1, 'D'),
        KeyCode::Home => (1, 'H'),
        KeyCode::End => (1, 'F'),
        KeyCode::F(1) => (1, 'P'),
        KeyCode::F(2) => (1, 'Q'),
        KeyCode::F(3) => (1, 'R'),
        KeyCode::F(4) => (1, 'S'),
        KeyCode::Insert => (2, '~'),
        KeyCode::Delete => (3, '~'),
        KeyCode::PageUp => (5, '~'),
        KeyCode::PageDown => (6, '~'),
        KeyCode::F(5) => (15, '~'),
        KeyCode::F(6) => (17, '~'),
        KeyCode::F(7) => (18, '~'),
        KeyCode::F(8) => (19, '~'),
        KeyCode::F(9) => (20, '~'),
        KeyCode::F(10) => (21, '~'),
        KeyCode::F(11) => (23, '~'),
        KeyCode::F(12) => (24, '~'),
        _ => return None,
    };
    Some(format!("\x1b[{n};{m}{final_byte}").into_bytes())
}
