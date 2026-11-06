---
phase: 51-ui-tweaks-and-command-parity
plan: 08
subsystem: colon_command / ui
tags: [agents-list, colon, menu, mouse-path, fuzz, UIT-01, UIT-03, UIT-04]
requires: ["51-02"]
provides:
  - "execute_agents_list_command: status from agents_list_apply + structured window summary via set_command_data"
  - "summary_json(wm): scalars-only summary object (active, pinned, focused, docked, filter, selected, row_count, scale_pct)"
  - "additive `agents_list` key in the :agents list / :agents / ls / status payload"
  - "menu entries: View > Agents List / Agent Demo / Stop Agent Demo; Window > Tile Windows & Panels"
key-files:
  created:
    - tests/phase51_agents_list_cmd.rs
  modified:
    - src/colon_command/execute_agents_list.rs
    - src/colon_command/parse_agents_list.rs
    - src/colon_command/execute_agents.rs
    - src/ui/modals.rs
decisions:
  - "bare `:agents list` now opens the window first and builds the payload after, so agents_list reports active/focused"
  - "absolute `scale <n>` stays on the generic PanelScaleOf executor (51-01), so it leaves no list payload"
status: complete
plan_head_before: 0d52cae2f4fc6ee98d42a2ff788b91c9621eed06
plan_head_after: 621f948
commits: 2
actuals:
  tokens: 12000
  tasks: 2
  commits: 2
---

# Phase 51 Plan 08: Agents-list command side and menu mouse paths Summary

Every `:agents list` verb now reports its status text and leaves a scalars-only window summary as RPC data; the `:agents list` payload gains an additive `agents_list` object; the View and Window menus get four new entries that lower to their Actions and work by keyboard and mouse.

Commits: `ffb3a8f` (Task 1), `621f948` (Task 2).

## Gates

- `cargo test -q --test phase51_agents_list_cmd`: 10 passed.
- `cargo test -q --test phase50_agent_state --test phase50_traffic --test phase51_tracer --test agents_launch_affordances --test input_modal_guards_coverage --test drawer_classic_parity`: all ok (7, 11, 30, 19, 11, 10 passed).
- `scripts/verify.sh` (full): `PASS 80/80 26s`.
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
- `git diff --exit-code HEAD -- src/input/mouse_wm.rs Cargo.toml Cargo.lock`: clean.
- Acceptance: set_command_data in execute_agents_list.rs = 1; `agents_list` in execute_agents.rs = 2; execute_agents.rs numstat `2 added 1 removed`, 304 lines; modals.rs 805 lines (cap 830, near-cap of 810 respected); `:agents demo stop` / `:agents list toggle` / `:arrange grid` in modals.rs = 1 each.

## Deviations from Plan

**1. [Rule 1 - ordering] Bare list executor order swapped.** `AgentsOp::List` arm is now `open_agents_list(); list(wm);` (was `list; open`) so the additive `agents_list` object reflects the opened window. Status text and every existing payload key/value unchanged.

**2. [Scope] Absolute `:agents list scale <n>` leaves no list payload.** It parses to `Action::PanelScaleOf` (frozen in 51-01) and runs through the generic panel-scale executor; `list_data_payload` skips these. `scale +n|-n` (list op) does return the payload.

**3. [Pre-existing, deferred] Open traffic window swallows clicks over the menu dropdown.** `handle_agents_window_mouse` (src/input/agents_window.rs) runs before the menubar branch in `handle_wm_mouse_event`, so with the AGENT TRAFFIC window open, a click on a dropdown row that lies under it is consumed by the window (found: Stop Agent Demo after `:agents demo`, which opens the traffic window). The mouse test closes the traffic window first. The same ordering applies to the AGENTS list window's mouse handler (51-07) unless it yields to an open menubar; recommend 51-07/51-12 give the menu priority (outside this plan's files: mouse_wm.rs/mouse.rs/agents_window.rs).

## Known Stubs

None added. Demo and verb semantics are 51-03/51-05; tests assert grammar, status plumbing, payload, and menu lowering as planned (`menu_pick_*` demo/stop assertions are behavior-agnostic).

## Threat Flags

None. T-51-20 mitigated (20000-sequence fuzz with 4 KB tokens, round-trip of every parsed op, edge-number Err tests); T-51-21 mitigated (summary has scalars only, asserted: payload contains no `name`).

## Self-Check: PASSED

Files exist; commits `ffb3a8f` and `621f948` are ancestors of HEAD.
