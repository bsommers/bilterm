//! Lifecycle of the persistent SSH + tmux link (Phase 51.1, PST-01, PST-02, PST-04).
//!
//! Coordinates `connect_ssh_tmux`, transport spawning through [`LinkConnector`] (default:
//! [`PtyLinkConnector`] in live mode, none in headless mode unless injected), the pure
//! [`LinkMachine`] supervisor, single-report disconnect classification (`on_tmux_control_down`),
//! per-pane banners, user verbs (`link_reconnect`, `link_disconnect`, `tmux_link_forget`) and
//! structured `bilterm::link` tracing without leaking secrets.

use crate::error::{BiltermError, Result};
use crate::pane::PaneId;
use crate::remote::{RemoteHostProfile, redact_ssh_argv};
use crate::tmux::link::{
    DIAG_LINE_CAP, DIAG_TAIL_LINES, DisconnectCause, ExitClass, LinkCmd, LinkConnector, LinkEvent,
    LinkExitSummary, LinkMachine, LinkState, LinkSummary, LinkTarget, ReconnectPolicy,
    TmuxDisconnect, TmuxLinkState,
};
use crate::tmux::reconnect::{classify_exit, disconnect_text, link_banner_text};
use crate::tmux::transport::PtyLinkConnector;
use crate::vt::sanitize_display;
use crate::wm::{SplitDirection, WindowManager};

/// Characters of a remote-controlled name kept in titles and status lines.
const NAME_CAP: usize = 120;

fn exit_class_str(class: ExitClass) -> &'static str {
    match class {
        ExitClass::Transient => "transient",
        ExitClass::Fatal => "fatal",
        ExitClass::Ended => "ended",
        ExitClass::User => "user",
    }
}

impl WindowManager {
    /// Milliseconds since the link epoch (the clock all link deadlines use).
    pub(crate) fn link_now_ms(&self) -> u64 {
        if let Some(tick_ms) = self
            .tmux_link
            .aux
            .notes
            .get("tick_now_ms")
            .and_then(|s| s.parse::<u64>().ok())
            && self.headless_mode
        {
            return tick_ms;
        }
        self.tmux_link.epoch.elapsed().as_millis() as u64
    }

    /// Body of [`WindowManager::connect_ssh_tmux`]: validates the target, enforces the single-link
    /// rule (or reuses the primary pane when reconnecting to the same inactive target), creates
    /// the primary pane + fleet node, and runs the initial `Connect` transition.
    pub(crate) fn connect_ssh_tmux_body(
        &mut self,
        target: &str,
        session_name: &str,
        tags: Vec<String>,
    ) -> Result<PaneId> {
        let profile = self
            .fleet
            .profiles
            .get(target)
            .cloned()
            .or_else(|| self.config.remotes.get(target).cloned())
            .unwrap_or_else(|| RemoteHostProfile::new(target, target));
        let dest = profile.destination.clone();
        crate::remote::validate_destination(&dest)?;

        // Check if a fleet node for the link was removed via `disconnect_fleet_node` (`:remote disconnect`):
        if let Some(ref fname) = self.tmux_link.fleet_node
            && !self.fleet.nodes.contains_key(fname)
        {
            self.tmux_link_forget();
        }

        if self.tmux_link.machine.state().is_active() {
            let (cur_dest, cur_session) = self
                .tmux_link
                .target
                .as_ref()
                .map(|t| (t.dest().to_string(), t.session.clone()))
                .unwrap_or_default();
            return Err(BiltermError::Tmux(format!(
                "a tmux link to {}/{} is already active; run :ssh-tmux disconnect first",
                sanitize_display(&cur_dest, NAME_CAP),
                sanitize_display(&cur_session, NAME_CAP)
            )));
        }

        // If an inactive link record exists (`Failed`, `Detached`, `Ended`), same target reuses the
        // primary pane via `link_reconnect`; a different target is refused until `tmux_link_forget`.
        if let Some(existing) = self.tmux_link.target.as_ref() {
            if existing.dest() == dest && existing.session == session_name {
                if let Some(id) = self.tmux_link.primary_pane
                    && self.panes.contains_key(&id)
                {
                    self.link_reconnect();
                    return Ok(id);
                }
            } else {
                let (cur_dest, cur_session) = (existing.dest().to_string(), existing.session.clone());
                return Err(BiltermError::Tmux(format!(
                    "a tmux link to {}/{} is already configured; run :remote disconnect tmux-{}-{} first",
                    sanitize_display(&cur_dest, NAME_CAP),
                    sanitize_display(&cur_session, NAME_CAP),
                    sanitize_display(&cur_dest, NAME_CAP),
                    sanitize_display(&cur_session, NAME_CAP)
                )));
            }
        }

        let new_id = self.split_focused(SplitDirection::Vertical)?;
        let badge = format!("{dest}/{session_name}");
        let shown_dest = sanitize_display(&dest, NAME_CAP);
        let shown_session = sanitize_display(session_name, NAME_CAP);
        if let Some(pane) = self.panes.get_mut(&new_id) {
            pane.set_title(format!(
                "TMUX-SSH:{} [#{new_id}]",
                sanitize_display(&badge, 2 * NAME_CAP)
            ));
            pane.set_remote_target(Some(badge.clone()));
            pane.set_status_message(Some(format!(
                "Connecting to {shown_dest} (tmux session '{shown_session}')..."
            )));
        }
        let mut merged_tags = profile.tags.clone();
        for t in tags {
            if !merged_tags.contains(&t) {
                merged_tags.push(t);
            }
        }
        let node_name = format!("tmux-{dest}-{session_name}");
        self.fleet.register_ssh_tmux(
            node_name.clone(),
            dest.clone(),
            session_name,
            vec![new_id],
            merged_tags.clone(),
        );
        if let Some(node) = self.fleet.nodes.get_mut(&node_name) {
            // Honest until the control stream starts (`on_tmux_control_line`).
            node.connected = false;
        }
        self.reset_link_record(LinkTarget {
            profile,
            session: session_name.to_string(),
            tags: merged_tags,
        });
        self.tmux_link.primary_pane = Some(new_id);
        self.tmux_link.fleet_node = Some(node_name);
        self.tmux_bridge.set_primary(new_id);
        self.tmux_bridge.set_badge(Some(badge));

        let now = self.link_now_ms();
        let cmds = self.tmux_link.machine.on_event(LinkEvent::Connect, now);
        self.execute_link_cmds(cmds, now);
        let _ = self.refresh_link_banners(now);
        Ok(new_id)
    }

    /// Replaces the link record for a fresh connect, keeping the connector, policy and epoch.
    fn reset_link_record(&mut self, target: LinkTarget) {
        if let Some(d) = self.tmux_link.driver.as_mut() {
            d.kill();
        }
        let old = std::mem::take(&mut self.tmux_link);
        self.tmux_link = TmuxLinkState {
            target: Some(target),
            machine: LinkMachine::new(old.machine.policy().clone()),
            connector: old.connector,
            epoch: old.epoch,
            generation: old.generation + 1,
            ..TmuxLinkState::default()
        };
        self.tmux_control = None;
    }

    /// Current link state (`Idle` when no link was ever started).
    pub fn tmux_link_state(&self) -> &LinkState {
        if let Some(ref fname) = self.tmux_link.fleet_node
            && !self.fleet.nodes.contains_key(fname)
        {
            return &LinkState::Idle;
        }
        self.tmux_link.machine.state()
    }

    /// Installs the connector used to start (and restart) transports.
    pub fn set_link_connector(&mut self, c: Box<dyn LinkConnector>) {
        self.tmux_link.connector = Some(c);
    }

    /// Replaces the reconnect policy (tests use short delays).
    pub fn set_link_policy(&mut self, policy: ReconnectPolicy) {
        self.tmux_link.machine.policy = policy;
    }

    /// Executes side-effect commands emitted by [`LinkMachine`].
    fn execute_link_cmds(&mut self, cmds: Vec<LinkCmd>, now_ms: u64) {
        for cmd in cmds {
            match cmd {
                LinkCmd::Spawn => self.spawn_transport(now_ms),
                LinkCmd::KillTransport => self.kill_transport(),
                LinkCmd::MarkConnected => self.apply_mark_connected(now_ms),
                LinkCmd::MarkDown { retry_in_ms } => self.apply_mark_down(retry_in_ms),
                LinkCmd::MarkFailed => self.apply_mark_failed(),
                LinkCmd::MarkEnded => self.apply_mark_ended(),
                LinkCmd::MarkDetached => self.apply_mark_detached(),
            }
        }
    }

    /// Terminates any active driver and drops the control channel.
    fn kill_transport(&mut self) {
        if let Some(d) = self.tmux_link.driver.as_mut() {
            d.kill();
        }
        self.tmux_link.driver = None;
        self.tmux_control = None;
        self.set_fleet_connected(false);
    }

    /// Spawns a new transport attempt via the injected connector or `PtyLinkConnector`.
    fn spawn_transport(&mut self, now_ms: u64) {
        let Some(target) = self.tmux_link.target.clone() else {
            return;
        };
        let attempt = self.tmux_link.machine.attempt();
        let shown_dest = sanitize_display(target.dest(), NAME_CAP);
        let shown_session = sanitize_display(&target.session, NAME_CAP);

        if let Some(id) = self.tmux_link.primary_pane {
            self.tmux_bridge.set_primary(id);
            if let Some(pane) = self.panes.get_mut(&id) {
                pane.set_remote_input_tap(true);
                pane.set_link_raw_input(true);
                let connecting = format!("Connecting to {shown_dest} (tmux session '{shown_session}')...");
                pane.set_status_message(Some(connecting.clone()));
                self.tmux_link.aux.last_status = Some(connecting);
            }
        }
        self.tmux_bridge.set_badge(Some(target.badge()));
        self.set_fleet_connected(false);

        let argv_redacted = target
            .profile
            .build_ssh_tmux_cc_args(&target.session, None)
            .map(|a| redact_ssh_argv(&a))
            .unwrap_or_default();

        if self.tmux_link.connector.is_none() && self.headless_mode {
            // Headless without an injected connector: keep the `Connecting to ...` status on the
            // primary pane and wait for tests to attach a `from_channels` transport if desired.
            self.tmux_link.generation += 1;
            self.tmux_link.down_reported_generation = None;
            self.tmux_link.diag_tail.clear();
            self.tmux_link.aux.control_seen = false;
            let _ = format!("Link {shown_dest}/{shown_session}: no transport (headless)");
            return;
        }

        tracing::info!(
            target: "bilterm::link",
            dest = %shown_dest,
            session = %shown_session,
            attempt = attempt,
            argv = %argv_redacted,
            "link.connect"
        );

        let size = self.default_size;
        let res = if let Some(conn) = self.tmux_link.connector.as_mut() {
            conn.connect(&target, size)
        } else {
            let mut default_conn = PtyLinkConnector::default();
            default_conn.connect(&target, size)
        };

        self.tmux_link.generation += 1;
        self.tmux_link.down_reported_generation = None;
        self.tmux_link.diag_tail.clear();
        self.tmux_link.aux.control_seen = false;

        match res {
            Ok(transport) => {
                self.tmux_link.driver = transport.driver;
                self.attach_tmux_control(transport.control);
            }
            Err(err) => {
                let msg = sanitize_display(&err.to_string(), DIAG_LINE_CAP);
                tracing::warn!(
                    target: "bilterm::link",
                    error = %msg,
                    "link.spawn_failed"
                );
                let down = TmuxDisconnect {
                    cause: DisconnectCause::SpawnFailed(msg),
                    reason: None,
                    code: None,
                    tail: Vec::new(),
                    at_ms: now_ms,
                    class: ExitClass::Fatal,
                };
                self.on_tmux_control_down(down);
            }
        }
    }

    fn apply_mark_connected(&mut self, now_ms: u64) {
        let Some(target) = self.tmux_link.target.as_ref() else {
            return;
        };
        let (dest, session) = (
            sanitize_display(target.dest(), NAME_CAP),
            sanitize_display(&target.session, NAME_CAP),
        );
        self.tmux_link.aux.control_seen = true;
        if let Some(id) = self.tmux_link.primary_pane
            && let Some(pane) = self.panes.get_mut(&id)
        {
            pane.set_link_raw_input(false);
            pane.set_remote_input_tap(true);
            pane.set_link_banner(None);
        }
        for &id in self.tmux_bridge.bilterm_to_tmux.keys() {
            if let Some(pane) = self.panes.get_mut(&id) {
                pane.set_link_banner(None);
            }
        }
        for id in self.tmux_bridge.stale_panes() {
            if let Some(pane) = self.panes.get_mut(&id) {
                pane.set_link_banner(None);
            }
        }
        self.tmux_link.aux.last_banner = None;
        let text = format!("Connected: tmux '{session}' on {dest}");
        self.set_link_status(text);
        self.set_fleet_connected(true);
        tracing::info!(
            target: "bilterm::link",
            dest = %dest,
            session = %session,
            after_ms = now_ms,
            "link.up"
        );
    }

    fn apply_mark_down(&mut self, retry_in_ms: u64) {
        self.kill_transport();
        let Some(target) = self.tmux_link.target.as_ref() else {
            return;
        };
        let dest = sanitize_display(target.dest(), NAME_CAP);
        let attempt = self.tmux_link.machine.attempt();
        let max = self.tmux_link.machine.policy().max_attempts;
        let secs = retry_in_ms.div_ceil(1000);
        let cause = self
            .tmux_link
            .last_exit
            .as_ref()
            .map_or_else(|| "connection lost".to_string(), disconnect_text);
        tracing::info!(
            target: "bilterm::link",
            attempt = attempt,
            max = max,
            delay_ms = retry_in_ms,
            "link.retry"
        );
        let text = format!("Link to {dest} lost ({cause}); retry {attempt}/{max} in {secs}s");
        self.set_link_status(text);
    }

    fn apply_mark_failed(&mut self) {
        self.kill_transport();
        let Some(target) = self.tmux_link.target.as_ref() else {
            return;
        };
        let dest = sanitize_display(target.dest(), NAME_CAP);
        let attempts = self.tmux_link.machine.attempt();
        let max = self.tmux_link.machine.policy().max_attempts;
        if attempts >= max && max > 0 {
            tracing::info!(
                target: "bilterm::link",
                attempts = attempts,
                "link.give_up"
            );
        }
        if let Some(id) = self.tmux_link.primary_pane
            && let Some(pane) = self.panes.get_mut(&id)
        {
            pane.set_link_raw_input(false);
        }
        let cause = self
            .tmux_link
            .last_exit
            .as_ref()
            .map_or_else(|| "unknown error".to_string(), disconnect_text);
        let text = format!("Link to {dest} failed: {cause}");
        self.set_link_status(text);
    }

    fn apply_mark_ended(&mut self) {
        self.kill_transport();
        let Some(target) = self.tmux_link.target.as_ref() else {
            return;
        };
        let dest = sanitize_display(target.dest(), NAME_CAP);
        let session = sanitize_display(&target.session, NAME_CAP);
        if let Some(id) = self.tmux_link.primary_pane
            && let Some(pane) = self.panes.get_mut(&id)
        {
            pane.set_link_raw_input(false);
        }
        let text = format!("Remote tmux session '{session}' on {dest} ended");
        self.set_link_status(text);
    }

    fn apply_mark_detached(&mut self) {
        self.kill_transport();
        let Some(target) = self.tmux_link.target.as_ref() else {
            return;
        };
        let dest = sanitize_display(target.dest(), NAME_CAP);
        let session = sanitize_display(&target.session, NAME_CAP);
        if let Some(id) = self.tmux_link.primary_pane
            && let Some(pane) = self.panes.get_mut(&id)
        {
            pane.set_link_raw_input(false);
        }
        let text = format!("Disconnected from {dest} (tmux session '{session}' kept on the remote)");
        self.set_link_status(text);
    }

    fn set_link_status(&mut self, text: String) {
        if let Some(id) = self.tmux_link.primary_pane
            && let Some(pane) = self.panes.get_mut(&id)
        {
            pane.set_status_message(Some(text.clone()));
        }
        self.active_pane_mut().set_status_message(Some(text.clone()));
        self.tmux_link.aux.last_status = Some(text);
    }

    fn set_fleet_connected(&mut self, connected: bool) {
        if let Some(name) = self.tmux_link.fleet_node.clone()
            && let Some(node) = self.fleet.nodes.get_mut(&name)
        {
            node.connected = connected;
        }
    }

    /// Recomputes and applies the link banner across primary, mapped and stale panes.
    /// Orphans keep their own `REMOTE PANE GONE` banner. Returns `true` when banner text changed.
    fn refresh_link_banners(&mut self, now_ms: u64) -> bool {
        let Some(target) = self.tmux_link.target.as_ref() else {
            return false;
        };
        let banner = link_banner_text(
            self.tmux_link.machine.state(),
            target,
            self.tmux_link.last_exit.as_ref(),
            self.tmux_link.machine.policy(),
            now_ms,
        );
        let new_text = banner.as_ref().map(|b| b.text.clone());
        let text_changed = new_text != self.tmux_link.aux.last_banner;
        self.tmux_link.aux.last_banner = new_text;

        let mut pane_ids = Vec::new();
        if let Some(p) = self.tmux_link.primary_pane {
            pane_ids.push(p);
        }
        for &p in self.tmux_bridge.bilterm_to_tmux.keys() {
            if !pane_ids.contains(&p) {
                pane_ids.push(p);
            }
        }
        for p in self.tmux_bridge.stale_panes() {
            if !pane_ids.contains(&p) {
                pane_ids.push(p);
            }
        }

        for id in pane_ids {
            if self.tmux_link.orphans.contains(&id) {
                continue;
            }
            if let Some(pane) = self.panes.get_mut(&id)
                && pane.link_banner() != banner.as_ref()
            {
                pane.set_link_banner(banner.clone());
            }
        }
        text_changed
    }

    /// One link tick: driver pump, `poll_tmux_control`, pumps, machine deadlines, banners.
    /// Returns `true` when something visible changed.
    pub fn tick_tmux_link(&mut self, now_ms: u64) -> bool {
        if let Some(ref fname) = self.tmux_link.fleet_node
            && !self.fleet.nodes.contains_key(fname)
        {
            self.tmux_link_forget();
            return true;
        }

        self.tmux_link
            .aux
            .notes
            .insert("tick_now_ms".to_string(), now_ms.to_string());
        let mut changed = false;
        let prev_status = self.tmux_link.aux.last_status.clone();

        // 1. Driver pump: raw output -> primary pane while awaiting control, diag -> diag_tail,
        //    control_started -> on_tmux_control_line, exited -> held until after poll_tmux_control.
        let mut driver_exit = None;
        if let Some(driver) = self.tmux_link.driver.as_mut() {
            let poll = driver.pump();
            if !poll.raw_output.is_empty()
                && matches!(self.tmux_link.machine.state(), LinkState::Connecting { .. })
                && let Some(id) = self.tmux_link.primary_pane
                && let Some(pane) = self.panes.get_mut(&id)
            {
                pane.count_payload_rx(poll.raw_output.len());
                pane.process_pty_output(&poll.raw_output);
                changed = true;
            }
            for d in poll.diag {
                if self.tmux_link.diag_tail.len() >= DIAG_TAIL_LINES {
                    self.tmux_link.diag_tail.pop_front();
                }
                self.tmux_link
                    .diag_tail
                    .push_back(sanitize_display(&d, DIAG_LINE_CAP));
            }
            if poll.control_started {
                self.on_tmux_control_line_at(now_ms);
                changed = true;
            }
            driver_exit = poll.exited;
        }

        // 2. Control stream poll:
        let prev_state = *self.tmux_link.machine.state();
        if self.poll_tmux_control() > 0 {
            changed = true;
            self.on_tmux_control_line_at(now_ms);
        }

        // Report driver exit after control lines were drained (once per generation):
        if let Some(exit) = driver_exit {
            let tail = self.tmux_link.diag_tail.iter().cloned().collect();
            let down = TmuxDisconnect {
                cause: DisconnectCause::ProcessExit(exit.code),
                reason: None,
                code: exit.code,
                tail,
                at_ms: now_ms,
                class: ExitClass::Transient,
            };
            self.on_tmux_control_down(down);
        }

        // 3. I/O pumps (plan 13):
        self.pump_tmux_input(now_ms);
        self.pump_tmux_outbound();
        self.pump_tmux_replies(now_ms);

        // 4. Machine tick (backoff deadline -> Spawn):
        let cmds = self.tmux_link.machine.tick(now_ms);
        if !cmds.is_empty() {
            self.execute_link_cmds(cmds, now_ms);
            changed = true;
        }

        // 5. Banners and status change detection:
        if self.refresh_link_banners(now_ms) {
            changed = true;
        }
        if *self.tmux_link.machine.state() != prev_state
            || self.tmux_link.aux.last_status != prev_status
        {
            changed = true;
        }
        changed
    }

    /// Called when a control transport is attached while a link record exists.
    pub(crate) fn on_tmux_control_attached(&mut self) {
        self.tmux_link.aux.control_seen = false;
    }

    /// First control line of a transport: `Connecting` becomes `Connected`.
    pub(crate) fn on_tmux_control_line(&mut self) {
        let now = self.link_now_ms();
        self.on_tmux_control_line_at(now);
    }

    fn on_tmux_control_line_at(&mut self, now_ms: u64) {
        if self.tmux_link.target.is_none() {
            return;
        }
        if !matches!(self.tmux_link.machine.state(), LinkState::Connecting { .. }) {
            return;
        }
        let cmds = self
            .tmux_link
            .machine
            .on_event(LinkEvent::ControlStarted, now_ms);
        self.execute_link_cmds(cmds, now_ms);
    }

    /// A transport loss was detected by `poll_tmux_control` (plan 05) or the driver (plan 12).
    /// Reported at most once per generation.
    pub(crate) fn on_tmux_control_down(&mut self, mut d: TmuxDisconnect) {
        let cur_gen = self.tmux_link.generation;
        if self.tmux_link.down_reported_generation == Some(cur_gen) {
            return;
        }
        self.tmux_link.down_reported_generation = Some(cur_gen);

        if d.tail.is_empty() && !self.tmux_link.diag_tail.is_empty() {
            d.tail = self.tmux_link.diag_tail.iter().cloned().collect();
        }
        d.class = classify_exit(&d.cause, d.reason.as_deref(), d.code, &d.tail);

        let (dest, session) = self
            .tmux_link
            .target
            .as_ref()
            .map(|t| {
                (
                    sanitize_display(t.dest(), NAME_CAP),
                    sanitize_display(&t.session, NAME_CAP),
                )
            })
            .unwrap_or_default();

        if matches!(d.cause, DisconnectCause::ControlExit) {
            tracing::info!(
                target: "bilterm::link",
                reason = ?d.reason,
                "link.control_exit"
            );
        }
        tracing::info!(
            target: "bilterm::link",
            dest = %dest,
            session = %session,
            cause = %disconnect_text(&d),
            class = %exit_class_str(d.class),
            code = ?d.code,
            reason = ?d.reason,
            tail_lines = d.tail.len(),
            "link.down"
        );

        let at_ms = d.at_ms;
        self.tmux_link.last_exit = Some(d.clone());
        let cmds = self.tmux_link.machine.on_event(LinkEvent::Down(d), at_ms);
        self.execute_link_cmds(cmds, at_ms);
        let _ = self.refresh_link_banners(at_ms);
    }

    /// Reconnects now, resetting the attempt counter and spawning immediately from any state.
    pub fn link_reconnect(&mut self) -> String {
        if let Some(ref fname) = self.tmux_link.fleet_node
            && !self.fleet.nodes.contains_key(fname)
        {
            self.tmux_link_forget();
        }
        let Some(target) = self.tmux_link.target.clone() else {
            let msg = "No tmux link to reconnect (use :ssh-tmux <dest> [session])".to_string();
            self.active_pane_mut().set_status_message(Some(msg.clone()));
            return msg;
        };
        let now = self.link_now_ms();
        let cmds = self
            .tmux_link
            .machine
            .on_event(LinkEvent::UserReconnect, now);
        self.execute_link_cmds(cmds, now);
        let _ = self.refresh_link_banners(now);
        let msg = self.tmux_link.aux.last_status.clone().unwrap_or_else(|| {
            format!(
                "Connecting to {} (tmux session '{}')...",
                sanitize_display(target.dest(), NAME_CAP),
                sanitize_display(&target.session, NAME_CAP)
            )
        });
        self.active_pane_mut().set_status_message(Some(msg.clone()));
        msg
    }

    /// Stops the link, keeping the remote tmux session and the mapped panes.
    pub fn link_disconnect(&mut self) -> String {
        if let Some(ref fname) = self.tmux_link.fleet_node
            && !self.fleet.nodes.contains_key(fname)
        {
            self.tmux_link_forget();
        }
        let Some(target) = self.tmux_link.target.clone() else {
            let msg = "No tmux link to disconnect".to_string();
            self.active_pane_mut().set_status_message(Some(msg.clone()));
            return msg;
        };
        let now = self.link_now_ms();
        let cmds = self
            .tmux_link
            .machine
            .on_event(LinkEvent::UserDisconnect, now);
        if cmds.is_empty() {
            // Already Detached: ensure status and banners reflect Detached.
            self.apply_mark_detached();
        } else {
            self.execute_link_cmds(cmds, now);
        }
        let _ = self.refresh_link_banners(now);
        let msg = format!(
            "Disconnected from {} (tmux session '{}' kept on the remote)",
            sanitize_display(target.dest(), NAME_CAP),
            sanitize_display(&target.session, NAME_CAP)
        );
        self.set_link_status(msg.clone());
        msg
    }

    /// Snapshot for `:ssh-tmux info`, `remote.link` and the drawer.
    pub fn tmux_link_summary(&self) -> LinkSummary {
        let l = &self.tmux_link;
        if let Some(ref fname) = l.fleet_node
            && !self.fleet.nodes.contains_key(fname)
        {
            return LinkSummary {
                max_attempts: l.machine.policy().max_attempts,
                ..LinkSummary::default()
            };
        }
        let now = self.link_now_ms();
        let mut s = LinkSummary {
            state: l.machine.state().as_str(),
            attempt: l.machine.attempt(),
            max_attempts: l.machine.policy().max_attempts,
            retry_in_ms: l.machine.retry_in_ms(now),
            primary_pane: l.primary_pane,
            stale_panes: self.tmux_bridge.stale_panes(),
            last_exit: l.last_exit.as_ref().map(|d| LinkExitSummary {
                cause: disconnect_text(d),
                class: exit_class_str(d.class),
                code: d.code,
                reason: d.reason.clone(),
                tail: d.tail.clone(),
                at_ms: d.at_ms,
            }),
            ..LinkSummary::default()
        };
        if let Some(t) = l.target.as_ref() {
            s.dest = Some(sanitize_display(t.dest(), NAME_CAP));
            s.session = Some(sanitize_display(&t.session, NAME_CAP));
            s.panes = self
                .tmux_bridge
                .mapped_panes()
                .into_iter()
                .map(|(t, p)| (p, t))
                .collect();
        }
        s
    }

    /// `:remote disconnect` of the link's fleet node: back to `Idle`, transport killed, panes untouched.
    pub(crate) fn tmux_link_forget(&mut self) {
        if let Some(d) = self.tmux_link.driver.as_mut() {
            d.kill();
        }
        let old = std::mem::take(&mut self.tmux_link);
        self.tmux_link = TmuxLinkState {
            machine: LinkMachine::new(old.machine.policy().clone()),
            last_exit: old.last_exit,
            connector: old.connector,
            epoch: old.epoch,
            generation: old.generation + 1,
            ..TmuxLinkState::default()
        };
        self.tmux_control = None;
    }

    /// `true` when `id` is mapped in the bridge (or is the primary) while the link is `Connected`.
    pub fn pane_is_linked(&self, id: PaneId) -> bool {
        matches!(self.tmux_link.machine.state(), LinkState::Connected { .. })
            && (self.tmux_link.primary_pane == Some(id)
                || self.tmux_bridge.bilterm_to_tmux.contains_key(&id))
    }
}
