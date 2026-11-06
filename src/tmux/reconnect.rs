//! Pure link state machine, backoff and exit classification (Phase 51.1, plan 04 fills the
//! bodies; frozen rules in 51.1-CONTRACT.md section 6).
//!
//! Seam state (plan 01): only `Idle + Connect` and `Connecting + ControlStarted` transition.

use crate::pane::LinkBanner;
use crate::tmux::link::{
    DisconnectCause, ExitClass, LinkCmd, LinkEvent, LinkMachine, LinkState, LinkTarget,
    ReconnectPolicy, TmuxDisconnect,
};

/// `0` for attempt `0`, otherwise `min(cap, base * factor^(attempt-1))` (plan 04).
pub fn backoff_delay_ms(_policy: &ReconnectPolicy, _attempt: u32) -> u64 {
    0
}

/// Classifies a disconnect into retry / give up / end / user (plan 04).
pub fn classify_exit(
    _cause: &DisconnectCause,
    _reason: Option<&str>,
    _code: Option<i32>,
    _tail: &[String],
) -> ExitClass {
    ExitClass::Transient
}

/// Cause text for banners and status lines (plan 04).
pub fn disconnect_text(_d: &TmuxDisconnect) -> String {
    String::new()
}

/// Banner for a link state; `None` for `Idle` and `Connected` (plan 04).
pub fn link_banner_text(
    _state: &LinkState,
    _target: &LinkTarget,
    _exit: Option<&TmuxDisconnect>,
    _policy: &ReconnectPolicy,
    _now_ms: u64,
) -> Option<LinkBanner> {
    None
}

impl LinkMachine {
    pub fn new(policy: ReconnectPolicy) -> Self {
        Self {
            policy,
            state: LinkState::Idle,
            attempt: 0,
            connected_since: None,
            transport_live: false,
        }
    }

    pub fn state(&self) -> &LinkState {
        &self.state
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    pub fn policy(&self) -> &ReconnectPolicy {
        &self.policy
    }

    pub fn on_event(&mut self, ev: LinkEvent, now_ms: u64) -> Vec<LinkCmd> {
        match (&self.state, ev) {
            (LinkState::Idle, LinkEvent::Connect) => {
                self.state = LinkState::Connecting {
                    since_ms: now_ms,
                    awaiting_control: true,
                };
                self.transport_live = true;
                vec![LinkCmd::Spawn]
            }
            (LinkState::Connecting { .. }, LinkEvent::ControlStarted) => {
                self.state = LinkState::Connected { since_ms: now_ms };
                self.connected_since = Some(now_ms);
                vec![LinkCmd::MarkConnected]
            }
            _ => Vec::new(),
        }
    }

    pub fn tick(&mut self, _now_ms: u64) -> Vec<LinkCmd> {
        Vec::new()
    }

    pub fn next_deadline_ms(&self) -> Option<u64> {
        None
    }

    pub fn retry_in_ms(&self, _now_ms: u64) -> Option<u64> {
        None
    }
}
