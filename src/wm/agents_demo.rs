//! Agent demo state seam (Phase 51-02, D-01). Seam by plan 51-02; plan 51-03 owns this file.
//!
//! Tracks the pane ids the `:agents demo` run touched so `:agents demo stop` can close only what
//! the demo spawned. Pane ids can be reused after a session reset or restore, so
//! [`WindowManager::clear_agents_demo`] runs there.

use crate::pane::PaneId;
use crate::wm::WindowManager;

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

    /// Seeds the demo state so tests can prove the reset hooks. Plan 51-03 may remove it once the
    /// real demo populates the state.
    #[doc(hidden)]
    pub fn seed_agents_demo_for_tests(&mut self, endpoints: Vec<PaneId>, spawned: Vec<PaneId>) {
        self.agents_demo = AgentsDemoState { endpoints, spawned };
    }
}
