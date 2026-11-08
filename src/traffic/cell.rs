//! Producer-owned atomic counter cell (std only, lock-free, relaxed ordering only).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering::Relaxed};

/// Counting enabled.
pub const F_ENABLED: u8 = 1;
/// Escape/control/printable classification enabled (consumer-thread cost).
pub const F_CLASSIFY: u8 = 2;
/// Publish rates for the OIA status indicator.
pub const F_OIA: u8 = 4;

static NEXT_ID: AtomicU32 = AtomicU32::new(1);
static DEFAULT_FLAGS: AtomicU8 = AtomicU8::new(F_ENABLED);

/// Sets the flags new cells start with (process-global knob; set before panes spawn).
pub fn set_default_flags(flags: u8) {
    DEFAULT_FLAGS.store(flags, Relaxed);
}

/// Plain-integer copy of every cumulative counter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellReading {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_msgs: u64,
    pub tx_msgs: u64,
    pub esc_bytes: u64,
    pub esc_seqs: u64,
    pub ctrl_bytes: u64,
    pub print_bytes: u64,
    pub events: u64,
}

/// Cumulative per-channel counters bumped by the owning I/O boundary.
#[derive(Debug)]
pub struct ChannelCounters {
    id: u32,
    flags: AtomicU8,
    rx_bytes: AtomicU64,
    tx_bytes: AtomicU64,
    rx_msgs: AtomicU64,
    tx_msgs: AtomicU64,
    esc_bytes: AtomicU64,
    esc_seqs: AtomicU64,
    ctrl_bytes: AtomicU64,
    print_bytes: AtomicU64,
    events: AtomicU64,
    last_rx_bps: AtomicU64,
    last_tx_bps: AtomicU64,
    cls_state: AtomicU8,
}

/// Shared handle to a [`ChannelCounters`].
pub type TrafficTap = Arc<ChannelCounters>;

impl ChannelCounters {
    /// New cell with the process default flags.
    pub fn new() -> Arc<Self> {
        Self::with_flags(DEFAULT_FLAGS.load(Relaxed))
    }

    /// New cell with explicit flags.
    pub fn with_flags(flags: u8) -> Arc<Self> {
        Arc::new(Self {
            id: NEXT_ID.fetch_add(1, Relaxed),
            flags: AtomicU8::new(flags),
            rx_bytes: AtomicU64::new(0),
            tx_bytes: AtomicU64::new(0),
            rx_msgs: AtomicU64::new(0),
            tx_msgs: AtomicU64::new(0),
            esc_bytes: AtomicU64::new(0),
            esc_seqs: AtomicU64::new(0),
            ctrl_bytes: AtomicU64::new(0),
            print_bytes: AtomicU64::new(0),
            events: AtomicU64::new(0),
            last_rx_bps: AtomicU64::new(0),
            last_tx_bps: AtomicU64::new(0),
            cls_state: AtomicU8::new(0),
        })
    }

    pub fn id(&self) -> u32 {
        self.id
    }
    #[inline]
    pub fn flags(&self) -> u8 {
        self.flags.load(Relaxed)
    }
    pub fn set_flags(&self, flags: u8) {
        self.flags.store(flags, Relaxed);
    }

    /// Records `n` received bytes as one message.
    #[inline]
    pub fn rx(&self, n: usize) {
        if self.flags.load(Relaxed) & F_ENABLED == 0 {
            return;
        }
        self.rx_bytes.fetch_add(n as u64, Relaxed);
        self.rx_msgs.fetch_add(1, Relaxed);
    }

    /// Records `n` transmitted bytes as one message.
    #[inline]
    pub fn tx(&self, n: usize) {
        if self.flags.load(Relaxed) & F_ENABLED == 0 {
            return;
        }
        self.tx_bytes.fetch_add(n as u64, Relaxed);
        self.tx_msgs.fetch_add(1, Relaxed);
    }

    /// Records one metadata-only event (clipboard, media, ...); the size is not stored.
    #[inline]
    pub fn event(&self, _size: usize) {
        if self.flags.load(Relaxed) & F_ENABLED == 0 {
            return;
        }
        self.events.fetch_add(1, Relaxed);
    }

    #[inline]
    pub fn classify_enabled(&self) -> bool {
        self.flags.load(Relaxed) & (F_ENABLED | F_CLASSIFY) == (F_ENABLED | F_CLASSIFY)
    }
    #[inline]
    pub fn oia_enabled(&self) -> bool {
        self.flags.load(Relaxed) & (F_ENABLED | F_OIA) == (F_ENABLED | F_OIA)
    }

    pub(super) fn add_class(&self, c: &super::classify::ClassCounts) {
        self.esc_bytes.fetch_add(c.esc_bytes, Relaxed);
        self.esc_seqs.fetch_add(c.esc_seqs, Relaxed);
        self.ctrl_bytes.fetch_add(c.ctrl_bytes, Relaxed);
        self.print_bytes.fetch_add(c.print_bytes, Relaxed);
    }
    pub(super) fn cls_load(&self) -> u8 {
        self.cls_state.load(Relaxed)
    }
    pub(super) fn cls_store(&self, v: u8) {
        self.cls_state.store(v, Relaxed);
    }

    /// Plain copy of the cumulative counters.
    pub fn read(&self) -> CellReading {
        CellReading {
            rx_bytes: self.rx_bytes.load(Relaxed),
            tx_bytes: self.tx_bytes.load(Relaxed),
            rx_msgs: self.rx_msgs.load(Relaxed),
            tx_msgs: self.tx_msgs.load(Relaxed),
            esc_bytes: self.esc_bytes.load(Relaxed),
            esc_seqs: self.esc_seqs.load(Relaxed),
            ctrl_bytes: self.ctrl_bytes.load(Relaxed),
            print_bytes: self.print_bytes.load(Relaxed),
            events: self.events.load(Relaxed),
        }
    }

    /// Zeroes every counter (used by `TrafficMeter::reset`).
    pub fn reset(&self) {
        for a in [
            &self.rx_bytes,
            &self.tx_bytes,
            &self.rx_msgs,
            &self.tx_msgs,
            &self.esc_bytes,
            &self.esc_seqs,
            &self.ctrl_bytes,
            &self.print_bytes,
            &self.events,
            &self.last_rx_bps,
            &self.last_tx_bps,
        ] {
            a.store(0, Relaxed);
        }
    }

    /// Last published `(rx, tx)` bytes-per-second (for the OIA indicator).
    pub fn rates(&self) -> (u64, u64) {
        (self.last_rx_bps.load(Relaxed), self.last_tx_bps.load(Relaxed))
    }
    pub fn publish_rates(&self, rx_bps: u64, tx_bps: u64) {
        self.last_rx_bps.store(rx_bps, Relaxed);
        self.last_tx_bps.store(tx_bps, Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_records_nothing() {
        let c = ChannelCounters::with_flags(0);
        c.rx(100);
        c.tx(5);
        c.event(1);
        assert_eq!(c.read(), CellReading::default());
    }

    #[test]
    fn enabled_records() {
        let c = ChannelCounters::with_flags(F_ENABLED);
        c.rx(100);
        c.tx(7);
        c.event(1);
        let r = c.read();
        assert_eq!((r.rx_bytes, r.rx_msgs, r.tx_bytes, r.tx_msgs, r.events), (100, 1, 7, 1, 1));
        assert!(!c.classify_enabled());
        c.set_flags(F_ENABLED | F_CLASSIFY);
        assert!(c.classify_enabled());
    }

    #[test]
    fn ids_unique() {
        assert_ne!(ChannelCounters::new().id(), ChannelCounters::new().id());
    }
}
