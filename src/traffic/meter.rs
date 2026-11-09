//! Cold registry of channels: adoption, one-second sampling, snapshots (std only).
//!
//! The mutex is touched only at adopt/open/close/sample/snapshot/config; hot paths use the
//! lock-free [`ChannelCounters`](super::cell::ChannelCounters) cells directly.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard};

use super::cell::{CellReading, ChannelCounters, F_CLASSIFY, F_ENABLED, F_OIA, TrafficTap};
use super::origin::{ConnectionId, ConnectionKey, Origin, Role, label};
use super::window::{Bucket, Rates, Ring};
use super::{BUCKETS, MAX_CHANNELS, MAX_CLOSED, SNAPSHOT_LIMIT_MAX};

const NO_SEC: u64 = u64::MAX;
const MAX_INTERN: usize = 64;
const MAX_CONN_IDS: usize = 1024;
const MAX_DROPPED_IDS: usize = 1024;

/// Plain config values (converted from `[traffic]` on the config side, so this layer never
/// imports `config`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrafficConfigView {
    pub enabled: bool,
    pub classify: bool,
    pub oia_indicator: bool,
    pub label_hosts: bool,
}

impl Default for TrafficConfigView {
    fn default() -> Self {
        Self { enabled: true, classify: false, oia_indicator: false, label_hosts: true }
    }
}

/// Lifetime sums (also the roll-up of evicted channels).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub rx: u64,
    pub tx: u64,
    pub msgs: u64,
    pub esc: u64,
    pub events: u64,
}

impl Totals {
    pub fn add_reading(&mut self, r: &CellReading) {
        self.rx += r.rx_bytes;
        self.tx += r.tx_bytes;
        self.msgs += r.rx_msgs + r.tx_msgs;
        self.esc += r.esc_bytes;
        self.events += r.events;
    }
    pub fn add(&mut self, o: &Totals) {
        self.rx += o.rx;
        self.tx += o.tx;
        self.msgs += o.msgs;
        self.esc += o.esc;
        self.events += o.events;
    }
}

/// Identity and attribution of one channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelMeta {
    pub channel_id: u32,
    pub connection: ConnectionKey,
    pub connection_id: ConnectionId,
    pub origin: Origin,
    pub label: String,
    pub pane: Option<usize>,
    pub role: Role,
    pub opened_ms: u64,
    pub closed_ms: Option<u64>,
}

impl ChannelMeta {
    /// Builds metadata; id, connection and timestamps are filled in by the meter on adopt.
    pub fn new(origin: Origin, label_text: &str, pane: Option<usize>, role: Role) -> Self {
        Self {
            channel_id: 0,
            connection: origin.connection_key(),
            connection_id: ConnectionId(0),
            origin,
            label: label::sanitize(label_text),
            pane,
            role,
            opened_ms: 0,
            closed_ms: None,
        }
    }
}

/// One channel in a snapshot.
#[derive(Clone, Debug)]
pub struct ChannelRow {
    pub meta: ChannelMeta,
    pub reading: CellReading,
    pub rates: Rates,
    /// Lifetime max single-second `(rx, tx)`.
    pub peak: (u64, u64),
    /// Max single-second `(rx, tx)` within the last minute.
    pub peak_1m: (u64, u64),
    /// Per-second rx bytes, oldest first.
    pub series_rx: [u64; BUCKETS],
    /// Per-second tx bytes, oldest first.
    pub series_tx: [u64; BUCKETS],
}

/// Immutable, `Send + Sync` copy of the registry.
#[derive(Clone, Debug, Default)]
pub struct TrafficSnapshot {
    pub at_ms: u64,
    pub channels: Vec<ChannelRow>,
    /// Lifetime sums of channels evicted from the registry.
    pub evicted: Totals,
    /// Lifetime sums of live channels cut by the snapshot `limit`.
    pub omitted: Totals,
    pub dropped_channels: u64,
    pub enabled: bool,
}

struct Chan {
    cell: TrafficTap,
    meta: ChannelMeta,
    prev: CellReading,
    ring: Ring,
}

struct Registry {
    chans: BTreeMap<u32, Chan>,
    conn_ids: BTreeMap<ConnectionKey, u32>,
    evicted: Totals,
    dropped_ids: BTreeSet<u32>,
    dropped: u64,
    now_ms: u64,
    last_sec: Option<u64>,
    enabled: bool,
    classify: bool,
    oia: bool,
    label_hosts: bool,
    intern: Vec<String>,
    last_cfg: Option<TrafficConfigView>,
    overflow_id: Option<u32>,
}

impl Registry {
    fn new() -> Self {
        Self {
            chans: BTreeMap::new(),
            conn_ids: BTreeMap::new(),
            evicted: Totals::default(),
            dropped_ids: BTreeSet::new(),
            dropped: 0,
            now_ms: 0,
            last_sec: None,
            enabled: true,
            classify: false,
            oia: false,
            label_hosts: true,
            intern: Vec::new(),
            last_cfg: None,
            overflow_id: None,
        }
    }

    fn flag_bits(&self) -> u8 {
        (if self.enabled { F_ENABLED } else { 0 })
            | (if self.classify { F_CLASSIFY } else { 0 })
            | (if self.oia { F_OIA } else { 0 })
    }

    fn apply_flags(&self) {
        let bits = self.flag_bits();
        for c in self.chans.values() {
            c.cell.set_flags(bits);
        }
    }

    fn anon_host(&mut self, host: &str) -> String {
        let idx = match self.intern.iter().position(|h| h == host) {
            Some(i) => Some(i),
            None if self.intern.len() < MAX_INTERN => {
                self.intern.push(host.to_string());
                Some(self.intern.len() - 1)
            }
            None => None,
        };
        match idx {
            Some(i) => format!("host-{}", i + 1),
            None => "host-x".to_string(),
        }
    }

    fn anonymize(&mut self, o: &Origin) -> Origin {
        match o {
            Origin::Ssh { host, port } => Origin::Ssh { host: self.anon_host(host), port: *port },
            Origin::TmuxLink { dest, session } => {
                Origin::TmuxLink { dest: self.anon_host(dest), session: session.clone() }
            }
            Origin::Stt { host } if o.locality() == super::origin::Locality::Network => {
                Origin::Stt { host: self.anon_host(host) }
            }
            other => other.clone(),
        }
    }

    fn conn_id(&mut self, key: &ConnectionKey) -> ConnectionId {
        if let Some(id) = self.conn_ids.get(key) {
            return ConnectionId(*id);
        }
        if self.conn_ids.len() >= MAX_CONN_IDS {
            return ConnectionId(u32::MAX);
        }
        let id = self.conn_ids.len() as u32 + 1;
        self.conn_ids.insert(key.clone(), id);
        ConnectionId(id)
    }

    fn closed_ids_oldest_first(&self) -> Vec<u32> {
        let mut v: Vec<(u64, u32)> = self
            .chans
            .iter()
            .filter_map(|(id, c)| c.meta.closed_ms.map(|ms| (ms, *id)))
            .collect();
        v.sort_unstable();
        v.into_iter().map(|(_, id)| id).collect()
    }

    fn evict_one(&mut self, id: u32) {
        if let Some(c) = self.chans.remove(&id) {
            self.evicted.add_reading(&c.cell.read());
            tracing::debug!(target: "bilterm::traffic", channel = id, "evict channel");
        }
    }

    fn evict_closed_over(&mut self, keep: usize) {
        let closed = self.closed_ids_oldest_first();
        if closed.len() > keep {
            for id in &closed[..closed.len() - keep] {
                self.evict_one(*id);
            }
        }
    }

    fn full(&self) -> bool {
        self.chans.len() >= MAX_CHANNELS - 1
    }

    fn ensure_overflow(&mut self) -> TrafficTap {
        if let Some(c) = self.overflow_id.and_then(|id| self.chans.get(&id)) {
            return c.cell.clone();
        }
        let cell = ChannelCounters::with_flags(self.flag_bits());
        let mut meta = ChannelMeta::new(Origin::LocalPty, "overflow", None, Role::Wire);
        meta.connection = ConnectionKey("overflow".to_string());
        meta.channel_id = cell.id();
        meta.connection_id = self.conn_id(&meta.connection.clone());
        meta.opened_ms = self.now_ms;
        self.overflow_id = Some(cell.id());
        self.chans.insert(
            cell.id(),
            Chan { cell: cell.clone(), meta, prev: CellReading::default(), ring: Ring::new() },
        );
        cell
    }

    fn note_dropped(&mut self, id: Option<u32>) {
        match id {
            Some(id) => {
                if self.dropped_ids.len() < MAX_DROPPED_IDS && self.dropped_ids.insert(id) {
                    self.dropped += 1;
                }
            }
            None => self.dropped += 1,
        }
    }

    /// Registers `cell`; returns its channel id (the overflow channel's id when full).
    fn adopt(&mut self, cell: TrafficTap, mut meta: ChannelMeta) -> u32 {
        let id = cell.id();
        if self.chans.contains_key(&id) {
            return id;
        }
        if self.full() {
            self.evict_closed_over(0);
        }
        if self.full() {
            self.note_dropped(Some(id));
            return self.ensure_overflow().id();
        }
        if !self.label_hosts {
            let anon = self.anonymize(&meta.origin);
            if anon != meta.origin {
                meta.origin = anon;
                meta.connection = meta.origin.connection_key();
                meta.label = label::sanitize(meta.connection.as_str());
            }
        }
        meta.channel_id = id;
        meta.connection_id = self.conn_id(&meta.connection.clone());
        meta.opened_ms = self.now_ms;
        meta.closed_ms = None;
        cell.set_flags(self.flag_bits());
        tracing::debug!(target: "bilterm::traffic", channel = id, conn = meta.connection.as_str(), "adopt channel");
        self.chans.insert(id, Chan { cell, meta, prev: CellReading::default(), ring: Ring::new() });
        id
    }

    fn sample(&mut self, now_ms: u64) {
        let sec = now_ms / 1000;
        let gap = self.last_sec.map_or(1, |l| sec.saturating_sub(l)) as usize;
        self.last_sec = Some(sec);
        self.now_ms = now_ms;
        for (id, c) in self.chans.iter_mut() {
            let r = c.cell.read();
            let d = |a: u64, b: u64| a.saturating_sub(b);
            let d32 = |a: u64, b: u64| u32::try_from(a.saturating_sub(b)).unwrap_or(u32::MAX);
            let bucket = Bucket {
                rx: d(r.rx_bytes, c.prev.rx_bytes),
                tx: d(r.tx_bytes, c.prev.tx_bytes),
                msgs: d32(r.rx_msgs + r.tx_msgs, c.prev.rx_msgs + c.prev.tx_msgs),
                esc: d32(r.esc_bytes, c.prev.esc_bytes),
                events: d32(r.events, c.prev.events),
            };
            if gap > 1 && c.ring.filled() > 0 {
                for _ in 0..(gap - 1).min(BUCKETS) {
                    c.ring.push_second(Bucket::default());
                }
            }
            c.ring.push_second(bucket);
            c.prev = r;
            let rates = c.ring.rates();
            c.cell.publish_rates(rates.rx_1, rates.tx_1);
            if c.meta.closed_ms.is_none() && Arc::strong_count(&c.cell) == 1 {
                c.meta.closed_ms = Some(now_ms);
                tracing::debug!(target: "bilterm::traffic", channel = *id, "close channel");
            }
        }
        self.evict_closed_over(MAX_CLOSED);
    }

    fn snapshot(&self, limit: usize) -> TrafficSnapshot {
        let limit = limit.clamp(1, SNAPSHOT_LIMIT_MAX);
        let mut rows: Vec<ChannelRow> = self
            .chans
            .values()
            .map(|c| {
                let to_arr = |v: Vec<u64>| {
                    let mut a = [0u64; BUCKETS];
                    a.copy_from_slice(&v);
                    a
                };
                ChannelRow {
                    meta: c.meta.clone(),
                    reading: c.cell.read(),
                    rates: c.ring.rates(),
                    peak: c.ring.peak(),
                    peak_1m: c.ring.peak_1m(),
                    series_rx: to_arr(c.ring.series(super::window::Dir::Rx, BUCKETS)),
                    series_tx: to_arr(c.ring.series(super::window::Dir::Tx, BUCKETS)),
                }
            })
            .collect();
        let total = |r: &ChannelRow| r.reading.rx_bytes + r.reading.tx_bytes;
        rows.sort_by(|a, b| total(b).cmp(&total(a)).then(a.meta.channel_id.cmp(&b.meta.channel_id)));
        let mut omitted = Totals::default();
        if rows.len() > limit {
            for r in &rows[limit..] {
                omitted.add_reading(&r.reading);
            }
            rows.truncate(limit);
        }
        TrafficSnapshot {
            at_ms: self.now_ms,
            channels: rows,
            evicted: self.evicted,
            omitted,
            dropped_channels: self.dropped,
            enabled: self.enabled,
        }
    }
}

struct Inner {
    last_sec: AtomicU64,
    reg: Mutex<Registry>,
}

/// Cheaply clonable handle to one registry (one per `WindowManager`; no process-global state).
#[derive(Clone)]
pub struct TrafficMeter {
    inner: Arc<Inner>,
}

impl Default for TrafficMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TrafficMeter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrafficMeter").finish_non_exhaustive()
    }
}

impl TrafficMeter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner { last_sec: AtomicU64::new(NO_SEC), reg: Mutex::new(Registry::new()) }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Registry> {
        self.inner.reg.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Registers an externally owned cell (idempotent per cell). Returns the channel id.
    pub fn adopt(&self, cell: TrafficTap, meta: ChannelMeta) -> u32 {
        self.lock().adopt(cell, meta)
    }

    /// Creates and registers a new cell (used by async tasks). When the registry is full the
    /// shared overflow cell is returned and `dropped_channels` is bumped.
    pub fn open_channel(&self, origin: Origin, label_text: &str, pane: Option<usize>, role: Role) -> TrafficTap {
        let mut reg = self.lock();
        if reg.full() {
            reg.evict_closed_over(0);
        }
        if reg.full() {
            reg.note_dropped(None);
            return reg.ensure_overflow();
        }
        let cell = ChannelCounters::with_flags(reg.flag_bits());
        reg.adopt(cell.clone(), ChannelMeta::new(origin, label_text, pane, role));
        cell
    }

    /// Marks a channel closed now (it is evicted later, oldest first).
    pub fn close(&self, id: u32) {
        let mut reg = self.lock();
        let now = reg.now_ms;
        if let Some(c) = reg.chans.get_mut(&id)
            && c.meta.closed_ms.is_none()
        {
            c.meta.closed_ms = Some(now);
            tracing::debug!(target: "bilterm::traffic", channel = id, "close channel");
        }
    }

    /// Clock-injected sampler: returns `false` (touching nothing) unless the second advanced.
    pub fn sample(&self, now_ms: u64) -> bool {
        let sec = now_ms / 1000;
        let last = self.inner.last_sec.load(Relaxed);
        if last != NO_SEC && sec <= last {
            return false;
        }
        self.lock().sample(now_ms);
        self.inner.last_sec.store(sec, Relaxed);
        true
    }

    pub fn snapshot(&self, limit: usize) -> TrafficSnapshot {
        self.lock().snapshot(limit)
    }

    pub fn enabled(&self) -> bool {
        self.lock().enabled
    }

    pub fn set_enabled(&self, on: bool) {
        let mut reg = self.lock();
        reg.enabled = on;
        reg.apply_flags();
    }

    pub fn set_classify(&self, on: bool) {
        let mut reg = self.lock();
        reg.classify = on;
        reg.apply_flags();
    }

    pub fn set_oia(&self, on: bool) {
        let mut reg = self.lock();
        reg.oia = on;
        reg.apply_flags();
    }

    /// Applies config values; a no-op when unchanged so runtime toggles are not clobbered.
    pub fn apply_config(&self, cfg: &TrafficConfigView) {
        let mut reg = self.lock();
        if reg.last_cfg.as_ref() == Some(cfg) {
            return;
        }
        reg.last_cfg = Some(*cfg);
        reg.enabled = cfg.enabled;
        reg.classify = cfg.classify;
        reg.oia = cfg.oia_indicator;
        reg.label_hosts = cfg.label_hosts;
        reg.apply_flags();
    }

    /// Zeroes rings, counters and roll-ups, drops closed channels, keeps live ones.
    pub fn reset(&self) {
        let mut reg = self.lock();
        let closed: Vec<u32> = reg.closed_ids_oldest_first();
        for id in closed {
            reg.chans.remove(&id);
        }
        for c in reg.chans.values_mut() {
            c.cell.reset();
            c.prev = CellReading::default();
            c.ring = Ring::new();
        }
        reg.evicted = Totals::default();
        reg.dropped = 0;
        reg.dropped_ids.clear();
        tracing::debug!(target: "bilterm::traffic", "reset");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn snapshot_is_send_sync_and_clamps() {
        assert_send_sync::<TrafficSnapshot>();
        assert_send_sync::<TrafficMeter>();
        let m = TrafficMeter::new();
        let _keep: Vec<_> = (0..5).map(|_| m.open_channel(Origin::Http, "h", None, Role::Wire)).collect();
        assert_eq!(m.snapshot(0).channels.len(), 1);
        assert_eq!(m.snapshot(3).channels.len(), 3);
        assert_eq!(m.snapshot(10_000).channels.len(), 5);
    }

    #[test]
    fn sample_is_boundary_gated_and_rates_exact() {
        let m = TrafficMeter::new();
        let c = m.open_channel(Origin::LocalPty, "p", Some(1), Role::Both);
        assert!(m.sample(0));
        c.rx(1000);
        assert!(!m.sample(500), "same second: no-op");
        assert!(m.sample(1000));
        c.rx(2000);
        assert!(m.sample(2000));
        let row = &m.snapshot(10).channels[0];
        assert_eq!(row.rates.rx_1, 2000);
        assert_eq!(row.rates.rx_10, (1000 + 2000) / 3);
        assert_eq!(row.peak.0, 2000);
        assert_eq!(&row.series_rx[BUCKETS - 3..], &[0, 1000, 2000]);
        // 5 s gap: zero buckets decay the 1s rate.
        assert!(m.sample(7000));
        let row = &m.snapshot(10).channels[0];
        assert_eq!(row.rates.rx_1, 0);
        assert_eq!(row.rates.rx_10, 3000 / 8);
        assert_eq!(c.rates(), (0, 0));
    }

    #[test]
    fn adopt_idempotent_and_close_detection() {
        let m = TrafficMeter::new();
        let cell = ChannelCounters::new();
        let meta = ChannelMeta::new(Origin::LocalPty, "p", Some(1), Role::Both);
        let a = m.adopt(cell.clone(), meta.clone());
        let b = m.adopt(cell.clone(), meta);
        assert_eq!(a, b);
        assert_eq!(m.snapshot(10).channels.len(), 1);
        cell.rx(10);
        m.sample(1000);
        assert_eq!(m.snapshot(10).channels[0].meta.closed_ms, None);
        drop(cell);
        m.sample(2000);
        let s = m.snapshot(10);
        assert_eq!(s.channels[0].meta.closed_ms, Some(2000));
        assert_eq!(s.channels[0].reading.rx_bytes, 10);
    }

    #[test]
    fn closed_eviction_conserves_totals() {
        let m = TrafficMeter::new();
        for i in 0..(MAX_CLOSED + 10) {
            let c = m.open_channel(Origin::Http, "h", None, Role::Wire);
            c.rx(i + 1);
        }
        m.sample(1000); // all handles dropped: all closed, oldest 10 evicted
        let s = m.snapshot(500);
        assert_eq!(s.channels.len(), MAX_CLOSED);
        let live: u64 = s.channels.iter().map(|c| c.reading.rx_bytes).sum();
        let all: u64 = (1..=(MAX_CLOSED + 10) as u64).sum();
        assert_eq!(live + s.evicted.rx, all);
    }

    #[test]
    fn channel_cap_folds_into_overflow() {
        let m = TrafficMeter::new();
        let keep: Vec<_> = (0..300).map(|_| m.open_channel(Origin::Http, "h", None, Role::Wire)).collect();
        for c in &keep {
            c.rx(1);
        }
        let s = m.snapshot(500);
        assert!(s.channels.len() <= MAX_CHANNELS, "{}", s.channels.len());
        assert_eq!(s.dropped_channels, 300 - (MAX_CHANNELS as u64 - 1));
        let sum: u64 = s.channels.iter().map(|c| c.reading.rx_bytes).sum();
        assert_eq!(sum, 300, "overflow channel still counts bytes");
    }

    #[test]
    fn set_enabled_toggles_flags() {
        let m = TrafficMeter::new();
        let c = m.open_channel(Origin::Mcp, "m", None, Role::Wire);
        c.rx(5);
        m.set_enabled(false);
        assert_eq!(c.flags() & F_ENABLED, 0);
        c.rx(5);
        assert_eq!(c.read().rx_bytes, 5);
        assert!(!m.snapshot(1).enabled);
        m.set_enabled(true);
        let c2 = m.open_channel(Origin::Mcp, "m", None, Role::Wire);
        assert_ne!(c2.flags() & F_ENABLED, 0);
    }

    #[test]
    fn label_hosts_false_anonymises() {
        let m = TrafficMeter::new();
        m.apply_config(&TrafficConfigView { label_hosts: false, ..Default::default() });
        let _c = m.open_channel(Origin::Ssh { host: "prod-db".into(), port: 22 }, "prod-db", Some(2), Role::Both);
        let s = m.snapshot(5);
        assert_eq!(s.channels[0].meta.connection.as_str(), "ssh:host-1:22");
        assert!(!s.channels[0].meta.label.contains("prod-db"));
    }

    #[test]
    fn reset_zeroes() {
        let m = TrafficMeter::new();
        let c = m.open_channel(Origin::Mcp, "m", None, Role::Wire);
        c.rx(50);
        m.sample(1000);
        m.reset();
        let s = m.snapshot(5);
        assert_eq!(s.channels[0].reading.rx_bytes, 0);
        assert_eq!(s.channels[0].rates.rx_1, 0);
    }
}
