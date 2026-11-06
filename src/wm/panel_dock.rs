//! Panel docking seam (Phase 51-02, D-04). Seam stubs by plan 51-02; plan 51-09 owns this file and
//! replaces the bodies.
//!
//! Until then every hook is an identity / no-op: with no panel docked the workspace area and the
//! `:arrange` output are byte-identical to the pre-Phase-51 behavior.

use ratatui::layout::Rect;

use crate::wm::WindowManager;

/// Panels that can be docked into the tiling strip by `:arrange`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockPanel {
    /// The AGENTS list window.
    AgentsList,
    /// The AGENT TRAFFIC window.
    AgentsTraffic,
}

impl WindowManager {
    /// Removes the dock strip from `area` (real cells). Identity stub.
    pub fn subtract_docked_panels(&self, area: Rect) -> Rect {
        area
    }

    /// Slot of a docked `panel` inside `ws` (both in the panel's own space). `None` stub.
    pub fn panel_dock_slot(&self, _ws: Rect, _panel: DockPanel) -> Option<Rect> {
        None
    }

    /// Docks every eligible open panel before `:arrange` re-flows the panes. No-op stub.
    pub fn dock_panels_on_arrange(&mut self) {}

    /// The only place that flips a `docked` flag; returns whether it changed. The stub flips the
    /// `AgentsList` flag only (refusing `on` while the list is inactive) and never reflows.
    pub fn set_panel_docked(&mut self, panel: DockPanel, on: bool) -> bool {
        match panel {
            DockPanel::AgentsList => {
                let want = on && self.agents_list.active;
                let changed = self.agents_list.docked != want;
                self.agents_list.docked = want;
                changed
            }
            DockPanel::AgentsTraffic => false,
        }
    }
}
