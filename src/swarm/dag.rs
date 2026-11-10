//! SWARM-02: topological wave scheduler, cycle detection, node state machine.

use super::{SwarmError, SwarmPlan, WaveTask};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    Pending,
    Running,
    Verifying,
    Passed,
    Failed,
}

impl NodeState {
    fn can_go(self, to: NodeState) -> bool {
        use NodeState::*;
        matches!(
            (self, to),
            (Pending, Running)
                | (Running, Verifying)
                | (Running, Passed) // nodes without verify_cmd
                | (Running, Failed)
                | (Verifying, Passed)
                | (Verifying, Failed)
                | (Verifying, Running) // self-heal retry
        )
    }
}

/// Kahn's algorithm grouped into waves. Errors on a cycle.
pub fn waves(nodes: &[WaveTask]) -> Result<Vec<Vec<String>>, SwarmError> {
    let mut indeg: BTreeMap<&str, usize> = BTreeMap::new();
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for n in nodes {
        indeg.entry(n.id.as_str()).or_insert(0);
    }
    for n in nodes {
        for d in &n.depends_on {
            *indeg.entry(n.id.as_str()).or_insert(0) += 1;
            children.entry(d.as_str()).or_default().push(n.id.as_str());
        }
    }
    let mut out = Vec::new();
    let mut seen = 0usize;
    let mut cur: Vec<&str> = indeg
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(k, _)| *k)
        .collect();
    while !cur.is_empty() {
        seen += cur.len();
        let mut next = BTreeSet::new();
        for id in &cur {
            for c in children.get(id).into_iter().flatten() {
                let e = indeg.get_mut(c).expect("child indexed");
                *e -= 1;
                if *e == 0 {
                    next.insert(*c);
                }
            }
        }
        out.push(cur.iter().map(|s| s.to_string()).collect());
        cur = next.into_iter().collect();
    }
    if seen != indeg.len() {
        let stuck = indeg
            .iter()
            .filter(|(_, d)| **d > 0)
            .map(|(k, _)| k.to_string())
            .collect();
        return Err(SwarmError::Cycle(stuck));
    }
    Ok(out)
}

#[derive(Debug)]
pub struct Scheduler {
    order: Vec<String>,
    deps: BTreeMap<String, Vec<String>>,
    states: BTreeMap<String, NodeState>,
    max_parallel: usize,
}

impl Scheduler {
    pub fn new(plan: &SwarmPlan) -> Self {
        Self {
            order: plan.nodes.iter().map(|n| n.id.clone()).collect(),
            deps: plan
                .nodes
                .iter()
                .map(|n| (n.id.clone(), n.depends_on.clone()))
                .collect(),
            states: plan
                .nodes
                .iter()
                .map(|n| (n.id.clone(), NodeState::Pending))
                .collect(),
            max_parallel: plan.max_parallel,
        }
    }

    pub fn state(&self, id: &str) -> Option<NodeState> {
        self.states.get(id).copied()
    }

    fn active(&self) -> usize {
        self.states
            .values()
            .filter(|s| matches!(s, NodeState::Running | NodeState::Verifying))
            .count()
    }

    /// Pending nodes whose deps all Passed, capped by free parallel slots.
    pub fn ready(&self) -> Vec<String> {
        let slots = self.max_parallel.saturating_sub(self.active());
        self.order
            .iter()
            .filter(|id| self.states[*id] == NodeState::Pending)
            .filter(|id| {
                self.deps[*id]
                    .iter()
                    .all(|d| self.states[d] == NodeState::Passed)
            })
            .take(slots)
            .cloned()
            .collect()
    }

    /// Pending nodes that can never run because an ancestor Failed.
    pub fn blocked(&self) -> Vec<String> {
        let mut dead: BTreeSet<&str> = self
            .states
            .iter()
            .filter(|(_, s)| **s == NodeState::Failed)
            .map(|(k, _)| k.as_str())
            .collect();
        loop {
            let before = dead.len();
            for id in &self.order {
                if self.states[id] == NodeState::Pending
                    && self.deps[id].iter().any(|d| dead.contains(d.as_str()))
                {
                    dead.insert(id.as_str());
                }
            }
            if dead.len() == before {
                break;
            }
        }
        self.order
            .iter()
            .filter(|id| self.states[*id] == NodeState::Pending && dead.contains(id.as_str()))
            .cloned()
            .collect()
    }

    pub fn transition(&mut self, id: &str, to: NodeState) -> Result<(), SwarmError> {
        let from = *self
            .states
            .get(id)
            .ok_or_else(|| SwarmError::UnknownNode(id.to_string()))?;
        if !from.can_go(to) {
            return Err(SwarmError::BadTransition {
                node: id.to_string(),
                from,
                to,
            });
        }
        self.states.insert(id.to_string(), to);
        Ok(())
    }

    /// Finished = nothing active and nothing left that can still run.
    pub fn is_finished(&self) -> bool {
        self.active() == 0 && self.ready().is_empty()
    }

    pub fn all_passed(&self) -> bool {
        self.states.values().all(|s| *s == NodeState::Passed)
    }
}
