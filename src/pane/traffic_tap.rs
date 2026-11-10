//! Pane-side traffic seam (Phase 64, `TMON-01`): cell access, classify-on-ingest, payload cell.
//!
//! A PTY-backed pane's bytes are counted once at the PTY chokepoints (`PtySession`); this module
//! only exposes the cell, classifies ingested bytes on the consumer thread when `CLASSIFY` is
//! set, and owns a lazily created *payload* cell for tmux-mapped (PTY-less) panes and for
//! metadata-only events (clipboard, media). Only `crate::traffic` is imported (pane DAG).

use super::CompositePane;
use crate::traffic::TrafficTap;

impl CompositePane {
    /// The pane's PTY traffic cell (`None` for PTY-less panes).
    pub fn traffic_cell(&self) -> Option<&TrafficTap> {
        self.transport.local_pty().map(|p| p.traffic_cell())
    }

    /// Lazily created payload cell (tmux `%output`, PTY-less input, clipboard/media events).
    pub fn payload_cell(&self) -> &TrafficTap {
        self.payload_traffic.get_or_init(crate::traffic::ChannelCounters::new)
    }

    /// The payload cell if it has been created (no allocation).
    pub fn existing_payload_cell(&self) -> Option<&TrafficTap> {
        self.payload_traffic.get()
    }

    /// Counts `n` decoded bytes received for this pane over a tmux link (not PTY bytes).
    pub fn count_payload_rx(&self, n: usize) {
        self.payload_cell().rx(n);
    }

    /// Published one-second `(rx, tx)` bytes/s for the OIA indicator.
    pub fn traffic_rates(&self) -> (u64, u64) {
        match self.traffic_cell() {
            Some(c) => c.rates(),
            None => self.payload_traffic.get().map_or((0, 0), |c| c.rates()),
        }
    }

    /// Whether the opt-in OIA rate indicator is enabled for this pane.
    pub fn oia_traffic_enabled(&self) -> bool {
        match self.traffic_cell() {
            Some(c) => c.oia_enabled(),
            None => self.payload_traffic.get().is_some_and(|c| c.oia_enabled()),
        }
    }

    /// Classifies ingested bytes (escape/control/printable) iff the rx source cell asks for it.
    pub(super) fn observe_ingest(&self, bytes: &[u8]) {
        // PTY-less panes classify only once a payload cell exists (no per-pane allocation here).
        let Some(cell) = self.traffic_cell().or_else(|| self.payload_traffic.get()) else {
            return;
        };
        if cell.classify_enabled() {
            crate::traffic::classify::observe(cell, bytes);
        }
    }

    /// Counts a PTY-less pane's input bytes (tmux-mapped panes); live PTY panes count at the PTY.
    pub(super) fn count_pty_less_tx(&self, n: usize) {
        if !self.transport.is_local() {
            self.payload_cell().tx(n);
        }
    }

    /// Counts one metadata-only event (clipboard, media passthrough, OSC 52) of `size` bytes.
    pub(super) fn count_event(&self, size: usize) {
        self.payload_cell().event(size);
    }
}
