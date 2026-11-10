//! UNI-R01 surface state and operation enum for `:swarm` (Phase 69). The engine lives in
//! [`crate::swarm`]; this module only owns its per-`WindowManager` instance. The Mission Control HUD
//! and turn replay (Phase 70) live in [`super::agents_hud`].

use crate::swarm::SwarmOrchestrator;

/// `:swarm ...` operation on the loaded [`SwarmOrchestrator`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwarmOp {
    /// Report node states and waves of the loaded plan.
    Status,
    /// Load and validate a `plan.toml` (replaces any loaded plan).
    Load(String),
    /// Drop the loaded plan.
    Clear,
}

impl SwarmOp {
    /// Canonical colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Status => ":swarm status".to_string(),
            Self::Load(p) => format!(":swarm load {p}"),
            Self::Clear => ":swarm clear".to_string(),
        }
    }
}

/// Engine instance owned by the window manager.
#[derive(Debug, Default)]
pub struct SwarmSurface {
    pub swarm: Option<SwarmOrchestrator>,
}
