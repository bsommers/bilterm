//! Exited-pane reaping/holding and the single respawn dispatcher (Phase 51.1, PST-03).
//!
//! Seam state (plan 01): `reap_or_hold_exited` is the legacy loop of `poll_all_ptys` moved
//! verbatim; plan 07 adds the held remote-pane branch. Every respawn caller (held-pane key, exit
//! guard choice, exit-protection policy, `:respawn-pane`) goes through `respawn_active_pane`.

use crate::pane::PaneId;
use crate::wm::WindowManager;

impl WindowManager {
    /// Handles panes whose PTY child exited. Legacy: a pane is removed only while more than one
    /// pane exists. Returns `true` when anything changed.
    pub(crate) fn reap_or_hold_exited(&mut self, ids: Vec<PaneId>) -> bool {
        let mut changed = false;
        for id in ids {
            if self.panes.len() > 1 && self.remove_pane(id) {
                changed = true;
            }
        }
        changed
    }

    /// Respawns the active pane in place: an interactive link banner reconnects the link,
    /// otherwise the pane restarts its shell (legacy behaviour).
    pub fn respawn_active_pane(&mut self) {
        if self.active_pane().link_banner().is_some_and(|b| b.interactive) {
            self.link_reconnect();
            return;
        }
        let _ = self.active_pane_mut().respawn_shell_in_place();
    }
}
