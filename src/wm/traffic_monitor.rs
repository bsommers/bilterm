//! TRAFFIC MONITOR window state (Phase 64). Placeholder created by Plan 64-01 so
//! `wm/mod.rs` declares it exactly once; Plan 64-03 owns and replaces the body.

/// Floating TRAFFIC MONITOR window state.
#[derive(Debug, Default)]
pub struct TrafficMonitorState {
    /// Window visibility (toggled by Plan 64-03's `Action::TrafficMonitor`).
    pub(crate) visible: bool,
}

impl TrafficMonitorState {
    /// Drops focus when another window is raised (no-op until Plan 64-03).
    pub fn blur(&mut self) {}
}
