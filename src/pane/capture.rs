//! Per-pane command capture outbox types (Phase 50.1, D-U2).
//!
//! 50.1-01 declares the shape; 50.1-05 / 50.1-10 fill the capture path in `CompositePane`.
//! Pane layer: imports nothing outside `std`/`crate::vt`.

use std::time::Instant;

/// Characters per stored command (`sanitize_display`).
pub const CMD_CAP: usize = 512;
/// Captured commands kept per pane between registry ticks.
pub const PANE_OUTBOX_CAP: usize = 64;

/// One submitted command waiting to be drained by `registry::capture::tick`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedCommand {
    /// Sanitised, `<= CMD_CAP` chars, never secret-looking.
    pub command: String,
    pub at: Instant,
    pub cwd: Option<String>,
    pub host: Option<String>,
    pub block_row: Option<usize>,
}

#[cfg(test)]
mod tests {
    use crate::pane::CompositePane;

    #[test]
    fn fresh_pane_outbox_is_empty_and_drain_resets() {
        let mut pane = CompositePane::new_headless(1, 10, 40);
        assert_eq!(pane.drain_captured(), (Vec::new(), 0));
        assert_eq!(pane.drain_captured(), (Vec::new(), 0));
    }
}
