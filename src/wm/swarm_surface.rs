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
    /// `:swarm run` - start every ready node (worktree + pane + agent + queued prompt).
    Run,
    /// `:swarm spawn <slug> [agent] [prompt...]` - one ad-hoc worktree agent (SWARM-01).
    Spawn { slug: String, agent: String, prompt: String },
    /// `:swarm verify <node>` - background `verify_cmd` with bounded self-healing retries.
    Verify(String),
    /// `:swarm diff <slug>` - diff stat of a tracked worktree branch vs the primary HEAD.
    Diff(String),
    /// `:swarm merge <slug> [--squash]`.
    Merge { slug: String, squash: bool },
    /// `:swarm clean <slug> [--force]`.
    Clean { slug: String, force: bool },
}

impl SwarmOp {
    /// Canonical colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Status => ":swarm status".to_string(),
            Self::Load(p) => format!(":swarm load {p}"),
            Self::Clear => ":swarm clear".to_string(),
            Self::Run => ":swarm run".to_string(),
            Self::Spawn { slug, agent, prompt } if prompt.is_empty() => format!(":swarm spawn {slug} {agent}"),
            Self::Spawn { slug, agent, prompt } => format!(":swarm spawn {slug} {agent} {prompt}"),
            Self::Verify(n) => format!(":swarm verify {n}"),
            Self::Diff(s) => format!(":swarm diff {s}"),
            Self::Merge { slug, squash } => format!(":swarm merge {slug}{}", if *squash { " --squash" } else { "" }),
            Self::Clean { slug, force } => format!(":swarm clean {slug}{}", if *force { " --force" } else { "" }),
        }
    }
}

/// Engine instance owned by the window manager.
#[derive(Debug, Default)]
pub struct SwarmSurface {
    pub swarm: Option<SwarmOrchestrator>,
    /// Runtime bookkeeping for `run`/`spawn`/`verify` (panes, heal loops, background verifies).
    pub runtime: super::swarm_cmd::SwarmState,
}
