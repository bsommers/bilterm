//! Byte-exact decoding of `tmux -CC` pane output and flow-control notifications.
//!
//! tmux octal-escapes only bytes below 0x20 and `\`; bytes >= 0x80 travel raw and tmux cuts
//! `%output` at arbitrary byte positions, so one UTF-8 character can be split across two lines.
//! Control lines travel through the link as `String`s, so [`ascii_safe_line`] re-escapes the high
//! bytes of an output line (a lossy UTF-8 conversion would turn each half into U+FFFD) and
//! [`Utf8Carry`] holds back an incomplete trailing sequence until the next chunk of that pane.
//!
//! With `refresh-client -f pause-after=N` tmux sends `%extended-output` instead of `%output` and
//! `%pause`/`%continue` when a pane falls behind; [`parse_flow_line`] reads those.

use super::{TmuxControlEvent, unescape_tmux_output};

/// Seconds a pane may lag before tmux pauses it (`refresh-client -f pause-after=N`).
pub const PAUSE_AFTER_SECS: u32 = 2;

const OUTPUT: &[u8] = b"%output ";
const EXTENDED: &[u8] = b"%extended-output ";

/// `refresh-client -f pause-after=N` (tmux 3.2+; older servers answer `%error`, which is ignored).
pub fn pause_after_command() -> String {
    format!("refresh-client -f pause-after={PAUSE_AFTER_SECS}\n")
}

/// `set-option destroy-unattached off` for the attached session: a remote `~/.tmux.conf` with
/// `destroy-unattached on` would otherwise destroy the session the moment the link drops.
pub fn keep_session_command() -> String {
    "set-option destroy-unattached off\n".to_string()
}

/// `refresh-client -A '%N:continue'`: resumes a paused pane. The argument must be quoted: tmux's
/// command parser rejects a bare leading `%` ("parse error: syntax error").
pub fn continue_pane_command(tmux_pane: usize) -> String {
    format!("refresh-client -A '%{tmux_pane}:continue'\n")
}

/// Turns one raw control line into a `String` without losing bytes: output lines get every byte
/// `>= 0x80` (and DEL) rewritten as `\ooo`; other lines are converted lossily (they are text).
pub fn ascii_safe_line(bytes: &[u8]) -> String {
    if !(bytes.starts_with(OUTPUT) || bytes.starts_with(EXTENDED)) || bytes.is_ascii() {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let mut out = String::with_capacity(bytes.len() + 16);
    for &b in bytes {
        if b < 0x80 {
            out.push(b as char);
        } else {
            out.push_str(&format!("\\{b:03o}"));
        }
    }
    out
}

fn pane_id(tok: &str) -> Option<usize> {
    tok.strip_prefix('%')?.parse().ok()
}

/// Parses `%extended-output %P AGE ... : DATA`, `%pause %P` and `%continue %P`.
pub fn parse_flow_line(line: &str) -> Option<TmuxControlEvent> {
    if let Some(rest) = line.strip_prefix("%extended-output ") {
        let (head, payload) = rest.split_once(" : ")?;
        let pane_id = pane_id(head.split(' ').next()?)?;
        return Some(TmuxControlEvent::Output {
            pane_id,
            data: unescape_tmux_output(payload),
        });
    }
    if let Some(rest) = line.strip_prefix("%pause ") {
        return Some(TmuxControlEvent::Pause { pane_id: pane_id(rest.trim())? });
    }
    if let Some(rest) = line.strip_prefix("%continue ") {
        return Some(TmuxControlEvent::Continue { pane_id: pane_id(rest.trim())? });
    }
    None
}

/// Length of an incomplete UTF-8 sequence at the end of `bytes` (0 when the tail is complete or
/// is not UTF-8 at all).
fn incomplete_tail(bytes: &[u8]) -> usize {
    let n = bytes.len();
    for back in 1..=n.min(3) {
        let b = bytes[n - back];
        if b & 0xC0 == 0x80 {
            continue;
        }
        let need = match b {
            0xC2..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF4 => 4,
            _ => return 0,
        };
        return if back < need { back } else { 0 };
    }
    0
}

/// Per-pane carry of an incomplete trailing UTF-8 sequence (at most 3 bytes).
#[derive(Clone, Debug, Default)]
pub struct Utf8Carry {
    held: Vec<u8>,
}

impl Utf8Carry {
    /// Prepends the bytes held from the previous chunk and holds back a new incomplete tail.
    pub fn feed(&mut self, data: &[u8]) -> Vec<u8> {
        let mut all = std::mem::take(&mut self.held);
        all.extend_from_slice(data);
        let cut = all.len() - incomplete_tail(&all);
        self.held = all.split_off(cut);
        all
    }

    /// Drops any held bytes (pane reset or closed).
    pub fn clear(&mut self) {
        self.held.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_bytes_survive_the_string_channel() {
        // "漢" = e6 bc a2, cut after the first byte the way tmux cuts `%output`.
        let a = ascii_safe_line(b"%output %1 x\xe6");
        let b = ascii_safe_line(b"%output %1 \xbc\xa2y");
        assert!(a.is_ascii() && b.is_ascii());
        let mut bytes = unescape_tmux_output(a.strip_prefix("%output %1 ").unwrap());
        bytes.extend(unescape_tmux_output(b.strip_prefix("%output %1 ").unwrap()));
        assert_eq!(String::from_utf8(bytes).unwrap(), "x漢y");
        // Non-output lines stay text.
        assert_eq!(ascii_safe_line("%window-renamed @1 漢".as_bytes()), "%window-renamed @1 漢");
    }

    #[test]
    fn carry_reassembles_split_characters() {
        let mut c = Utf8Carry::default();
        assert_eq!(c.feed(b"a\xf0\x9f"), b"a");
        assert_eq!(c.feed(b"\x98\x80b"), "😀b".as_bytes());
        // Invalid bytes are not held forever.
        assert_eq!(c.feed(b"\xff"), b"\xff");
        assert_eq!(c.feed(b"\x80\x80\x80"), b"\x80\x80\x80");
    }

    #[test]
    fn continue_command_is_quoted_for_the_tmux_parser() {
        assert_eq!(continue_pane_command(4), "refresh-client -A '%4:continue'\n");
        assert_eq!(pause_after_command(), "refresh-client -f pause-after=2\n");
    }

    #[test]
    fn flow_lines_parse_and_garbage_is_rejected() {
        assert_eq!(
            parse_flow_line("%extended-output %3 1234 : a\\033[mb"),
            Some(TmuxControlEvent::Output { pane_id: 3, data: b"a\x1b[mb".to_vec() })
        );
        assert_eq!(parse_flow_line("%pause %7"), Some(TmuxControlEvent::Pause { pane_id: 7 }));
        assert_eq!(parse_flow_line("%continue %7"), Some(TmuxControlEvent::Continue { pane_id: 7 }));
        for bad in ["%extended-output %3 12", "%pause 7", "%pause %x", "%continue", "%extended-output x 1 : y"] {
            assert_eq!(parse_flow_line(bad), None, "{bad}");
        }
    }
}
