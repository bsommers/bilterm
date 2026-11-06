---
phase: 51-ui-tweaks-and-command-parity
plan: 12
subsystem: tests / input
tags: [uniformity, parity-matrix, UNI-R01, UIT-01, UIT-02, UIT-03, UIT-04]
requires: ["51-03", "51-04", "51-05", "51-06", "51-07", "51-08", "51-09", "51-10"]
provides:
  - "tests/phase51_uniformity.rs: capability x surface matrix (12 tests), exception-set equality, drawn-control click test"
  - "tests/phase50_agents_window.rs: sample_ops covers AgentsOp::Demo and AgentsOp::DemoStop"
  - "src/input/mouse.rs: a modal drawn over a floating agents window takes the click (found by the matrix)"
key-files:
  created:
    - tests/phase51_uniformity.rs
  modified:
    - tests/phase50_agents_window.rs
    - src/input/mouse.rs
decisions:
  - "state is compared through one signature (list summary, list rect, traffic window flags, demo summary, pane count/order/rects/sizes, ring and topology counts, menu and palette visibility), never status text or bytes"
  - "the six exceptions are a literal (capability, surface, reason) table; cells are key/mouse only; palette is checked as exact entry, runnable entry of the verb family, or exclusion row"
status: complete
plan_head_before: f1f548c
plan_head_after: 86986b9
commits: 3
actuals:
  tokens: 40000
  tasks: 2
  commits: 3
---

# Phase 51 Plan 12: UNI-R01 parity matrix Summary

One table-driven matrix drives every Phase 51 capability through colon, `wm.command`, key, mouse, palette (Enter and click), menu (keyboard and click), `agents.list` / `agents.demo` and the MCP tools and asserts identical state; the six contract exceptions are asserted by literal set equality.

Commits: `2c91abe` (Task 1), `b53a936` (src fix, see Defects), `86986b9` (Task 2).

## What the matrix covers

35 capability rows: list open/close/toggle, dismiss, blur, select (down/up/first/last/page/id), jump (+id), zoom (+id), pin (toggle/set/unpin), scale (up/down), filter (toggle/agents/all), dock (toggle/set/float), move, resize, rect, size, scroll, status, demo start, demo stop, `:arrange grid` with the list and traffic window open. Every row first proves its colon command changes state (a no-op row fails), then compares each surface to it. Other tests: `exceptions_are_exactly_the_recorded_six`, `every_list_op_is_in_the_matrix` (exhaustive `capability_of` match: a new `AgentsListOp` variant fails to compile; also ties `AGENTS_LIST_BINDINGS` to key cells), `list_key_rows_match_colon_twins`, `list_completion_help_palette_per_verb`, `drawn_controls_are_clickable`, `palette_entry_reachable_by_key_and_mouse` (+ `bilterm://commands` exclusion), `phase51_verb_families_are_palette_covered`, `arrange_surfaces_agree`, `demo_surfaces_agree`, `mcp_and_rpc_agree`, `modal_click_beats_floating_windows_under_it`.

## Gates

- `cargo test -q --test phase51_uniformity`: 12 passed.
- `cargo test -q --test phase51_uniformity --test phase50_agents_window --test phase51_palette_parity --test phase51_agents_list_input --test phase51_panel_tiling --test phase51_demo_stop --test phase51_api_parity`: all 7 binaries ok (70, 16, 12, 16, 6, 18, 12 tests; phase50_agents_window stays at its baseline 70).
- `cargo clippy -q --all-targets -- -D warnings`: clean.
- `scripts/verify.sh` (full, own target dir): `PASS 89/89 30s`.
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
- Acceptance greps: `fn uniformity_matrix` 1; `EXCEPTIONS` 6; no `phase51_uniformity` snapshot; Cargo.toml/Cargo.lock untouched.

## Deviations from Plan

**1. [Rule 1 - Bug, small and owned by 51-07/51-08] Modal under a floating window lost the click.** `handle_wm_mouse_event` offered every press to the AGENTS list and the traffic window before the modal handler. A palette entry lying over either window was swallowed by the window (traffic window: palette click never executed) and, with the list focused, the palette click blurred the list while the Enter path left it focused. Found by the matrix (`palette click` surface of `demo.stop` and `list.*`). Fix: `src/input/mouse.rs`, +6/-2 lines, the floating-window block now skips events inside the active modal rect (same pattern as 51-08's menubar fix). Verified RED without it (3 tests fail) and GREEN with it; full verify passes. This touches `src` although the plan was test-only, so `git diff --exit-code HEAD -- src` is non-zero by this one file; it is its own commit.

**2. [Interpretation] Exception 5.** Recorded cells: `list.dismiss` mouse, `select.page` mouse and `scroll` key (the mirror of the Page/wheel twin pairing: PgUp/PgDn is Page, the wheel is Scroll). The Minimize badge = dismiss is asserted in `exceptions_are_exactly_the_recorded_six`.

**3. [Doc] Contract spelling.** `:agents list status` is a usage error; the data-only form is `:agents status` (or bare `:agents list`). The `status` row uses `:agents status` and `agents.list {}`.

## Observations (not fixed)

- `tests/drawer_behaviour_coverage.rs` (6), `phase50_1_capture`, `phase50_1_find_ops`, `phase50_1_registry`, `rect_mode...` fail when run with the user's real `XDG_STATE_HOME` (saved sessions) and pass under `scripts/verify.sh`, which isolates it. Pre-existing and unrelated (identical failures with the src fix reverted).
- `tests/phase51_uniformity.rs` is 1317 lines (test file; the 1200 hard backstop applies to src files and `session_lessons_and_regression_suite.rs`).

## Known Stubs

None.

## Threat Flags

None. T-51-30 mitigated (matrix and exception-set equality wired into `scripts/verify.sh` as stage `t:phase51_uniformity`); T-51-SC: no dependency change.

## Self-Check: PASSED

`tests/phase51_uniformity.rs` exists; commits `2c91abe`, `b53a936`, `86986b9` are ancestors of HEAD.
