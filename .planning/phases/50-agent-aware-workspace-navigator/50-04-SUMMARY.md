---
phase: 50-herdr-style-left-sidebar-agent-aware-workspace-navigator
plan: 04
subsystem: ui
tags: [drawer, herdr, agent-state, attention, leader-key, colon-command]
status: complete

requires:
  - phase: 50-02
    provides: herdr drawer tree, renderer, HerdrRowMeta
  - phase: 50-03
    provides: AgentState tracker, agent_state/agent_counts, report_agent_state
provides:
  - "HerdrRowMeta.agent (live pane rows) and .counts (live session/workspace row)"
  - "State dot glyph/colour tables (unicode + ASCII) and ui::drawer::herdr::{herdr_dot_cells, HerdrDotCell, counts_segments, dot_color}"
  - "DrawerNavOp::NextAttention, WindowManager::{apply_drawer_nav, jump_next_attention, set_drawer_style}"
  - "Action::DrawerNav, Action::DrawerStyle; :drawer attention, :drawer style [classic|herdr|toggle]; leader !"
  - "tick_agent_ui returns true only for visible herdr changes"
affects: [50-05, 50-06, 50-11, 50-12]

actuals:
  tokens: 9600
  tasks: 2
  commits: 3
plan_head_before: 276a924f1004405b3b6c76206c0e8217403af464
plan_head_after: 113e78f3d3b4d9873fffbf93d2f37134b215ed98

tech-stack:
  added: []
  patterns:
    - "Shared HerdrLayout geometry used by both the renderer and herdr_dot_cells (single source of dot positions)"
    - "Dot column reserved in the pane label text, then overwritten and coloured from herdr_dot_cells"

key-files:
  created:
    - src/wm/drawer/nav.rs
  modified:
    - src/wm/drawer.rs
    - src/wm/drawer/herdr.rs
    - src/ui/drawer/herdr.rs
    - src/wm/agent_state.rs
    - src/action.rs
    - src/colon_command/parse_visual.rs
    - src/colon_command/execute_wm.rs
    - src/input/keymap.rs
    - src/pane/completion.rs
    - src/ui/help_catalog.rs
    - tests/phase50_herdr_drawer.rs

key-decisions:
  - "Glyphs: working = braille spinner (ASCII *), blocked ▲/!, idle ○/-, done ●/+, errored ✗/x; colours from DesignTokens (success, warning, text_muted, accent, danger)"
  - "Counts attach to the live session row at any depth and replace the Np rollup there; Np is the fallback when the tracker has no panes yet"
  - "Saved roots render `<n>p -`; the 50-02 rollup test was updated to expect the trailing dash"
  - "Counts segments are dropped whole from the right when they do not fit the space left after the name"
  - "Attention picks the highest non-empty tier (Blocked > Errored > Done), cycling in ordered_pane_ids order after the active pane, wrapping"
  - "WorkspaceDrawer::spinner_tick accessor lives in drawer/herdr.rs so drawer.rs only gained the two declaration lines"

requirements-completed: [FLT-01]

coverage:
  - id: D1
    description: "Live pane rows show a state dot (five states, distinct glyph and colour, ASCII fallback); live workspace row shows compact counts; saved roots show `<n>p -`"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_herdr_drawer.rs#dots_render_all_five_states, dots_real_signal_errored, dots_workspace_counts_summary, dots_saved_workspace_shows_count_and_dash, attention_tracer_blocked_dot_and_jump"
        status: pass
    human_judgment: false
  - id: D2
    description: "`:drawer attention` and leader `!` focus the next attention pane (blocked, errored, done), wrap, and no-op with a status message"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_herdr_drawer.rs#attention_prefers_blocked_then_errored_then_done, attention_cycles_and_wraps, attention_noop_when_none_sets_status, attention_leader_bang_with_drawer_closed, keymap_leader_bindings_have_unique_keys"
        status: pass
    human_judgment: false
  - id: D3
    description: "`:drawer style [classic|herdr|toggle]` switches style via Action and reflows; redraw only for visible herdr state changes"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "style_colon_drawer_style_sets_and_toggles; src/wm/agent_state.rs#dots_tick_agent_ui_redraws_only_for_visible_herdr"
        status: pass
    human_judgment: false
  - id: D4
    description: "Colour and glyph appearance in a real terminal"
    requirement: FLT-01
    verification: []
    human_judgment: true
    rationale: "Tests assert glyph cells and ASCII fallback; colour contrast on the reversed selection bar is only visually checkable"
---

# Phase 50 Plan 04: Agent State Dots, Counts and Attention Navigation Summary

**The herdr sidebar now shows a coloured state dot per pane (working spinner, blocked, idle, done, errored; ASCII fallback), per-workspace count summaries, and `:drawer attention` / leader `!` jump to the next pane needing attention, with `:drawer style [classic|herdr|toggle]` switching styles through `Action`.**

## Performance

- Tasks: 2 (1 tracer, 1 tdd auto); files: 1 created, 11 modified; 602 insertions, 33 deletions
- Commits: 3 (`409fa95` tracer, `1e6febf` RED, `113e78f` GREEN)
- Started 2026-11-03, completed 2026-11-04T00:56Z

## Accomplishments

- Tracer: rows carry `agent` and `counts`, the renderer draws dots from `herdr_dot_cells`, `DrawerNavOp::NextAttention` lowers through `Action::DrawerNav`, `:drawer attention` focuses a blocked pane.
- Full state set: five glyphs in both packs, token colours, right-aligned counts summary (Blocked, Errored, Working, Done, Idle), saved roots with `<n>p -`.
- Attention tiers Blocked > Errored > Done with wrap-around and a `No pane needs attention` status when none.
- `:drawer style`, leader `!`, completion and help/palette rows.
- `tick_agent_ui` now requests a redraw only when the herdr drawer is visible, leaving the classic redraw cadence unchanged.

## Deviations from Plan

None - plan executed exactly as written. Notes:

- The Task 1 tracer implemented all three attention tiers in `jump_next_attention` at once (a strict superset of the Blocked-only scope); the Task 2 RED commit therefore only had two failing tests (`attention_leader_bang_with_drawer_closed`, `style_colon_drawer_style_sets_and_toggles`) plus the in-crate tick test. The other Task 2 tests passed immediately and still lock the behaviour.
- The 50-02 test `tree_saved_session_rollup` was updated from `ends_with("3p")` to `ends_with("3p -")` because this plan specifies the saved-root `-` marker.
- `dots_tick_agent_ui_redraws_only_for_visible_herdr` is an in-crate unit test (`src/wm/agent_state.rs`) because `tick_agent_ui` is `pub(crate)`.

## Known Stubs

None. Mouse hit-testing and drawer key handling for herdr rows remain scheduled for 50-06 (as noted in 50-02).

## Threat Flags

None. T-50-09 and T-50-10 accepted as planned (attention jump is user-initiated only); T-50-SC: `git diff Cargo.toml Cargo.lock` clean.

## Verification

- `scripts/verify.sh` (full): `PASS 54/54`
- `scripts/verify.sh --only clippy unit t:drawer_classic_parity t:phase50_herdr_drawer t:phase50_agent_state t:slide_out_drawer_and_telemetry t:api_conformance t:session_lessons_and_regression_suite`: `PASS 54/54`
- `cargo test --test phase50_herdr_drawer`: 24 passed
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- Acceptance: `Char('!')` in keymap.rs 1; `DrawerStyle(Option<crate::wm::drawer::DrawerStyle>)` in action.rs 1; `DrawerNav(crate::wm::drawer::DrawerNavOp)` 1; `"attention"` in completion.rs 1; `src/wm/drawer.rs` 1031 lines (cap 1032)

## Next Phase Readiness

Ready for 50-05/06: `DrawerNavOp` is the extension point for key/mouse navigation, and `herdr_dot_cells` is ready for the GUI icon pass in 50-11.

## Self-Check: PASSED

Created file `src/wm/drawer/nav.rs` exists; commits 409fa95, 1e6febf, 113e78f are ancestors of HEAD.
