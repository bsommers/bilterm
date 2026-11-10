---
phase: 68-semantic-agent-introspection-and-context-bus
plan: 01
type: execute
wave: 1
depends_on:
  - 67-first-class-agent-pane-workspace-cli
files_modified:
  - src/agent/turn.rs
  - src/agent/gate.rs
  - src/agent/queue.rs
  - src/context/mod.rs
  - src/context/store.rs
  - src/wm/drawer_peek.rs
  - src/wm/agents_window.rs
  - src/mcp/resources.rs
  - src/cli/context.rs
  - src/action.rs
  - tests/phase68_beyond_herdr.rs
autonomous: true
requirements:
  - BEYOND-01
  - BEYOND-02
  - BEYOND-03
  - BEYOND-04
recommended_model: sonnet
must_haves:
  truths:
    - "agent.last_turn returns prompt, stripped markdown response, fenced code_blocks, files_touched, duration_ms, and exit status for the most recent completed turn."
    - "A Blocked agent exposes a classified reason (permission, confirmation, question, error) and [Approve]/[Deny] actions work from Drawer Peek, :agents list, GUI toast, CLI, and MCP."
    - "Per-pane FIFO prompt queue dispatches the next item exactly once when the pane transitions Working->Idle; queue is bounded and inspectable."
    - "Context bus keys are namespaced, size-bounded, persisted with 0600 files, and observable via watch and bilterm://context/<key>."
  artifacts:
    - path: "src/agent/turn.rs"
      provides: "Turn segmentation and structured extraction"
    - path: "src/agent/gate.rs"
      provides: "Blocked-reason classifier and approve/deny dispatch"
    - path: "src/agent/queue.rs"
      provides: "Per-pane FIFO prompt queue"
    - path: "src/context/store.rs"
      provides: "Shared context bus store with watchers"
    - path: "tests/phase68_beyond_herdr.rs"
      provides: "Tests for BEYOND-01..04"
---

# Phase 68 Plan: Semantic Agent Introspection & Cross-Agent Context Bus (BEYOND-01..04)

## Objective

Go beyond Herdr with structured, machine-usable agent semantics:
1. Structured turn delta & artifact extraction (`BEYOND-01`).
2. Permission & confirmation gate interceptor (`BEYOND-02`).
3. Per-pane FIFO prompt queue (`BEYOND-03`).
4. Cross-agent shared context bus & scratchpad (`BEYOND-04`).

---

## Detailed Requirements

### BEYOND-01: Structured Turn Delta
- Turn boundaries from prompt-submit event to next `Working->Idle|Done` (or OSC 133;D).
- `agent.last_turn {selector}` → `{prompt, response_markdown, code_blocks:[{lang,text}], files_touched:[path], duration_ms, exit_status}`.
- Response ANSI-stripped; fenced blocks parsed with unterminated-fence tolerance; `files_touched` from agent tool-call patterns with a conservative allowlist and workspace-relative normalization.
- Bounded retention (last N turns per pane, byte cap).

### BEYOND-02: Permission Gate Interceptor
- Classifier over visible screen region + known agent prompt patterns → `BlockedReason {Permission, Confirmation, Question, Error, Unknown}` with the proposed command/text.
- Actions: Approve (send agent-specific affirmative key/line), Deny, from Drawer Peek buttons, `:agents list` row actions, GUI toast, `bilterm agent approve|deny`, MCP `bilterm_agent_approve`.
- Approve requires the classifier to be confident; `Unknown` requires explicit `--force`. Optional `[agents.auto_approve]` allowlist is OFF by default.

### BEYOND-03: Per-Pane FIFO Prompt Queue
- `:agent queue [list|add|clear|drop <n>]`; `bilterm agent prompt --queue`.
- Dispatch on `Working->Idle` via the Phase 67 injection engine; dispatch is edge-triggered and idempotent; max depth configurable (default 32).

### BEYOND-04: Shared Context Bus
- `bilterm context set|get|watch|list`, `:context`, RPC `context.*`, MCP resources `bilterm://context/<key>` with change notifications.
- Keys validated by `validate_safe_name`-style rules; values capped (default 256 KiB); persisted via `src/secure_fs.rs`; optional TTL.

---

## Threat Model & Security Considerations

| Threat ID | Threat Category | Description | Mitigation Strategy |
|-----------|-----------------|-------------|---------------------|
| **T-GATE-01** | Unsafe Auto-Approval | Misclassified screen text approves a destructive command. | Confidence threshold, `Unknown` requires `--force`, auto-approve off by default, audit log of approvals. |
| **T-CTX-01** | Prompt Injection Between Agents | One agent poisons context read by another. | Context values treated as untrusted data; MCP resource marked untrusted; provenance (`from_pane`) recorded. |
| **T-CTX-02** | Path Traversal / Disk Fill | Malicious key or huge values. | Key validation, size and count caps, `0600` atomic writes. |
| **T-Q-01** | Duplicate Dispatch | Flapping state sends queued prompt twice. | Edge-triggered with dispatch token; idempotency test. |
| **T-TURN-01** | Memory Growth | Unbounded turn history. | Ring retention with byte cap (`bounding-untrusted-resource-use`). |

---

## Technical Design & Wave Breakdown

### Wave 1: Turn extraction & queue (`BEYOND-01`, `BEYOND-03`)
1. `src/agent/turn.rs`, `src/agent/queue.rs`; hook turn events from Phase 67 state machine.

### Wave 2: Gate & context bus (`BEYOND-02`, `BEYOND-04`)
1. `src/agent/gate.rs` + Drawer Peek/`:agents`/toast UI; `src/context/*`; MCP resources.

### Wave 3: Surface parity & verification
1. `Action` variants, colon verbs, CLI, RPC, MCP for all four features.
2. `tests/phase68_beyond_herdr.rs`; `scripts/verify.sh`.

---

## Verification Gates

- [ ] **Gate 1 (Turns)**: Fixture transcripts per agent produce expected structured output.
- [ ] **Gate 2 (Gate)**: Classifier fixtures incl. adversarial lookalike text; approve/deny byte assertions.
- [ ] **Gate 3 (Queue)**: Exactly-once dispatch under flapping; bounds enforced.
- [ ] **Gate 4 (Context)**: Watch notifications, caps, persistence, MCP resource listing.
- [ ] **Gate 5 (Quality)**: `scripts/verify.sh` PASS; `GOLDENS OK 11`; `ARCH-R04`.
