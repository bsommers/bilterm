//! Splits a transport byte stream into raw terminal text and tmux control lines (Phase 51.1,
//! plan 03 fills the bodies; frozen rules in 51.1-CONTRACT.md section 4).
//!
//! Seam state (plan 01): everything is `Raw` and control mode is never entered.

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
#[derive(Clone, Debug, Default)]
pub struct ControlLineSplitter {}

impl ControlLineSplitter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one chunk and returns the pieces it completes.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<SplitOut> {
        if bytes.is_empty() {
            Vec::new()
        } else {
            vec![SplitOut::Raw(bytes.to_vec())]
        }
    }

    /// `true` between `ControlStart` and `ControlEnd`.
    pub fn in_control(&self) -> bool {
        false
    }

    /// Emits any buffered partial line (end of stream).
    pub fn flush_partial(&mut self) -> Vec<SplitOut> {
        Vec::new()
    }
}

/// Sanitised, non-empty diagnostic lines from raw transport text.
pub fn diag_lines(_raw: &[u8]) -> Vec<String> {
    Vec::new()
}
