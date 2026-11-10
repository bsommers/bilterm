---
phase: 67-first-class-agent-pane-workspace-cli
plan: 01
type: execute
wave: 1
depends_on:
  - 66-foreground-rpc-and-instance-discovery
files_modified:
  - src/selector.rs
  - src/cli/agent.rs
  - src/cli/pane.rs
  - src/cli/workspace.rs
  - src/api/agent_wait.rs
  - src/agent/inject.rs
  - src/action.rs
  - src/colon_command/execute_wm.rs
  - tests/phase67_agent_cli.rs
autonomous: true
requirements:
  - HERDR-01
  - HERDR-02
  - HERDR-03
  - HERDR-04
recommended_model: sonnet
must_haves:
  truths:
    - "Selectors w<N>:p<M>, @claude, @agy, @idle, @working, @blocked resolve to exactly one pane or return an explicit ambiguity/not-found error listing candidates."
    - "Every bilterm agent|pane|workspace subcommand lowers to crate::action::Action and executes via WindowManager::apply."
    - "Prompt injection into a shell pane uses FloatingPrompt submission; into a raw TUI uses DECSET 2004 bracketed paste followed by the agent's submit chord."
    - "agent.wait blocks server-side with no client polling and wakes on the exact transition or OSC 133;D, honoring --timeout."
  artifacts:
    - path: "src/selector.rs"
      provides: "Selector grammar, parser, resolver"
    - path: "src/api/agent_wait.rs"
      provides: "Waiter registry woken by state-transition events"
    - path: "src/agent/inject.rs"
      provides: "Smart prompt injection engine"
    - path: "tests/phase67_agent_cli.rs"
      provides: "Selector, CLI lowering, injection, and wait tests"
---

# Phase 67 Plan: First-Class Agent/Pane/Workspace CLI & Zero-Polling Wait (HERDR-01..04)

## Objective

Match and exceed Herdr's agent CLI on top of the Phase 66 live socket:
1. Hierarchical and semantic selectors (`HERDR-01`).
2. First-class `bilterm agent|pane|workspace` subcommands (`HERDR-02`).
3. Smart prompt injection for shells and raw TUIs (`HERDR-03`).
4. Zero-polling server-side blocking wait (`HERDR-04`).

---

## Detailed Requirements

### HERDR-01: Selectors
- Grammar: `w<N>` , `w<N>:p<M>`, `@claude`, `@agy`, `@idle`, `@working`, `@blocked`, combinable (`@claude@idle`).
- Resolution is deterministic (workspace order, then pane order); multiple matches => `AmbiguousSelector` listing `w:p` ids; `--first`/`--all` opt-in.
- Reuses Phase 50 agent classification for `@claude`/`@agy`.

### HERDR-02: CLI Subcommands
- `bilterm agent list|prompt|wait|approve|interrupt|last-turn`
- `bilterm pane list|read|split|focus|zoom|send`
- `bilterm workspace list|select|new|rename`
- All support `--json`, `--instance`, selectors; each lowers to an `Action` variant and has colon/palette/RPC/MCP equivalents (`UNI-R01`).

### HERDR-03: Smart Prompt Injection
- Detect target mode via the three-signal router (OSC 133, `tcgetpgrp`, termios `!ECHO`, `DECSET 1049/2004`).
- Shell: submit through `FloatingPrompt` path. Raw TUI: wrap in `ESC[200~ … ESC[201~` when DECSET 2004 active, then send submit chord (`--submit enter|ctrl-enter|none`, per-agent default).
- Payload sanitization: strip embedded `ESC[201~` to prevent paste-escape injection.

### HERDR-04: Zero-Polling Wait
- Waiter registry keyed by pane; state-machine events (`Working->Idle|Blocked|Done`, OSC 133;D) wake waiters directly.
- `agent.wait {selector, until: idle|blocked|done|any, timeout_ms}` returns `{state, reason, elapsed_ms}`; already-satisfied conditions return immediately; pane removal wakes with `PaneGone`.

---

## Threat Model & Security Considerations

| Threat ID | Threat Category | Description | Mitigation Strategy |
|-----------|-----------------|-------------|---------------------|
| **T-SEL-01** | Misdirected Input | Ambiguous selector sends a prompt to the wrong agent. | Fail closed on ambiguity unless `--all`/`--first`. |
| **T-INJ-01** | Escape Injection | Prompt text contains `ESC[201~` ending the paste early. | Strip/neutralize control sequences in payload. |
| **T-WAIT-01** | Resource Exhaustion | Unbounded blocked waiters. | Cap waiters per client/instance; timeouts mandatory (default bounded). |
| **T-CLI-01** | Privilege Confusion | Child agent controls panes outside its workspace. | Optional `[rpc] scope = workspace` limits `from_pane` clients. |

---

## Technical Design & Wave Breakdown

### Wave 1: Selectors & workspace/pane CLI (`HERDR-01`, `HERDR-02`)
1. `src/selector.rs` parser/resolver + unit tests.
2. `src/cli/{pane,workspace}.rs`; new `Action` variants; colon verbs.

### Wave 2: Injection & agent CLI (`HERDR-03`, `HERDR-02`)
1. `src/agent/inject.rs` engine; `src/cli/agent.rs` (`list|prompt|approve|interrupt`).

### Wave 3: Wait & verification (`HERDR-04`)
1. `src/api/agent_wait.rs`; hook state-transition emit points; `agent wait` CLI.
2. `tests/phase67_agent_cli.rs`; `scripts/verify.sh`.

---

## Verification Gates

- [ ] **Gate 1 (Selectors)**: Table-driven tests incl. ambiguity, empty, malformed.
- [ ] **Gate 2 (Lowering)**: Every subcommand verified to produce its `Action` (no side-door state mutation).
- [ ] **Gate 3 (Injection)**: Shell vs bracketed-paste paths asserted on captured PTY bytes; payload sanitization verified.
- [ ] **Gate 4 (Wait)**: Wake-on-transition, immediate-satisfied, timeout, and pane-gone cases; no polling loop in implementation.
- [ ] **Gate 5 (Quality)**: `scripts/verify.sh` PASS; `GOLDENS OK 11`; `ARCH-R01/R04` respected.
