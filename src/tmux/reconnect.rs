//! Pure link state machine, backoff and exit classification (Phase 51.1, plan 04; frozen rules
//! in 51.1-CONTRACT.md section 6). No clock, no sleeping, no I/O: every function takes `now_ms`
//! and the machine returns [`LinkCmd`]s that the window manager executes.

use crate::pane::LinkBanner;
use crate::tmux::link::{
    DisconnectCause, ExitClass, LinkCmd, LinkEvent, LinkMachine, LinkState, LinkTarget,
    ReconnectPolicy, TmuxDisconnect,
};
use crate::vt::sanitize_display;

/// Longest cause fragment embedded in a banner, so the fixed key hints always fit in 160 chars.
const CAUSE_CAP: usize = 80;
const TEXT_CAP: usize = 160;

/// `0` for attempt `0`, otherwise `min(cap, base * factor^(attempt-1))`, saturating.
pub fn backoff_delay_ms(policy: &ReconnectPolicy, attempt: u32) -> u64 {
    if attempt == 0 || policy.base_ms == 0 {
        return 0;
    }
    let cap = policy.cap_ms;
    let factor = u64::from(policy.factor);
    let mut delay = policy.base_ms;
    if factor <= 1 {
        return delay.min(cap);
    }
    // At most `attempt - 1` multiplications, and the loop stops as soon as the cap is reached,
    // so a huge `attempt` costs a handful of iterations.
    for _ in 1..attempt {
        if delay >= cap {
            break;
        }
        delay = delay.saturating_mul(factor);
    }
    delay.min(cap)
}

const FATAL_EVIDENCE: [&str; 5] = [
    "permission denied",
    "host key verification failed",
    "no matching host key",
    "command not found",
    "no such file",
];

const CONTROL_TRANSIENT: [&str; 4] = [
    "server exited unexpectedly",
    "lost tty",
    "too far behind",
    "connection",
];

fn any_line_contains(lines: &[String], needles: &[&str]) -> bool {
    lines.iter().any(|l| {
        let low = l.to_lowercase();
        needles.iter().any(|n| low.contains(n))
    })
}

/// Classifies a disconnect into retry / give up / end / user. Case-insensitive; every tail line
/// is inspected.
pub fn classify_exit(
    cause: &DisconnectCause,
    reason: Option<&str>,
    code: Option<i32>,
    tail: &[String],
) -> ExitClass {
    match cause {
        DisconnectCause::UserDetach => ExitClass::User,
        DisconnectCause::SpawnFailed(_) => ExitClass::Fatal,
        DisconnectCause::ControlExit => {
            let low = reason.unwrap_or("").to_lowercase();
            if CONTROL_TRANSIENT.iter().any(|n| low.contains(n)) {
                ExitClass::Transient
            } else {
                ExitClass::Ended
            }
        }
        DisconnectCause::Eof | DisconnectCause::ProcessExit(_) => {
            let code = match cause {
                DisconnectCause::ProcessExit(Some(c)) => Some(*c),
                _ => code,
            };
            let reason_line: Vec<String> = reason.map(str::to_string).into_iter().collect();
            if any_line_contains(tail, &FATAL_EVIDENCE)
                || any_line_contains(&reason_line, &FATAL_EVIDENCE)
                || code == Some(127)
            {
                ExitClass::Fatal
            } else if code == Some(0) {
                // The remote command ended normally without `%exit`.
                ExitClass::Ended
            } else {
                // Transient evidence (connection refused, timed out, broken pipe, connection
                // reset, closed by remote host, could not resolve hostname, network is
                // unreachable), 255, plain EOF or no evidence at all: retry, bounded by the
                // policy. Only the Fatal evidence above stops the loop.
                ExitClass::Transient
            }
        }
    }
}

fn clip(raw: &str) -> String {
    sanitize_display(raw.trim(), CAUSE_CAP)
}

/// Cause text for banners and status lines: `tmux exited[: reason]`, `connection closed`,
/// `ssh exited {code}[: first tail line]`, `spawn failed: {err}`, `stopped by user`.
pub fn disconnect_text(d: &TmuxDisconnect) -> String {
    let first_tail = || {
        d.tail
            .iter()
            .map(|l| l.trim())
            .find(|l| !l.is_empty())
            .map(clip)
    };
    let text = match &d.cause {
        DisconnectCause::ControlExit => match d.reason.as_deref().map(clip) {
            Some(r) if !r.is_empty() => format!("tmux exited: {r}"),
            _ => "tmux exited".to_string(),
        },
        DisconnectCause::Eof => "connection closed".to_string(),
        DisconnectCause::ProcessExit(code) => {
            let head = match code {
                Some(c) => format!("ssh exited {c}"),
                None => "ssh exited".to_string(),
            };
            match first_tail() {
                Some(t) => format!("{head}: {t}"),
                None => head,
            }
        }
        DisconnectCause::SpawnFailed(err) => format!("spawn failed: {}", clip(err)),
        DisconnectCause::UserDetach => "stopped by user".to_string(),
    };
    sanitize_display(&text, TEXT_CAP)
}

fn banner(text: String, interactive: bool) -> LinkBanner {
    LinkBanner {
        text: sanitize_display(&text, TEXT_CAP),
        interactive,
    }
}

/// Banner for a link state; `None` for `Idle` and `Connected`. Only `Connecting` is not
/// interactive, so typed ssh prompts (passwords) are never swallowed by banner keys.
pub fn link_banner_text(
    state: &LinkState,
    target: &LinkTarget,
    exit: Option<&TmuxDisconnect>,
    policy: &ReconnectPolicy,
    now_ms: u64,
) -> Option<LinkBanner> {
    let cause = |fallback: &str| exit.map_or_else(|| fallback.to_string(), disconnect_text);
    match state {
        LinkState::Idle | LinkState::Connected { .. } => None,
        LinkState::Connecting { .. } => Some(banner(
            format!(
                "LINK CONNECTING {} | type here for ssh prompts | Ctrl+Alt+R: restart",
                sanitize_display(&target.badge(), 60)
            ),
            false,
        )),
        LinkState::Backoff {
            attempt,
            retry_at_ms,
        } => {
            let secs = retry_at_ms.saturating_sub(now_ms).div_ceil(1000);
            Some(banner(
                format!(
                    "LINK DOWN ({}) | retry {}/{} in {}s | Enter/r: now | x: stop | d: detach",
                    cause("connection lost"),
                    attempt,
                    policy.max_attempts,
                    secs
                ),
                true,
            ))
        }
        LinkState::Failed { .. } => Some(banner(
            format!(
                "LINK FAILED ({}) | Enter/r: retry | d: detach",
                cause("unknown error")
            ),
            true,
        )),
        LinkState::Detached => Some(banner(
            "LINK STOPPED | Enter/r: reconnect | d: detach".to_string(),
            true,
        )),
        LinkState::Ended => Some(banner(
            "REMOTE SESSION ENDED | Enter/r: start a new one | d: detach".to_string(),
            true,
        )),
    }
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

    fn start_connecting(&mut self, now_ms: u64) {
        self.state = LinkState::Connecting {
            since_ms: now_ms,
            awaiting_control: true,
        };
        self.transport_live = true;
    }

    fn on_down(&mut self, class: ExitClass, now_ms: u64) -> Vec<LinkCmd> {
        let connected_since = self.connected_since.take();
        self.transport_live = false;
        match class {
            ExitClass::Transient => {
                let stable = connected_since
                    .is_some_and(|t| now_ms.saturating_sub(t) >= self.policy.stable_after_ms);
                if stable {
                    self.attempt = 0;
                }
                self.attempt = self.attempt.saturating_add(1);
                if self.attempt > self.policy.max_attempts {
                    self.attempt = self.policy.max_attempts;
                    self.state = LinkState::Failed {
                        attempts: self.attempt,
                    };
                    return vec![LinkCmd::MarkFailed];
                }
                let delay = backoff_delay_ms(&self.policy, self.attempt);
                self.state = LinkState::Backoff {
                    attempt: self.attempt,
                    retry_at_ms: now_ms.saturating_add(delay),
                };
                vec![LinkCmd::MarkDown { retry_in_ms: delay }]
            }
            ExitClass::Fatal => {
                self.state = LinkState::Failed {
                    attempts: self.attempt,
                };
                vec![LinkCmd::MarkFailed]
            }
            ExitClass::Ended => {
                self.state = LinkState::Ended;
                vec![LinkCmd::MarkEnded]
            }
            ExitClass::User => {
                self.state = LinkState::Detached;
                vec![LinkCmd::MarkDetached]
            }
        }
    }

    pub fn on_event(&mut self, ev: LinkEvent, now_ms: u64) -> Vec<LinkCmd> {
        match ev {
            LinkEvent::Connect => match self.state {
                LinkState::Idle
                | LinkState::Detached
                | LinkState::Ended
                | LinkState::Failed { .. } => {
                    self.attempt = 0;
                    self.connected_since = None;
                    self.start_connecting(now_ms);
                    vec![LinkCmd::Spawn]
                }
                _ => Vec::new(),
            },
            LinkEvent::ControlStarted => match self.state {
                LinkState::Connecting { .. } => {
                    self.state = LinkState::Connected { since_ms: now_ms };
                    self.connected_since = Some(now_ms);
                    vec![LinkCmd::MarkConnected]
                }
                _ => Vec::new(),
            },
            LinkEvent::Down(d) => match self.state {
                LinkState::Connecting { .. } | LinkState::Connected { .. } => {
                    self.on_down(d.class, now_ms)
                }
                // Stale transport (the link already moved on): nothing to update.
                _ => Vec::new(),
            },
            LinkEvent::UserDisconnect => match self.state {
                LinkState::Idle | LinkState::Detached => Vec::new(),
                _ => {
                    self.state = LinkState::Detached;
                    self.transport_live = false;
                    self.connected_since = None;
                    vec![LinkCmd::KillTransport, LinkCmd::MarkDetached]
                }
            },
            LinkEvent::UserReconnect => {
                let mut cmds = Vec::new();
                if self.transport_live {
                    cmds.push(LinkCmd::KillTransport);
                }
                self.attempt = 0;
                self.connected_since = None;
                self.start_connecting(now_ms);
                cmds.push(LinkCmd::Spawn);
                cmds
            }
        }
    }

    pub fn tick(&mut self, now_ms: u64) -> Vec<LinkCmd> {
        match self.state {
            LinkState::Backoff { retry_at_ms, .. } if now_ms >= retry_at_ms => {
                self.start_connecting(now_ms);
                vec![LinkCmd::Spawn]
            }
            _ => Vec::new(),
        }
    }

    pub fn next_deadline_ms(&self) -> Option<u64> {
        match self.state {
            LinkState::Backoff { retry_at_ms, .. } => Some(retry_at_ms),
            _ => None,
        }
    }

    pub fn retry_in_ms(&self, now_ms: u64) -> Option<u64> {
        self.next_deadline_ms().map(|t| t.saturating_sub(now_ms))
    }
}
