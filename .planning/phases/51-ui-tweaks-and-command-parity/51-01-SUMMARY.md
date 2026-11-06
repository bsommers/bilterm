---
phase: 51-ui-tweaks-and-command-parity
plan: 01
subsystem: wm / colon_command / ui
tags: [tracer, agents-list, demo-stop, command-contract, UIT-01, UIT-03]
requires: []
provides:
  - "AgentsListOp (final, 51-CONTRACT 2.1), ListFilter, AgentsListOp::colon_form"
  - "Action::AgentsList, PanelTarget::AgentsList, AgentsOp::DemoStop"
  - "AgentsListState/Row/Summary, WindowManager::agents_list_apply and accessors"
  - "full :agents list grammar (parse_agents_list::parse), :agents demo stop"
  - "render_agents_list (tracer frame + rows)"
affects: [51-02, 51-03, 51-04, 51-05, 51-06, 51-07, 51-08, 51-09, 51-10]
key-files:
  created:
    - src/wm/agents_list_cmd.rs
    - src/wm/agents_list.rs
    - src/colon_command/parse_agents_list.rs
    - src/colon_command/execute_agents_list.rs
    - src/ui/agents_list_ui.rs
    - tests/phase51_tracer.rs
  modified:
    - src/action.rs
    - src/wm/mod.rs
    - src/wm/agents_window.rs
    - src/wm/agents_cmd.rs
    - src/ui/typography.rs
    - src/ui/mod.rs
    - src/colon_command/mod.rs
    - src/colon_command/parse_agents.rs
    - src/colon_command/execute_agents.rs
    - src/colon_command/execute_agents_topology.rs
    - src/colon_command/execute_wm.rs
    - tests/phase50_agents_window.rs
decisions:
  - "pane ids in :agents list verbs are parsed as u32 first (usize on 64-bit would accept 99999999999)"
  - "open_agents_list refreshes the agent tracker when it never ran, because agents_list_rows is &self"
status: complete
base_sha: 7af6d4173834f86ebd19f2f81738fcc1597f76c3
plan_head_before: 7af6d4173834f86ebd19f2f81738fcc1597f76c3
plan_head_after: c586b13f0a6c7769a1c19c09dd1db9de2e84e299
commits: 2
actuals:
  tokens: 14000
  tasks: 2
  commits: 2
---

# Phase 51 Plan 01: Tracer and command contract Summary

`:agents list` now opens a real rendered AGENTS window through parse -> `Action::AgentsList` -> `WindowManager::apply` -> `AgentsListState` -> `WorkspaceWidget`, and the whole `:agents list` / `:agents demo stop` grammar is frozen and round-trip tested.

BASE sha: `7af6d41` (Phase 50.1 complete). Commits: `c8e65d1` (Task 1 tracer), `c586b13` (Task 2 grammar + DemoStop).

## Files and line counts (after)

| File | Lines |
|------|-------|
| src/wm/agents_list_cmd.rs (new) | 107 |
| src/wm/agents_list.rs (new) | 221 |
| src/colon_command/parse_agents_list.rs (new) | 109 |
| src/colon_command/execute_agents_list.rs (new) | 16 |
| src/ui/agents_list_ui.rs (new) | 79 |
| tests/phase51_tracer.rs (new, 10 tests) | 201 |
| src/action.rs | 850 |
| src/wm/mod.rs | 913 |
| src/wm/agents_window.rs | 1168 (was 1167; +1 line) |
| src/colon_command/parse_agents.rs | 252 |
| src/colon_command/execute_agents.rs | 303 |
| src/colon_command/execute_wm.rs | 870 |
| src/ui/mod.rs | 823 |
| src/ui/typography.rs | 709 |
| src/input/keymap.rs | 958 (untouched) |

`handle_wm_key_event` untouched. No Cargo.toml/Cargo.lock change.

## Gates

- `cargo test -q --test phase51_tracer`: 10 passed (1 tracer + RPC `wm.command` equivalence + blur + dismiss + 6 grammar/stub tests).
- `scripts/verify.sh --only clippy unit t:phase51_tracer t:phase50_agents_window t:phase50_agent_state t:phase50_traffic t:drawer_classic_parity t:api_conformance t:session_lessons_and_regression_suite`: `PASS 78/78 40s`.
- `scripts/verify.sh` (full): `PASS 78/78 71s`.
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
- `git diff --exit-code HEAD -- Cargo.toml Cargo.lock`: clean.

## Deviations from Plan

None in behavior. Anchors re-verified against the post-50.1 repo; all plan line numbers had drifted by at most a few lines, no structural conflicts. No keymap rows were needed in this plan.

Minor implementation choices (not deviations):
- `Action` exhaustive matches elsewhere needed no edits (only the three `action.rs` sites).
- The agents-list name column in the tracer render is `{state:<8} {name} #{id}`; 51-06 replaces it.

## What later waves must know (contract deltas / facts)

- `agents_list_rows(&self)` cannot refresh the tracker (it takes `&self`); `open_agents_list()` refreshes it once if the tracker never ran. Rows are otherwise as fresh as the last `update_agent_states` tick.
- `AgentsOp::List` executor now = data read `list(wm)` followed by `wm.open_agents_list()`. `:agents status` and bare `:agents` therefore also open the window (parse level unchanged).
- `agents_list_apply` has real arms only for `Open/Close/Toggle/Dismiss/Blur`; every other variant is a grouped `return None` arm with a comment naming 51-05. 51-05 must make that match semantic while keeping it exhaustive (no wildcard).
- `agents_list_rect` (minimal, contract default formula) lives in `src/wm/agents_list.rs`; 51-02 moves it to `agents_list_geometry.rs` and deletes it from here (acceptance: `grep -c "fn agents_list_rect" src/wm/agents_list.rs` is 0).
- `AGENTS_LIST_USAGE` is a single-line usage listing all verbs; the executor `execute_agents_list_command` returns `false` and shows only the `Option<String>` from `agents_list_apply` via the active pane status.
- `:agents list scale <75..200>[%]` lowers to `Action::PanelScaleOf(PanelTarget::AgentsList, n)`; `PanelTarget::AgentsList` is included in `set_panel_scale_pct` (`:panel scale n`). The existing `PanelScaleOf` executor path handles it through `scale_setting`/`set_scale`/`panel_name`; no further wiring needed.
- `:agents list scale +n|-n` parses to `ScaleStep(n)`; the executor arm that applies it is 51-05's (currently the grouped no-op).
- `:agents demo` with any extra word other than `stop` is `Err("Usage: :agents demo [stop]")`; `USAGE` text now mentions `list [verb]` and `demo [stop]`.
- `execute_agents_topology::demo_stop(wm)` is a stub (`No agent demo to stop (run :agents demo first)`); 51-03 replaces the body.
- `tests/phase50_agents_window.rs::agents_op_colon_form` has the `DemoStop` arm; `sample_ops` is still extended by 51-12.
- `agents_window.rs` is 1168 lines: 51-09's allowance (contract: at most 25 added) is effectively 24 to stay under the 1200 backstop.
- `AgentsListState` derives `Clone, Debug, Default, PartialEq, Eq` (fields as contract 2.2); `ListDrag` derives `Clone, Debug, PartialEq, Eq`.
- Pane-id arguments in `:agents list select|jump|zoom` accept `#` prefix and are capped at `u32::MAX` (stricter than the existing `:agents zoom` parser).

## Known Stubs

- `agents_list_apply` grouped no-op arm for 16 variants (51-05 owns).
- `execute_agents_topology::demo_stop` (51-03 owns).
- `render_agents_list` draws only the frame and rows (51-06 owns the real chrome).
No ledger entries appended (all are plan-assigned seams).

## Threat Flags

None. T-51-01 (checked numeric parsing, garbage test) and T-51-02 (`sanitize_display` on row names) mitigated as planned.

## Self-Check: PASSED

All created files exist; commits `c8e65d1` and `c586b13` are ancestors of HEAD.
