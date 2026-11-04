---
phase: 50-herdr-style-left-sidebar-agent-aware-workspace-navigator
plan: 03
subsystem: wm
tags: [agent-state, colon-command, rpc, config, hysteresis]
status: complete

requires:
  - phase: 50-01
    provides: classic drawer goldens and immutability gate
provides:
  - "AgentState {Working, Blocked, Idle, Done, Errored}, pure classify(), AgentTracker on WindowManager"
  - "WindowManager::agent_state / agent_statuses / agent_counts / update_agent_states / report_agent_state / tick_agent_ui"
  - ":agents [list] command (Action::Agents(AgentsOp::List)) with structured data over RPC wm.command"
  - "[agents.<name>] profiles (AgentProfile, BiltermConfig.agents) with capped substring patterns"
affects: [50-04, 50-05, 50-06, 50-07, 50-08, 50-09, 50-10, 50-11, 50-12, 50-14, phase-51]

actuals:
  tokens: 11400
  tasks: 3
  commits: 4
plan_head_before: 37b59e6b79fd6ce88d76d55d862a047b9f9b070a
plan_head_after: e34bfa4c54f6fbbd70ae2432e7bd9cbaaa193c58

tech-stack:
  added: []
  patterns:
    - "Pure classifier (AgentSignals -> AgentState) plus a stateful tracker for hysteresis, seen semantics and an override seam"
    - "Synthetic Instant parameters in tests; no sleeps"
    - "Structured command data via set_command_data, same as execute_config"

key-files:
  created:
    - src/wm/agent_state.rs
    - src/wm/agents_cmd.rs
    - src/config/agents.rs
    - src/colon_command/parse_agents.rs
    - src/colon_command/execute_agents.rs
    - tests/phase50_agent_state.rs
  modified:
    - src/wm/mod.rs
    - src/action.rs
    - src/config/mod.rs
    - src/colon_command/mod.rs
    - src/colon_command/parse.rs
    - src/colon_command/execute_wm.rs
    - src/pane/completion.rs
    - src/ui/help_catalog.rs

key-decisions:
  - "First observation of a pane applies its state without dwell; only later Working/Idle flips need the 500 ms dwell"
  - "A finished command is detected from the OSC 133 CommandFinished transition or a change of (last_exit_code, last_duration_ms), because real shells move D to A/B before the next poll"
  - "Unseen Done/Errored clear when the pane is focused or when a new command starts; exited-process Done/Errored stay while the pane exists; an exit with unknown code counts as Done"
  - "Tail text = up to 8 rows ending at the last non-empty visible row (trailing blank rows trimmed), scanned only when profiles exist"
  - "Identity token uses the base name of the first command token (so /usr/bin/claude becomes claude) before the [A-Za-z0-9._-] filter"
  - "No built-in profiles (A3), no regex dependency, :agent (singular) not aliased"

requirements-completed: [FLT-01]

coverage:
  - id: D1
    description: "Every live pane has an agent state derived from existing signals with no per-agent configuration"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_agent_state.rs state_ tests (13 core: 2 tracer, 10 behaviour, classify table)"
        status: pass
    human_judgment: false
  - id: D2
    description: ":agents list returns per-pane state JSON plus counts locally and over RPC; :state keys unchanged"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_agent_state.rs#state_tracer_list_over_rpc, tests/config_commands_edge.rs, tests/api_conformance.rs"
        status: pass
    human_judgment: false
  - id: D3
    description: "Hysteresis, seen semantics and explicit-report override"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "state_working_idle_flip_respects_dwell, state_nonzero_exit_unseen_is_errored, state_reported_overrides_heuristics"
        status: pass
    human_judgment: false
  - id: D4
    description: "Optional [agents.<name>] profiles with capped substring patterns; empty table not serialised"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "state_profile_* and state_empty_agents_table_not_serialized and state_list_reports_profile_name"
        status: pass
    human_judgment: false
---

# Phase 50 Plan 03: Per-pane Agent State and `:agents list` Summary

**Every pane now carries a working / blocked / idle / done / errored state derived purely from existing signals (OSC 133, running command, output bytes, password prompt, notifications, exit), exposed through `:agents list` with structured data over RPC, plus optional `[agents.<name>]` substring profiles and a `report_agent_state` override seam for Phase 51.**

## Performance

- Tasks: 3 (1 tracer, 2 tdd auto); files: 6 created, 8 modified
- Commits: 4 (`4ebc3e9` tracer, `01bf4ba` RED, `cdf6f7a` GREEN, `e34bfa4` profiles)

## Accomplishments

- Tracer: `AgentState`, `classify`, `AgentTracker`, `Action::Agents`, `:agents [list|ls|status]` parser/executor, completion/help/palette rows, `tick_agent_ui` hooked after `update_drawer_telemetry` in `poll_all_ptys` (returns `false`, classic redraw cadence unchanged).
- Rule table: Blocked (password / profile pattern) > Errored > Done > Blocked (unread notification while unfocused) > Working (running and recent output or profile working pattern) > Idle, with 1500 ms hold and 500 ms dwell for Working/Idle flips; attention transitions apply immediately.
- Seen semantics: unseen finished commands persist as Done/Errored until the pane is focused or a new command starts.
- Profiles: sanitised (trim, lower-case, 16 patterns x 80 chars), resolved once per tail scan, name reported in `"profile"`.
- `:agents list` data: `{agents:[{pane_id,name,state,since_ms,profile,reported}], counts:{...}}`; never contains tail text or command arguments (T-50-06).

## Deviations from Plan

None - plan executed exactly as written, with these notes:

- Task 3 was committed as a single `feat` commit (tests written first and seen failing for the 4 tracker-dependent tests; the config struct and `sanitized()` were added alongside so the test file compiles), rather than separate RED and GREEN commits.
- The dwell test drives "not running" through OSC 133;D rather than clearing `running_command`, because headless submissions leave the OSC phase at `CommandExecuted`.
- A fixture bug in `state_profile_patterns_are_capped` (pattern `pat3` is a substring of `pat39`) was fixed in the test, not the code.

## Known Stubs

None. `tick_agent_ui` intentionally returns `false` until 50-04 wires visible herdr changes.

## Threat Flags

None. T-50-06 (data carries only id, token, state, timing, profile name), T-50-07 (8-row tail, scan on byte change or every 250 ms, substring only, 16 x 80 caps, skipped entirely when no profiles) mitigated; T-50-SC: `git diff Cargo.toml Cargo.lock` clean.

## Verification

- `scripts/verify.sh` (full): `PASS 54/54`
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `cargo test --test phase50_agent_state`: 19 passed, 0 `thread::sleep`
- Acceptance greps: `Agents(crate::wm::agents_cmd::AgentsOp)` in action.rs 1, `tick_agent_ui` in wm/mod.rs 1, `skip_serializing_if = "BTreeMap::is_empty"` in config/mod.rs 1; `src/wm/mod.rs` 873 lines

## Next Phase Readiness

Ready for 50-04 (state dots, attention jump, `tick_agent_ui` returning true on visible changes) and later plans extending `AgentsOp`.

## Self-Check: PASSED

Created files exist (`src/wm/agent_state.rs`, `src/wm/agents_cmd.rs`, `src/config/agents.rs`, `src/colon_command/parse_agents.rs`, `src/colon_command/execute_agents.rs`, `tests/phase50_agent_state.rs`); commits 4ebc3e9, 01bf4ba, cdf6f7a, e34bfa4 are ancestors of HEAD.
