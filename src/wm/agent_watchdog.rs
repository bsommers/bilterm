//! Autonomous stall / loop watchdog (`HUD-03`, Phase 70).
//!
//! Pure state fed by the caller with plain `u64` second timestamps (fake-clock friendly).
//! Alerts are deduplicated per pane and kind with a cooldown (`T-WD-01`); the watchdog never
//! kills or writes to a pane, it only reports to the orchestrator pane.

use std::collections::{HashMap, VecDeque};

use crate::pane::PaneId;

/// Environment variable naming the orchestrator pane.
pub const ORCHESTRATOR_ENV: &str = "BILTERM_ORCHESTRATOR_PANE";
/// Upper bound for `loop_window`.
pub const MAX_LOOP_WINDOW: usize = 256;
/// Maximum watched panes.
pub const MAX_PANES: usize = 256;
/// Maximum characters in a normalized signature.
pub const MAX_SIGNATURE: usize = 256;
/// Upper bound for `stall_secs` / `cooldown_secs` (one day).
pub const MAX_SECS: u64 = 86_400;

/// Watchdog configuration (`[agents.watchdog]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchdogConfig {
    pub enabled: bool,
    pub stall_secs: u64,
    pub loop_window: usize,
    pub loop_repeat: usize,
    pub cooldown_secs: u64,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            stall_secs: 120,
            loop_window: 20,
            loop_repeat: 4,
            cooldown_secs: 300,
        }
    }
}

impl WatchdogConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.stall_secs == 0 || self.stall_secs > MAX_SECS {
            return Err(format!("stall_secs must be 1..={MAX_SECS}"));
        }
        if self.cooldown_secs > MAX_SECS {
            return Err(format!("cooldown_secs must be <= {MAX_SECS}"));
        }
        if self.loop_window < 2 || self.loop_window > MAX_LOOP_WINDOW {
            return Err(format!("loop_window must be 2..={MAX_LOOP_WINDOW}"));
        }
        if self.loop_repeat < 2 || self.loop_repeat > self.loop_window {
            return Err("loop_repeat must be 2..=loop_window".to_string());
        }
        Ok(())
    }
}

/// What the watchdog detected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlertKind {
    Stall,
    Loop { signature: String, count: usize },
}

impl AlertKind {
    fn slot(&self) -> usize {
        match self {
            Self::Stall => 0,
            Self::Loop { .. } => 1,
        }
    }
}

/// One alert addressed to the orchestrator pane (when known).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchdogAlert {
    pub pane: PaneId,
    pub kind: AlertKind,
    pub target: Option<PaneId>,
}

/// Parses the orchestrator pane id; `None` when absent or invalid.
pub fn resolve_orchestrator(value: Option<&str>) -> Option<PaneId> {
    value?.trim().parse::<PaneId>().ok()
}

/// Reads [`ORCHESTRATOR_ENV`] and parses it.
pub fn orchestrator_from_env() -> Option<PaneId> {
    resolve_orchestrator(std::env::var(ORCHESTRATOR_ENV).ok().as_deref())
}

/// Normalizes a line: trims, collapses whitespace, masks digit runs and hex tokens.
pub fn normalize_signature(line: &str) -> String {
    let mut out = String::new();
    for tok in line.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        let hex_body = tok.strip_prefix("0x").unwrap_or(tok);
        let is_hex = !hex_body.is_empty() && hex_body.chars().all(|c| c.is_ascii_hexdigit());
        if is_hex && (tok.starts_with("0x") || hex_body.len() >= 7) {
            out.push_str("<hex>");
            continue;
        }
        let mut prev_digit = false;
        for c in tok.chars() {
            if c.is_ascii_digit() {
                if !prev_digit {
                    out.push('#');
                }
                prev_digit = true;
            } else {
                out.push(c);
                prev_digit = false;
            }
        }
    }
    out.chars().take(MAX_SIGNATURE).collect()
}

#[derive(Debug, Default)]
struct PaneWatch {
    working: bool,
    last_output: u64,
    lines: VecDeque<String>,
    cmds: VecDeque<String>,
    last_alert: [Option<u64>; 2],
}

fn push_capped(q: &mut VecDeque<String>, s: String, cap: usize) {
    q.push_back(s);
    while q.len() > cap {
        q.pop_front();
    }
}

/// Most repeated signature in the window (ties broken lexicographically).
fn top_repeat(q: &VecDeque<String>) -> Option<(String, usize)> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for s in q {
        *counts.entry(s.as_str()).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(s, n)| (s.to_string(), n))
}

/// Per-pane stall and loop tracking.
#[derive(Debug)]
pub struct AgentWatchdog {
    cfg: WatchdogConfig,
    panes: HashMap<PaneId, PaneWatch>,
    orchestrator: Option<PaneId>,
}

impl AgentWatchdog {
    pub fn new(cfg: WatchdogConfig) -> Self {
        Self {
            cfg,
            panes: HashMap::new(),
            orchestrator: None,
        }
    }
    pub fn config(&self) -> &WatchdogConfig {
        &self.cfg
    }
    pub fn set_orchestrator(&mut self, pane: Option<PaneId>) {
        self.orchestrator = pane;
    }
    pub fn forget(&mut self, pane: PaneId) {
        self.panes.remove(&pane);
    }
    pub fn watched(&self) -> usize {
        self.panes.len()
    }

    fn entry(&mut self, pane: PaneId) -> Option<&mut PaneWatch> {
        if !self.panes.contains_key(&pane) && self.panes.len() >= MAX_PANES {
            return None;
        }
        Some(self.panes.entry(pane).or_default())
    }

    pub fn observe_line(&mut self, pane: PaneId, line: &str, now: u64) {
        let (window, enabled) = (self.cfg.loop_window.min(MAX_LOOP_WINDOW), self.cfg.enabled);
        let Some(p) = self.entry(pane) else { return };
        p.last_output = now;
        if !enabled {
            return;
        }
        let sig = normalize_signature(line);
        if !sig.is_empty() {
            push_capped(&mut p.lines, sig, window);
        }
    }

    pub fn observe_command(&mut self, pane: PaneId, cmd: &str, now: u64) {
        let (window, enabled) = (self.cfg.loop_window.min(MAX_LOOP_WINDOW), self.cfg.enabled);
        let Some(p) = self.entry(pane) else { return };
        p.last_output = now;
        if !enabled {
            return;
        }
        let sig = normalize_signature(cmd);
        if !sig.is_empty() {
            push_capped(&mut p.cmds, sig, window);
        }
    }

    pub fn set_working(&mut self, pane: PaneId, working: bool, now: u64) {
        let Some(p) = self.entry(pane) else { return };
        if working && !p.working {
            p.last_output = now;
        }
        p.working = working;
    }

    /// Evaluates all panes; returns alerts that are not within their cooldown.
    pub fn tick(&mut self, now: u64) -> Vec<WatchdogAlert> {
        let mut out = Vec::new();
        if !self.cfg.enabled {
            return out;
        }
        let (cfg, target) = (self.cfg.clone(), self.orchestrator);
        let mut ids: Vec<PaneId> = self.panes.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let Some(p) = self.panes.get_mut(&id) else {
                continue;
            };
            let mut found: Vec<AlertKind> = Vec::new();
            if p.working && now.saturating_sub(p.last_output) > cfg.stall_secs {
                found.push(AlertKind::Stall);
            }
            let line = top_repeat(&p.lines);
            let cmd = top_repeat(&p.cmds).map(|(s, n)| (format!("cmd: {s}"), n));
            let best = match (line, cmd) {
                (Some(a), Some(b)) => Some(if b.1 > a.1 { b } else { a }),
                (a, b) => a.or(b),
            };
            if let Some((signature, count)) = best
                && count >= cfg.loop_repeat
            {
                found.push(AlertKind::Loop { signature, count });
            }
            for kind in found {
                let slot = kind.slot();
                if let Some(t) = p.last_alert[slot]
                    && now.saturating_sub(t) < cfg.cooldown_secs
                {
                    continue;
                }
                p.last_alert[slot] = Some(now);
                if slot == 1 {
                    p.lines.clear();
                    p.cmds.clear();
                }
                out.push(WatchdogAlert {
                    pane: id,
                    kind,
                    target,
                });
            }
        }
        out
    }
}
