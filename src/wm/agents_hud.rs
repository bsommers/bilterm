//! Mission Control HUD window state and operations (Phase 70, HUD-01), plus the replay view
//! (HUD-02) and watchdog status (HUD-03) ops. The feed that keeps the data live is in
//! [`super::agents_hud_feed`].
//!
//! The HUD is a floating utility window: a grid of per-pane tiles. It only *reads* pane
//! state (agent state, traffic rate, queue depth, screen tail); opening, refreshing and rendering
//! it never resizes a pane or PTY. Approve is the one write, and it runs the exact path of RPC
//! `agent.approve` ([`WindowManager::agent_approve_pane`]).

use std::cell::Cell;
use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;

use ratatui::layout::Rect;

use super::WindowManager;
use super::agent_state::AgentState;
use super::agent_watchdog::{AgentWatchdog, WatchdogConfig};
use super::agents_hud_cmd::{HudOp, ReplayOp};
use super::mission_control_hud::{AgentCardTelemetry, HudButtonKind, MissionControlHud};
use super::traffic::{TrafficEndpoint, TrafficKind};
use super::turn_replay::TurnReplayLog;
use super::swarm_surface::SwarmOp;
use crate::action::Action;
use crate::api::json::JsonValue;
use crate::pane::PaneId;

/// Recent watchdog alerts kept for `agent.watchdog` / `:agents watchdog`.
pub const MAX_ALERT_HISTORY: usize = 32;
/// Alerts waiting to become RPC events.
pub const MAX_ALERT_OUTBOX: usize = 64;

/// One raised watchdog alert (`kind` is `stall` or `loop`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertRecord {
    pub pane: PaneId,
    pub kind: &'static str,
    pub detail: String,
    pub count: usize,
    pub at_secs: u64,
    pub target: Option<PaneId>,
}

/// Per-pane output cursor used by the feed.
#[derive(Clone, Debug, Default)]
pub(super) struct FeedCursor {
    pub gen_seen: Option<u64>,
    pub rows: Vec<String>,
    pub blocks: usize,
    pub recorded: usize,
}

/// HUD, replay and watchdog state owned by the window manager.
#[derive(Debug)]
pub struct AgentsHudState {
    /// The HUD window is open.
    pub active: bool,
    /// The HUD holds the keyboard.
    pub focused: bool,
    pub(super) hud: MissionControlHud,
    pub(super) gen_states: BTreeMap<PaneId, AgentState>,
    pub(super) generations: BTreeMap<PaneId, u64>,
    pub(super) log_states: BTreeMap<PaneId, AgentState>,
    pub(super) logs: BTreeMap<PaneId, TurnReplayLog>,
    pub(super) feed: BTreeMap<PaneId, FeedCursor>,
    pub(super) pending_prompt: BTreeMap<PaneId, String>,
    pub(super) replay: Option<PaneId>,
    pub(super) watchdog: AgentWatchdog,
    pub(super) alerts: VecDeque<AlertRecord>,
    pub(super) outbox: VecDeque<AlertRecord>,
    pub(super) last_spark_ms: u64,
    pub(super) epoch: Instant,
    page_size: Cell<usize>,
    viewport: Cell<usize>,
}

impl Default for AgentsHudState {
    fn default() -> Self {
        Self {
            active: false,
            focused: false,
            hud: MissionControlHud::new(),
            gen_states: BTreeMap::new(),
            generations: BTreeMap::new(),
            log_states: BTreeMap::new(),
            logs: BTreeMap::new(),
            feed: BTreeMap::new(),
            pending_prompt: BTreeMap::new(),
            replay: None,
            watchdog: AgentWatchdog::new(WatchdogConfig::default()),
            alerts: VecDeque::new(),
            outbox: VecDeque::new(),
            last_spark_ms: 0,
            epoch: Instant::now(),
            page_size: Cell::new(super::mission_control_hud::MAX_TILES_PER_PAGE),
            viewport: Cell::new(20),
        }
    }
}

impl AgentsHudState {
    /// The HUD or the replay view owns the keyboard.
    pub fn captures_input(&self) -> bool {
        self.focused && (self.active || self.replay.is_some())
    }
    /// Releases the keyboard without closing the HUD or the replay view.
    pub fn blur(&mut self) {
        self.focused = false;
    }
    /// A window (HUD or replay) is on screen.
    pub fn visible(&self) -> bool {
        self.active || self.replay.is_some()
    }
    /// The HUD model (cards, selection, paging).
    pub fn hud(&self) -> &MissionControlHud {
        &self.hud
    }
    /// The pane shown in the replay view.
    pub fn replay_pane(&self) -> Option<PaneId> {
        self.replay
    }
    /// The turn log of a pane.
    pub fn log(&self, pane: PaneId) -> Option<&TurnReplayLog> {
        self.logs.get(&pane)
    }
    /// Recent alerts, oldest first.
    pub fn alerts(&self) -> impl Iterator<Item = &AlertRecord> {
        self.alerts.iter()
    }
    /// The watchdog and its configuration.
    pub fn watchdog(&self) -> &AgentWatchdog {
        &self.watchdog
    }
    /// Tiles per page at the last known window size.
    pub fn page_size(&self) -> usize {
        self.page_size.get()
    }
    /// Lines of the replay body at the last known window size.
    pub fn viewport(&self) -> usize {
        self.viewport.get()
    }
    /// Queues the text of a prompt injected into `pane`; it becomes the next replay turn's prompt.
    pub(super) fn note_prompt(&mut self, pane: PaneId, text: &str) {
        if self.pending_prompt.len() < 256 || self.pending_prompt.contains_key(&pane) {
            self.pending_prompt
                .insert(pane, text.chars().take(4096).collect());
        }
    }
}

/// The HUD / replay window rectangle inside the workspace.
pub fn hud_outer(workspace: Rect) -> Rect {
    let w = workspace
        .width
        .saturating_sub(4)
        .max(1)
        .min(workspace.width);
    let h = workspace
        .height
        .saturating_sub(2)
        .max(1)
        .min(workspace.height);
    Rect::new(
        workspace.x + (workspace.width - w) / 2,
        workspace.y + (workspace.height - h) / 2,
        w,
        h,
    )
}

/// The tile area inside the window frame (title row and footer row excluded).
pub fn hud_inner(outer: Rect) -> Rect {
    Rect::new(
        outer.x.saturating_add(1),
        outer.y.saturating_add(1),
        outer.width.saturating_sub(2),
        outer.height.saturating_sub(3),
    )
}

impl WindowManager {
    /// Read access to the HUD / replay / watchdog state.
    pub fn agents_hud(&self) -> &AgentsHudState {
        &self.agents_hud
    }

    /// Window rectangle when the HUD or the replay view is visible.
    pub fn agents_hud_rect(&self, workspace: Rect) -> Option<Rect> {
        if !self.agents_hud.visible() || workspace.width < 20 || workspace.height < 8 {
            return None;
        }
        let outer = hud_outer(workspace);
        let inner = hud_inner(outer);
        self.agents_hud
            .page_size
            .set(MissionControlHud::page_size_for(inner).max(1));
        self.agents_hud
            .viewport
            .set(usize::from(inner.height.saturating_sub(2)).max(1));
        Some(outer)
    }

    /// Executes one HUD operation; `Ok(Some(msg))` is a status line, `Err` a refusal.
    pub fn agents_hud_apply(&mut self, op: &HudOp) -> Result<Option<String>, String> {
        if self.agents_hud.active && !matches!(op, HudOp::Close | HudOp::Open) {
            // Every user action works on current tiles (state and generation tokens).
            self.agents_hud_refresh(false);
        }
        match op {
            HudOp::Open => {
                self.agents_hud.active = true;
                self.agents_hud.focused = true;
                self.agents_hud_refresh(true);
                let first = self.agents_hud.hud.card_ids().first().copied();
                if self.agents_hud.hud.selected().is_none()
                    && let Some(id) = first
                {
                    self.agents_hud.hud.select(id);
                }
                Ok(Some("Mission Control HUD".to_string()))
            }
            HudOp::Close => {
                self.agents_hud.active = false;
                self.agents_hud.focused = false;
                Ok(None)
            }
            HudOp::Toggle => {
                let next = if self.agents_hud.active {
                    HudOp::Close
                } else {
                    HudOp::Open
                };
                self.agents_hud_apply(&next)
            }
            HudOp::Step(n) => {
                let page = self.agents_hud.page_size();
                if *n < 0 {
                    self.agents_hud.hud.select_prev(page);
                } else {
                    self.agents_hud.hud.select_next(page);
                }
                Ok(None)
            }
            HudOp::Page(n) => {
                let page = self.agents_hud.page_size();
                if *n < 0 {
                    self.agents_hud.hud.prev_page();
                } else {
                    self.agents_hud.hud.next_page(page);
                }
                Ok(None)
            }
            HudOp::SelectPane(id) => {
                if self.agents_hud.hud.select(*id) {
                    Ok(None)
                } else {
                    Err(format!("HUD has no tile for pane #{id}"))
                }
            }
            HudOp::Press {
                pane,
                kind,
                generation,
            } => self.agents_hud_press(*pane, *kind, *generation),
        }
    }

    fn agents_hud_press(
        &mut self,
        pane: Option<PaneId>,
        kind: HudButtonKind,
        generation: Option<u64>,
    ) -> Result<Option<String>, String> {
        self.agents_hud_refresh(false);
        let id = pane
            .or_else(|| self.agents_hud.hud.selected())
            .ok_or_else(|| "HUD has no selected tile".to_string())?;
        let card = self
            .agents_hud
            .hud
            .card(id)
            .ok_or_else(|| format!("HUD has no tile for pane #{id}"))?;
        match kind {
            HudButtonKind::Approve => {
                if !card.approve_enabled() {
                    return Err(format!("pane #{id} is {}, not blocked", card.state.as_str()));
                }
                if generation.is_some_and(|g| g != card.generation) {
                    return Err(format!("stale tile for pane #{id}: its state changed, approve again"));
                }
                let source = if self.api_origin { TrafficEndpoint::Api } else { TrafficEndpoint::User };
                self.agent_approve_pane(id, "yes", false, false, source)?;
                self.agents_hud.hud.mark_dirty();
                Ok(Some(format!("Approved pane #{id}")))
            }
            HudButtonKind::Prompt => {
                self.agents_hud.active = false;
                self.agents_hud.focused = false;
                self.focus_and_raise(id);
                Ok(Some(format!("Prompt for pane #{id}: type and press Enter to inject")))
            }
            HudButtonKind::Zoom => {
                self.agents_hud.active = false;
                self.agents_hud.focused = false;
                self.focus_and_raise(id);
                if self.zoomed_pane() != Some(id) {
                    self.apply(Action::ZoomToggle).map_err(|e| e.to_string())?;
                }
                Ok(Some(format!("Zoomed pane #{id}")))
            }
            HudButtonKind::Diff => {
                let Some(info) = self.swarm_node_info(id) else {
                    return Ok(Some(format!("Diff: pane #{id} is not part of a swarm (no worktree to diff)")));
                };
                let v = self.swarm_apply(SwarmOp::Diff(info.slug.clone()))?;
                let n = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
                let files: Vec<String> = match v.get("files") {
                    Some(JsonValue::Array(a)) => a.iter().filter_map(|f| f.get("path").and_then(|p| p.as_str()).map(str::to_string)).take(5).collect(),
                    _ => Vec::new(),
                };
                Ok(Some(format!(
                    "Diff {} ({}): {} file(s) +{} -{} {}",
                    info.node, info.slug, n("files_changed"), n("insertions"), n("deletions"), files.join(" ")
                ).trim_end().to_string()))
            }
        }
    }

    /// `agent.approve` / HUD Approve: answers a blocked agent's approval prompt.
    ///
    /// `answer`: `yes`/`enter`, `y`, `n`, `no`/`esc`. Needs the pane to be blocked unless `force`;
    /// refuses read-only panes and `!ECHO` password prompts unless `allow_echo`. Returns the state
    /// the pane was in.
    pub fn agent_approve_pane(
        &mut self,
        id: PaneId,
        answer: &str,
        force: bool,
        allow_echo: bool,
        source: TrafficEndpoint,
    ) -> Result<&'static str, String> {
        let pane = self
            .panes
            .get(&id)
            .ok_or_else(|| format!("pane #{id} not found"))?;
        if pane.is_read_only() {
            return Err(format!("pane #{id} is locked read-only (PROT)"));
        }
        let redact = pane.is_password_prompt();
        if redact && !allow_echo {
            return Err(format!(
                "pane #{id} is in !ECHO LOCK password prompt; set allow_echo_lock=true to override"
            ));
        }
        let state = self.agent_state(id).map_or("unknown", |st| st.as_str());
        if state != "blocked" && !force {
            return Err(format!(
                "pane #{id} is {state}, not blocked (pass force=true to send anyway)"
            ));
        }
        let answer = answer.to_ascii_lowercase();
        let bytes: &[u8] = match answer.as_str() {
            "yes" | "enter" => b"\r",
            "y" => b"y\r",
            "n" | "no-text" => b"n\r",
            "no" | "esc" | "escape" => b"\x1b",
            other => return Err(format!("unknown answer '{other}' (yes, y, n, no, esc)")),
        };
        self.panes
            .get_mut(&id)
            .ok_or_else(|| format!("pane #{id} not found"))?
            .write_input(bytes)
            .map_err(|e| e.to_string())?;
        self.record_traffic_with(
            source,
            TrafficEndpoint::Pane(id),
            TrafficKind::Keys,
            &answer,
            redact,
            Instant::now(),
        );
        Ok(state)
    }

    /// Notes prompt text injected into a pane (`agent.prompt`) as the prompt of its next replay turn.
    pub fn agents_hud_note_prompt(&mut self, pane: PaneId, text: &str) {
        self.agents_hud.note_prompt(pane, text);
    }

    /// Executes one replay operation (read-only: no PTY writer is reachable from here).
    pub fn agents_replay_apply(&mut self, op: &ReplayOp) -> Result<Option<String>, String> {
        let viewport = self.agents_hud.viewport();
        let focused = self.active_pane_id();
        // `show` and the prev|next|first|last verbs fall back to the focused pane's replay.
        let implicit_open = matches!(
            op,
            ReplayOp::Show | ReplayOp::Prev | ReplayOp::Next | ReplayOp::First | ReplayOp::Last
        );
        if implicit_open && self.agents_hud.replay.is_none() {
            return self.agents_replay_apply(&ReplayOp::Open(focused));
        }
        if matches!(op, ReplayOp::Show) {
            let id = self.agents_hud.replay.unwrap_or(focused);
            let n = self.agents_hud.logs.get(&id).map_or(0, TurnReplayLog::len);
            return Ok(Some(format!("Replay pane #{id}: {n} turn(s)")));
        }
        if let ReplayOp::Open(id) = op {
            if !self.panes.contains_key(id) {
                return Err(format!("pane #{id} not found"));
            }
            self.agents_hud.logs.entry(*id).or_default();
            self.agents_hud.replay = Some(*id);
            self.agents_hud.focused = true;
            let n = self.agents_hud.logs.get(id).map_or(0, TurnReplayLog::len);
            return Ok(Some(format!("Replay pane #{id}: {n} turn(s)")));
        }
        if matches!(op, ReplayOp::Close) {
            self.agents_hud.replay = None;
            return Ok(None);
        }
        let id = self
            .agents_hud
            .replay
            .ok_or_else(|| "no replay view is open (:agent replay <pane>)".to_string())?;
        let log = self.agents_hud.logs.entry(id).or_default();
        let moved = match op {
            ReplayOp::Prev => log.prev(),
            ReplayOp::Next => log.next(),
            ReplayOp::First => log.first(),
            ReplayOp::Last => log.last(),
            ReplayOp::Goto(t) => log.goto(*t),
            ReplayOp::Scroll(n) => {
                let k = usize::from(n.unsigned_abs());
                if *n < 0 {
                    log.scroll_up(k);
                } else {
                    log.scroll_down(k, viewport);
                }
                true
            }
            ReplayOp::Show | ReplayOp::Open(_) | ReplayOp::Close => true,
        };
        Ok((!moved).then(|| "No more turns in that direction".to_string()))
    }
}

/// Builds a fresh card for `pane`, carrying over the sparkline of `old`.
pub(super) fn base_card(
    pane: PaneId,
    title: &str,
    state: AgentState,
    old: Option<&AgentCardTelemetry>,
) -> AgentCardTelemetry {
    let mut card = AgentCardTelemetry::new(pane, title, state);
    if let Some(o) = old {
        card.spark = o.spark.clone();
    }
    card
}
