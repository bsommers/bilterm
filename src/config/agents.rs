//! Optional `[agents.<name>]` profiles (Phase 50-03, D-03).
//!
//! A profile teaches Bilterm an agent's screen phrases; nothing ships enabled by default.

use serde::{Deserialize, Serialize};

/// Maximum patterns kept per list.
pub const MAX_PATTERNS: usize = 16;
/// Maximum characters kept per pattern.
pub const MAX_PATTERN_CHARS: usize = 80;

/// User-authored recognition patterns for one agent program.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentProfile {
    /// Substrings of the running command / title that select this profile (defaults to the profile name).
    pub matches: Vec<String>,
    /// Case-insensitive substrings of the visible tail that mean the agent is working.
    pub working: Vec<String>,
    /// Case-insensitive substrings of the visible tail that mean the agent is blocked on the user.
    pub blocked: Vec<String>,
}

fn clean(list: &[String]) -> Vec<String> {
    list.iter()
        .map(|p| p.trim().chars().take(MAX_PATTERN_CHARS).collect::<String>().trim_end().to_lowercase())
        .filter(|p| !p.is_empty())
        .take(MAX_PATTERNS)
        .collect()
}

impl AgentProfile {
    /// Trimmed, lower-cased copy without empty entries, at most 16 patterns of at most 80 chars per list.
    pub fn sanitized(&self) -> AgentProfile {
        AgentProfile {
            matches: clean(&self.matches),
            working: clean(&self.working),
            blocked: clean(&self.blocked),
        }
    }
}

/// Default [`AgentsWindowConfig::edge_fade_secs`].
pub const DEFAULT_EDGE_FADE_SECS: u32 = 60;
/// Allowed range of [`AgentsWindowConfig::edge_fade_secs`].
pub const EDGE_FADE_RANGE: std::ops::RangeInclusive<u32> = 1..=3600;

/// Agent traffic window settings (`[agents_window]`). The profile tables live under `[agents.<name>]`,
/// so window settings cannot share that table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsWindowConfig {
    /// Seconds over which a graph edge's brightness from recent traffic decays to its floor
    /// (`1..=3600`, default `60`). An edge never disappears once its pair has spoken.
    pub edge_fade_secs: u32,
}

impl Default for AgentsWindowConfig {
    fn default() -> Self {
        Self { edge_fade_secs: DEFAULT_EDGE_FADE_SECS }
    }
}

impl AgentsWindowConfig {
    /// The fade window as a duration, with an out-of-range configured value clamped.
    pub fn edge_fade(&self) -> std::time::Duration {
        let s = self.edge_fade_secs.clamp(*EDGE_FADE_RANGE.start(), *EDGE_FADE_RANGE.end());
        std::time::Duration::from_secs(u64::from(s))
    }
}

/// Allowed `alert` sinks of `[agents.watchdog]`.
pub const WATCHDOG_ALERT_SINKS: [&str; 3] = ["oia", "toast", "event"];

/// `[agents.watchdog]` (Phase 70, HUD-03): stall and loop detection for agent panes. Never kills.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsWatchdogConfig {
    /// Master switch (default `true`); `false` makes the watchdog a no-op.
    pub enabled: bool,
    /// Working with no output for this many seconds raises a stall alert (`1..=86400`).
    pub stall_secs: u64,
    /// Number of recent output lines inspected for repeats (`2..=256`).
    pub loop_window: usize,
    /// The same normalized line seen this many times in the window raises a loop alert.
    pub loop_repeat: usize,
    /// Where alerts go: any of `"oia"` (status text), `"toast"`, `"event"` (RPC `events.poll`).
    pub alert: Vec<String>,
    /// Seconds before the same alert kind repeats for the same pane (`0..=86400`).
    #[serde(alias = "cooldown")]
    pub cooldown_secs: u64,
}

impl Default for AgentsWatchdogConfig {
    fn default() -> Self {
        let d = crate::wm::agent_watchdog::WatchdogConfig::default();
        Self {
            enabled: d.enabled,
            stall_secs: d.stall_secs,
            loop_window: d.loop_window,
            loop_repeat: d.loop_repeat,
            alert: WATCHDOG_ALERT_SINKS.iter().map(ToString::to_string).collect(),
            cooldown_secs: d.cooldown_secs,
        }
    }
}

impl AgentsWatchdogConfig {
    /// The detector settings (without the alert sinks).
    pub fn to_runtime(&self) -> crate::wm::agent_watchdog::WatchdogConfig {
        crate::wm::agent_watchdog::WatchdogConfig {
            enabled: self.enabled,
            stall_secs: self.stall_secs,
            loop_window: self.loop_window,
            loop_repeat: self.loop_repeat,
            cooldown_secs: self.cooldown_secs,
        }
    }

    /// Whether `sink` (`oia` / `toast` / `event`) is enabled.
    pub fn sends_to(&self, sink: &str) -> bool {
        self.alert.iter().any(|a| a == sink)
    }

    /// Range and sink validation; the error names the offending key.
    pub fn validate(&self) -> Result<(), String> {
        self.to_runtime().validate().map_err(|e| format!("[agents.watchdog] {e}"))?;
        if self.alert.len() > WATCHDOG_ALERT_SINKS.len() {
            return Err("[agents.watchdog] alert lists too many entries".to_string());
        }
        for a in &self.alert {
            if !WATCHDOG_ALERT_SINKS.contains(&a.as_str()) {
                return Err(format!("[agents.watchdog] unknown alert sink '{a}' (oia, toast, event)"));
            }
        }
        Ok(())
    }
}

/// The `[agents]` table: `[agents.watchdog]` plus the free-form `[agents.<name>]` profiles.
///
/// Derefs to the profile map so profile lookups read as before.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsSection {
    /// Watchdog settings (`[agents.watchdog]`).
    #[serde(skip_serializing_if = "AgentsWatchdogConfig::is_default")]
    pub watchdog: AgentsWatchdogConfig,
    /// Recognition profiles (`[agents.<name>]`).
    #[serde(flatten)]
    pub profiles: std::collections::BTreeMap<String, AgentProfile>,
}

impl AgentsWatchdogConfig {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl AgentsSection {
    /// True when nothing is configured (the section is then not serialised).
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty() && self.watchdog.is_default()
    }
}

impl std::ops::Deref for AgentsSection {
    type Target = std::collections::BTreeMap<String, AgentProfile>;
    fn deref(&self) -> &Self::Target {
        &self.profiles
    }
}

impl std::ops::DerefMut for AgentsSection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.profiles
    }
}
