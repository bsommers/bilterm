//! Size and flow control of the tmux link.
//!
//! * Size: the remote tmux client is as large as the linked pane's terminal, not the whole
//!   Bilterm window, so remote programs wrap and draw for the pane they are shown in. A remote
//!   window shown in one pane of its own gets its own size (`refresh-client -C @W:CxR`).
//! * Flow: tmux pauses a pane whose output the client cannot keep up with (`pause-after`); the
//!   pane is then re-captured and continued, so a flooding pane costs one screen per pause
//!   instead of an unbounded backlog.

use std::collections::BTreeMap;

use crate::tmux::decode::continue_pane_command;
use crate::tmux::hydrate::{ReplyKind, capture_pane_command};
use crate::tmux::link::LinkState;
use crate::tmux::pane_state::{pane_states_command, parse_pane_states, state_marker};
use crate::wm::WindowManager;

impl WindowManager {
    /// `(rows, cols)` of the linked pane that defines the remote client size: the primary pane
    /// when it is mapped, else the first mapped pane, else the workspace default.
    pub(crate) fn link_client_size(&self) -> (u16, u16) {
        let mapped = self.tmux_bridge.mapped_panes();
        let pick = self
            .tmux_link
            .primary_pane
            .filter(|p| mapped.iter().any(|(_, b)| b == p))
            .or_else(|| mapped.first().map(|&(_, b)| b))
            .or(self.tmux_link.primary_pane);
        match pick.and_then(|id| self.panes.get(&id)) {
            Some(pane) => {
                let (rows, cols) = pane.vt().size();
                (rows.max(1), cols.max(1))
            }
            None => (self.default_size.0.saturating_sub(1).max(1), self.default_size.1.max(1)),
        }
    }

    /// Sizes wanted per remote window that is shown in exactly one pane of its own.
    fn link_window_sizes(&self) -> BTreeMap<usize, (u16, u16)> {
        let mut per_window: BTreeMap<usize, Vec<crate::pane::PaneId>> = BTreeMap::new();
        for (tmux, pane) in self.tmux_bridge.mapped_panes() {
            if let Some(w) = self.tmux_bridge.window_of(tmux) {
                per_window.entry(w).or_default().push(pane);
            }
        }
        per_window
            .into_iter()
            .filter(|(_, panes)| panes.len() == 1)
            .filter_map(|(w, panes)| {
                let (rows, cols) = self.panes.get(&panes[0])?.vt().size();
                Some((w, (rows.max(1), cols.max(1))))
            })
            .collect()
    }

    fn queue_link_command(&mut self, cmd: String) {
        // The bridge bounds its queue itself (oldest plain command dropped beyond `OUTBOUND_CAP`).
        self.tmux_bridge.queue_command(cmd, ReplyKind::Ignore);
    }

    /// Sends size changes of linked panes to tmux while `Connected` (after the hydrate batch set
    /// the first size).
    pub(crate) fn pump_tmux_resize(&mut self) {
        let is_connected = matches!(self.tmux_link.machine.state(), LinkState::Connected { .. });
        let has_transport = self.tmux_control.is_some() || self.tmux_link.driver.is_some();
        let global = self.link_client_size();
        if !is_connected || !has_transport {
            self.tmux_link.last_resize = Some(global);
            self.tmux_link.aux.sent_sizes.clear();
            return;
        }
        let Some(&sent_global) = self.tmux_link.aux.sent_sizes.get(&None) else {
            // First tick of this transport: remember the size; only later changes are sent (the
            // hydrate batch announces the size itself).
            self.tmux_link.aux.sent_sizes.insert(None, global);
            self.tmux_link.last_resize = Some(global);
            return;
        };
        if sent_global != global {
            self.tmux_link.aux.sent_sizes.insert(None, global);
            self.tmux_link.last_resize = Some(global);
            self.queue_link_command(crate::tmux::format_refresh_client_size(global.1, global.0));
            if let Some(driver) = self.tmux_link.driver.as_mut() {
                driver.resize(global.0, global.1);
            }
        }
        for (window, size) in self.link_window_sizes() {
            let sent = self.tmux_link.aux.sent_sizes.get(&Some(window)).copied();
            if sent.unwrap_or(global) != size {
                self.tmux_link.aux.sent_sizes.insert(Some(window), size);
                self.queue_link_command(format!("refresh-client -C @{window}:{}x{}\n", size.1, size.0));
            }
        }
    }

    /// First word of pane-state replies for the current attach: set once per transport
    /// generation, with a nonce in live mode so captured screen text cannot imitate a reply.
    pub(crate) fn link_state_marker(&mut self) -> String {
        let generation = self.tmux_link.generation.to_string();
        if self.tmux_link.aux.notes.get("state_marker_gen") != Some(&generation) {
            self.tmux_link.aux.notes.insert("state_marker_gen".to_string(), generation);
            let marker = if self.headless_mode {
                crate::tmux::pane_state::STATE_MARKER.to_string()
            } else {
                let nanos = self.tmux_link.epoch.elapsed().as_nanos() as u64;
                let pid = u64::from(std::process::id());
                state_marker(nanos ^ pid.rotate_left(32) ^ self.tmux_link.generation.rotate_left(48))
            };
            self.tmux_bridge.set_state_marker(marker);
        }
        self.tmux_bridge.state_marker().to_string()
    }

    /// Stores the panes' screen state for the capture replies that follow it.
    pub(crate) fn note_pane_states_reply(&mut self, ok: bool, lines: &[String]) {
        self.tmux_link.aux.pane_states.clear();
        if ok
            && let Some(states) = lines
                .first()
                .and_then(|l| parse_pane_states(self.tmux_bridge.state_marker(), l))
        {
            self.tmux_link.aux.pane_states = states;
        }
    }

    /// Panes tmux paused: drop their backlog, re-capture the screen and continue.
    pub(crate) fn pump_tmux_flow(&mut self) {
        let paused = self.tmux_bridge.take_paused();
        if paused.is_empty()
            || !matches!(self.tmux_link.machine.state(), LinkState::Connected { .. })
        {
            return;
        }
        let marker = self.link_state_marker();
        for tmux_pane in paused {
            self.tmux_bridge.hold_output(tmux_pane);
            self.tmux_bridge
                .queue_command(pane_states_command(&marker), ReplyKind::PaneStates);
            self.tmux_bridge
                .queue_command(capture_pane_command(tmux_pane), ReplyKind::CapturePane { tmux_pane });
            self.queue_link_command(continue_pane_command(tmux_pane));
            tracing::info!(target: "bilterm::link", pane = tmux_pane, "link.pause");
        }
    }
}
