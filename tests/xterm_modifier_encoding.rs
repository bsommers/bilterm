//! xterm modifier encoding for cursor, editing, and function keys sent to the PTY
//! (`CSI 1;m X` / `CSI n;m ~`, `m = 1 + Shift + 2*Alt + 4*Ctrl`), plus byte-identical unmodified encodings.

use bilterm::input::encode_key_to_pty_bytes_full;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn enc(code: KeyCode, mods: KeyModifiers, app_cursor: bool) -> Vec<u8> {
    encode_key_to_pty_bytes_full(KeyEvent::new(code, mods), app_cursor, false, 0)
}

/// (key, CSI number, final byte) for every special key with an xterm modified form.
const SPECIAL_KEYS: &[(KeyCode, u8, char)] = &[
    (KeyCode::Up, 1, 'A'),
    (KeyCode::Down, 1, 'B'),
    (KeyCode::Right, 1, 'C'),
    (KeyCode::Left, 1, 'D'),
    (KeyCode::Home, 1, 'H'),
    (KeyCode::End, 1, 'F'),
    (KeyCode::F(1), 1, 'P'),
    (KeyCode::F(2), 1, 'Q'),
    (KeyCode::F(3), 1, 'R'),
    (KeyCode::F(4), 1, 'S'),
    (KeyCode::Insert, 2, '~'),
    (KeyCode::Delete, 3, '~'),
    (KeyCode::PageUp, 5, '~'),
    (KeyCode::PageDown, 6, '~'),
    (KeyCode::F(5), 15, '~'),
    (KeyCode::F(6), 17, '~'),
    (KeyCode::F(7), 18, '~'),
    (KeyCode::F(8), 19, '~'),
    (KeyCode::F(9), 20, '~'),
    (KeyCode::F(10), 21, '~'),
    (KeyCode::F(11), 23, '~'),
    (KeyCode::F(12), 24, '~'),
];

const MODIFIER_COMBOS: &[(KeyModifiers, u8)] = &[
    (KeyModifiers::SHIFT, 2),
    (KeyModifiers::ALT, 3),
    (KeyModifiers::SHIFT.union(KeyModifiers::ALT), 4),
    (KeyModifiers::CONTROL, 5),
    (KeyModifiers::CONTROL.union(KeyModifiers::SHIFT), 6),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT), 7),
    (KeyModifiers::CONTROL.union(KeyModifiers::ALT).union(KeyModifiers::SHIFT), 8),
];

#[test]
fn every_special_key_times_every_modifier_combo_uses_xterm_csi_encoding() {
    for &(code, n, fin) in SPECIAL_KEYS {
        for &(mods, m) in MODIFIER_COMBOS {
            let expected = format!("\x1b[{n};{m}{fin}").into_bytes();
            for app_cursor in [false, true] {
                assert_eq!(
                    enc(code, mods, app_cursor),
                    expected,
                    "{code:?} + {mods:?} (DECCKM={app_cursor})"
                );
            }
        }
    }
}

#[test]
fn common_word_jump_and_selection_chords() {
    let ctrl = KeyModifiers::CONTROL;
    assert_eq!(enc(KeyCode::Left, ctrl, false), b"\x1b[1;5D");
    assert_eq!(enc(KeyCode::Right, ctrl, true), b"\x1b[1;5C");
    assert_eq!(enc(KeyCode::F(5), KeyModifiers::SHIFT, false), b"\x1b[15;2~");
    assert_eq!(enc(KeyCode::Home, KeyModifiers::ALT, false), b"\x1b[1;3H");
    assert_eq!(enc(KeyCode::Up, ctrl | KeyModifiers::SHIFT, false), b"\x1b[1;6A");
    assert_eq!(enc(KeyCode::Delete, ctrl, false), b"\x1b[3;5~");
}

#[test]
fn unmodified_special_keys_are_byte_identical_including_decckm() {
    let none = KeyModifiers::NONE;
    let table: &[(KeyCode, &[u8], &[u8])] = &[
        (KeyCode::Up, b"\x1b[A", b"\x1bOA"),
        (KeyCode::Down, b"\x1b[B", b"\x1bOB"),
        (KeyCode::Right, b"\x1b[C", b"\x1bOC"),
        (KeyCode::Left, b"\x1b[D", b"\x1bOD"),
        (KeyCode::Home, b"\x1b[H", b"\x1bOH"),
        (KeyCode::End, b"\x1b[F", b"\x1bOF"),
        (KeyCode::Delete, b"\x1b[3~", b"\x1b[3~"),
        (KeyCode::PageUp, b"\x1b[5~", b"\x1b[5~"),
        (KeyCode::PageDown, b"\x1b[6~", b"\x1b[6~"),
        (KeyCode::F(1), b"\x1bOP", b"\x1bOP"),
        (KeyCode::F(2), b"\x1bOQ", b"\x1bOQ"),
        (KeyCode::F(3), b"\x1bOR", b"\x1bOR"),
        (KeyCode::F(4), b"\x1bOS", b"\x1bOS"),
        (KeyCode::F(5), b"\x1b[15~", b"\x1b[15~"),
        (KeyCode::F(6), b"\x1b[17~", b"\x1b[17~"),
        (KeyCode::F(7), b"\x1b[18~", b"\x1b[18~"),
        (KeyCode::F(8), b"\x1b[19~", b"\x1b[19~"),
        (KeyCode::F(9), b"\x1b[20~", b"\x1b[20~"),
        (KeyCode::F(10), b"\x1b[21~", b"\x1b[21~"),
        (KeyCode::F(11), b"\x1b[23~", b"\x1b[23~"),
        (KeyCode::F(12), b"\x1b[24~", b"\x1b[24~"),
        (KeyCode::Enter, b"\r", b"\r"),
        (KeyCode::Tab, b"\t", b"\t"),
        (KeyCode::BackTab, b"\x1b[Z", b"\x1b[Z"),
        (KeyCode::Esc, b"\x1b", b"\x1b"),
        (KeyCode::Backspace, b"\x7f", b"\x7f"),
    ];
    for &(code, normal, app) in table {
        assert_eq!(enc(code, none, false), normal, "{code:?} normal");
        assert_eq!(enc(code, none, true), app, "{code:?} DECCKM");
    }
}

#[test]
fn unmodified_insert_sends_vt220_insert_sequence() {
    // Insert previously encoded to nothing, so the key was silently dropped.
    assert_eq!(enc(KeyCode::Insert, KeyModifiers::NONE, false), b"\x1b[2~");
    assert_eq!(enc(KeyCode::Insert, KeyModifiers::NONE, true), b"\x1b[2~");
}

#[test]
fn printable_ctrl_and_alt_chars_are_unchanged() {
    assert_eq!(enc(KeyCode::Char('a'), KeyModifiers::CONTROL, false), [0x01]);
    assert_eq!(enc(KeyCode::Char('C'), KeyModifiers::CONTROL | KeyModifiers::SHIFT, false), [0x03]);
    assert_eq!(enc(KeyCode::Char('b'), KeyModifiers::ALT, false), b"\x1bb");
    assert_eq!(enc(KeyCode::Char('X'), KeyModifiers::SHIFT, false), b"X");
    assert_eq!(enc(KeyCode::Char('é'), KeyModifiers::NONE, false), "é".as_bytes());
    assert_eq!(enc(KeyCode::BackTab, KeyModifiers::SHIFT, false), b"\x1b[Z");
}

#[test]
fn kitty_mode_special_keys_also_carry_modifiers() {
    let bytes = encode_key_to_pty_bytes_full(
        KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL),
        false,
        false,
        0x01,
    );
    assert_eq!(bytes, b"\x1b[1;5D");
}
