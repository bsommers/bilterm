---
phase: 51-ui-tweaks-and-command-parity
plan: 05
subsystem: wm
tags: [agents-list, selection, scroll, geometry, dock, UIT-03]
requires: ["51-01", "51-02"]
provides:
  - "complete agents_list_apply (exhaustive, no wildcard, no placeholder arm)"
  - "agents_list_workspace(), agents_list_tick_changed() (pub(crate))"
  - "tests/phase51_agents_list_core.rs (14 tests)"
affects: [51-06, 51-07, 51-08, 51-09, 51-10]
key-files:
  created:
    - tests/phase51_agents_list_core.rs
  modified:
    - src/wm/agents_list.rs
    - src/wm/agent_state.rs
decisions:
  - "ops that need an open list (Step/First/Last/Page/Scroll/Jump/Zoom/Pin/Dock/Move/Resize/SetRect/Size) return 'Agents list is not open' when closed; SelectPane opens it; Filter and ScaleStep work on a closed list"
  - "Dock success returns None; failure on `on` returns 'No room to dock the agents list'"
status: complete
plan_head_before: 0d52cae2f4fc6ee98d42a2ff788b91c9621eed06
plan_head_after: dbc2484e7b34883de2a6d68f04cd91f7c138b5f0
commits: 1
actuals:
  tokens: 12000
  tasks: 2
  commits: 1
---

# Phase 51 Plan 05: Agents list core behavior Summary

Every `AgentsListOp` now has tested semantics: stable PaneId selection with nearest-row fallback, scroll-in-view through `agents_list_layout(ws).visible_rows`, Jump/Zoom/Pin/ScaleStep, Dock through `set_panel_docked`, clamped geometry ops that release the dock, and live row refresh from `tick_agent_ui`.

## Gates

- `cargo test -q --test phase51_agents_list_core`: 14 passed.
- `cargo test -q --test phase50_agent_state --test phase50_navigator_drawer --test phase51_tracer --test phase51_seams --test phase50_agents_window`: all passed (19, 70, 93, 10, 10).
- `cargo clippy -q --all-targets -- -D warnings`: clean.
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
- `scripts/verify.sh` (full): `PASS 80/80 70s`.
- `git diff --exit-code HEAD -- Cargo.toml Cargo.lock`: clean.
- Greps: `AgentsListOp::` in agents_list.rs = 29 (baseline after 51-01: 16 in the grouped arm + 5 real arms); `set_panel_docked` = 3; `step_scale(PanelTarget::AgentsList` = 1; `agents_list` in agent_state.rs = 2; `fn normalize_selection` = 1; agents_list.rs = 500 lines; agent_state.rs numstat +3/-2.

## Deviations from Plan

None in behavior. Wave-2 deltas honored: the match stays exhaustive; every dock change goes through `set_panel_docked` (only called when `docked` is set for manual-geometry/close release); `agents_list_layout(ws).visible_rows` drives Page/scroll clamping.

Notes:
- `tick_agent_ui` early-return path and the Navigator path both OR in `agents_list_tick_changed()` (it is folded into `traffic`, so one edit covers both).
- `AgentsListState` gained two private fields (`sel_idx`, `rows_sig`); `Default`/`PartialEq` still derive.
- `open_agents_list` now selects the active pane's row on a fresh open (first row when filtered out) and re-normalises on re-open.
- The per-plan commit ledger file could not be written by the sandbox shell; `plan_head_before` is the known merged tip `0d52cae`.
- Geometry test checks containment in the full 120x40 area (headless workspace is a subset of it).

## Known Stubs

None added. `set_panel_docked` is still the 51-02 stub (51-09 owns).

## Threat Flags

None. T-51-12 (names via `identity_token` + `sanitize_display`, test asserts no control/bidi chars), T-51-13 (i64/i32 saturating and clamped arithmetic, extreme-delta geometry test), T-51-14 (`focus_and_raise`/`shuffle_deck_next` blur; `only_one_window_owns_keyboard`) mitigated.

## Self-Check: PASSED

tests/phase51_agents_list_core.rs exists; commit dbc2484 is an ancestor of HEAD.
