//! Communication topology of the agent traffic bus and the edge heat derived from it.
//!
//! Two independent pieces live here, both fed by [`TrafficBus::record`](crate::wm::traffic::TrafficBus::record):
//!
//! * [`Topology`] - one [`PairRecord`] per unordered pair of endpoints that ever communicated in
//!   this session: total count, first / last event (bus sequence number and time) and a short list
//!   of the most recent event times. It is bounded to [`TOPOLOGY_CAP`] pairs (the least recently
//!   active pair is dropped first) and, unlike the 512-event ring, is never trimmed by traffic
//!   volume, so "who talked to whom" survives ring eviction (WR-03). It holds labels, counts and
//!   times only, never content.
//! * [`edge_heat`] / [`edge_brightness`] - pure functions of event times and `now` that decide how
//!   brightly the graph draws an edge: the more often and the more recently a pair talked inside
//!   the fade window, the brighter; after the window the edge decays to a floor
//!   ([`MIN_EDGE_BRIGHTNESS`]) and never disappears.

use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use crate::wm::agents_window::NodeKey;
use crate::wm::traffic::TrafficEndpoint;

/// Maximum number of pairs kept; the pair whose last event is oldest is dropped first.
pub const TOPOLOGY_CAP: usize = 256;
/// Most recent event times kept per pair (the input of [`edge_heat`]).
pub const RECENT_CAP: usize = 64;
/// Brightness of an edge whose pair has spoken but not within the fade window. Never zero.
pub const MIN_EDGE_BRIGHTNESS: f32 = 0.35;
/// Weighted-event count at which the heat reaches one half.
const HEAT_HALF: f32 = 1.5;

/// Heat in `[0, 1)` of a pair from its event times: every event inside the fade window adds a
/// weight that falls linearly from `1` (just now) to `0` (one fade window ago); the sum `s` maps to
/// `s / (s + 1.5)`. More events and fresher events both raise it; with no event in the window it is `0`.
///
/// Pure: depends only on its arguments (no clock, no state), uses `+ - * /` only.
pub fn edge_heat(times: impl IntoIterator<Item = Instant>, now: Instant, fade: Duration) -> f32 {
    let fade_s = fade.as_secs_f32().max(0.001);
    let sum: f32 = times
        .into_iter()
        .map(|t| (1.0 - now.saturating_duration_since(t).as_secs_f32() / fade_s).clamp(0.0, 1.0))
        .sum();
    sum / (sum + HEAT_HALF)
}

/// Draw brightness of a pair that has spoken: [`MIN_EDGE_BRIGHTNESS`] at zero heat up to (below) `1`.
pub fn edge_brightness(heat: f32) -> f32 {
    MIN_EDGE_BRIGHTNESS + (1.0 - MIN_EDGE_BRIGHTNESS) * heat.clamp(0.0, 1.0)
}

/// What is known about one unordered pair of endpoints.
#[derive(Clone, Debug)]
pub struct PairRecord {
    a: TrafficEndpoint,
    b: TrafficEndpoint,
    count: u64,
    first_seq: u64,
    last_seq: u64,
    first_at: Instant,
    last_at: Instant,
    recent: VecDeque<Instant>,
}

impl PairRecord {
    /// The two endpoints, ordered by node key (`a` sorts before `b`).
    pub fn endpoints(&self) -> (&TrafficEndpoint, &TrafficEndpoint) {
        (&self.a, &self.b)
    }

    /// Sanitised labels of the two endpoints.
    pub fn labels(&self) -> (String, String) {
        (self.a.label(), self.b.label())
    }

    /// Node keys of the two endpoints (`a < b`).
    pub fn keys(&self) -> (NodeKey, NodeKey) {
        (NodeKey::from_endpoint(&self.a), NodeKey::from_endpoint(&self.b))
    }

    /// Number of events exchanged (either direction) since the session began.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Bus sequence number of the first event.
    pub fn first_seq(&self) -> u64 {
        self.first_seq
    }

    /// Bus sequence number of the newest event.
    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// Time of the first event.
    pub fn first_at(&self) -> Instant {
        self.first_at
    }

    /// Time of the newest event.
    pub fn last_at(&self) -> Instant {
        self.last_at
    }

    /// Times of the most recent events (at most [`RECENT_CAP`], oldest first).
    pub fn recent(&self) -> impl Iterator<Item = Instant> + '_ {
        self.recent.iter().copied()
    }

    /// [`edge_heat`] of this pair at `now`.
    pub fn heat(&self, now: Instant, fade: Duration) -> f32 {
        edge_heat(self.recent(), now, fade)
    }
}

/// Bounded, session-long record of which pairs communicated.
#[derive(Clone, Debug, Default)]
pub struct Topology {
    pairs: BTreeMap<(NodeKey, NodeKey), PairRecord>,
    dropped: u64,
}

impl Topology {
    /// Notes one event. Events from an endpoint to itself carry no topology and are ignored.
    pub fn record(&mut self, from: &TrafficEndpoint, to: &TrafficEndpoint, seq: u64, at: Instant) {
        let (kf, kt) = (NodeKey::from_endpoint(from), NodeKey::from_endpoint(to));
        if kf == kt {
            return;
        }
        let (key, a, b) = if kf < kt {
            ((kf, kt), from, to)
        } else {
            ((kt, kf), to, from)
        };
        let rec = self.pairs.entry(key).or_insert_with(|| PairRecord {
            a: a.clone(),
            b: b.clone(),
            count: 0,
            first_seq: seq,
            last_seq: seq,
            first_at: at,
            last_at: at,
            recent: VecDeque::new(),
        });
        rec.count = rec.count.saturating_add(1);
        rec.last_seq = rec.last_seq.max(seq);
        rec.last_at = rec.last_at.max(at);
        rec.recent.push_back(at);
        while rec.recent.len() > RECENT_CAP {
            rec.recent.pop_front();
        }
        while self.pairs.len() > TOPOLOGY_CAP {
            let Some(oldest) = self.pairs.iter().min_by_key(|(_, r)| r.last_seq).map(|(k, _)| k.clone()) else {
                break;
            };
            self.pairs.remove(&oldest);
            self.dropped += 1;
        }
    }

    /// All pairs, ordered by node key.
    pub fn iter(&self) -> impl Iterator<Item = &PairRecord> {
        self.pairs.values()
    }

    /// The record of the pair `{a, b}` (either order).
    pub fn pair(&self, a: &NodeKey, b: &NodeKey) -> Option<&PairRecord> {
        let key = if a < b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) };
        self.pairs.get(&key)
    }

    /// Number of pairs held.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// `true` when nothing communicated yet.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Pairs dropped by the [`TOPOLOGY_CAP`] since the session began.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(base: Instant, s: u64) -> Instant {
        base + Duration::from_secs(s)
    }

    #[test]
    fn heat_is_zero_without_events_in_window_and_bounded() {
        let base = Instant::now();
        let fade = Duration::from_secs(30);
        assert_eq!(edge_heat([], base, fade), 0.0);
        assert_eq!(edge_heat([base], at(base, 30), fade), 0.0, "event exactly one window ago is spent");
        let h = edge_heat(vec![at(base, 100); 1000], at(base, 100), fade);
        assert!(h > 0.99 && h < 1.0);
    }

    #[test]
    fn brightness_floor_is_never_zero() {
        assert_eq!(edge_brightness(0.0), MIN_EDGE_BRIGHTNESS);
        assert!(edge_brightness(0.0) > 0.0);
        assert!(edge_brightness(1.0) <= 1.0);
        assert_eq!(edge_brightness(-5.0), MIN_EDGE_BRIGHTNESS);
    }

    #[test]
    fn cap_drops_least_recent_pair() {
        let base = Instant::now();
        let mut t = Topology::default();
        for i in 0..(TOPOLOGY_CAP as u64 + 5) {
            t.record(&TrafficEndpoint::Session(format!("s{i}")), &TrafficEndpoint::User, i + 1, base);
        }
        assert_eq!(t.len(), TOPOLOGY_CAP);
        assert_eq!(t.dropped(), 5);
        assert!(t.iter().all(|r| r.last_seq > 5));
    }
}
