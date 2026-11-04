//! Per-pane agent state (Phase 50-03, D-03, FLT-01).
//!
//! Every live pane gets one of five states derived only from signals Bilterm already tracks
//! (OSC 133 phase, running command, output byte counter, password prompt, notifications, exit).
//! [`classify`] is a pure rule table; [`AgentTracker`] adds hysteresis, "seen" semantics and the
//! explicit-report override seam that Phase 51 uses (`WindowManager::report_agent_state`).

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::config::agents::AgentProfile;
use crate::pane::{CompositePane, PaneId};
use crate::wm::WindowManager;

/// A silent running command stays `Working` for this long after the last output byte.
pub const WORKING_HOLD: Duration = Duration::from_millis(1500);
/// `Working` <-> `Idle` flips apply only after the candidate state persisted this long.
pub const MIN_DWELL: Duration = Duration::from_millis(500);
/// Minimum spacing between screen-tail rescans when no new output arrived.
pub const TAIL_RESCAN: Duration = Duration::from_millis(250);

/// Live state of the agent / program running in a pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgentState {
    Working,
    Blocked,
    Idle,
    Done,
    Errored,
}

impl AgentState {
    /// Stable lower-case name used in `:agents list` data.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Blocked => "blocked",
            Self::Idle => "idle",
            Self::Done => "done",
            Self::Errored => "errored",
        }
    }

    /// States that want the user's eyes (`Blocked`, `Errored`, `Done`).
    pub fn is_attention(self) -> bool {
        matches!(self, Self::Blocked | Self::Errored | Self::Done)
    }
}

/// Outcome of a finished command that the user has not looked at yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishKind {
    Success,
    Failure,
}

/// Pure classifier input; built per pane by the tracker.
#[derive(Clone, Debug)]
pub struct AgentSignals {
    /// Password / canvas password prompt is waiting for the user.
    pub echo_locked: bool,
    /// The pane's process exited (`exit_code` carries the status when known).
    pub exited: bool,
    pub exit_code: Option<i32>,
    /// A command finished while the pane was not focused and has not been seen yet.
    pub unseen_finish: Option<FinishKind>,
    /// Unread OSC 9 / OSC 777 notification present.
    pub unread_notification: bool,
    /// The pane is the focused pane.
    pub focused: bool,
    /// A command is currently running.
    pub running: bool,
    /// Time since the last output byte (`Duration::MAX` when none was ever seen).
    pub quiet_for: Duration,
    /// Lower-cased text of the last visible rows.
    pub tail: String,
}

impl Default for AgentSignals {
    fn default() -> Self {
        Self {
            echo_locked: false,
            exited: false,
            exit_code: None,
            unseen_finish: None,
            unread_notification: false,
            focused: false,
            running: false,
            quiet_for: Duration::MAX,
            tail: String::new(),
        }
    }
}

/// Pure first-match rule table (RESEARCH Pattern 2, thresholds are the recorded A2 assumption).
pub fn classify(s: &AgentSignals, profile: Option<&AgentProfile>) -> AgentState {
    if s.running && s.quiet_for <= WORKING_HOLD {
        return AgentState::Working;
    }
    let _ = profile;
    AgentState::Idle
}

/// One row of `:agents list`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentStatus {
    pub pane_id: PaneId,
    pub name: String,
    pub state: AgentState,
    pub since_ms: u64,
    pub profile: Option<String>,
    pub reported: bool,
}

/// One usize per state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AgentCounts {
    pub working: usize,
    pub blocked: usize,
    pub idle: usize,
    pub done: usize,
    pub errored: usize,
}

impl AgentCounts {
    pub fn total(&self) -> usize {
        self.working + self.blocked + self.idle + self.done + self.errored
    }
}

#[derive(Clone, Debug)]
struct PaneHistory {
    last_bytes: usize,
    last_activity: Option<Instant>,
    state: AgentState,
    since: Instant,
    /// Candidate for a `Working` <-> `Idle` flip and when it was first proposed.
    candidate: Option<(AgentState, Instant)>,
    reported: Option<AgentState>,
    name: String,
    last_scan: Option<Instant>,
    profile: Option<String>,
    /// `false` until the first observation, which applies its state without dwell.
    observed: bool,
}

impl PaneHistory {
    fn new(id: PaneId, now: Instant) -> Self {
        Self {
            last_bytes: 0,
            last_activity: None,
            state: AgentState::Idle,
            since: now,
            candidate: None,
            reported: None,
            name: format!("#{id}"),
            last_scan: None,
            profile: None,
            observed: false,
        }
    }
}

/// Per-pane state history; stored on `WindowManager` as `pub(crate) agents`.
#[derive(Clone, Debug, Default)]
pub struct AgentTracker {
    histories: BTreeMap<PaneId, PaneHistory>,
    last_now: Option<Instant>,
}

impl AgentTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` once [`WindowManager::update_agent_states`] ran at least once.
    pub fn has_updated(&self) -> bool {
        self.last_now.is_some()
    }
}

/// Identity token of a pane: first whitespace token of the running command (else of the title),
/// base name only, `[A-Za-z0-9._-]`, at most 32 chars; `#<id>` when nothing usable remains.
fn identity_token(id: PaneId, command: Option<&str>, title: &str) -> String {
    let first = |s: &str| s.split_whitespace().next().unwrap_or("").to_string();
    let raw = match command.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => first(c),
        None => first(title),
    };
    let base = raw.rsplit('/').next().unwrap_or("");
    let token: String = base
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .take(32)
        .collect();
    if token.is_empty() { format!("#{id}") } else { token }
}

fn collect_signals(h: &PaneHistory, pane: &CompositePane, focused: bool, now: Instant) -> AgentSignals {
    let running = pane.prompt().running_command.is_some();
    AgentSignals {
        focused,
        running,
        quiet_for: h.last_activity.map_or(Duration::MAX, |t| now.saturating_duration_since(t)),
        ..AgentSignals::default()
    }
}

/// Updates one pane's history; returns `true` when its displayed state changed.
fn observe(h: &mut PaneHistory, id: PaneId, pane: &CompositePane, focused: bool, now: Instant) -> bool {
    let bytes = pane.vt().total_bytes_processed();
    let bytes_changed = bytes != h.last_bytes;
    if bytes_changed {
        h.last_bytes = bytes;
        h.last_activity = Some(now);
    }
    let scan_due = bytes_changed || h.last_scan.is_none_or(|t| now.saturating_duration_since(t) >= TAIL_RESCAN);
    if scan_due {
        h.last_scan = Some(now);
        h.name = identity_token(id, pane.prompt().running_command.as_deref(), pane.title());
    }
    let signals = collect_signals(h, pane, focused, now);
    let raw = h.reported.unwrap_or_else(|| classify(&signals, None));
    let first_look = !h.observed;
    h.observed = true;
    if raw == h.state {
        h.candidate = None;
        return false;
    }
    let flip = matches!(raw, AgentState::Working | AgentState::Idle)
        && matches!(h.state, AgentState::Working | AgentState::Idle)
        && h.reported.is_none()
        && !first_look;
    if flip {
        match h.candidate {
            Some((c, _)) if c == raw => {}
            _ => h.candidate = Some((raw, now)),
        }
        let since = h.candidate.map_or(now, |(_, t)| t);
        if now.saturating_duration_since(since) < MIN_DWELL {
            return false;
        }
    }
    h.candidate = None;
    h.state = raw;
    h.since = now;
    true
}

impl WindowManager {
    /// State of pane `id`, `None` when it is not tracked (yet).
    pub fn agent_state(&self, id: PaneId) -> Option<AgentState> {
        self.agents.histories.get(&id).map(|h| h.state)
    }

    /// One status per tracked pane in pane-id order.
    pub fn agent_statuses(&self) -> Vec<AgentStatus> {
        let now = self.agents.last_now;
        self.agents
            .histories
            .iter()
            .map(|(&pane_id, h)| AgentStatus {
                pane_id,
                name: h.name.clone(),
                state: h.state,
                since_ms: now.map_or(0, |n| n.saturating_duration_since(h.since).as_millis() as u64),
                profile: h.profile.clone(),
                reported: h.reported.is_some(),
            })
            .collect()
    }

    /// Number of panes per state.
    pub fn agent_counts(&self) -> AgentCounts {
        let mut c = AgentCounts::default();
        for h in self.agents.histories.values() {
            match h.state {
                AgentState::Working => c.working += 1,
                AgentState::Blocked => c.blocked += 1,
                AgentState::Idle => c.idle += 1,
                AgentState::Done => c.done += 1,
                AgentState::Errored => c.errored += 1,
            }
        }
        c
    }

    /// Re-evaluates every pane at `now`; returns `true` when any displayed state changed.
    pub fn update_agent_states(&mut self, now: Instant) -> bool {
        let active = if self.quick_terminal.visible { None } else { Some(self.active_pane_id()) };
        let mut changed = false;
        let panes = &self.panes;
        self.agents.histories.retain(|id, _| panes.contains_key(id));
        for (&id, pane) in &self.panes {
            let h = self.agents.histories.entry(id).or_insert_with(|| {
                changed = true;
                PaneHistory::new(id, now)
            });
            if observe(h, id, pane, active == Some(id), now) {
                changed = true;
            }
        }
        self.agents.last_now = Some(now);
        changed
    }

    /// Phase 51 seam: an explicit state report overrides heuristics until cleared with `None`.
    /// Takes effect at the next [`Self::update_agent_states`].
    pub fn report_agent_state(&mut self, id: PaneId, state: Option<AgentState>) {
        if let Some(h) = self.agents.histories.get_mut(&id) {
            h.reported = state;
        } else if self.panes.contains_key(&id) {
            let now = self.agents.last_now.unwrap_or_else(Instant::now);
            let mut h = PaneHistory::new(id, now);
            h.reported = state;
            self.agents.histories.insert(id, h);
        }
    }

    /// Poll-loop hook. Returns `false` in 50-03 so the classic redraw cadence is unchanged (RESEARCH
    /// Pitfall 8); 50-04 makes it return `true` for visible herdr changes.
    pub(crate) fn tick_agent_ui(&mut self, now: Instant) -> bool {
        let _ = self.update_agent_states(now);
        false
    }
}
