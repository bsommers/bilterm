//! I/O pumps between panes and the tmux link (Phase 51.1, plan 13 fills the bodies).
//!
//! Seam state (plan 01): all three pumps are no-ops. Contract: 51.1-CONTRACT.md section 7
//! (`Input`, `Replies`, `Order inside tick_tmux_link`).

use crate::wm::WindowManager;

impl WindowManager {
    /// Converts typed bytes of tapped panes into `send-keys` (or raw ssh input while connecting).
    pub(crate) fn pump_tmux_input(&mut self, _now_ms: u64) {}

    /// Writes queued tmux commands to the transport and registers their reply kinds.
    pub(crate) fn pump_tmux_outbound(&mut self) {}

    /// Feeds correlated replies and drives the attach hydrate.
    pub(crate) fn pump_tmux_replies(&mut self, _now_ms: u64) {}
}
