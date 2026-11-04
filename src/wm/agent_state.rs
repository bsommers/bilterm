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
use crate::vt::Osc133Phase;
use crate::wm::WindowManager;

/// A silent running command stays `Working` for this long after the last output byte.
pub const WORKING_HOLD: Duration = Duration::from_millis(1500);
/// `Working` <-> `Idle` flips apply only after the candidate state persisted this long.
pub const MIN_DWELL: Duration = Duration::from_millis(500);
/// Minimum spacing between screen-tail rescans when no new output arrived.
pub const TAIL_RESCAN: Duration = Duration::from_millis(250);
/// Number of trailing visible rows scanned for profile patterns.
pub const TAIL_ROWS: u16 = 8;
/// Minimum gap between animation-only redraws while a pane is Working and the navigator drawer is visible.
pub const ANIM_INTERVAL: Duration = Duration::from_millis(120);

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

fn contains_any(tail: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| !p.is_empty() && tail.contains(p.as_str()))
}

/// Pure first-match rule table (RESEARCH Pattern 2, thresholds are the recorded A2 assumption):
/// 1. Blocked: password prompt, or a profile `blocked` pattern in the tail.
/// 2. Errored: exited with non-zero code, or an unseen finished command with non-zero exit.
/// 3. Done: exited with code 0 (or unknown), or an unseen finished command with exit code 0.
/// 4. Blocked: unread notification while the pane is not focused.
/// 5. Working: command running and (recent output, or a profile `working` pattern in the tail).
/// 6. Idle: everything else.
///
/// Profile patterns must already be sanitised (lower-case); `tail` is lower-case.
pub fn classify(s: &AgentSignals, profile: Option<&AgentProfile>) -> AgentState {
    if s.echo_locked || profile.is_some_and(|p| contains_any(&s.tail, &p.blocked)) {
        return AgentState::Blocked;
    }
    let exit_failed = s.exited && s.exit_code.is_some_and(|c| c != 0);
    if exit_failed || s.unseen_finish == Some(FinishKind::Failure) {
        return AgentState::Errored;
    }
    if s.exited || s.unseen_finish == Some(FinishKind::Success) {
        return AgentState::Done;
    }
    if s.unread_notification && !s.focused {
        return AgentState::Blocked;
    }
    let pattern_working = profile.is_some_and(|p| contains_any(&s.tail, &p.working));
    if s.running && (s.quiet_for <= WORKING_HOLD || pattern_working) {
        return AgentState::Working;
    }
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
    /// Matched profile (name, sanitised copy), resolved once per tail scan.
    profile: Option<(String, AgentProfile)>,
    /// `false` until the first observation, which applies its state without dwell.
    observed: bool,
    last_phase: Option<Osc133Phase>,
    last_blocks: usize,
    /// `(last_exit_code, last_duration_ms)` at the previous look; a change means a command finished.
    last_finish_sig: (Option<i32>, Option<u64>),
    /// A command finished while the pane was unfocused and has not been seen.
    unseen_finish: Option<FinishKind>,
    /// Lower-cased text of the last visible rows (rescanned at most every `TAIL_RESCAN`).
    tail: String,
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
            last_phase: None,
            last_blocks: 0,
            last_finish_sig: (None, None),
            unseen_finish: None,
            tail: String::new(),
        }
    }
}

/// Per-pane state history; stored on `WindowManager` as `pub(crate) agents`.
#[derive(Clone, Debug, Default)]
pub struct AgentTracker {
    histories: BTreeMap<PaneId, PaneHistory>,
    last_now: Option<Instant>,
    /// Time of the last redraw requested for the working animation.
    last_anim: Option<Instant>,
    /// Traffic bus sequence at the last redraw requested for the open agents window.
    last_traffic_seq: u64,
}

impl AgentTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` once [`WindowManager::update_agent_states`] ran at least once.
    pub fn has_updated(&self) -> bool {
        self.last_now.is_some()
    }

    /// Instant of the last [`WindowManager::update_agent_states`] run (the clock message ages are
    /// measured against, so rendering stays deterministic under synthetic time).
    pub fn last_update(&self) -> Option<Instant> {
        self.last_now
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

/// Lower-cased text of up to `TAIL_ROWS` rows ending at the last non-empty visible row.
fn scan_tail(pane: &CompositePane) -> String {
    let (rows, _) = pane.vt().size();
    let lines: Vec<String> = (0..rows).map(|r| pane.vt().row_text(r)).collect();
    let end = lines.iter().rposition(|l| !l.trim().is_empty()).map_or(0, |i| i + 1);
    let start = end.saturating_sub(usize::from(TAIL_ROWS));
    lines[start..end].join("\n").to_lowercase()
}

/// Detects command start / finish since the previous look and maintains `unseen_finish`.
fn track_finish(h: &mut PaneHistory, pane: &CompositePane, focused: bool) {
    let vt = pane.vt();
    let phase = vt.osc133_phase();
    let blocks = vt.command_blocks().len();
    let sig = (vt.last_exit_code(), vt.last_duration_ms());
    let started = (phase == Some(Osc133Phase::CommandExecuted) && h.last_phase != phase) || blocks > h.last_blocks;
    if started && !matches!(phase, Some(Osc133Phase::CommandFinished(_))) {
        h.unseen_finish = None;
    }
    let phase_finish = match phase {
        Some(Osc133Phase::CommandFinished(code)) if h.last_phase != phase => Some(code),
        _ => None,
    };
    let sig_finish = (sig != h.last_finish_sig && sig.0.is_some()).then_some(sig.0);
    if let Some(code) = sig_finish.or(phase_finish)
        && !focused
    {
        h.unseen_finish = Some(if code.is_none_or(|c| c == 0) { FinishKind::Success } else { FinishKind::Failure });
    }
    if focused {
        h.unseen_finish = None;
    }
    h.last_phase = phase;
    h.last_blocks = blocks;
    h.last_finish_sig = sig;
}

/// First profile (map order) whose sanitised `matches` (else its own name) occurs in the pane identity.
fn resolve_profile(
    profiles: &BTreeMap<String, AgentProfile>,
    command: Option<&str>,
    title: &str,
) -> Option<(String, AgentProfile)> {
    let identity = format!("{} {}", command.unwrap_or(""), title).to_lowercase();
    profiles.iter().find_map(|(name, raw)| {
        let p = raw.sanitized();
        let hit = if p.matches.is_empty() {
            let own = name.trim().to_lowercase();
            !own.is_empty() && identity.contains(&own)
        } else {
            p.matches.iter().any(|m| identity.contains(m.as_str()))
        };
        hit.then(|| (name.clone(), p))
    })
}

fn collect_signals(h: &PaneHistory, pane: &CompositePane, focused: bool, now: Instant) -> AgentSignals {
    let phase = pane.vt().osc133_phase();
    let finished = matches!(phase, Some(Osc133Phase::CommandFinished(_) | Osc133Phase::PromptStart));
    let running = phase == Some(Osc133Phase::CommandExecuted)
        || (pane.prompt().running_command.is_some() && !finished);
    let (exited, exit_code) = match pane.held_exited_shell() {
        Some(held) => (true, held.exit_code),
        None => (pane.has_exited(), None),
    };
    AgentSignals {
        echo_locked: pane.is_password_prompt(),
        exited,
        exit_code,
        unseen_finish: h.unseen_finish,
        unread_notification: !pane.vt().notifications().is_empty(),
        focused,
        running,
        quiet_for: h.last_activity.map_or(Duration::MAX, |t| now.saturating_duration_since(t)),
        tail: h.tail.clone(),
    }
}

/// Updates one pane's history; returns `true` when its displayed state changed.
fn observe(
    h: &mut PaneHistory,
    id: PaneId,
    pane: &CompositePane,
    focused: bool,
    now: Instant,
    profiles: &BTreeMap<String, AgentProfile>,
) -> bool {
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
        if profiles.is_empty() {
            h.profile = None;
            h.tail.clear();
        } else {
            h.profile = resolve_profile(profiles, pane.prompt().running_command.as_deref(), pane.title());
            h.tail = scan_tail(pane);
        }
    }
    track_finish(h, pane, focused);
    let signals = collect_signals(h, pane, focused, now);
    let raw = h.reported.unwrap_or_else(|| classify(&signals, h.profile.as_ref().map(|(_, p)| p)));
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
                profile: h.profile.as_ref().map(|(n, _)| n.clone()),
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
        let profiles = &self.config.agents;
        self.agents.histories.retain(|id, _| panes.contains_key(id));
        for (&id, pane) in &self.panes {
            let h = self.agents.histories.entry(id).or_insert_with(|| {
                changed = true;
                PaneHistory::new(id, now)
            });
            if observe(h, id, pane, active == Some(id), now, profiles) {
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

    /// Poll-loop hook. Returns `true` only while the navigator drawer is visible, and then when a state
    /// changed or a pane is Working and [`ANIM_INTERVAL`] passed since the last animation redraw, so the
    /// classic redraw cadence is unchanged (RESEARCH Pitfall 8).
    pub(crate) fn tick_agent_ui(&mut self, now: Instant) -> bool {
        let changed = self.update_agent_states(now);
        let traffic = self.tick_agents_window_traffic(now);
        if self.config.drawer.style != crate::wm::drawer::DrawerStyle::Navigator
            || !self.is_drawer_effectively_visible()
        {
            return traffic;
        }
        let working = self.agents.histories.values().any(|h| h.state == AgentState::Working);
        let animate = working
            && self.agents.last_anim.is_none_or(|t| now.saturating_duration_since(t) >= ANIM_INTERVAL);
        if working && (changed || animate) {
            self.agents.last_anim = Some(now);
        }
        changed || animate || traffic
    }

    /// `true` when the agents window is open and new bus events arrived since the last redraw it
    /// requested; shares the [`ANIM_INTERVAL`] throttle with the working animation.
    fn tick_agents_window_traffic(&mut self, now: Instant) -> bool {
        if !self.agents_window.active {
            return false;
        }
        let seq = self.traffic().latest_seq();
        if seq == self.agents.last_traffic_seq {
            return false;
        }
        let due = self.agents.last_anim.is_none_or(|t| now.saturating_duration_since(t) >= ANIM_INTERVAL);
        if due {
            self.agents.last_traffic_seq = seq;
            self.agents.last_anim = Some(now);
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wm::drawer::DrawerStyle;

    #[test]
    fn dots_tick_agent_ui_redraws_only_for_visible_navigator() {
        let mut wm = WindowManager::new_headless(30, 100);
        wm.config_mut().drawer.style = DrawerStyle::Navigator;
        let t0 = Instant::now();
        let _ = wm.tick_agent_ui(t0); // first look registers the pane
        wm.toggle_drawer();
        assert!(wm.is_drawer_effectively_visible());

        wm.report_agent_state(1, Some(AgentState::Blocked));
        assert!(wm.tick_agent_ui(t0), "visible navigator drawer + state change -> redraw");
        assert!(!wm.tick_agent_ui(t0), "no change -> no redraw");

        wm.report_agent_state(1, Some(AgentState::Errored));
        wm.config_mut().drawer.style = DrawerStyle::Classic;
        assert!(!wm.tick_agent_ui(t0), "classic style keeps its redraw cadence");

        wm.report_agent_state(1, Some(AgentState::Done));
        wm.config_mut().drawer.style = DrawerStyle::Navigator;
        wm.toggle_drawer();
        assert!(!wm.is_drawer_effectively_visible());
        assert!(!wm.tick_agent_ui(t0), "hidden drawer -> no redraw");
    }

    #[test]
    fn anim_working_requests_throttled_redraw() {
        use std::time::Duration;
        let mut wm = WindowManager::new_headless(30, 100);
        wm.config_mut().drawer.style = DrawerStyle::Navigator;
        wm.toggle_drawer();
        assert!(wm.is_drawer_effectively_visible());
        let t0 = Instant::now();
        wm.report_agent_state(1, Some(AgentState::Working));
        assert!(wm.tick_agent_ui(t0), "first look at a Working pane redraws");
        assert_eq!(wm.agent_state(1), Some(AgentState::Working));
        assert!(!wm.tick_agent_ui(t0 + Duration::from_millis(50)), "inside the throttle window");
        assert!(wm.tick_agent_ui(t0 + Duration::from_millis(130)), "interval elapsed -> animation frame");
        assert!(!wm.tick_agent_ui(t0 + Duration::from_millis(140)), "throttled again");

        wm.config_mut().drawer.style = DrawerStyle::Classic;
        assert!(!wm.tick_agent_ui(t0 + Duration::from_millis(500)), "classic never animates");
        wm.config_mut().drawer.style = DrawerStyle::Navigator;
        wm.report_agent_state(1, Some(AgentState::Idle));
        let _ = wm.tick_agent_ui(t0 + Duration::from_millis(700));
        assert!(!wm.tick_agent_ui(t0 + Duration::from_millis(900)), "no Working pane -> no animation");
    }

    #[test]
    fn window_tick_redraws_on_new_traffic() {
        use crate::wm::traffic::{TrafficEndpoint, TrafficKind};
        use std::time::Duration;

        let mut wm = WindowManager::new_headless(30, 100);
        let t0 = Instant::now();
        let _ = wm.tick_agent_ui(t0);
        let send = |wm: &mut WindowManager, at: Instant| {
            wm.record_traffic(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Broadcast, "x", at);
        };

        send(&mut wm, t0);
        assert!(!wm.tick_agent_ui(t0), "window closed: traffic does not request a redraw");

        wm.open_agents_window(None);
        assert!(wm.tick_agent_ui(t0), "open window + unseen traffic -> redraw");
        assert!(!wm.tick_agent_ui(t0), "no new traffic -> no redraw");

        send(&mut wm, t0);
        assert!(!wm.tick_agent_ui(t0 + Duration::from_millis(50)), "throttled inside 120 ms");
        assert!(wm.tick_agent_ui(t0 + Duration::from_millis(130)), "due after 120 ms");
    }
}
