//! Optional `[agents.<name>]` profiles (Phase 50-03, D-03).
//!
//! A profile teaches Bilterm an agent's screen phrases; nothing ships enabled by default.

use serde::{Deserialize, Serialize};

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
