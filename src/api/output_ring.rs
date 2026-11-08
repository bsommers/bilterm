//! Per-pane output sequencing and replay ring for WebSocket clients (`WEB-FE-01`, Phase 60).
//!
//! Each pane's output becomes chunks with a strictly monotonic 1-based `seq`. The ring keeps the
//! most recent `RING_CAP_BYTES` of chunk data; a client resuming from an evicted `seq` is told
//! about the hole with an `agent:gap` range before the retained chunks are replayed.
//! UTF-8 integrity: an incomplete trailing code point is held back until its next bytes arrive.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::pane::PaneId;

/// Retained output per pane (bytes of chunk data).
pub const RING_CAP_BYTES: usize = 256 * 1024;
/// Largest single chunk; bigger pushes are split so one chunk never exceeds a WS frame budget.
pub const MAX_CHUNK_BYTES: usize = 64 * 1024;
/// Most chunks one `agent:history` reply may carry.
pub const MAX_HISTORY_LIMIT: usize = 2000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputChunk {
    pub seq: u64,
    pub ts: u64,
    pub data: String,
}

/// Chunks a resuming client must receive, plus the evicted range it missed (if any).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Replay {
    pub gap: Option<(u64, u64)>,
    pub chunks: Vec<OutputChunk>,
}

#[derive(Debug, Default)]
pub struct PaneRing {
    last_seq: u64,
    chunks: VecDeque<OutputChunk>,
    bytes: usize,
    pending_utf8: Vec<u8>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Splits `buf` into a lossless-as-possible string and the incomplete UTF-8 tail (<= 3 bytes).
fn split_utf8(buf: &[u8]) -> (String, Vec<u8>) {
    match std::str::from_utf8(buf) {
        Ok(s) => (s.to_string(), Vec::new()),
        Err(e) if e.error_len().is_none() => {
            let cut = e.valid_up_to();
            let head = String::from_utf8_lossy(&buf[..cut]).into_owned();
            (head, buf[cut..].to_vec())
        }
        Err(_) => {
            // Invalid bytes mid-stream: keep a possibly-incomplete tail, replace the rest.
            let tail_start = (buf.len().saturating_sub(3)..buf.len())
                .find(|&i| std::str::from_utf8(&buf[i..]).is_err_and(|e| e.error_len().is_none() && e.valid_up_to() == 0))
                .unwrap_or(buf.len());
            (
                String::from_utf8_lossy(&buf[..tail_start]).into_owned(),
                buf[tail_start..].to_vec(),
            )
        }
    }
}

impl PaneRing {
    /// Appends output; returns the last `seq` assigned, or `None` if nothing complete was added.
    pub fn push(&mut self, bytes: &[u8]) -> Option<u64> {
        if bytes.is_empty() {
            return None;
        }
        let mut buf = std::mem::take(&mut self.pending_utf8);
        buf.extend_from_slice(bytes);
        let (text, tail) = split_utf8(&buf);
        self.pending_utf8 = tail;
        if text.is_empty() {
            return None;
        }
        let ts = now_ms();
        let mut rest = text.as_str();
        while !rest.is_empty() {
            let mut cut = rest.len().min(MAX_CHUNK_BYTES);
            while !rest.is_char_boundary(cut) {
                cut -= 1;
            }
            let (piece, tail) = rest.split_at(cut);
            rest = tail;
            self.last_seq += 1;
            self.bytes += piece.len();
            self.chunks.push_back(OutputChunk { seq: self.last_seq, ts, data: piece.to_string() });
        }
        while self.bytes > RING_CAP_BYTES {
            match self.chunks.pop_front() {
                Some(c) => self.bytes -= c.data.len(),
                None => break,
            }
        }
        Some(self.last_seq)
    }

    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// Oldest retained `seq` (`last_seq + 1` when empty).
    pub fn oldest_seq(&self) -> u64 {
        self.chunks.front().map_or(self.last_seq + 1, |c| c.seq)
    }

    pub fn retained_bytes(&self) -> usize {
        self.bytes
    }

    /// Chunks with `seq > after`; `gap` is set when some of them were already evicted.
    pub fn replay_after(&self, after: u64) -> Replay {
        let oldest = self.oldest_seq();
        let gap = (after + 1 < oldest && after < self.last_seq).then(|| (after + 1, oldest - 1));
        Replay {
            gap,
            chunks: self.chunks.iter().filter(|c| c.seq > after).cloned().collect(),
        }
    }

    /// History page: chunks in `(after_seq, before_seq)`; newest `limit` when only `before` is set.
    pub fn history(&self, before: Option<u64>, after: Option<u64>, limit: usize) -> (Vec<OutputChunk>, bool) {
        let limit = limit.clamp(1, MAX_HISTORY_LIMIT);
        let sel: Vec<&OutputChunk> = self
            .chunks
            .iter()
            .filter(|c| before.is_none_or(|b| c.seq < b) && after.is_none_or(|a| c.seq > a))
            .collect();
        let has_more = sel.len() > limit;
        let page: Vec<OutputChunk> = if after.is_some() {
            sel.into_iter().take(limit).cloned().collect()
        } else {
            let skip = sel.len().saturating_sub(limit);
            sel.into_iter().skip(skip).cloned().collect()
        };
        (page, has_more)
    }
}

/// Shared per-pane rings (`Arc<Mutex<..>>`), written by the plane owner and read by WS tasks.
#[derive(Clone, Debug, Default)]
pub struct OutputHub {
    rings: Arc<Mutex<BTreeMap<PaneId, PaneRing>>>,
}

impl OutputHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, pane: PaneId, bytes: &[u8]) -> Option<u64> {
        self.rings.lock().ok()?.entry(pane).or_default().push(bytes)
    }

    pub fn last_seq(&self, pane: PaneId) -> u64 {
        self.rings
            .lock()
            .ok()
            .and_then(|m| m.get(&pane).map(PaneRing::last_seq))
            .unwrap_or(0)
    }

    pub fn replay_after(&self, pane: PaneId, after: u64) -> Replay {
        self.rings
            .lock()
            .ok()
            .and_then(|m| m.get(&pane).map(|r| r.replay_after(after)))
            .unwrap_or_default()
    }

    pub fn history(&self, pane: PaneId, before: Option<u64>, after: Option<u64>, limit: usize) -> (Vec<OutputChunk>, bool) {
        self.rings
            .lock()
            .ok()
            .and_then(|m| m.get(&pane).map(|r| r.history(before, after, limit)))
            .unwrap_or_default()
    }

    /// Drops rings for panes not in `live`.
    pub fn retain(&self, live: &[PaneId]) {
        if let Ok(mut m) = self.rings.lock() {
            m.retain(|id, _| live.contains(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_is_monotonic_and_utf8_is_never_split() {
        let mut r = PaneRing::default();
        assert_eq!(r.push(b"ab"), Some(1));
        let euro = "€".as_bytes();
        assert_eq!(r.push(&euro[..2]), None);
        assert_eq!(r.push(&euro[2..]), Some(2));
        assert_eq!(r.replay_after(1).chunks[0].data, "€");
        assert_eq!(r.replay_after(0).gap, None);
    }

    #[test]
    fn eviction_reports_gap() {
        let mut r = PaneRing::default();
        let big = "x".repeat(MAX_CHUNK_BYTES);
        for _ in 0..6 {
            r.push(big.as_bytes());
        }
        assert!(r.retained_bytes() <= RING_CAP_BYTES);
        assert_eq!(r.oldest_seq(), 3);
        let rep = r.replay_after(1);
        assert_eq!(rep.gap, Some((2, 2)));
        assert_eq!(rep.chunks.first().map(|c| c.seq), Some(3));
        assert_eq!(r.replay_after(6).chunks.len(), 0);
    }

    #[test]
    fn history_pages() {
        let mut r = PaneRing::default();
        for i in 0..10 {
            r.push(format!("{i}").as_bytes());
        }
        let (page, more) = r.history(None, None, 3);
        assert_eq!(page.iter().map(|c| c.seq).collect::<Vec<_>>(), vec![8, 9, 10]);
        assert!(more);
        let (page, more) = r.history(None, Some(8), 5);
        assert_eq!(page.iter().map(|c| c.seq).collect::<Vec<_>>(), vec![9, 10]);
        assert!(!more);
    }
}
