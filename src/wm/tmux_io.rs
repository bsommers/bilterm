//! I/O pumps between panes and the tmux link (Phase 51.1, plan 13; PST-02).
//!
//! Contract: `51.1-CONTRACT.md` section 7 (`Input`, `Replies`, `Hydrate`, `WindowClose`, `Resize`,
//! `Fleet`, `Order inside tick_tmux_link`).

use std::collections::BTreeSet;

use crate::pane::{LinkBanner, PaneId};
use crate::tmux::format_send_keys_hex;
use crate::tmux::hydrate::{HydratePhase, ReplyKind, render_capture};
use crate::tmux::link::{
    HYDRATE_REPLY_LINES_CAP, INPUT_BACKLOG_CAP, LinkDriver, LinkEvent, LinkState, OUTBOUND_CAP, SEND_KEYS_CHUNK,
};
use crate::wm::WindowManager;

const ORPHAN_BANNER_TEXT: &str = "REMOTE PANE GONE | close with :close";
const LINK_DOWN_INPUT_STATUS: &str = "Link down: input not delivered (Enter/r reconnects)";
const OUTBOUND_OVERFLOW_STATUS: &str = "Tmux command queue full: oldest input was dropped";
const INPUT_OVERFLOW_STATUS: &str = "Paste too large for the tmux link: input was not sent";
const HYDRATE_SETTLE_MS: u64 = 300;

impl WindowManager {
    /// Installs or clears the active transport [`LinkDriver`] (used by lifecycle and headless tests).
    pub fn set_link_driver(&mut self, driver: Option<Box<dyn LinkDriver>>) {
        self.tmux_link.driver = driver;
    }

    /// Current attach hydrate phase of the link.
    pub fn link_hydrate_phase(&self) -> HydratePhase {
        self.tmux_link.hydrate.phase()
    }

    /// Orphaned panes retained after reattach reconcile or primary/final window closure.
    pub fn link_orphans(&self) -> &BTreeSet<PaneId> {
        &self.tmux_link.orphans
    }

    /// Advances the link transport generation counter.
    pub fn bump_link_generation(&mut self) -> u64 {
        self.tmux_link.generation = self.tmux_link.generation.saturating_add(1);
        self.tmux_link.generation
    }

    /// Feeds a [`LinkEvent`] into the link state machine at `now_ms`.
    pub fn feed_link_event(&mut self, ev: LinkEvent, now_ms: u64) {
        let _ = self.tmux_link.machine.on_event(ev, now_ms);
    }

    /// Number of outbound tmux commands currently waiting in the bridge queue.
    pub fn tmux_outbound_queued(&self) -> usize {
        self.tmux_bridge.outbound_commands.len()
    }

    /// Queues a raw tmux command on the bridge with its [`ReplyKind`].
    pub fn queue_tmux_command(&mut self, cmd: impl Into<String>, kind: ReplyKind) {
        let will_overflow = self.tmux_bridge.outbound_commands.len() >= OUTBOUND_CAP;
        self.tmux_bridge.queue_command(cmd.into(), kind);
        if will_overflow {
            self.note_outbound_overflow();
        }
    }

    /// Resets per-generation I/O state (`hydrate`, `orphans`, `last_resize`, pump notes) when the
    /// transport generation advances or `reset_for_attach` starts a fresh attach.
    fn sync_io_generation(&mut self) {
        let prev_blocks = self
            .tmux_link
            .aux
            .notes
            .get("last_blocks_closed")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let cur_blocks = self.tmux_bridge.blocks_closed();
        let reset_by_attach = cur_blocks < prev_blocks
            || (!self.tmux_bridge.stale_panes().is_empty()
                && self.tmux_bridge.mapped_panes().is_empty()
                && self.tmux_link.hydrate.phase() != HydratePhase::AwaitingGreeting
                && cur_blocks == 0);

        if self.tmux_link.generation != self.tmux_link.io_generation || reset_by_attach {
            let old_orphans: Vec<PaneId> = self.tmux_link.orphans.iter().copied().collect();
            for orphan_id in old_orphans {
                if let Some(pane) = self.panes.get_mut(&orphan_id)
                    && pane.link_banner().is_some_and(|b| b.text == ORPHAN_BANNER_TEXT)
                {
                    pane.set_link_banner(None);
                }
            }
            self.tmux_link.io_generation = self.tmux_link.generation;
            self.tmux_link.hydrate.reset();
            self.tmux_link.orphans.clear();
            self.tmux_link.last_resize = Some(self.default_size);
            self.tmux_link.aux.last_new_pane_ms = 0;
            self.tmux_link.aux.hydrate_sent_ms = None;
            self.tmux_link.aux.notes.clear();
            self.tmux_link.aux.pane_states.clear();
            self.tmux_link.aux.input_backlog.clear();
            self.tmux_link.aux.sent_sizes.clear();
        }
        self.tmux_link
            .aux
            .notes
            .insert("last_blocks_closed".to_string(), cur_blocks.to_string());
    }

    /// Updates greeting progress on the hydrate planner from bridge telemetry and clock ticks.
    fn sync_hydrate_greeting(&mut self, now_ms: u64) {
        if self.tmux_link.aux.control_seen
            || self.tmux_bridge.blocks_closed() > 0
            || !self.tmux_bridge.mapped_panes().is_empty()
            || self.tmux_bridge.active_session_name.is_some()
        {
            self.tmux_link.hydrate.note_line(now_ms);
        }
        if self.tmux_bridge.blocks_closed() > 0 {
            self.tmux_link.hydrate.note_block_end();
        }
        self.tmux_link.hydrate.tick(now_ms);
    }

    /// Tracks when the bridge's mapped pane set gains any newly mapped tmux pane.
    fn update_mapped_set_timestamp(&mut self, now_ms: u64) {
        let mapped = self.tmux_bridge.mapped_panes();
        let current_key: String = mapped
            .iter()
            .map(|(t, _)| t.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let prev_key = self
            .tmux_link
            .aux
            .notes
            .get("mapped_tmux_ids")
            .cloned()
            .unwrap_or_default();
        if current_key != prev_key {
            let prev_set: BTreeSet<&str> = prev_key.split(',').filter(|s| !s.is_empty()).collect();
            let has_new_pane = mapped
                .iter()
                .any(|(t, _)| !prev_set.contains(t.to_string().as_str()));
            self.tmux_link
                .aux
                .notes
                .insert("mapped_tmux_ids".to_string(), current_key);
            if has_new_pane {
                self.tmux_link.aux.last_new_pane_ms = now_ms;
            }
        }
    }

    /// Sets a one-time status message when the outbound tmux command queue overflows `OUTBOUND_CAP`.
    fn note_outbound_overflow(&mut self) {
        if self
            .tmux_link
            .aux
            .notes
            .insert("outbound_overflow".to_string(), "1".to_string())
            .is_none()
        {
            self.active_pane_mut()
                .set_status_message(Some(OUTBOUND_OVERFLOW_STATUS.to_string()));
            if let Some(primary) = self.tmux_link.primary_pane
                && let Some(pane) = self.panes.get_mut(&primary)
            {
                pane.set_status_message(Some(OUTBOUND_OVERFLOW_STATUS.to_string()));
            }
        }
    }

    /// Reaps panes queued in `bridge.take_removed_panes()` via `remove_pane` (keeping the primary
    /// and the last workspace pane with an orphan banner) and refreshes the fleet node's `pane_ids`.
    fn reap_removed_tmux_panes_and_sync_fleet(&mut self) {
        let existing_fleet = self.tmux_link.fleet_node.as_ref().and_then(|name| {
            self.fleet.nodes.get(name).map(|node| {
                let ids = match &node.kind {
                    crate::remote::FleetNodeKind::SshTmux { pane_ids, .. } => pane_ids.clone(),
                    _ => Vec::new(),
                };
                (node.connected, node.last_dispatched_command.clone(), ids)
            })
        });

        let removed = self.tmux_bridge.take_removed_panes();
        for pane_id in removed {
            let is_primary = self.tmux_link.primary_pane == Some(pane_id);
            if is_primary || self.panes.len() <= 1 {
                if let Some(pane) = self.panes.get_mut(&pane_id) {
                    pane.set_remote_input_tap(false);
                    pane.set_link_banner(Some(LinkBanner {
                        text: ORPHAN_BANNER_TEXT.to_string(),
                        interactive: false,
                    }));
                }
                self.tmux_link.orphans.insert(pane_id);
            } else {
                self.tmux_link.orphans.remove(&pane_id);
                let _ = self.remove_pane(pane_id);
            }
        }

        // Clear orphan state if a pane was re-adopted by the bridge.
        let readopted: Vec<PaneId> = self
            .tmux_link
            .orphans
            .iter()
            .copied()
            .filter(|id| self.tmux_bridge.bilterm_to_tmux.contains_key(id))
            .collect();
        for id in readopted {
            self.tmux_link.orphans.remove(&id);
            if let Some(pane) = self.panes.get_mut(&id)
                && pane.link_banner().is_some_and(|b| b.text == ORPHAN_BANNER_TEXT)
            {
                pane.set_link_banner(None);
            }
        }

        if let (Some(node_name), Some(target), Some((old_connected, old_last_cmd, old_ids))) = (
            self.tmux_link.fleet_node.clone(),
            self.tmux_link.target.clone(),
            existing_fleet,
        ) {
            let mut pane_set = BTreeSet::new();
            if let Some(primary) = self.tmux_link.primary_pane
                && self.panes.contains_key(&primary)
            {
                pane_set.insert(primary);
            }
            for (_, pane_id) in self.tmux_bridge.mapped_panes() {
                if self.panes.contains_key(&pane_id) {
                    pane_set.insert(pane_id);
                }
            }
            let desired_ids: Vec<PaneId> = pane_set.into_iter().collect();
            let current_matches = self
                .fleet
                .nodes
                .get(&node_name)
                .is_some_and(|n| match &n.kind {
                    crate::remote::FleetNodeKind::SshTmux { pane_ids, .. } => {
                        *pane_ids == desired_ids
                    }
                    _ => false,
                });
            if !current_matches || old_ids != desired_ids {
                self.fleet.register_ssh_tmux(
                    node_name.clone(),
                    target.dest().to_string(),
                    target.session.clone(),
                    desired_ids,
                    target.tags.clone(),
                );
                if let Some(node) = self.fleet.nodes.get_mut(&node_name) {
                    node.connected = old_connected;
                    node.last_dispatched_command = old_last_cmd;
                }
            }
        }
        self.tmux_link.aux.last_mapped_count = self.tmux_bridge.mapped_panes().len();
    }

    /// Converts typed bytes of tapped panes into `send-keys` (or raw ssh input while connecting).
    pub(crate) fn pump_tmux_input(&mut self, now_ms: u64) {
        self.sync_io_generation();
        self.reap_removed_tmux_panes_and_sync_fleet();
        self.sync_hydrate_greeting(now_ms);
        self.update_mapped_set_timestamp(now_ms);

        if self.tmux_link.target.is_some()
            && let Some(primary) = self.tmux_link.primary_pane
            && !self.tmux_link.orphans.contains(&primary)
            && let Some(pane) = self.panes.get_mut(&primary)
        {
            pane.set_remote_input_tap(true);
        }
        for (_, pane_id) in self.tmux_bridge.mapped_panes() {
            if let Some(pane) = self.panes.get_mut(&pane_id) {
                pane.set_remote_input_tap(true);
            }
        }

        let pane_ids: Vec<PaneId> = self.panes.keys().copied().collect();
        let state = *self.tmux_link.machine.state();
        let has_target = self.tmux_link.target.is_some();

        for pane_id in pane_ids {
            let Some(pane) = self.panes.get_mut(&pane_id) else {
                continue;
            };
            if !pane.remote_input_tap() {
                continue;
            }
            let fresh = pane.take_remote_input();
            let mut bytes = self.tmux_link.aux.input_backlog.remove(&pane_id).unwrap_or_default();
            if bytes.len().saturating_add(fresh.len()) > INPUT_BACKLOG_CAP {
                pane.set_status_message(Some(INPUT_OVERFLOW_STATUS.to_string()));
            } else {
                bytes.extend_from_slice(&fresh);
            }
            if bytes.is_empty() {
                continue;
            }
            let is_primary = self.tmux_link.primary_pane == Some(pane_id);
            let mut mapped_tmux = self.tmux_bridge.bilterm_to_tmux.get(&pane_id).copied();
            if mapped_tmux.is_none() && is_primary && matches!(state, LinkState::Connected { .. }) {
                mapped_tmux = self.ensure_provisional_primary();
            }

            if matches!(
                state,
                LinkState::Connecting {
                    awaiting_control: true,
                    ..
                }
            ) && is_primary
                && mapped_tmux.is_none()
            {
                if let Some(driver) = self.tmux_link.driver.as_mut() {
                    let _ = driver.write_raw(&bytes);
                }
            } else if let Some(tmux_pane) = mapped_tmux
                && (matches!(state, LinkState::Connected { .. } | LinkState::Connecting { .. })
                    || (!has_target && matches!(state, LinkState::Idle)))
            {
                // Never queue more than the command queue can hold (it drops its oldest entries
                // when full, which would lose typed bytes); the rest waits for the next tick.
                let room = (OUTBOUND_CAP / 2).saturating_sub(self.tmux_bridge.outbound_commands.len());
                let take = room.saturating_mul(SEND_KEYS_CHUNK).min(bytes.len());
                for chunk in bytes[..take].chunks(SEND_KEYS_CHUNK) {
                    let cmd = format_send_keys_hex(tmux_pane, chunk);
                    self.tmux_bridge.queue_command(cmd, ReplyKind::Ignore);
                }
                if take < bytes.len() {
                    self.tmux_link.aux.input_backlog.insert(pane_id, bytes.split_off(take));
                }
            } else if let Some(pane) = self.panes.get_mut(&pane_id)
                && pane.status_message() != Some(LINK_DOWN_INPUT_STATUS)
            {
                pane.set_status_message(Some(LINK_DOWN_INPUT_STATUS.to_string()));
            }
        }

        self.pump_tmux_resize();
    }

    /// Writes queued tmux commands to the transport and registers their reply kinds.
    pub(crate) fn pump_tmux_outbound(&mut self) {
        self.sync_io_generation();
        if self.tmux_bridge.blocks_closed() > 0 {
            self.tmux_link.hydrate.note_block_end();
        }
        if self.tmux_bridge.outbound_commands.len() >= OUTBOUND_CAP {
            self.note_outbound_overflow();
        }
        let state = *self.tmux_link.machine.state();
        let can_send = matches!(state, LinkState::Connected { .. })
            || (self.tmux_link.target.is_none() && matches!(state, LinkState::Idle));
        if !can_send || self.tmux_link.hydrate.phase() == HydratePhase::AwaitingGreeting {
            return;
        }
        let Some(subproc) = self.tmux_control.as_ref() else {
            return;
        };
        let tx = subproc.cmd_tx.clone();
        for cmd in self.tmux_bridge.take_outbound_commands() {
            let _ = tx.send(cmd);
        }
    }

    /// Feeds correlated replies and drives the attach hydrate.
    pub(crate) fn pump_tmux_replies(&mut self, now_ms: u64) {
        self.sync_io_generation();
        self.sync_hydrate_greeting(now_ms);
        self.update_mapped_set_timestamp(now_ms);

        let mapped = self.tmux_bridge.mapped_panes();
        if self.tmux_link.hydrate.phase() == HydratePhase::Ready
            && !mapped.is_empty()
            && now_ms
                >= self
                    .tmux_link
                    .aux
                    .last_new_pane_ms
                    .saturating_add(HYDRATE_SETTLE_MS)
        {
            let tmux_ids: Vec<usize> = mapped.iter().map(|&(t, _)| t).collect();
            let (rows, cols) = self.link_client_size();
            let marker = self.link_state_marker();
            let planned = self.tmux_link.hydrate.plan_with_state(&tmux_ids, cols, rows, &marker);
            self.tmux_link.aux.sent_sizes.clear();
            self.tmux_link.aux.sent_sizes.insert(None, (rows, cols));
            self.tmux_link
                .aux
                .notes
                .insert("hydrated_panes".to_string(), "0".to_string());
            self.tmux_link
                .aux
                .notes
                .insert("hydrated_lines".to_string(), "0".to_string());
            for (cmd, kind) in planned {
                if let ReplyKind::CapturePane { tmux_pane } = kind {
                    self.tmux_bridge.hold_output(tmux_pane);
                }
                let will_overflow = self.tmux_bridge.outbound_commands.len() >= OUTBOUND_CAP;
                self.tmux_bridge.queue_command(cmd, kind);
                if will_overflow {
                    self.note_outbound_overflow();
                }
            }
            self.tmux_link.hydrate.mark_sent();
            self.tmux_link.aux.hydrate_sent_ms = Some(now_ms);
            self.pump_tmux_outbound();
        }

        let was_done = self.tmux_link.hydrate.phase() == HydratePhase::Done;
        let replies = self.tmux_bridge.take_replies();
        let mut hydrated_panes = self
            .tmux_link
            .aux
            .notes
            .get("hydrated_panes")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let mut hydrated_lines = self
            .tmux_link
            .aux
            .notes
            .get("hydrated_lines")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        for reply in replies {
            if reply.kind == ReplyKind::ListPanes {
                if reply.ok {
                    self.apply_list_panes_reply(&reply.lines);
                }
                continue;
            }
            if reply.kind == ReplyKind::PaneStates {
                self.note_pane_states_reply(reply.ok, &reply.lines);
                continue;
            }
            if let ReplyKind::CapturePane { tmux_pane } = reply.kind {
                let capped = if reply.lines.len() > HYDRATE_REPLY_LINES_CAP {
                    &reply.lines[..HYDRATE_REPLY_LINES_CAP]
                } else {
                    &reply.lines[..]
                };
                if reply.ok {
                    if let Some(&pane_id) = self.tmux_bridge.tmux_to_bilterm.get(&tmux_pane)
                        && let Some(pane) = self.panes.get_mut(&pane_id)
                    {
                        let rendered = match self.tmux_link.aux.pane_states.remove(&tmux_pane) {
                            Some(st) => crate::tmux::pane_state::render_capture_with_state(capped, &st),
                            None => render_capture(capped),
                        };
                        if !rendered.is_empty() {
                            pane.count_payload_rx(rendered.len());
                            pane.process_pty_output(&rendered);
                        }
                    }
                    hydrated_panes += 1;
                    hydrated_lines += capped.len();
                }
                self.tmux_bridge.release_output(tmux_pane);
                self.tmux_link.hydrate.note_capture_done(tmux_pane);
            }
        }

        self.tmux_link
            .aux
            .notes
            .insert("hydrated_panes".to_string(), hydrated_panes.to_string());
        self.tmux_link
            .aux
            .notes
            .insert("hydrated_lines".to_string(), hydrated_lines.to_string());

        if !was_done && self.tmux_link.hydrate.phase() == HydratePhase::Done {
            let orphans = self.tmux_bridge.finish_reconcile();
            for orphan_id in orphans {
                if let Some(pane) = self.panes.get_mut(&orphan_id) {
                    pane.set_remote_input_tap(false);
                    pane.set_link_banner(Some(LinkBanner {
                        text: ORPHAN_BANNER_TEXT.to_string(),
                        interactive: false,
                    }));
                }
                self.tmux_link.orphans.insert(orphan_id);
            }
            tracing::debug!(
                target: "bilterm::link",
                panes = hydrated_panes,
                lines = hydrated_lines,
                "link.hydrate"
            );
            // Flow control starts once the screen is in place (tmux 3.2+; older servers answer
            // `%error`, which is ignored).
            self.tmux_bridge
                .queue_command(crate::tmux::decode::pause_after_command(), ReplyKind::Ignore);
        }
    }
}
