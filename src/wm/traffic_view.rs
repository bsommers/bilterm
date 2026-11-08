//! `WindowManager` seam for the `traffic` metrics layer (Phase 64).
//!
//! Owns no counters: it exposes the meter, builds the late-bound pane attribution map and ticks
//! the sampler once per `poll_all_ptys`. Plan 64-02 extends `traffic_tick` with cell adoption.

use crate::traffic::{AttributionMap, TrafficMeter, TrafficSnapshot};
use crate::wm::WindowManager;

impl WindowManager {
    /// Cheap clone of this window manager's traffic meter handle (for async tasks).
    pub fn traffic_meter(&self) -> TrafficMeter {
        self.traffic_meter.clone()
    }

    /// Immutable traffic snapshot, `limit` clamped to `1..=500`.
    pub fn traffic_snapshot(&self, limit: usize) -> TrafficSnapshot {
        self.traffic_meter.snapshot(limit)
    }

    /// Late-bound `pane -> (session, window title)` over active panes, background sessions and
    /// the quick terminal (same resolution order as `resolve_pane_session_and_title`).
    #[allow(dead_code)] // consumed by Plan 64-03 (window/RPC snapshots)
    pub(crate) fn traffic_attribution(&self) -> AttributionMap {
        let mut map = AttributionMap::default();
        let active = self.config.history_session.clone();
        for (id, p) in &self.panes {
            map.insert(*id, active.clone(), p.title().to_string());
        }
        for (sess, bg) in &self.window_manage.background_sessions {
            for (id, p) in &bg.panes {
                map.0
                    .entry(*id)
                    .or_insert_with(|| (sess.clone(), p.title().to_string()));
            }
        }
        let q = &self.quick_terminal.pane;
        map.0
            .entry(q.id())
            .or_insert_with(|| (active, q.title().to_string()));
        map
    }

    /// Applies config (idempotent) and samples the meter. Returns `true` only when a new
    /// one-second bucket was produced while the monitor window is visible (so an idle workspace
    /// never gains extra redraws and classic goldens are unchanged).
    pub fn traffic_tick(&mut self, now_ms: u64) -> bool {
        self.traffic_meter.apply_config(&self.config.traffic.to_view());
        let sampled = self.traffic_meter.sample(now_ms);
        sampled && self.traffic_monitor.visible
    }
}
