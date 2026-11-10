//! Live feed of the Mission Control HUD, the turn-replay logs and the watchdog (Phase 70).
//!
//! Called from [`WindowManager::tick_agent_ui`] on the poll tick. Everything here only *reads*
//! pane state; nothing resizes a pane or writes to a PTY. Panes in a no-echo (password) prompt
//! contribute neither output lines nor preview text (`T-HUD-02`).

use std::collections::BTreeSet;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::WindowManager;
use super::agent_state::AgentState;
use super::agent_watchdog::{
    AgentWatchdog, AlertKind, WatchdogAlert, WatchdogConfig, orchestrator_from_env,
};
use super::agents_hud::{AlertRecord, MAX_ALERT_HISTORY, MAX_ALERT_OUTBOX, base_card};
use super::mission_control_hud::MAX_ARCS;
use crate::pane::{CompositePane, PaneId};

/// Longest window over which pane-to-pane traffic draws an arc.
const ARC_WINDOW_SECS: u64 = 30;
/// Output lines fed per pane and tick.
const MAX_NEW_LINES: usize = 64;
/// Visible rows kept per pane as the diff baseline.
const MAX_ROWS: usize = 200;
/// Preview lines kept on a tile.
const PREVIEW_LINES: usize = 3;
/// How long a toast alert stays up.
const TOAST_MS: u64 = 8_000;

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Non-empty visible rows of a pane, right-trimmed (bounded).
fn visible_rows(pane: &CompositePane) -> Vec<String> {
    let (rows, _) = pane.vt().size();
    (0..rows)
        .map(|r| pane.vt().row_text(r).trim_end().to_string())
        .filter(|l| !l.is_empty())
        .take(MAX_ROWS)
        .collect()
}

/// Lines in `cur` that were not on screen in `prev` (scroll overlap first, set difference otherwise).
fn new_lines_between(prev: &[String], cur: &[String]) -> Vec<String> {
    if prev == cur {
        return Vec::new();
    }
    for k in (1..=prev.len().min(cur.len())).rev() {
        if prev[prev.len() - k..] == cur[..k] {
            return cur[k..].iter().take(MAX_NEW_LINES).cloned().collect();
        }
    }
    cur.iter()
        .filter(|l| !prev.contains(l))
        .take(MAX_NEW_LINES)
        .cloned()
        .collect()
}

impl WindowManager {
    /// Poll-tick hook; returns `true` when something visible changed.
    pub(crate) fn tick_agents_hud(&mut self, now: Instant) -> bool {
        self.agents_hud_tick_at(now, unix_secs())
    }

    /// [`Self::tick_agents_hud`] with an explicit wall clock in seconds (deterministic tests).
    pub fn agents_hud_tick_at(&mut self, _now: Instant, now_secs: u64) -> bool {
        self.sync_watchdog_config();
        // Feed changes are only visible while a HUD / replay window is open; alerts always are.
        let mut changed = self.hud_feed(now_secs) && self.agents_hud.visible();
        for alert in self.agents_hud.watchdog.tick(now_secs) {
            self.raise_watchdog_alert(&alert, now_secs);
            changed = true;
        }
        if self.agents_hud.active {
            let ms = u64::try_from(self.agents_hud.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
            let sample = ms.saturating_sub(self.agents_hud.last_spark_ms) >= 1000;
            if sample || changed {
                self.agents_hud.hud.mark_dirty();
            }
            if self.agents_hud.hud.take_refresh(ms) {
                self.agents_hud_refresh(sample);
                self.agents_hud.hud.clear_dirty();
                if sample {
                    self.agents_hud.last_spark_ms = ms;
                }
                changed = true;
            }
        }
        changed
    }

    /// Rebuilds the watchdog when `[agents.watchdog]` changed (an invalid section disables it).
    fn sync_watchdog_config(&mut self) {
        let section = &self.config.agents.watchdog;
        let mut cfg: WatchdogConfig = section.to_runtime();
        if section.validate().is_err() {
            cfg.enabled = false;
        }
        if *self.agents_hud.watchdog.config() != cfg {
            let mut wd = AgentWatchdog::new(cfg);
            wd.set_orchestrator(orchestrator_from_env());
            self.agents_hud.watchdog = wd;
        }
    }

    /// Feeds pane output and agent-state transitions to the replay logs and the watchdog.
    fn hud_feed(&mut self, now_secs: u64) -> bool {
        let mut changed = false;
        let ids: Vec<PaneId> = self.panes.keys().copied().collect();
        let alive: BTreeSet<PaneId> = ids.iter().copied().collect();
        let hud = &mut self.agents_hud;
        hud.feed.retain(|id, _| alive.contains(id));
        hud.logs.retain(|id, _| alive.contains(id));
        hud.log_states.retain(|id, _| alive.contains(id));
        hud.gen_states.retain(|id, _| alive.contains(id));
        hud.generations.retain(|id, _| alive.contains(id));
        hud.pending_prompt.retain(|id, _| alive.contains(id));
        let gone: Vec<PaneId> = hud
            .hud
            .card_ids()
            .into_iter()
            .filter(|id| !alive.contains(id))
            .collect();
        for id in gone {
            hud.hud.remove_card(id);
            hud.watchdog.forget(id);
        }
        if hud.replay.is_some_and(|id| !alive.contains(&id)) {
            hud.replay = None;
            changed = true;
        }
        for id in ids {
            let st = self.agent_state(id).unwrap_or(AgentState::Idle);
            let Some(pane) = self.panes.get(&id) else {
                continue;
            };
            let hud = &mut self.agents_hud;
            let gen_now = pane.vt().generation();
            let masked = pane.is_password_prompt();
            let blocks = pane.vt().command_blocks();
            let (n_blocks, last_cmd) = (blocks.len(), blocks.last().map(|b| b.command.clone()));
            let cur = hud.feed.entry(id).or_default();
            let mut fresh = Vec::new();
            if cur.gen_seen != Some(gen_now) {
                let rows = visible_rows(pane);
                if cur.gen_seen.is_some() {
                    fresh = new_lines_between(&cur.rows, &rows);
                }
                cur.gen_seen = Some(gen_now);
                cur.rows = rows;
                hud.watchdog.observe_line(id, "", now_secs);
            }
            if masked {
                fresh.clear();
            }
            for l in &fresh {
                hud.watchdog.observe_line(id, l, now_secs);
            }
            if n_blocks > cur.blocks
                && let Some(cmd) = &last_cmd
            {
                hud.watchdog.observe_command(id, cmd, now_secs);
            }
            cur.blocks = n_blocks;
            hud.watchdog
                .set_working(id, st == AgentState::Working, now_secs);

            let prev = hud.log_states.insert(id, st);
            let log = hud.logs.entry(id).or_default();
            if st == AgentState::Working {
                if !log.in_turn() {
                    let prompt = hud
                        .pending_prompt
                        .remove(&id)
                        .or(last_cmd)
                        .unwrap_or_else(|| "(agent turn)".to_string());
                    log.begin_turn(&prompt, now_secs);
                    cur.recorded = 0;
                    changed = true;
                }
                log.mark_working(now_secs);
            }
            if log.in_turn() {
                for l in &fresh {
                    log.record_output(l);
                    cur.recorded += 1;
                }
                let finished = matches!(prev, Some(AgentState::Working | AgentState::Blocked))
                    && matches!(
                        st,
                        AgentState::Idle | AgentState::Done | AgentState::Errored
                    );
                if finished {
                    let snap = if cur.recorded == 0 && !masked {
                        cur.rows.clone()
                    } else {
                        Vec::new()
                    };
                    log.end_turn(now_secs, snap);
                    changed = true;
                }
            }
            if !fresh.is_empty() || prev != Some(st) {
                changed = true;
            }
        }
        changed
    }

    /// Recomputes every tile from live pane state (read-only; never touches a PTY).
    pub(crate) fn agents_hud_refresh(&mut self, sample_spark: bool) {
        let named: std::collections::BTreeMap<PaneId, (String, AgentState)> = self
            .agent_statuses()
            .into_iter()
            .map(|s| (s.pane_id, (s.name, s.state)))
            .collect();
        let ids: Vec<PaneId> = self.panes.keys().copied().collect();
        let hud = &mut self.agents_hud;
        for gone in hud
            .hud
            .card_ids()
            .into_iter()
            .filter(|id| !self.panes.contains_key(id))
        {
            hud.hud.remove_card(gone);
        }
        for id in ids {
            let Some(pane) = self.panes.get(&id) else {
                continue;
            };
            let (name, state) = named
                .get(&id)
                .cloned()
                .unwrap_or_else(|| (pane.title().to_string(), AgentState::Idle));
            if hud.gen_states.insert(id, state) != Some(state) {
                *hud.generations.entry(id).or_insert(0) += 1;
            }
            let mut card = base_card(id, &name, state, hud.hud.card(id));
            card.generation = hud.generations.get(&id).copied().unwrap_or(1);
            card.masked = pane.is_password_prompt();
            card.queue_depth = self.prompt_queue.len(id);
            if sample_spark {
                card.push_rate(u32::try_from(pane.traffic_rates().0).unwrap_or(u32::MAX));
            }
            if !card.masked {
                let rows = visible_rows(pane);
                card.preview = rows[rows.len().saturating_sub(PREVIEW_LINES)..].to_vec();
            }
            hud.hud.upsert_card(card);
        }
        let now = Instant::now();
        let mut arcs: Vec<(PaneId, PaneId)> = Vec::new();
        for e in self.traffic.iter() {
            if let (
                super::traffic::TrafficEndpoint::Pane(a),
                super::traffic::TrafficEndpoint::Pane(b),
            ) = (&e.from, &e.to)
                && now.saturating_duration_since(e.at).as_secs() <= ARC_WINDOW_SECS
                && !arcs.contains(&(*a, *b))
                && arcs.len() < MAX_ARCS
            {
                arcs.push((*a, *b));
            }
        }
        self.agents_hud.hud.set_arcs(&arcs);
    }

    fn raise_watchdog_alert(&mut self, alert: &WatchdogAlert, now_secs: u64) {
        let (kind, detail, count) = match &alert.kind {
            AlertKind::Stall => (
                "stall",
                format!("no output for >{}s", self.config.agents.watchdog.stall_secs),
                0,
            ),
            AlertKind::Loop { signature, count } => ("loop", signature.clone(), *count),
        };
        let rec = AlertRecord {
            pane: alert.pane,
            kind,
            detail,
            count,
            at_secs: now_secs,
            target: alert.target,
        };
        let msg = match kind {
            "stall" => format!("WATCHDOG #{} stalled: {}", rec.pane, rec.detail),
            _ => format!("WATCHDOG #{} loop x{}: {}", rec.pane, rec.count, rec.detail),
        };
        let cfg = &self.config.agents.watchdog;
        let (oia, toast, event) = (
            cfg.sends_to("oia"),
            cfg.sends_to("toast"),
            cfg.sends_to("event"),
        );
        if oia {
            self.active_pane_mut().set_status_message(Some(msg));
        } else if toast {
            self.active_pane_mut().set_status_toast(msg, TOAST_MS);
        }
        let hud = &mut self.agents_hud;
        if event {
            if hud.outbox.len() >= MAX_ALERT_OUTBOX {
                hud.outbox.pop_front();
            }
            hud.outbox.push_back(rec.clone());
        }
        if hud.alerts.len() >= MAX_ALERT_HISTORY {
            hud.alerts.pop_front();
        }
        hud.alerts.push_back(rec);
    }

    /// Drains alerts destined for RPC events (`[agents.watchdog] alert` contains `"event"`).
    pub fn take_watchdog_events(&mut self) -> Vec<AlertRecord> {
        self.agents_hud.outbox.drain(..).collect()
    }
}
