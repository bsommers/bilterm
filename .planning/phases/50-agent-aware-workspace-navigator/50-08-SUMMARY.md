---
phase: 50-agent-aware-workspace-navigator
plan: 08
subsystem: input
tags: [agent-traffic, keymap, colon-command, keyboard-cursor, leader]
status: complete

requires:
  - phase: 50-07
    provides: "AGENT TRAFFIC window state/API (open/close, selection model)"
  - phase: 50-06
    provides: "scoped key-table pattern, tick_agent_ui throttle"
provides:
  - "AgentsOp::{Zoom, Close, Dismiss, Select, CycleEdge, Activate} with :agents twins"
  - "AgentsKeyScope, AGENTS_WINDOW_BINDINGS, agents_window_action in src/input/keymap.rs"
  - "src/input/agents_window.rs::handle_agents_window_key (scope resolution + table lookup + wm.apply only)"
  - "leader `a` -> Action::Agents(AgentsOp::Zoom(None))"
  - "tick_agent_ui requests a throttled redraw on new bus traffic while the window is open"
affects: [50-09, 50-10, 50-15, 50-16]

actuals:
  tokens: 14000
  tasks: 2
  commits: 2
plan_head_before: fbc4176f0fbc10b3170cb9cda3512de71da9f8b4
plan_head_after: d01328d24c9897e9ba61912fe295fc75165123ed

tech-stack:
  added: []
  patterns:
    - "Declarative per-scope key table + thin input module that only calls wm.apply (gate-checked)"
    - "Exhaustive agents_op_colon_form test helper (no wildcard) so later AgentsOp variants must be listed"

key-files:
  created:
    - src/input/agents_window.rs
  modified:
    - src/wm/agents_cmd.rs
    - src/colon_command/parse_agents.rs
    - src/colon_command/execute_agents.rs
    - src/input/keymap.rs
    - src/input/mod.rs
    - src/input/modal.rs
    - src/wm/agent_state.rs
    - src/pane/completion.rs
    - src/ui/help_catalog.rs
    - tests/phase50_agents_window.rs

key-decisions:
  - "Dismiss/Activate share one helper: unpinned closes the window, pinned only releases the keyboard"
  - "Unmatched keys are swallowed while the window captures input; the leader key is never consumed"
  - "Traffic redraw shares the 120 ms last_anim throttle; seq tracked in AgentTracker.last_traffic_seq"

duration: 25min
completed: 2026-11-03
---

# Phase 50 Plan 08: Agents window keys and commands Summary

Keyboard half of the AGENT TRAFFIC zoom window: `:agents zoom|close|dismiss|select|edge|enter`, leader `a`, Esc, arrows, Tab/Shift+Tab and Enter-to-focus, all as rows of `AGENTS_WINDOW_BINDINGS` lowering to `Action::Agents` via `WindowManager::apply`.

## Tasks

1. **Tracer** (02d8fa4): `:agents zoom` opens the window, Esc/`dismiss`/`close` close it; table, lookup fn, input module and modal delegation added.
2. **Keyboard cursor** (d01328d): Select/CycleEdge/Activate ops and Graph-scope rows, leader `a`, `:agents zoom <id>` selection, completion/help rows, traffic redraw cadence, drift/uniqueness/no-shadow/parity tests.

## Verification

- `scripts/verify.sh`: PASS 56/56; targeted run with the plan's stages: PASS 56/56.
- `scripts/check_classic_goldens.sh`: GOLDENS OK 11.
- `wm.` allow-list gate on src/input/agents_window.rs prints nothing; `Cargo.toml`/`Cargo.lock` unchanged.
- modal.rs 937 lines, keymap.rs 831 lines (both under 1000).
- Tests: 19 in tests/phase50_agents_window.rs plus lib test `window_tick_redraws_on_new_traffic` (needs pub(crate) `tick_agent_ui`, so it lives in src/wm/agent_state.rs).

## Deviations from Plan

None of substance. Notes:
- [Minor] `window_tick_redraws_on_new_traffic` is a unit test in `src/wm/agent_state.rs` because `tick_agent_ui` is `pub(crate)`.
- [Minor] The `:agents list` help-row check accepts the existing `:agents [list]` row.

## Known Stubs

None.

## Threat Flags

None. T-50-34 mitigated (leader not consumed, Esc always closes/releases, no global chord rebound; tests present).

## Self-Check: PASSED
