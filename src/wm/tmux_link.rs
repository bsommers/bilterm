//! Lifecycle of the persistent SSH + tmux link (Phase 51.1; plan 12 fills the skeleton bodies).
//!
//! Seam state (plan 01): `connect_ssh_tmux_body` is real (honest status, link record, primary
//! pane, fleet node) and `tick_tmux_link` drives `poll_tmux_control` plus the first-line hook, so
//! a headless `from_channels` transport reaches `Connected`. Reconnect, disconnect, transport
//! spawn and backoff are plan 12.

use crate::error::{BiltermError, Result};
use crate::pane::PaneId;
use crate::remote::RemoteHostProfile;
use crate::tmux::link::{
    LinkCmd, LinkConnector, LinkEvent, LinkMachine, LinkState, LinkSummary, LinkTarget,
    ReconnectPolicy, TmuxDisconnect, TmuxLinkState,
};
use crate::vt::sanitize_display;
use crate::wm::{SplitDirection, WindowManager};

/// Characters of a remote-controlled name kept in titles and status lines.
const NAME_CAP: usize = 120;

impl WindowManager {
    /// Milliseconds since the link epoch (the clock all link deadlines use).
    pub(crate) fn link_now_ms(&self) -> u64 {
        self.tmux_link.epoch.elapsed().as_millis() as u64
    }

    /// Body of [`WindowManager::connect_ssh_tmux`]: records the link, creates the primary pane
    /// and reports honestly (`Connecting ...`) until the control stream starts. Headless: no
    /// transport is spawned (the tests attach a `from_channels` transport).
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
        let now = self.link_now_ms();
        let _cmds = self.tmux_link.machine.on_event(LinkEvent::Connect, now);
        self.tmux_bridge.set_primary(new_id);
        self.tmux_bridge.set_badge(Some(badge));
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
        self.tmux_link.machine.state()
    }

    /// Installs the connector used to start (and restart) transports (plan 12).
    pub fn set_link_connector(&mut self, c: Box<dyn LinkConnector>) {
        self.tmux_link.connector = Some(c);
    }

    /// Replaces the reconnect policy (tests use short delays).
    pub fn set_link_policy(&mut self, policy: ReconnectPolicy) {
        self.tmux_link.machine.policy = policy;
    }

    /// One link tick: transport output, control lines, pumps, machine deadlines. `true` = redraw.
    pub fn tick_tmux_link(&mut self, now_ms: u64) -> bool {
        let mut changed = false;
        if self.poll_tmux_control() > 0 {
            changed = true;
            self.on_tmux_control_line();
        }
        self.pump_tmux_input(now_ms);
        self.pump_tmux_outbound();
        self.pump_tmux_replies(now_ms);
        changed
    }

    /// Called when a control transport is attached while a link record exists.
    pub(crate) fn on_tmux_control_attached(&mut self) {
        self.tmux_link.aux.control_seen = false;
    }

    /// First control line of a transport: `Connecting` becomes `Connected`.
    pub(crate) fn on_tmux_control_line(&mut self) {
        let Some(target) = self.tmux_link.target.as_ref() else {
            return;
        };
        if !matches!(self.tmux_link.machine.state(), LinkState::Connecting { .. }) {
            return;
        }
        let (dest, session) = (
            sanitize_display(target.dest(), NAME_CAP),
            sanitize_display(&target.session, NAME_CAP),
        );
        let now = self.link_now_ms();
        let cmds = self.tmux_link.machine.on_event(LinkEvent::ControlStarted, now);
        if !cmds.contains(&LinkCmd::MarkConnected) {
            return;
        }
        self.tmux_link.aux.control_seen = true;
        let text = format!("Connected: tmux '{session}' on {dest}");
        if let Some(id) = self.tmux_link.primary_pane
            && let Some(pane) = self.panes.get_mut(&id)
        {
            pane.set_status_message(Some(text.clone()));
        }
        self.tmux_link.aux.last_status = Some(text);
        if let Some(name) = self.tmux_link.fleet_node.clone()
            && let Some(node) = self.fleet.nodes.get_mut(&name)
        {
            node.connected = true;
        }
    }

    /// A transport loss was detected by `poll_tmux_control` (plan 05) or the driver (plan 12).
    #[allow(dead_code)] // wired by plans 51.1-05 and 51.1-12
    pub(crate) fn on_tmux_control_down(&mut self, d: TmuxDisconnect) {
        self.tmux_link.last_exit = Some(d);
    }

    /// Reconnects now (plan 12). Returns the status line it also sets.
    pub fn link_reconnect(&mut self) -> String {
        let msg = match self.tmux_link.target.as_ref() {
            None => "No tmux link to reconnect (use :ssh-tmux <dest> [session])".to_string(),
            Some(t) => format!(
                "Link {}: reconnect is not available yet",
                sanitize_display(&t.badge(), 2 * NAME_CAP)
            ),
        };
        self.active_pane_mut().set_status_message(Some(msg.clone()));
        msg
    }

    /// Stops the link, keeping the remote tmux session (plan 12). Returns the status line.
    pub fn link_disconnect(&mut self) -> String {
        let msg = match self.tmux_link.target.as_ref() {
            None => "No tmux link to disconnect".to_string(),
            Some(t) => format!(
                "Link {}: disconnect is not available yet",
                sanitize_display(&t.badge(), 2 * NAME_CAP)
            ),
        };
        self.active_pane_mut().set_status_message(Some(msg.clone()));
        msg
    }

    /// Snapshot for `:ssh-tmux info`, `remote.link` and the drawer.
    pub fn tmux_link_summary(&self) -> LinkSummary {
        let l = &self.tmux_link;
        let mut s = LinkSummary {
            state: l.machine.state().as_str(),
            attempt: l.machine.attempt(),
            max_attempts: l.machine.policy().max_attempts,
            primary_pane: l.primary_pane,
            ..LinkSummary::default()
        };
        if let Some(t) = l.target.as_ref() {
            s.dest = Some(sanitize_display(t.dest(), NAME_CAP));
            s.session = Some(sanitize_display(&t.session, NAME_CAP));
            s.panes = self
                .tmux_bridge
                .bilterm_to_tmux
                .iter()
                .map(|(&p, &t)| (p, t))
                .collect();
        }
        s
    }

    /// `:remote disconnect` of the link's fleet node: back to `Idle`, panes untouched (plan 12).
    #[allow(dead_code)] // wired by plan 51.1-12 (`:remote disconnect` of the link node)
    pub(crate) fn tmux_link_forget(&mut self) {
        if let Some(d) = self.tmux_link.driver.as_mut() {
            d.kill();
        }
        let old = std::mem::take(&mut self.tmux_link);
        self.tmux_link = TmuxLinkState {
            machine: LinkMachine::new(old.machine.policy().clone()),
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
