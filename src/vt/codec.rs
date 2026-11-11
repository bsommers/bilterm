/// Sanitizes pasted text by replacing dangerous C0/DEL control bytes (`NUL`, `ESC`, `DEL`,
/// `VINTR 0x03`, `VQUIT 0x1C`, `VSUSP 0x1A`, `VWERASE 0x17`, `VLNEXT 0x16`, etc.) with spaces
/// (`FND-01`, modelled after Ghostty's `src/input/paste.zig`).
pub fn sanitize_paste_bytes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let code = ch as u32;
        if matches!(
            code,
            0x00
                | 0x03
                | 0x04
                | 0x05
                | 0x08
                | 0x0F
                | 0x11
                | 0x12
                | 0x13
                | 0x15
                | 0x16
                | 0x17
                | 0x1A
                | 0x1B
                | 0x1C
                | 0x7F
        ) {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

/// Returns `true` when pasting `raw` into a raw PTY presents an immediate execution or
/// bracketed-paste escape breakout hazard (`FND-01`).
pub fn is_unsafe_paste(raw: &str, bracketed_paste_active: bool) -> bool {
    if raw.contains("\x1b[201~") {
        return true;
    }
    if !bracketed_paste_active && (raw.contains('\n') || raw.contains('\r')) {
        return true;
    }
    false
}

pub(super) fn hex_encode_ascii(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push_str(&format!("{b:02X}"));
    }
    out
}

pub(super) fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub(super) fn hex_decode_ascii(hex: &str) -> Option<String> {
    // Untrusted input: work on bytes (a `str` slice panics on a char boundary) and accept
    // only real hex digits (`from_str_radix` would also take a leading `+`).
    let hex = hex.as_bytes();
    if !hex.len().is_multiple_of(2) || hex.is_empty() {
        return None;
    }
    let nibble = |b: u8| (b as char).to_digit(16).filter(|_| b.is_ascii_hexdigit());
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for pair in hex.chunks_exact(2) {
        bytes.push((nibble(pair[0])? << 4 | nibble(pair[1])?) as u8);
    }
    String::from_utf8(bytes).ok()
}

/// Returns the longest prefix of `s` that is at most `max_bytes` long and ends on a char boundary.
pub(super) fn truncate_on_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Longest run of CSI parameter bytes (after the first parameter) held while waiting for the
/// final byte; longer sequences are passed through (small count) or dropped (oversized count).
const MAX_HELD_CSI_PARAM_BYTES: usize = 256;

#[derive(Clone, Debug, Default)]
enum CsiClampState {
    #[default]
    Ground,
    /// `ESC` seen and withheld.
    Esc,
    /// `ESC [` seen; the whole sequence is withheld until its final byte.
    Csi,
    /// Inside a CSI whose remaining bytes are forwarded (`true`) or swallowed (`false`).
    Pass(bool),
}

/// Streaming filter in front of `vt100-ctt`, which loops `count` times without clamping for
/// ICH / IL / SD (`CSI 65535 @` costs tens of thousands of `Vec` inserts). Clamps the count of
/// `CSI Pn @ L M P S T X` to the grid size. State survives chunk boundaries, and `ESC` / `ESC [`
/// are withheld until resolved so a split or control-interleaved sequence cannot slip through.
#[derive(Clone, Debug, Default)]
pub(super) struct CsiCountClamp {
    state: CsiClampState,
    first_param: u16,
    has_digits: bool,
    rest: Vec<u8>,
}

impl CsiCountClamp {
    /// Drops any withheld partial sequence (the backend parser was fed out-of-band bytes).
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    fn emit_held(&mut self, out: &mut Vec<u8>, first_param: u16) {
        out.extend_from_slice(b"\x1b[");
        if self.has_digits {
            out.extend_from_slice(first_param.to_string().as_bytes());
        }
        out.append(&mut self.rest);
    }

    /// Returns `bytes` with oversized counts clamped to `limit` (the larger grid dimension).
    pub(super) fn filter<'a>(&mut self, bytes: &'a [u8], limit: u16) -> std::borrow::Cow<'a, [u8]> {
        if matches!(self.state, CsiClampState::Ground) && !bytes.contains(&0x1b) {
            return std::borrow::Cow::Borrowed(bytes);
        }
        let mut out = Vec::with_capacity(bytes.len());
        for &b in bytes {
            match self.state {
                CsiClampState::Ground => {
                    if b == 0x1b {
                        self.state = CsiClampState::Esc;
                    } else {
                        out.push(b);
                    }
                }
                // The backend ignores DEL and non-ASCII bytes inside ESC / CSI states.
                CsiClampState::Esc | CsiClampState::Csi if b >= 0x7f => {}
                CsiClampState::Esc => match b {
                    0x1b => {}
                    0x18 | 0x1a => {
                        out.push(b);
                        self.state = CsiClampState::Ground;
                    }
                    0x00..=0x1f => out.push(b),
                    b'[' => {
                        self.first_param = 0;
                        self.has_digits = false;
                        self.rest.clear();
                        self.state = CsiClampState::Csi;
                    }
                    _ => {
                        out.extend_from_slice(&[0x1b, b]);
                        self.state = CsiClampState::Ground;
                    }
                },
                CsiClampState::Csi => match b {
                    // An aborted sequence is discarded by the backend as well.
                    0x1b => self.state = CsiClampState::Esc,
                    0x18 | 0x1a => {
                        out.push(b);
                        self.state = CsiClampState::Ground;
                    }
                    0x00..=0x1f => out.push(b),
                    b'0'..=b'9' if self.rest.is_empty() => {
                        self.has_digits = true;
                        self.first_param = self
                            .first_param
                            .saturating_mul(10)
                            .saturating_add(u16::from(b - b'0'));
                    }
                    0x40..=0x7e => {
                        let plain = !self
                            .rest
                            .iter()
                            .any(|c| matches!(c, 0x20..=0x2f | 0x3c..=0x3f));
                        let count = if plain && matches!(b, b'@' | b'L' | b'M' | b'P' | b'S' | b'T' | b'X') {
                            self.first_param.min(limit)
                        } else {
                            self.first_param
                        };
                        self.emit_held(&mut out, count);
                        out.push(b);
                        self.state = CsiClampState::Ground;
                    }
                    _ => {
                        let private = !self.has_digits && self.rest.is_empty() && b >= 0x3c;
                        self.rest.push(b);
                        if private || self.rest.len() > MAX_HELD_CSI_PARAM_BYTES {
                            let forward = private || self.first_param <= limit;
                            if forward {
                                let first_param = self.first_param;
                                self.emit_held(&mut out, first_param);
                            }
                            self.rest.clear();
                            self.state = CsiClampState::Pass(forward);
                        }
                    }
                },
                CsiClampState::Pass(forward) => match b {
                    0x1b => self.state = CsiClampState::Esc,
                    0x18 | 0x1a => {
                        out.push(b);
                        self.state = CsiClampState::Ground;
                    }
                    0x00..=0x1f => out.push(b),
                    _ => {
                        if forward {
                            out.push(b);
                        }
                        if (0x40..=0x7e).contains(&b) {
                            self.state = CsiClampState::Ground;
                        }
                    }
                },
            }
        }
        std::borrow::Cow::Owned(out)
    }
}

/// Encodes arbitrary UTF-8 text into a standard `OSC 52` clipboard escape sequence (`\x1b]52;c;<base64>\x07`, `PTY-04`).
pub fn encode_osc52_clipboard(text: &str) -> String {
    let encoded = base64_encode(text.as_bytes());
    format!("\x1b]52;c;{encoded}\x07")
}

/// Upper bound on the base64 payload of an `OSC 52` clipboard write forwarded to the host terminal (1 MiB).
pub const MAX_OSC52_PAYLOAD_BYTES: usize = 1024 * 1024;

/// Returns `true` when `body` (the OSC text between `ESC ]` and its BEL/ST terminator) is a
/// well-formed `OSC 52` clipboard *write* that is safe to forward to the host terminal:
/// `52;<selection>;<base64>` with selection chars in `[cpqs0-7]`, only base64 alphabet characters,
/// and a payload no larger than [`MAX_OSC52_PAYLOAD_BYTES`]. Clipboard read queries (`?`) are rejected.
pub fn is_forwardable_osc52_write(body: &str) -> bool {
    let Some((selection, payload)) = body
        .strip_prefix("52;")
        .and_then(|rest| rest.split_once(';'))
    else {
        return false;
    };
    selection
        .bytes()
        .all(|b| matches!(b, b'c' | b'p' | b'q' | b's' | b'0'..=b'7'))
        && payload.len() <= MAX_OSC52_PAYLOAD_BYTES
        && payload
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

/// Pure-Rust RFC 4648 Base64 encoder for `OSC 52` clipboard payloads.
pub fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Pure-Rust RFC 4648 Base64 decoder for Kitty Graphics (`APC G`) payloads (`GFX-01`).
pub fn base64_decode(input: &str) -> Vec<u8> {
    fn val(b: u8) -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = input
        .bytes()
        .filter(|&b| !b.is_ascii_whitespace() && b != b'=')
        .filter_map(val)
        .collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        if chunk.len() >= 2 {
            out.push((chunk[0] << 2) | (chunk[1] >> 4));
        }
        if chunk.len() >= 3 {
            out.push((chunk[1] << 4) | (chunk[2] >> 2));
        }
        if chunk.len() >= 4 {
            out.push((chunk[2] << 6) | chunk[3]);
        }
    }
    out
}

/// Detects raw binary noise chunks (such as `tail -f /dev/random` or `cat /dev/urandom`) by
/// checking for invalid UTF-8 combined with raw binary C0 control bytes (`0x00..=0x06`, `0x0E..=0x1A`, `0x1C..=0x1F`).
pub(super) fn is_binary_noise_chunk(bytes: &[u8]) -> bool {
    if bytes.len() < 8 || std::str::from_utf8(bytes).is_ok() {
        return false;
    }
    let binary_ctrl_count = bytes
        .iter()
        .filter(|&&b| matches!(b, 0x00..=0x06 | 0x0E..=0x1A | 0x1C..=0x1F))
        .count();
    binary_ctrl_count >= 2
}

/// Converts a raw binary stream chunk into safe printable ASCII + `\r\n` line breaks so
/// `tail -f /dev/random` scrolls smoothly without triggering bogus VT100 escape sequences.
pub(super) fn sanitize_binary_noise_chunk(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 32);
    for &b in bytes {
        match b {
            b'\n' => {
                out.push(b'\r');
                out.push(b'\n');
            }
            b'\r' => {}
            0x20..=0x7E => out.push(b),
            _ => {
                // Map non-printable/binary bytes to deterministic visible ASCII characters
                out.push(b'!' + (b % 94));
            }
        }
    }
    out
}

/// Sanitises untrusted display text (pane titles set by PTY children via OSC 0/2) before layout.
///
/// Tab/CR/LF become one space; other C0 controls, DEL, C1 controls (U+0080-U+009F), bidi
/// embedding/override/isolate controls (U+202A-U+202E, U+2066-U+2069), LRM/RLM (U+200E/F), the
/// BOM/zero-width no-break space (U+FEFF), zero-width and invisible format characters
/// (U+200B-U+200D, U+2060-U+2064), the Arabic letter mark (U+061C), line/paragraph separators
/// (U+2028/9) and the Unicode tag block (U+E0000-U+E007F) are dropped. The result has at most `max_chars` chars;
/// when the text is cut, the last char is `…`.
pub fn sanitize_display(raw: &str, max_chars: usize) -> String {
    let cleaned: Vec<char> = raw
        .chars()
        .filter_map(|ch| match ch {
            '\t' | '\r' | '\n' => Some(' '),
            '\u{0000}'..='\u{001F}'
            | '\u{007F}'..='\u{009F}'
            | '\u{061C}'
            | '\u{200B}'..='\u{200F}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{2060}'..='\u{2064}'
            | '\u{E0000}'..='\u{E007F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}' => None,
            other => Some(other),
        })
        .collect();
    if cleaned.len() <= max_chars {
        return cleaned.into_iter().collect();
    }
    if max_chars == 0 {
        return String::new();
    }
    let mut out: String = cleaned.into_iter().take(max_chars - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod sanitize_display_tests {
    use super::sanitize_display;

    #[test]
    fn sanitize_display_drops_c0_and_del() {
        assert_eq!(sanitize_display("a\u{0}b\u{1b}c\u{7}d\u{7f}e", 64), "abcde");
    }

    #[test]
    fn sanitize_display_maps_whitespace_controls_to_space() {
        assert_eq!(sanitize_display("a\tb\r\nc", 64), "a b  c");
    }

    #[test]
    fn sanitize_display_drops_c1_controls() {
        assert_eq!(sanitize_display("x\u{80}y\u{9b}z\u{9f}", 64), "xyz");
    }

    #[test]
    fn sanitize_display_drops_bidi_and_zero_width_controls() {
        let raw = "a\u{202A}\u{202B}\u{202C}\u{202D}\u{202E}b\u{2066}\u{2067}\u{2068}\u{2069}c\u{200E}\u{200F}d\u{FEFF}e";
        assert_eq!(sanitize_display(raw, 64), "abcde");
    }

    #[test]
    fn sanitize_display_drops_invisible_format_characters() {
        let raw = "a\u{200B}\u{200C}\u{200D}b\u{2060}\u{2061}\u{2064}c\u{061C}d\u{2028}\u{2029}e\u{E0041}\u{E007F}f";
        assert_eq!(sanitize_display(raw, 64), "abcdef");
    }

    #[test]
    fn sanitize_display_keeps_printable_unicode() {
        assert_eq!(sanitize_display("héllo 世界 ✓", 64), "héllo 世界 ✓");
    }

    #[test]
    fn sanitize_display_caps_length_with_ellipsis() {
        let out = sanitize_display(&"x".repeat(500), 64);
        assert_eq!(out.chars().count(), 64);
        assert!(out.ends_with('…'));
        assert_eq!(sanitize_display("abc", 3), "abc");
        assert_eq!(sanitize_display("abcd", 3), "ab…");
        assert_eq!(sanitize_display("abc", 0), "");
    }
}
