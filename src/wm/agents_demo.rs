//! Agent demo: `:agents demo` start and `:agents demo stop` (Phase 51-03, D-01, UIT-01).
//!
//! The state records which pane ids the demo drew traffic between (`endpoints`) and which panes it
//! spawned itself (`spawned`, the only ones `stop` may close). Pane ids can be reused after a
//! session reset or restore, so [`WindowManager::clear_agents_demo`] runs there.
//!
//! Stop deletes by predicate, never by flag (the bus schema stays as ADR-008 defines it): an event
//! is a demo event when it is a `Message` whose preview starts with `[demo] ` between two demo
//! endpoints. The demo records its (constant, synthetic) lines unredacted so that prefix survives
//! even when an endpoint is in a password prompt. Panes close through [`WindowManager::remove_pane`]
//! only (`ARCH-R02`), the same path `:close-pane` uses; dropping a pane kills its shell
//! (`impl Drop for PtySession`).

use std::time::{Duration, Instant};

use crate::pane::PaneId;
use crate::wm::agents_window::{GraphSelection, NodeKey};
use crate::wm::traffic::{TrafficEndpoint, TrafficKind};
use crate::wm::{SplitDirection, WindowManager};

/// Prefix of every synthetic demo line.
const DEMO_PREFIX: &str = "[demo] ";

/// Synthetic conversation: `(from index, to index, seconds ago, text)` over three panes, skewed so
/// edge brightness and width differ (A<->B busy, B->C occasional, A->C rare).
pub const DEMO_SCRIPT: &[(usize, usize, u64, &str)] = &[
    (0, 1, 19, "[demo] plan: split the refactor in two"),
    (1, 0, 18, "[demo] ack, taking the parser half"),
    (0, 1, 16, "[demo] ping 1"),
    (1, 0, 15, "[demo] pong 1"),
    (1, 2, 14, "[demo] relay: parser done, please review"),
    (0, 1, 12, "[demo] ping 2"),
    (1, 0, 11, "[demo] pong 2"),
    (2, 1, 9, "[demo] review: two nits, see line 40"),
    (0, 1, 7, "[demo] ping 3"),
    (1, 0, 6, "[demo] pong 3"),
    (0, 2, 4, "[demo] rare: heads-up, merging at the top of the hour"),
    (0, 1, 2, "[demo] ping 4"),
    (1, 0, 1, "[demo] pong 4"),
];

/// Pane ids touched by the running demo (private: `ARCH-R02`).
#[derive(Clone, Debug, Default)]
pub(crate) struct AgentsDemoState {
    /// Panes the demo drew traffic between.
    pub(crate) endpoints: Vec<PaneId>,
    /// Panes the demo itself spawned (the only ones `demo stop` may close).
    pub(crate) spawned: Vec<PaneId>,
}

/// Read-only view of the demo state (RPC / MCP `agents_demo` object, 51-10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentsDemoSummary {
    pub running: bool,
    pub endpoints: Vec<PaneId>,
    pub spawned: Vec<PaneId>,
}

/// What `:agents demo` did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DemoStartReport {
    /// The panes the traffic was drawn between (empty when the demo could not start).
    pub endpoints: Vec<PaneId>,
    /// Number of `[demo]` events recorded.
    pub events: usize,
}

/// What `:agents demo stop` did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DemoStopReport {
    /// `false` when no demo was running (nothing was touched).
    pub was_running: bool,
    /// Panes closed.
    pub closed: Vec<PaneId>,
    /// Spawned panes that stayed because `remove_pane` refused (the last pane).
    pub kept: Vec<PaneId>,
    /// Spawned panes the user had already closed.
    pub skipped: usize,
    /// `[demo]` events removed from the ring.
    pub events_removed: usize,
}

impl WindowManager {
    /// Summary of the demo state.
    pub fn agents_demo_summary(&self) -> AgentsDemoSummary {
        let d = &self.agents_demo;
        AgentsDemoSummary {
            running: !d.endpoints.is_empty() || !d.spawned.is_empty(),
            endpoints: d.endpoints.clone(),
            spawned: d.spawned.clone(),
        }
    }

    /// Forgets the demo's pane ids (session reset / restore: ids may be reused).
    pub fn clear_agents_demo(&mut self) {
        self.agents_demo = AgentsDemoState::default();
    }

    /// Seeds the demo state so tests can prove the reset hooks.
    #[doc(hidden)]
    pub fn seed_agents_demo_for_tests(&mut self, endpoints: Vec<PaneId>, spawned: Vec<PaneId>) {
        self.agents_demo = AgentsDemoState { endpoints, spawned };
    }

    /// Starts the demo: splits the focused pane until there are three, then records the
    /// [`DEMO_SCRIPT`] between the first three panes. A re-run REPLACES the earlier demo events and
    /// topology pairs and keeps every pane either run spawned in `spawned` (no pane leaks).
    pub fn start_agents_demo(&mut self) -> DemoStartReport {
        if self.agents_demo_summary().running {
            let old = self.agents_demo.endpoints.clone();
            self.purge_demo_records(&old);
        }
        let before: Vec<PaneId> = self.panes.keys().copied().collect();
        let mut guard = 0;
        while self.panes.len() < 3 && guard < 2 {
            let dir = if guard == 0 { SplitDirection::Vertical } else { SplitDirection::Horizontal };
            if self.split_focused(dir).is_err() {
                break;
            }
            guard += 1;
        }
        let created: Vec<PaneId> = self.panes.keys().copied().filter(|id| !before.contains(id)).collect();
        for id in created {
            if !self.agents_demo.spawned.contains(&id) {
                self.agents_demo.spawned.push(id);
            }
        }
        let ids: Vec<PaneId> = self.panes.keys().copied().take(3).collect();
        if ids.len() < 2 {
            return DemoStartReport::default();
        }
        let now = Instant::now();
        let mut events = 0;
        for &(from, to, ago, text) in DEMO_SCRIPT {
            let (Some(&f), Some(&t)) = (ids.get(from), ids.get(to)) else { continue };
            let at = now.checked_sub(Duration::from_secs(ago)).unwrap_or(now);
            self.record_traffic_with(TrafficEndpoint::Pane(f), TrafficEndpoint::Pane(t), TrafficKind::Message, text, false, at);
            events += 1;
        }
        self.agents_demo.endpoints = ids.clone();
        DemoStartReport { endpoints: ids, events }
    }

    /// Stops the demo: removes its `[demo]` events and topology pairs, closes the panes it spawned
    /// (reverse creation order, skipping missing ones, keeping what `remove_pane` refuses) and
    /// resets graph selection and snoop filters that pointed at what is gone. No demo running:
    /// nothing is touched and the ring is not scanned.
    pub fn stop_agents_demo(&mut self) -> DemoStopReport {
        let state = std::mem::take(&mut self.agents_demo);
        if state.endpoints.is_empty() && state.spawned.is_empty() {
            return DemoStopReport::default();
        }
        let mut report = DemoStopReport { was_running: true, ..DemoStopReport::default() };
        report.events_removed = self.purge_demo_records(&state.endpoints);
        for &id in state.spawned.iter().rev() {
            if !self.panes.contains_key(&id) {
                report.skipped += 1;
            } else if self.remove_pane(id) {
                report.closed.push(id);
            } else {
                report.kept.push(id);
            }
        }
        self.reset_demo_ui_refs(&report.closed);
        report
    }

    /// Removes the demo events between `endpoints` and then the topology pairs among them that no
    /// remaining event justifies. Returns the number of events removed.
    fn purge_demo_records(&mut self, endpoints: &[PaneId]) -> usize {
        let is_demo = |e: &crate::wm::traffic::TrafficEvent| {
            e.kind == TrafficKind::Message
                && e.preview.starts_with(DEMO_PREFIX)
                && matches!((&e.from, &e.to), (TrafficEndpoint::Pane(a), TrafficEndpoint::Pane(b)) if endpoints.contains(a) && endpoints.contains(b))
        };
        let removed = self.traffic.retain_events(|e| !is_demo(e));
        for (i, &a) in endpoints.iter().enumerate() {
            for &b in &endpoints[i + 1..] {
                let still_linked = self.traffic.iter().any(|e| {
                    matches!((&e.from, &e.to), (TrafficEndpoint::Pane(x), TrafficEndpoint::Pane(y)) if (*x, *y) == (a, b) || (*x, *y) == (b, a))
                });
                if !still_linked {
                    self.traffic.remove_topology_pair(&NodeKey::pane(a), &NodeKey::pane(b));
                }
            }
        }
        removed
    }

    /// Clears selection, snoop filters and anchors that reference closed panes, removed pairs or
    /// removed events (a stale reference would otherwise point at a node that no longer exists).
    fn reset_demo_ui_refs(&mut self, closed: &[PaneId]) {
        let gone: Vec<NodeKey> = closed.iter().map(|&id| NodeKey::pane(id)).collect();
        let topo_has = |t: &crate::wm::agents_topology::Topology, p: &(NodeKey, NodeKey)| t.pair(&p.0, &p.1).is_some();
        let w = &mut self.agents_window;
        let bad_edge = w.selection.edge.as_ref().is_some_and(|p| gone.contains(&p.0) || gone.contains(&p.1) || !topo_has(self.traffic.topology(), p));
        if w.selection.node.as_ref().is_some_and(|n| gone.contains(n)) {
            w.selection = GraphSelection::default();
        } else if bad_edge {
            w.selection.edge = None;
        }
        let s = &mut w.snoop;
        if s.filter_agent.as_ref().is_some_and(|k| gone.contains(k)) {
            s.filter_agent = None;
        }
        if s.filter_pair.as_ref().is_some_and(|(a, b)| gone.contains(a) || gone.contains(b)) {
            s.filter_pair = None;
        }
        let held = |seq: u64| self.traffic.iter().any(|e| e.seq == seq);
        if s.selected_seq.is_some_and(|q| !held(q)) {
            s.selected_seq = None;
        }
        if s.scroll_seq.is_some_and(|q| !held(q)) {
            (s.follow, s.scroll, s.scroll_seq) = (true, 0, None);
        }
    }
}
