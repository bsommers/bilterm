//! Drawer navigation operations (`FLT-01`, Phase 50 D-04/D-05).
//!
//! Every user-triggerable drawer navigation (leader chord, colon command, RPC) lowers to
//! [`DrawerNavOp`] inside [`crate::action::Action::DrawerNav`] and runs through
//! [`WindowManager::apply_drawer_nav`]. Later Phase 50 plans extend the enum.

use super::DrawerStyle;
use crate::pane::PaneId;
use crate::wm::WindowManager;
use crate::wm::agent_state::AgentState;

/// One drawer navigation operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawerNavOp {
    /// Focus the next pane that needs attention (blocked, then errored, then done).
    NextAttention,
}

/// Attention tiers in priority order.
const ATTENTION_TIERS: [AgentState; 3] = [AgentState::Blocked, AgentState::Errored, AgentState::Done];

impl WindowManager {
    /// Applies one drawer navigation op; returns `true` when it changed focus or state.
    pub fn apply_drawer_nav(&mut self, op: DrawerNavOp) -> bool {
        match op {
            DrawerNavOp::NextAttention => self.jump_next_attention().is_some(),
        }
    }

    /// Focuses the next pane needing attention and returns its id (`None` plus a status message
    /// when no pane needs attention).
    ///
    /// Picks the highest non-empty tier (Blocked > Errored > Done); inside the tier the search
    /// cycles in [`WindowManager::ordered_pane_ids`] order starting after the active pane and wraps.
    pub fn jump_next_attention(&mut self) -> Option<PaneId> {
        let ids = self.ordered_pane_ids();
        let active = self.active_pane_id();
        let start = ids.iter().position(|&id| id == active).map_or(0, |i| i + 1);
        let target = ATTENTION_TIERS.iter().find_map(|&tier| {
            (0..ids.len())
                .map(|off| ids[(start + off) % ids.len()])
                .find(|&id| self.agent_state(id) == Some(tier))
                .map(|id| (id, tier))
        });
        let Some((id, tier)) = target else {
            self.active_pane_mut().set_status_message(Some("No pane needs attention".to_string()));
            return None;
        };
        self.focus_and_raise(id);
        self.sync_drawer_selection_to_active_pane();
        self.active_pane_mut().set_status_message(Some(format!("Attention: #{id} {}", tier.as_str())));
        Some(id)
    }

    /// Sets (`Some`) or toggles (`None`) the drawer style, reflows the workspace and returns the new style.
    pub fn set_drawer_style(&mut self, style: Option<DrawerStyle>) -> DrawerStyle {
        let new = style.unwrap_or_else(|| self.config.drawer.style.toggle());
        self.config.drawer.style = new;
        let len = self.drawer_rows().len();
        if self.drawer.selected_index >= len {
            self.drawer.selected_index = len.saturating_sub(1);
        }
        self.reflow_default_viewport();
        self.active_pane_mut().set_status_message(Some(format!("Drawer style: {}", new.label())));
        new
    }
}
