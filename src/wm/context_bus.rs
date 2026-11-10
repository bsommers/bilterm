//! In-memory artifact scratchpad + revisioned pub/sub store (`BEYOND-02`, Phase 68).
//!
//! `ContextBus` is a cheap-to-clone handle (`Arc<Mutex<..>>`) so panes, RPC handlers and
//! agent tasks can share one scratchpad. Every mutation (set, delete, TTL expiry) bumps a
//! single global monotonic revision and appends to a bounded change log that
//! `watch_since(rev)` replays. Deletions and expiries appear as tombstones (`value == None`).
//!
//! Time is injected: every operation has an `_at(now_ms)` variant (milliseconds since an
//! arbitrary epoch; the plain variants use wall-clock unix millis) so tests need no sleeps.
//! All sizes are bounded; violations return a typed [`ContextBusError`].

use crate::pane::PaneId;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

/// Maximum live entries.
pub const MAX_ENTRIES: usize = 1024;
/// Maximum key length in bytes.
pub const MAX_KEY_BYTES: usize = 256;
/// Maximum value length in bytes.
pub const MAX_VALUE_BYTES: usize = 64 * 1024;
/// Maximum retained change-log records.
pub const MAX_LOG: usize = 4096;

/// Typed rejection reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextBusError {
    EmptyKey,
    KeyTooLong { len: usize, max: usize },
    InvalidKeyChar,
    ValueTooLarge { len: usize, max: usize },
    TooManyEntries { max: usize },
}

impl std::fmt::Display for ContextBusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyKey => write!(f, "key is empty"),
            Self::KeyTooLong { len, max } => write!(f, "key is {len} bytes (max {max})"),
            Self::InvalidKeyChar => write!(f, "key contains a control character"),
            Self::ValueTooLarge { len, max } => write!(f, "value is {len} bytes (max {max})"),
            Self::TooManyEntries { max } => write!(f, "context bus is full ({max} entries)"),
        }
    }
}

impl std::error::Error for ContextBusError {}

/// A live scratchpad entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEntry {
    pub key: String,
    pub value: String,
    pub sender: Option<PaneId>,
    pub revision: u64,
    /// Time of the write (ms).
    pub set_at: u64,
    /// Absolute expiry time (ms), if any.
    pub expires_at: Option<u64>,
}

/// One change-log record. `value == None` is a tombstone (delete or expiry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextChange {
    pub key: String,
    pub value: Option<String>,
    pub sender: Option<PaneId>,
    pub revision: u64,
    pub at: u64,
}

#[derive(Debug)]
struct Inner {
    entries: BTreeMap<String, ContextEntry>,
    log: VecDeque<ContextChange>,
    revision: u64,
    default_ttl_ms: Option<u64>,
}

impl Inner {
    fn record(&mut self, key: String, value: Option<String>, sender: Option<PaneId>, at: u64) -> u64 {
        self.revision += 1;
        if self.log.len() >= MAX_LOG {
            self.log.pop_front();
        }
        self.log.push_back(ContextChange { key, value, sender, revision: self.revision, at });
        self.revision
    }

    /// Removes expired entries (emitting tombstones in key order).
    fn purge(&mut self, now: u64) {
        let dead: Vec<String> = self
            .entries
            .values()
            .filter(|e| e.expires_at.is_some_and(|t| t <= now))
            .map(|e| e.key.clone())
            .collect();
        for k in dead {
            self.entries.remove(&k);
            self.record(k, None, None, now);
        }
    }
}

/// Shared, thread-safe context bus handle. `Clone` shares the same store.
#[derive(Debug, Clone)]
pub struct ContextBus {
    inner: Arc<Mutex<Inner>>,
}

impl Default for ContextBus {
    fn default() -> Self {
        Self::new()
    }
}

fn wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Validates a key (non-empty, bounded, no control characters).
pub fn validate_key(key: &str) -> Result<(), ContextBusError> {
    if key.is_empty() {
        return Err(ContextBusError::EmptyKey);
    }
    if key.len() > MAX_KEY_BYTES {
        return Err(ContextBusError::KeyTooLong { len: key.len(), max: MAX_KEY_BYTES });
    }
    if key.chars().any(char::is_control) {
        return Err(ContextBusError::InvalidKeyChar);
    }
    Ok(())
}

impl ContextBus {
    /// Empty bus with no default TTL.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                entries: BTreeMap::new(),
                log: VecDeque::new(),
                revision: 0,
                default_ttl_ms: None,
            })),
        }
    }

    /// Empty bus whose entries expire after `ttl_ms` unless a per-entry TTL is given.
    pub fn with_default_ttl(ttl_ms: u64) -> Self {
        let bus = Self::new();
        bus.lock().default_ttl_ms = Some(ttl_ms);
        bus
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A poisoned lock still holds consistent data (no partial updates span a panic point).
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Current global revision (0 when nothing was ever written).
    pub fn revision(&self) -> u64 {
        self.lock().revision
    }

    /// Sets `key`, returning the new revision.
    pub fn set(&self, key: &str, value: &str, sender: Option<PaneId>) -> Result<u64, ContextBusError> {
        self.set_at(key, value, sender, None, wall_ms())
    }

    /// Sets `key` with an explicit TTL (ms).
    pub fn set_with_ttl(
        &self,
        key: &str,
        value: &str,
        sender: Option<PaneId>,
        ttl_ms: u64,
    ) -> Result<u64, ContextBusError> {
        self.set_at(key, value, sender, Some(ttl_ms), wall_ms())
    }

    /// Deterministic `set`: `ttl_ms` overrides the default TTL; `now` is the injected clock.
    pub fn set_at(
        &self,
        key: &str,
        value: &str,
        sender: Option<PaneId>,
        ttl_ms: Option<u64>,
        now: u64,
    ) -> Result<u64, ContextBusError> {
        validate_key(key)?;
        if value.len() > MAX_VALUE_BYTES {
            return Err(ContextBusError::ValueTooLarge { len: value.len(), max: MAX_VALUE_BYTES });
        }
        let mut g = self.lock();
        g.purge(now);
        if !g.entries.contains_key(key) && g.entries.len() >= MAX_ENTRIES {
            return Err(ContextBusError::TooManyEntries { max: MAX_ENTRIES });
        }
        let rev = g.record(key.to_string(), Some(value.to_string()), sender, now);
        let expires_at = ttl_ms.or(g.default_ttl_ms).map(|t| now.saturating_add(t));
        g.entries.insert(
            key.to_string(),
            ContextEntry {
                key: key.to_string(),
                value: value.to_string(),
                sender,
                revision: rev,
                set_at: now,
                expires_at,
            },
        );
        Ok(rev)
    }

    pub fn get(&self, key: &str) -> Option<ContextEntry> {
        self.get_at(key, wall_ms())
    }

    /// Returns the live entry (expired entries are purged first).
    pub fn get_at(&self, key: &str, now: u64) -> Option<ContextEntry> {
        let mut g = self.lock();
        g.purge(now);
        g.entries.get(key).cloned()
    }

    pub fn delete(&self, key: &str) -> bool {
        self.delete_at(key, None, wall_ms())
    }

    /// Deletes `key`; returns whether it existed. Emits a tombstone and bumps the revision.
    pub fn delete_at(&self, key: &str, sender: Option<PaneId>, now: u64) -> bool {
        let mut g = self.lock();
        g.purge(now);
        if g.entries.remove(key).is_some() {
            g.record(key.to_string(), None, sender, now);
            true
        } else {
            false
        }
    }

    pub fn list(&self, prefix: Option<&str>) -> Vec<ContextEntry> {
        self.list_at(prefix, wall_ms())
    }

    /// Live entries sorted by key, optionally filtered by key prefix.
    pub fn list_at(&self, prefix: Option<&str>, now: u64) -> Vec<ContextEntry> {
        let mut g = self.lock();
        g.purge(now);
        g.entries
            .values()
            .filter(|e| prefix.is_none_or(|p| e.key.starts_with(p)))
            .cloned()
            .collect()
    }

    /// Changes with `revision > rev`, in order (tombstones included). Expiry is applied
    /// against the wall clock first; use [`Self::watch_since_at`] for determinism.
    pub fn watch_since(&self, rev: u64) -> Vec<ContextChange> {
        self.watch_since_at(rev, wall_ms())
    }

    pub fn watch_since_at(&self, rev: u64, now: u64) -> Vec<ContextChange> {
        let mut g = self.lock();
        g.purge(now);
        g.log.iter().filter(|c| c.revision > rev).cloned().collect()
    }
}
