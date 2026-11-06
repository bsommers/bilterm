//! Splits a transport byte stream into raw terminal text and tmux control lines (Phase 51.1,
//! plan 03; frozen rules in 51.1-CONTRACT.md section 4).
//!
//! Before control mode every byte is `Raw` (ssh banners, host-key and password prompts, errors).
//! Control mode starts at the DCS sentinel `ESC P 1000 p` (which may be split across chunks) or,
//! as a fallback, at a raw line starting with `%begin ` or `%output `. In control mode bytes are
//! accumulated per `\n`; `ESC \` ends it. Memory is bounded: a possible escape/prefix match holds
//! back at most 8 bytes, a control line at most `CONTROL_LINE_MAX` bytes.

use crate::tmux::link::{CONTROL_LINE_MAX, DIAG_LINE_CAP};
use crate::vt::sanitize_display;

const ESC: u8 = 0x1b;
/// `ESC P 1000 p`, printed by tmux when a `-CC` client starts.
const SENTINEL: &[u8] = b"\x1bP1000p";
const FALLBACK_BEGIN: &[u8] = b"%begin ";
const FALLBACK_OUTPUT: &[u8] = b"%output ";

/// One piece of a split transport stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SplitOut {
    /// Ordinary terminal bytes (ssh banner, prompts, errors).
    Raw(Vec<u8>),
    /// The `ESC P 1000 p` sentinel (or the fallback `%begin`/`%output` line) was seen.
    ControlStart,
    /// One control line without its line terminator.
    Line(String),
    /// `ESC \` ended control mode.
    ControlEnd,
    /// A control line longer than `CONTROL_LINE_MAX` was discarded (its length).
    Dropped(usize),
}

/// Incremental splitter between ssh text and `tmux -CC` control lines.
#[derive(Clone, Debug)]
pub struct ControlLineSplitter {
    control: bool,
    /// Raw mode: held-back bytes of a possible sentinel / fallback prefix.
    /// Control mode: the current line.
    buf: Vec<u8>,
    /// Raw mode: the next byte starts a line.
    at_line_start: bool,
    /// Control mode: the current line passed the cap; skip to the next newline.
    discarding: bool,
    /// Control mode: length of the line so far (counts discarded bytes too).
    line_len: usize,
    /// Control mode: the previous byte was `ESC`.
    esc_pending: bool,
}

impl Default for ControlLineSplitter {
    fn default() -> Self {
        Self {
            control: false,
            buf: Vec::new(),
            at_line_start: true,
            discarding: false,
            line_len: 0,
            esc_pending: false,
        }
    }
}

impl ControlLineSplitter {
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` between `ControlStart` and `ControlEnd`.
    pub fn in_control(&self) -> bool {
        self.control
    }

    /// Bytes currently held (escape prefix or partial line); bounded by `CONTROL_LINE_MAX + 1`.
    pub fn buffered_len(&self) -> usize {
        self.buf.len()
    }

    /// Feeds one chunk and returns the pieces it completes.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<SplitOut> {
        let mut out = Vec::new();
        let mut raw: Vec<u8> = Vec::new();
        for &b in bytes {
            if self.control {
                self.push_control_byte(b, &mut out);
            } else {
                self.push_raw_byte(b, &mut raw, &mut out);
            }
        }
        if !raw.is_empty() {
            out.push(SplitOut::Raw(raw));
        }
        out
    }

    /// Emits any buffered partial line or held-back bytes (end of stream).
    pub fn flush_partial(&mut self) -> Vec<SplitOut> {
        let mut out = Vec::new();
        if self.control {
            self.esc_pending = false;
            if self.discarding {
                out.push(SplitOut::Dropped(self.line_len));
            } else if !self.buf.is_empty() {
                out.push(SplitOut::Line(line_text(&self.buf)));
            }
            self.reset_line();
        } else if !self.buf.is_empty() {
            let held = std::mem::take(&mut self.buf);
            self.at_line_start = held.last() == Some(&b'\n');
            out.push(SplitOut::Raw(held));
        }
        out
    }

    fn reset_line(&mut self) {
        self.buf.clear();
        self.discarding = false;
        self.line_len = 0;
    }

    fn emit_raw_byte(&mut self, b: u8, raw: &mut Vec<u8>) {
        raw.push(b);
        self.at_line_start = b == b'\n';
    }

    fn start_control(&mut self, raw: &mut Vec<u8>, out: &mut Vec<SplitOut>) {
        if !raw.is_empty() {
            out.push(SplitOut::Raw(std::mem::take(raw)));
        }
        out.push(SplitOut::ControlStart);
        self.control = true;
        self.esc_pending = false;
        self.discarding = false;
        self.line_len = self.buf.len();
    }

    fn push_raw_byte(&mut self, b: u8, raw: &mut Vec<u8>, out: &mut Vec<SplitOut>) {
        if self.buf.is_empty() {
            if b == ESC || (self.at_line_start && b == b'%') {
                self.buf.push(b);
            } else {
                self.emit_raw_byte(b, raw);
            }
            return;
        }
        self.buf.push(b);
        if self.buf[0] == ESC {
            if self.buf.as_slice() == SENTINEL {
                self.buf.clear();
                self.start_control(raw, out);
                return;
            }
            if SENTINEL.starts_with(&self.buf) {
                return;
            }
        } else {
            for pat in [FALLBACK_BEGIN, FALLBACK_OUTPUT] {
                if self.buf.as_slice() == pat {
                    // Keep the matched prefix as the start of the first control line.
                    self.start_control(raw, out);
                    return;
                }
            }
            if FALLBACK_BEGIN.starts_with(&self.buf) || FALLBACK_OUTPUT.starts_with(&self.buf) {
                return;
            }
        }
        // Mismatch: release everything but the new byte, then treat that byte afresh.
        let last = self.buf.pop().unwrap_or(b);
        let held = std::mem::take(&mut self.buf);
        for h in held {
            self.emit_raw_byte(h, raw);
        }
        self.push_raw_byte(last, raw, out);
    }

    fn push_control_byte(&mut self, b: u8, out: &mut Vec<SplitOut>) {
        if self.esc_pending {
            self.esc_pending = false;
            if b == b'\\' {
                // ST: a pending partial line is delivered before the end marker.
                if self.discarding {
                    out.push(SplitOut::Dropped(self.line_len));
                } else if !self.buf.is_empty() {
                    out.push(SplitOut::Line(line_text(&self.buf)));
                }
                self.reset_line();
                self.control = false;
                self.at_line_start = true;
                out.push(SplitOut::ControlEnd);
                return;
            }
            self.add_line_byte(ESC);
        }
        match b {
            b'\n' => {
                if self.discarding {
                    out.push(SplitOut::Dropped(self.line_len));
                } else {
                    out.push(SplitOut::Line(line_text(&self.buf)));
                }
                self.reset_line();
            }
            ESC => self.esc_pending = true,
            _ => self.add_line_byte(b),
        }
    }

    fn add_line_byte(&mut self, b: u8) {
        self.line_len += 1;
        if self.discarding {
            return;
        }
        if self.buf.len() >= CONTROL_LINE_MAX {
            self.discarding = true;
            self.buf = Vec::new();
            return;
        }
        self.buf.push(b);
    }
}

/// Line text without any trailing `\r`/`\n`; invalid UTF-8 is replaced.
fn line_text(bytes: &[u8]) -> String {
    let mut end = bytes.len();
    while end > 0 && matches!(bytes[end - 1], b'\r' | b'\n') {
        end -= 1;
    }
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Sanitised, non-empty diagnostic lines from raw transport text.
pub fn diag_lines(raw: &[u8]) -> Vec<String> {
    let stripped = strip_csi(raw);
    stripped
        .split(|b| matches!(b, b'\r' | b'\n'))
        .map(|part| sanitize_display(&String::from_utf8_lossy(part), DIAG_LINE_CAP))
        .filter(|line| !line.trim().is_empty())
        .collect()
}

/// Removes `ESC [ params final` sequences so colour codes do not leave `[31m` residue.
fn strip_csi(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == ESC && raw.get(i + 1) == Some(&b'[') {
            i += 2;
            while i < raw.len() && !(0x40..=0x7e).contains(&raw[i]) {
                i += 1;
            }
            i += 1;
            continue;
        }
        out.push(raw[i]);
        i += 1;
    }
    out
}
