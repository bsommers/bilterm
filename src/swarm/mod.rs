//! Ephemeral git-worktree swarm orchestration (Phase 69, SWARM-01..04).
//!
//! Pure, testable building blocks: worktree specs, a TOML DAG plan with wave
//! scheduling, diff/merge review helpers, and a bounded self-healing loop.

pub mod dag;
pub mod heal;
pub mod ops;
pub mod review;
pub mod worktree;

use serde::Deserialize;
use std::collections::HashSet;

pub use dag::{NodeState, Scheduler};
pub use worktree::WorktreeSpec;

/// Hard caps (T-SW-02 / T-SW-03).
pub const MAX_NODES: usize = 64;
pub const MAX_PARALLEL_CAP: usize = 16;
pub const DEFAULT_MAX_PARALLEL: usize = 4;
pub const DEFAULT_MAX_RETRIES: u32 = 2;
pub const MAX_RETRIES_CEILING: u32 = 10;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SwarmError {
    #[error("plan parse error: {0}")]
    Parse(String),
    #[error("invalid identifier: {0}")]
    InvalidName(String),
    #[error("duplicate node id: {0}")]
    DuplicateId(String),
    #[error("node '{node}' depends on unknown node '{dep}'")]
    UnknownDep { node: String, dep: String },
    #[error("dependency cycle involving: {0:?}")]
    Cycle(Vec<String>),
    #[error("plan has {0} nodes, exceeding the cap of {1}")]
    TooManyNodes(usize, usize),
    #[error("plan has no nodes")]
    Empty,
    #[error("limit out of range: {0}")]
    Limit(String),
    #[error("illegal transition for '{node}': {from:?} -> {to:?}")]
    BadTransition {
        node: String,
        from: NodeState,
        to: NodeState,
    },
    #[error("unknown node: {0}")]
    UnknownNode(String),
    #[error("git: {0}")]
    Git(String),
}

/// One DAG node: an agent working in its own worktree.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WaveTask {
    pub id: String,
    #[serde(default = "default_agent")]
    pub agent: String,
    /// Worktree slug; defaults to the node id.
    #[serde(default)]
    pub worktree: Option<String>,
    pub prompt: String,
    #[serde(default)]
    pub verify_cmd: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub max_retries: Option<u32>,
}

fn default_agent() -> String {
    "claude".to_string()
}

impl WaveTask {
    pub fn worktree_slug(&self) -> &str {
        self.worktree.as_deref().unwrap_or(&self.id)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawSettings {
    max_parallel: Option<usize>,
    max_retries: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawPlan {
    #[serde(default)]
    swarm: RawSettings,
    #[serde(default)]
    node: Vec<WaveTask>,
}

/// A validated swarm plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmPlan {
    pub max_parallel: usize,
    pub max_retries: u32,
    pub nodes: Vec<WaveTask>,
}

impl SwarmPlan {
    /// Parse and fully validate a `plan.toml` document.
    pub fn parse(src: &str) -> Result<Self, SwarmError> {
        let raw: RawPlan = toml::from_str(src).map_err(|e| SwarmError::Parse(e.to_string()))?;
        let max_parallel = raw.swarm.max_parallel.unwrap_or(DEFAULT_MAX_PARALLEL);
        if max_parallel == 0 || max_parallel > MAX_PARALLEL_CAP {
            return Err(SwarmError::Limit(format!(
                "max_parallel must be 1..={MAX_PARALLEL_CAP}, got {max_parallel}"
            )));
        }
        let max_retries = raw.swarm.max_retries.unwrap_or(DEFAULT_MAX_RETRIES);
        if max_retries > MAX_RETRIES_CEILING {
            return Err(SwarmError::Limit(format!(
                "max_retries must be <= {MAX_RETRIES_CEILING}, got {max_retries}"
            )));
        }
        let plan = SwarmPlan {
            max_parallel,
            max_retries,
            nodes: raw.node,
        };
        plan.validate()?;
        Ok(plan)
    }

    fn validate(&self) -> Result<(), SwarmError> {
        if self.nodes.is_empty() {
            return Err(SwarmError::Empty);
        }
        if self.nodes.len() > MAX_NODES {
            return Err(SwarmError::TooManyNodes(self.nodes.len(), MAX_NODES));
        }
        let mut ids = HashSet::new();
        for n in &self.nodes {
            crate::secure_fs::validate_safe_name(&n.id).map_err(SwarmError::InvalidName)?;
            crate::secure_fs::validate_safe_name(n.worktree_slug())
                .map_err(SwarmError::InvalidName)?;
            if let Some(r) = n.max_retries
                && r > MAX_RETRIES_CEILING
            {
                return Err(SwarmError::Limit(format!(
                    "node '{}' max_retries {r}",
                    n.id
                )));
            }
            if !ids.insert(n.id.as_str()) {
                return Err(SwarmError::DuplicateId(n.id.clone()));
            }
        }
        let mut slugs = HashSet::new();
        for n in &self.nodes {
            if !slugs.insert(n.worktree_slug()) {
                return Err(SwarmError::DuplicateId(format!(
                    "worktree {}",
                    n.worktree_slug()
                )));
            }
            for d in &n.depends_on {
                if !ids.contains(d.as_str()) {
                    return Err(SwarmError::UnknownDep {
                        node: n.id.clone(),
                        dep: d.clone(),
                    });
                }
            }
        }
        dag::waves(&self.nodes).map(|_| ())
    }

    /// Topological waves (each wave's nodes are mutually independent).
    pub fn waves(&self) -> Vec<Vec<String>> {
        dag::waves(&self.nodes).unwrap_or_default()
    }

    pub fn node(&self, id: &str) -> Option<&WaveTask> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn retries_for(&self, id: &str) -> u32 {
        self.node(id)
            .and_then(|n| n.max_retries)
            .unwrap_or(self.max_retries)
    }
}

/// Drives a plan: wave scheduling plus per-node state.
#[derive(Debug)]
pub struct SwarmOrchestrator {
    plan: SwarmPlan,
    sched: Scheduler,
}

impl SwarmOrchestrator {
    pub fn new(plan: SwarmPlan) -> Self {
        let sched = Scheduler::new(&plan);
        Self { plan, sched }
    }

    pub fn plan(&self) -> &SwarmPlan {
        &self.plan
    }

    pub fn scheduler(&self) -> &Scheduler {
        &self.sched
    }

    /// Start every currently-ready node (bounded by `max_parallel`) and return them.
    pub fn start_ready(&mut self) -> Result<Vec<WaveTask>, SwarmError> {
        let ids = self.sched.ready();
        let mut out = Vec::new();
        for id in ids {
            self.sched.transition(&id, NodeState::Running)?;
            if let Some(n) = self.plan.node(&id) {
                out.push(n.clone());
            }
        }
        Ok(out)
    }

    pub fn begin_verify(&mut self, id: &str) -> Result<(), SwarmError> {
        self.sched.transition(id, NodeState::Verifying)
    }

    pub fn pass(&mut self, id: &str) -> Result<(), SwarmError> {
        self.sched.transition(id, NodeState::Passed)
    }

    pub fn fail(&mut self, id: &str) -> Result<(), SwarmError> {
        self.sched.transition(id, NodeState::Failed)
    }

    /// Verification failed but retries remain: go back to Running.
    pub fn retry(&mut self, id: &str) -> Result<(), SwarmError> {
        self.sched.transition(id, NodeState::Running)
    }

    pub fn is_finished(&self) -> bool {
        self.sched.is_finished()
    }
}
