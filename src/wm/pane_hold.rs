//! Exited-pane reaping/holding and the single respawn dispatcher (Phase 51.1, PST-03).
//!
//! Seam state (plan 01): `reap_or_hold_exited` is the legacy loop of `poll_all_ptys` moved
//! verbatim; plan 07 adds the held remote-pane branch. Every respawn caller (held-pane key, exit
//! guard choice, exit-protection policy, `:respawn-pane`) goes through `respawn_active_pane`.

use crate::pane::{ExitProtectionPolicy, PaneId};
use crate::remote::FleetNodeKind;
use crate::wm::WindowManager;

impl WindowManager {
    /// Handles panes whose PTY child exited. Remote panes are held with code and reason unless
    /// exit_protection is Off; local panes are removed only while more than one pane exists.
    /// Returns `true` when anything changed.
    pub(crate) fn reap_or_hold_exited(&mut self, ids: Vec<PaneId>) -> bool {
        let mut changed = false;
        let policy = self.config.exit_protection;
        for id in ids {
            if let Some(pane) = self.panes.get_mut(&id)
                && let Some(target) = pane.remote_target().map(str::to_string)
                && policy != ExitProtectionPolicy::Off
            {
                let code = pane.exit_code();
                let reason = pane.last_output_line();
                pane.hold_remote_exit(code, reason.as_deref());
                for node in self.fleet.nodes.values_mut() {
                    if let FleetNodeKind::SshPane { pane_id, .. } = node.kind
                        && pane_id == id
                    {
                        node.connected = false;
                    }
                }
                tracing::info!(
                    target: "bilterm::link",
                    pane = id,
                    code = ?code,
                    reason = ?reason,
                    remote_target = %target,
                    "pane.exit"
                );
                changed = true;
                continue;
            }
            if self.panes.len() > 1 && self.remove_pane(id) {
                changed = true;
            }
        }
        changed
    }

    /// Respawns the active pane in place: an interactive link banner reconnects the link,
    /// a held remote pane reconnects its ssh session,
    /// otherwise the pane restarts its shell (legacy behaviour).
    pub fn respawn_active_pane(&mut self) {
        if self.active_pane().link_banner().is_some_and(|b| b.interactive) {
            self.link_reconnect();
            return;
        }
        let active_id = self.active_pane_id();
        let is_held_remote = self.active_pane().held_exited_shell().is_some()
            && self.active_pane().remote_target().is_some();
        if is_held_remote {
            let target = self
                .active_pane()
                .remote_target()
                .map(str::to_string)
                .unwrap_or_default();
            let prev_held = self.active_pane().held_exited_shell().cloned();
            if let Err(err) = self.active_pane_mut().respawn_shell_in_place() {
                if let Some(held) = prev_held {
                    self.active_pane_mut().restore_held_shell(held);
                }
                self.active_pane_mut()
                    .set_status_message(Some(format!("Reconnect failed: {err}")));
                return;
            }
            for node in self.fleet.nodes.values_mut() {
                if let FleetNodeKind::SshPane { pane_id, .. } = node.kind
                    && pane_id == active_id
                {
                    node.connected = true;
                }
            }
            self.active_pane_mut()
                .set_status_message(Some(format!("Reconnecting ssh: {target}")));
            return;
        }
        let _ = self.active_pane_mut().respawn_shell_in_place();
    }
}
