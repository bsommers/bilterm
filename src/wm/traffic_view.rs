//! `WindowManager` seam for the `traffic` metrics layer (Phase 64).
//!
//! Owns no counters: it exposes the meter, builds the late-bound pane attribution map and ticks
//! the sampler once per `poll_all_ptys`. Plan 64-02 extends `traffic_tick` with cell adoption.

use crate::pane::CompositePane;
use crate::traffic::{
    AttributionMap, ChannelMeta, Origin, Role, TrafficMeter, TrafficSnapshot, TrafficTap,
};
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
        if sampled {
            self.adopt_traffic_cells();
        }
        sampled && self.traffic_monitor.visible
    }

    /// Lazily adopts every pane, link and clipboard cell (idempotent per cell; the sampler
    /// closes cells whose owner was dropped, e.g. on respawn). Runs once per sampled second.
    fn adopt_traffic_cells(&mut self) {
        let mut found: Vec<(TrafficTap, ChannelMeta)> = Vec::new();
        for p in self.panes.values() {
            collect_pane_cells(p, &mut found);
        }
        for bg in self.window_manage.background_sessions.values() {
            for p in bg.panes.values() {
                collect_pane_cells(p, &mut found);
            }
        }
        collect_pane_cells(&self.quick_terminal.pane, &mut found);
        if let Some(cell) = self.tmux_link.driver.as_ref().and_then(|d| d.traffic_cell()) {
            let (dest, session) = match self.tmux_link.target.as_ref() {
                Some(t) => (host_part(t.dest()), t.session.clone()),
                None => ("unknown".to_string(), String::new()),
            };
            let origin = Origin::TmuxLink { dest, session };
            found.push((cell, ChannelMeta::new(origin, "tmux-link", None, Role::Wire)));
        }
        if let Some(cell) = self.tmux_control.as_ref().and_then(|c| c.traffic.clone()) {
            found.push((cell, ChannelMeta::new(Origin::TmuxLocal, "tmux-local", None, Role::Wire)));
        }
        for (cell, meta) in found {
            self.traffic_meter.adopt(cell, meta);
        }
        if !self.headless_mode && !crate::clipboard::has_traffic_tap() {
            crate::clipboard::set_traffic_tap(Some(self.traffic_meter.open_channel(
                Origin::ClipboardOs,
                "clipboard",
                None,
                Role::Wire,
            )));
        }
    }
}

/// `user@host` -> `host`.
fn host_part(dest: &str) -> String {
    dest.rsplit('@').next().unwrap_or(dest).to_string()
}

fn collect_pane_cells(p: &CompositePane, out: &mut Vec<(TrafficTap, ChannelMeta)>) {
    let id = p.id();
    let ssh = std::path::Path::new(p.shell_program())
        .file_name()
        .is_some_and(|n| n == "ssh");
    let origin = if p.traffic_cell().is_some() && ssh {
        match crate::remote::ssh_destination_from_args(p.shell_args()) {
            Some((d, port)) => Origin::Ssh { host: host_part(&d), port: port.unwrap_or(22) },
            None => Origin::Ssh { host: "unknown".into(), port: 22 },
        }
    } else if p.traffic_cell().is_none()
        && let Some(t) = p.remote_target()
    {
        let (dest, session) = t.rsplit_once('/').unwrap_or((t, ""));
        Origin::TmuxLink { dest: host_part(dest), session: session.to_string() }
    } else {
        Origin::LocalPty
    };
    let label = format!("pane {id}");
    if let Some(c) = p.traffic_cell() {
        out.push((c.clone(), ChannelMeta::new(origin.clone(), &label, Some(id), Role::Both)));
    }
    if let Some(c) = p.existing_payload_cell() {
        out.push((c.clone(), ChannelMeta::new(origin, &label, Some(id), Role::Payload)));
    }
}
