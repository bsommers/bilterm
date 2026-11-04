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
