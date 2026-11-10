---
phase: 69-ephemeral-worktree-swarm-and-dag-runner
plan: 01
type: execute
wave: 1
depends_on:
  - 66-foreground-rpc-and-instance-discovery
  - 67-first-class-agent-pane-workspace-cli
  - 68-semantic-agent-introspection-and-context-bus
files_modified:
  - src/swarm/mod.rs
  - src/swarm/worktree.rs
  - src/swarm/plan.rs
  - src/swarm/dag.rs
  - src/swarm/verify.rs
  - src/swarm/review.rs
  - src/cli/swarm.rs
  - src/action.rs
  - src/colon_command/swarm.rs
  - tests/phase69_swarm.rs
autonomous: true
requirements:
  - SWARM-01
  - SWARM-02
  - SWARM-03
  - SWARM-04
recommended_model: sonnet
must_haves:
  truths:
    - "agent spawn atomically creates a git worktree, a pane rooted in it, and starts the agent; any failure rolls back all created resources."
    - "swarm run executes a validated acyclic DAG in waves; a node starts only after all depends_on nodes pass verify_cmd."
    - "swarm review/diff/merge operate only on worktrees created by Bilterm (tracked in a manifest) and never touch the user's primary checkout without explicit merge."
    - "Self-healing loop retries at most max_retries times, feeding bounded failing verify_cmd output back to the agent pane, then marks the node Failed."
  artifacts:
    - path: "src/swarm/worktree.rs"
      provides: "Atomic worktree create/remove with manifest and rollback"
    - path: "src/swarm/dag.rs"
      provides: "Plan DAG validation, wave scheduling"
    - path: "src/swarm/verify.rs"
      provides: "verify_cmd runner and retry loop"
    - path: "tests/phase69_swarm.rs"
      provides: "Tests against temporary git repositories"
---

# Phase 69 Plan: Ephemeral Worktree Swarm & DAG Runner (SWARM-01..04)

## Objective

Orchestrate swarms of agents in isolated worktrees:
1. Atomic git-worktree agent spawner (`SWARM-01`).
2. Declarative DAG wave orchestrator (`SWARM-02`).
3. Worktree diff inspector & one-key merge (`SWARM-03`).
4. Autonomous self-healing verification retry loop (`SWARM-04`).

---

## Detailed Requirements

### SWARM-01: Atomic Spawner
- `:agent spawn` / `bilterm agent spawn --worktree <slug> --agent claude '<prompt>'`.
- Steps: validate slug (`validate_safe_name`) → `git worktree add` under `<repo>/.bilterm/worktrees/<slug>` on branch `swarm/<slug>` → create pane with cwd → launch agent → inject prompt (Phase 67). Reverse-order rollback on any failure; manifest entry written atomically (`secure_fs`).

### SWARM-02: DAG Wave Orchestrator
- `plan.toml`:
  ```toml
  [[node]]
  id = "api"
  agent = "claude"
  worktree = "api"
  prompt = "Implement …"
  verify_cmd = "cargo test -p api"
  depends_on = []
  ```
- Validate: unique ids, known deps, acyclic (Kahn), bounded node count and concurrency (`max_parallel`).
- Waves computed topologically; downstream nodes may receive upstream summaries via the Phase 68 context bus.

### SWARM-03: Review & Merge
- `:swarm review` opens a diff inspector window; CLI `swarm status|diff|merge|clean`.
- Merge: refuses on dirty primary tree or conflicts (reports files); `--squash` option; `clean` removes worktree+branch only for manifest-tracked, merged-or-forced entries.

### SWARM-04: Self-Healing Retry
- After agent goes idle, run `verify_cmd` (argv-split, no shell interpolation of agent output) in the worktree with timeout; on failure, send "verification failed" + tail (bounded, ANSI-stripped) to the pane and re-wait; stop at `max_retries` (default 2) → `Failed`.

---

## Threat Model & Security Considerations

| Threat ID | Threat Category | Description | Mitigation Strategy |
|-----------|-----------------|-------------|---------------------|
| **T-WT-01** | Path Traversal | Slug like `../../x` creates worktrees outside the repo. | `validate_safe_name`, canonical path prefix check. |
| **T-WT-02** | Data Loss | `clean` deletes user work or the primary checkout. | Manifest-only deletion, refuse unmerged without `--force`, never operate on primary worktree. |
| **T-SW-01** | Command Injection | Hostile `plan.toml` `verify_cmd` executes arbitrary commands. | Plans are user-authored trusted input but require explicit run confirmation showing commands; no agent-output interpolation into commands. |
| **T-SW-02** | Resource Exhaustion | Huge DAG or fan-out spawns too many agents. | Node and `max_parallel` caps. |
| **T-SW-03** | Infinite Retry | Loop burns tokens indefinitely. | Hard `max_retries` ceiling and total wall-time budget. |

---

## Technical Design & Wave Breakdown

### Wave 1: Worktree spawner (`SWARM-01`)
1. `src/swarm/worktree.rs` with manifest, rollback; `Action::AgentSpawn`; CLI/colon.

### Wave 2: DAG + verify loop (`SWARM-02`, `SWARM-04`)
1. `plan.rs`, `dag.rs`, `verify.rs`; scheduler driven by Phase 67 wait events (no polling).

### Wave 3: Review/merge & verification (`SWARM-03`)
1. `review.rs`, diff window, CLI; `tests/phase69_swarm.rs` on temp git repos; `scripts/verify.sh`.

---

## Verification Gates

- [ ] **Gate 1 (Atomicity)**: Injected failure at each spawn step leaves no worktree/branch/pane.
- [ ] **Gate 2 (DAG)**: Cycle, missing dep, wave ordering, parallel cap tests.
- [ ] **Gate 3 (Merge Safety)**: Dirty tree, conflict, clean-of-unmerged refusal.
- [ ] **Gate 4 (Retry)**: Pass-after-N, exhaust-at-max, timeout, bounded feedback size.
- [ ] **Gate 5 (Quality)**: `scripts/verify.sh` PASS; `GOLDENS OK 11`; `ARCH-R04`.
