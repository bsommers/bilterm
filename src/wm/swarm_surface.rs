//! UNI-R01 surface state and operation enums for `:swarm`, `:agents hud` and `:agent replay`
//! (Phases 69/70). The engines live in [`crate::swarm`], [`super::mission_control_hud`] and
//! [`super::turn_replay`]; this module only owns their per-`WindowManager` instances.

use super::mission_control_hud::MissionControlHud;
use super::turn_replay::TurnReplayLog;
use crate::pane::PaneId;
use crate::swarm::SwarmOrchestrator;
use std::collections::BTreeMap;

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

/// `:agents hud [open|close|toggle]` - Mission Control HUD window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudOp {
    Toggle,
    Open,
    Close,
}

impl HudOp {
    /// Canonical colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Toggle => ":agents hud toggle",
            Self::Open => ":agents hud open",
            Self::Close => ":agents hud close",
        }
        .to_string()
    }
}

/// `:agent replay [show|prev|next|first|last]` - read-only per-turn time travel for the active pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayOp {
    Show,
    Prev,
    Next,
    First,
    Last,
}

impl ReplayOp {
    /// Canonical colon form.
    pub fn colon_form(&self) -> String {
        let v = match self {
            Self::Show => "show",
            Self::Prev => "prev",
            Self::Next => "next",
            Self::First => "first",
            Self::Last => "last",
        };
        format!(":agent replay {v}")
    }
}

/// Engine instances owned by the window manager.
#[derive(Debug, Default)]
pub struct SwarmSurface {
    pub swarm: Option<SwarmOrchestrator>,
    pub hud: MissionControlHud,
    pub hud_open: bool,
    pub replay: BTreeMap<PaneId, TurnReplayLog>,
}
