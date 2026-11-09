//! Incremental escape / control / printable byte classifier (std only).

use super::cell::ChannelCounters;

/// Parser state carried across chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ClsState {
    #[default]
    Ground,
    /// After ESC.
    Esc,
    /// Inside a CSI sequence.
    Csi,
    /// Inside an OSC/DCS/APC/PM/SOS string.
    Str,
    /// ESC seen inside a string (awaiting `\`).
    StrEsc,
}

impl ClsState {
    fn to_u8(self) -> u8 {
        self as u8
    }
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Esc,
            2 => Self::Csi,
            3 => Self::Str,
            4 => Self::StrEsc,
            _ => Self::Ground,
        }
    }
}

/// Byte-class totals for one slice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassCounts {
    pub esc_bytes: u64,
    pub esc_seqs: u64,
    pub ctrl_bytes: u64,
    pub print_bytes: u64,
}

#[inline]
fn is_ctrl(b: u8) -> bool {
    b < 0x20 || b == 0x7f
}

/// Classifies `bytes`, advancing `state`. Pure.
pub fn classify_slice(state: &mut ClsState, bytes: &[u8]) -> ClassCounts {
    let mut c = ClassCounts::default();
    let mut i = 0;
    while i < bytes.len() {
        match *state {
            ClsState::Ground => {
                // Fast path: a run of printable bytes.
                let run = bytes[i..].iter().position(|&b| is_ctrl(b)).unwrap_or(bytes.len() - i);
                if run > 0 {
                    c.print_bytes += run as u64;
                    i += run;
                    continue;
                }
                if bytes[i] == 0x1b {
                    c.esc_bytes += 1;
                    c.esc_seqs += 1;
                    *state = ClsState::Esc;
                } else {
                    c.ctrl_bytes += 1;
                }
                i += 1;
            }
            ClsState::Esc => {
                let b = bytes[i];
                if b == 0x1b {
                    // ESC ESC: restart.
                    c.esc_bytes += 1;
                    c.esc_seqs += 1;
                } else {
                    c.esc_bytes += 1;
                    *state = match b {
                        b'[' => ClsState::Csi,
                        b']' | b'P' | b'X' | b'^' | b'_' => ClsState::Str,
                        0x20..=0x2f => ClsState::Esc, // intermediate byte
                        _ => ClsState::Ground,
                    };
                }
                i += 1;
            }
            ClsState::Csi => {
                let b = bytes[i];
                if b == 0x1b {
                    c.esc_bytes += 1;
                    c.esc_seqs += 1;
                    *state = ClsState::Esc;
                } else {
                    c.esc_bytes += 1;
                    if (0x40..=0x7e).contains(&b) {
                        *state = ClsState::Ground;
                    }
                }
                i += 1;
            }
            ClsState::Str => {
                let b = bytes[i];
                c.esc_bytes += 1;
                match b {
                    0x07 => *state = ClsState::Ground,
                    0x1b => *state = ClsState::StrEsc,
                    _ => {}
                }
                i += 1;
            }
            ClsState::StrEsc => {
                if bytes[i] == b'\\' {
                    c.esc_bytes += 1;
                    *state = ClsState::Ground;
                    i += 1;
                } else {
                    // Not ST: the ESC began a fresh sequence; reprocess this byte in Esc.
                    c.esc_seqs += 1;
                    *state = ClsState::Esc;
                }
            }
        }
    }
    c
}

/// Classifies `bytes` into `cell` when classification is enabled; persists state in the cell.
pub fn observe(cell: &ChannelCounters, bytes: &[u8]) {
    if !cell.classify_enabled() {
        return;
    }
    let mut st = ClsState::from_u8(cell.cls_load());
    let c = classify_slice(&mut st, bytes);
    cell.cls_store(st.to_u8());
    cell.add_class(&c);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic::cell::{F_CLASSIFY, F_ENABLED};

    const SAMPLE: &[u8] = b"hi\x1b[31mred\x1b[0m\r\n";

    #[test]
    fn counts_sample() {
        let mut st = ClsState::Ground;
        let c = classify_slice(&mut st, SAMPLE);
        assert_eq!((c.esc_bytes, c.esc_seqs, c.ctrl_bytes, c.print_bytes), (9, 2, 2, 5));
    }

    #[test]
    fn split_at_every_offset_same_totals() {
        for k in 0..=SAMPLE.len() {
            let cell = ChannelCounters::with_flags(F_ENABLED | F_CLASSIFY);
            observe(&cell, &SAMPLE[..k]);
            observe(&cell, &SAMPLE[k..]);
            let r = cell.read();
            assert_eq!((r.esc_bytes, r.esc_seqs, r.ctrl_bytes, r.print_bytes), (9, 2, 2, 5), "k={k}");
        }
    }

    #[test]
    fn osc_and_st_strings() {
        let mut st = ClsState::Ground;
        let c = classify_slice(&mut st, b"\x1b]0;title\x07");
        assert_eq!((c.esc_seqs, c.esc_bytes), (1, 10));
        assert_eq!(st, ClsState::Ground);
        let c = classify_slice(&mut st, b"\x1bPq#0\x1b\\x");
        assert_eq!((c.esc_seqs, c.esc_bytes, c.print_bytes), (1, 7, 1));
        assert_eq!(st, ClsState::Ground);
        // Unterminated stays in-state across chunks.
        let _ = classify_slice(&mut st, b"\x1b_apc data");
        assert_eq!(st, ClsState::Str);
        let _ = classify_slice(&mut st, b"more");
        assert_eq!(st, ClsState::Str);
        let _ = classify_slice(&mut st, b"\x07");
        assert_eq!(st, ClsState::Ground);
    }

    #[test]
    fn disabled_observe_is_noop() {
        let cell = ChannelCounters::with_flags(F_ENABLED);
        observe(&cell, SAMPLE);
        assert_eq!(cell.read().esc_bytes, 0);
    }
}
