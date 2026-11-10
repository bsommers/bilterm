//! `:swarm` family: ops, state and execution (Phase 69 wiring, SWARM-01..04).
//!
//! One implementation behind every surface: colon commands, RPC (`swarm.*`, reachable from
//! `bilterm ctl`) and `Action::Swarm` all lower to [`SwarmOp`] and run [`WindowManager::swarm_apply`].

use super::WindowManager;
use crate::api::json::JsonValue;
use crate::pane::PaneId;
use crate::swarm::heal::{HealDecision, HealLoop, HealPolicy, VerifyResult, run_verify};
use crate::swarm::{NodeState, SwarmOrchestrator, SwarmPlan, WaveTask, WorktreeSpec, ops};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

pub const MAX_PLAN_BYTES: u64 = 1024 * 1024;
pub const VERIFY_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SwarmOp {
    /// `:swarm load <plan.toml>` - parse + validate; reports every verify command (nothing runs).
    Load(String),
    /// `:swarm status` - node states, panes and tracked worktrees.
    Status,
    /// `:swarm run` - start every ready node (worktree + pane + agent + queued prompt).
    Run,
    /// `:swarm spawn <slug> [agent] [prompt...]` - one ad-hoc worktree agent (SWARM-01).
    Spawn { slug: String, agent: String, prompt: String },
    /// `:swarm verify <node>` - run the node's `verify_cmd` in the background; self-heal on failure.
    Verify(String),
    /// `:swarm diff <slug>` - diff stat of a tracked worktree branch vs the primary HEAD.
    Diff(String),
    /// `:swarm merge <slug> [--squash]`.
    Merge { slug: String, squash: bool },
    /// `:swarm clean <slug> [--force]`.
    Clean { slug: String, force: bool },
}

impl SwarmOp {
    pub fn colon_form(&self) -> String {
        match self {
            Self::Load(p) => format!(":swarm load {p}"),
            Self::Status => ":swarm status".into(),
            Self::Run => ":swarm run".into(),
            Self::Spawn { slug, agent, prompt } if prompt.is_empty() => format!(":swarm spawn {slug} {agent}"),
            Self::Spawn { slug, agent, prompt } => format!(":swarm spawn {slug} {agent} {prompt}"),
            Self::Verify(n) => format!(":swarm verify {n}"),
            Self::Diff(s) => format!(":swarm diff {s}"),
            Self::Merge { slug, squash } => format!(":swarm merge {slug}{}", if *squash { " --squash" } else { "" }),
            Self::Clean { slug, force } => format!(":swarm clean {slug}{}", if *force { " --force" } else { "" }),
        }
    }
}

/// Per-window-manager swarm state.
#[derive(Default)]
pub struct SwarmState {
    repo: Option<PathBuf>,
    orch: Option<SwarmOrchestrator>,
    panes: BTreeMap<String, PaneId>,
    healers: BTreeMap<String, HealLoop>,
    pending: BTreeMap<String, Receiver<VerifyResult>>,
}

impl std::fmt::Debug for SwarmState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwarmState").field("repo", &self.repo).field("panes", &self.panes).finish()
    }
}

fn s(v: impl Into<String>) -> JsonValue {
    JsonValue::String(v.into())
}

fn strs(v: impl IntoIterator<Item = String>) -> JsonValue {
    JsonValue::Array(v.into_iter().map(JsonValue::String).collect())
}

fn state_name(st: NodeState) -> &'static str {
    match st {
        NodeState::Pending => "pending",
        NodeState::Running => "running",
        NodeState::Verifying => "verifying",
        NodeState::Passed => "passed",
        NodeState::Failed => "failed",
    }
}

impl WindowManager {
    /// Executes one swarm op; the JSON result is shared by every surface.
    pub fn swarm_apply(&mut self, op: SwarmOp) -> Result<JsonValue, String> {
        self.swarm_poll();
        match op {
            SwarmOp::Load(path) => self.swarm_load(&path),
            SwarmOp::Status => self.swarm_status(),
            SwarmOp::Run => self.swarm_run(),
            SwarmOp::Spawn { slug, agent, prompt } => {
                let task = WaveTask {
                    id: slug.clone(),
                    agent,
                    worktree: Some(slug.clone()),
                    prompt,
                    verify_cmd: None,
                    depends_on: vec![],
                    max_retries: None,
                };
                let repo = self.swarm_repo()?;
                let pane = self.swarm_spawn_node(&repo, &task)?;
                Ok(JsonValue::obj([("slug", s(slug)), ("pane_id", JsonValue::Number(pane as i64))]))
            }
            SwarmOp::Verify(id) => self.swarm_verify(&id),
            SwarmOp::Diff(slug) => {
                let repo = self.swarm_repo()?;
                let d = ops::diff_stat(&repo, &slug).map_err(|e| e.to_string())?;
                Ok(JsonValue::obj([
                    ("slug", s(slug)),
                    ("files_changed", JsonValue::Number(d.files_changed as i64)),
                    ("insertions", JsonValue::Number(d.insertions as i64)),
                    ("deletions", JsonValue::Number(d.deletions as i64)),
                    (
                        "files",
                        JsonValue::Array(
                            d.files
                                .iter()
                                .map(|f| {
                                    JsonValue::obj([
                                        ("path", s(f.path.clone())),
                                        ("changes", JsonValue::Number(f.changes as i64)),
                                        ("binary", JsonValue::Bool(f.binary)),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ]))
            }
            SwarmOp::Merge { slug, squash } => {
                let repo = self.swarm_repo()?;
                ops::merge(&repo, &slug, squash).map_err(|e| e.to_string())?;
                Ok(JsonValue::obj([("slug", s(slug)), ("merged", JsonValue::Bool(true)), ("squash", JsonValue::Bool(squash))]))
            }
            SwarmOp::Clean { slug, force } => {
                let repo = self.swarm_repo()?;
                ops::clean(&repo, &slug, force).map_err(|e| e.to_string())?;
                Ok(JsonValue::obj([("slug", s(slug)), ("cleaned", JsonValue::Bool(true))]))
            }
        }
    }

    fn swarm_repo(&mut self) -> Result<PathBuf, String> {
        if let Some(r) = &self.swarm.repo {
            return Ok(r.clone());
        }
        let cwd = self.active_pane().resolve_cwd();
        let root = ops::repo_root(&cwd).map_err(|e| e.to_string())?;
        self.swarm.repo = Some(root.clone());
        Ok(root)
    }

    fn swarm_load(&mut self, path: &str) -> Result<JsonValue, String> {
        let p = Path::new(path);
        let p = if p.is_relative() { self.active_pane().resolve_cwd().join(p) } else { p.to_path_buf() };
        let meta = std::fs::metadata(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        if meta.len() > MAX_PLAN_BYTES {
            return Err(format!("plan exceeds {MAX_PLAN_BYTES} bytes"));
        }
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let plan = SwarmPlan::parse(&text).map_err(|e| e.to_string())?;
        let out = JsonValue::obj([
            ("nodes", JsonValue::Number(plan.nodes.len() as i64)),
            ("max_parallel", JsonValue::Number(plan.max_parallel as i64)),
            ("waves", JsonValue::Array(plan.waves().into_iter().map(strs).collect())),
            (
                "verify_commands",
                strs(plan.nodes.iter().filter_map(|n| n.verify_cmd.as_ref().map(|c| format!("{}: {c}", n.id)))),
            ),
        ]);
        self.swarm.orch = Some(SwarmOrchestrator::new(plan));
        self.swarm.panes.clear();
        self.swarm.healers.clear();
        self.swarm.pending.clear();
        Ok(out)
    }

    fn swarm_status(&mut self) -> Result<JsonValue, String> {
        let nodes: Vec<JsonValue> = match &self.swarm.orch {
            Some(o) => o
                .plan()
                .nodes
                .iter()
                .map(|n| {
                    let st = o.scheduler().state(&n.id).unwrap_or(NodeState::Pending);
                    JsonValue::obj([
                        ("id", s(n.id.clone())),
                        ("state", s(state_name(st))),
                        ("pane_id", self.swarm.panes.get(&n.id).map_or(JsonValue::Null, |p| JsonValue::Number(*p as i64))),
                        ("retries", JsonValue::Number(self.swarm.healers.get(&n.id).map_or(0, |h| h.retries_used()) as i64)),
                    ])
                })
                .collect(),
            None => vec![],
        };
        let worktrees = match self.swarm_repo() {
            Ok(r) => ops::tracked(&r).map(|v| v.into_iter().map(|w| w.branch).collect()).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        Ok(JsonValue::obj([("nodes", JsonValue::Array(nodes)), ("worktrees", strs(worktrees))]))
    }

    fn swarm_run(&mut self) -> Result<JsonValue, String> {
        let repo = self.swarm_repo()?;
        let tasks = self.swarm.orch.as_mut().ok_or("no plan loaded (use :swarm load <plan.toml>)")?.start_ready().map_err(|e| e.to_string())?;
        let mut started = Vec::new();
        let mut failed = Vec::new();
        for t in tasks {
            match self.swarm_spawn_node(&repo, &t) {
                Ok(pane) => {
                    self.swarm.panes.insert(t.id.clone(), pane);
                    started.push(t.id);
                }
                Err(e) => {
                    if let Some(o) = self.swarm.orch.as_mut() {
                        let _ = o.fail(&t.id);
                    }
                    failed.push(format!("{}: {e}", t.id));
                }
            }
        }
        Ok(JsonValue::obj([("started", strs(started)), ("failed", strs(failed))]))
    }

    /// SWARM-01: worktree -> pane rooted in it -> agent -> queued prompt; reverse rollback on failure.
    fn swarm_spawn_node(&mut self, repo: &Path, t: &WaveTask) -> Result<PaneId, String> {
        let spec = WorktreeSpec::new(repo, t.worktree_slug()).map_err(|e| e.to_string())?;
        crate::secure_fs::validate_safe_name(&t.agent).map_err(|e| format!("agent: {e}"))?;
        spec.create().map_err(|e| e.to_string())?;
        let target = self.active_pane_id();
        let pane = match self.split_focused_in(crate::wm::SplitDirection::Vertical, Some(spec.path.clone())) {
            Ok(p) => p,
            Err(e) => {
                let _ = spec.cleanup(true);
                return Err(e.to_string());
            }
        };
        let launched = self.pane_mut(pane).ok_or("pane vanished").map(|p| p.inject_prompt(&t.agent));
        let queued = if t.prompt.is_empty() { Ok(0) } else { self.prompt_queue_mut().enqueue(pane, t.prompt.clone()).map_err(|e| format!("{e:?}")) };
        if !matches!(launched, Ok(Ok(_))) || queued.is_err() {
            self.remove_pane(pane);
            let _ = spec.cleanup(true);
            self.focus_and_raise(target);
            return Err(match (launched, queued) {
                (Ok(Err(e)), _) => e.to_string(),
                (Err(e), _) => e.to_string(),
                (_, Err(e)) => e,
                _ => "launch failed".into(),
            });
        }
        self.swarm.panes.insert(t.id.clone(), pane);
        Ok(pane)
    }

    fn swarm_verify(&mut self, id: &str) -> Result<JsonValue, String> {
        let repo = self.swarm_repo()?;
        let orch = self.swarm.orch.as_mut().ok_or("no plan loaded")?;
        let task = orch.plan().node(id).cloned().ok_or_else(|| format!("unknown node '{id}'"))?;
        let retries = orch.plan().retries_for(id);
        let Some(cmd) = task.verify_cmd.clone() else {
            orch.pass(id).map_err(|e| e.to_string())?;
            return Ok(JsonValue::obj([("node", s(id)), ("state", s("passed"))]));
        };
        orch.begin_verify(id).map_err(|e| e.to_string())?;
        let spec = WorktreeSpec::new(&repo, task.worktree_slug()).map_err(|e| e.to_string())?;
        let (tx, rx) = channel();
        let c = cmd.clone();
        std::thread::spawn(move || {
            let r = run_verify(&c, &spec.path, VERIFY_TIMEOUT)
                .unwrap_or_else(|e| VerifyResult { exit_code: -1, output: e, timed_out: false });
            let _ = tx.send(r);
        });
        self.swarm.pending.insert(id.to_string(), rx);
        self.swarm.healers.entry(id.to_string()).or_insert_with(|| HealLoop::new(HealPolicy::new(retries)));
        Ok(JsonValue::obj([("node", s(id)), ("state", s("verifying")), ("cmd", s(cmd))]))
    }

    /// Applies finished background verifications (pass, self-heal retry, or fail). Cheap; no polling thread.
    pub fn swarm_poll(&mut self) {
        let ids: Vec<String> = self.swarm.pending.keys().cloned().collect();
        for id in ids {
            let res = match self.swarm.pending[&id].try_recv() {
                Ok(r) => r,
                Err(TryRecvError::Empty) => continue,
                Err(TryRecvError::Disconnected) => VerifyResult { exit_code: -1, output: "verify thread died".into(), timed_out: false },
            };
            self.swarm.pending.remove(&id);
            let Some(orch) = self.swarm.orch.as_mut() else { continue };
            if res.exit_code == 0 && !res.timed_out {
                let _ = orch.pass(&id);
                continue;
            }
            let cmd = orch.plan().node(&id).and_then(|n| n.verify_cmd.clone()).unwrap_or_default();
            let decision = self.swarm.healers.get_mut(&id).map(|h| h.on_failure(&cmd, res.exit_code, &res.output));
            match decision {
                Some(HealDecision::Retry { prompt, .. }) => {
                    let _ = orch.retry(&id);
                    if let Some(p) = self.swarm.panes.get(&id).copied()
                        && let Some(pane) = self.pane_mut(p)
                    {
                        let _ = pane.inject_prompt(&prompt);
                    }
                }
                _ => {
                    let _ = orch.fail(&id);
                }
            }
        }
    }
}
