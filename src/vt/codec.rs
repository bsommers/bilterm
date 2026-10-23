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
    if !hex.len().is_multiple_of(2) || hex.is_empty() {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let b = u8::from_str_radix(&hex[i..i + 2], 16).ok()?;
        bytes.push(b);
    }
    String::from_utf8(bytes).ok()
}

/// Encodes arbitrary UTF-8 text into a standard `OSC 52` clipboard escape sequence (`\x1b]52;c;<base64>\x07`, `PTY-04`).
pub fn encode_osc52_clipboard(text: &str) -> String {
    let encoded = base64_encode(text.as_bytes());
    format!("\x1b]52;c;{encoded}\x07")
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
