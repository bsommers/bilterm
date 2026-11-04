---
phase: 50-agent-aware-workspace-navigator
plan: 06
subsystem: ui
tags: [drawer, navigator, keymap, colon-command, animation, session-tree]
status: complete

requires:
  - phase: 50-02
    provides: navigator drawer tree, renderer, NavigatorRowMeta
  - phase: 50-04
    provides: DrawerNavOp, Action::DrawerNav, NextAttention, set_drawer_style
provides:
  - "DrawerKeyScope, NAVIGATOR_DRAWER_BINDINGS, navigator_drawer_action in src/input/keymap.rs"
  - "src/input/drawer_navigator.rs::handle_navigator_drawer_key (scope resolution + table lookup + wm.apply)"
  - "DrawerNavOp Fold/Activate/Next/Prev/Row/Scroll/Widen/NewSession/NewPane/RenamePrompt/Rename/Close/ExpandAll/CollapseAll/AddWindow/AddPane"
  - ":drawer fold|enter|next|prev|row|scroll|widen|new-session|new-pane|rename|close-pane|expand all|collapse all|add window|add pane and the :navigator alias"
  - "ANIM_INTERVAL (120 ms) throttled working-animation redraw in tick_agent_ui"
affects: [50-07, 50-12, 50-14, 50-15]

actuals:
  tokens: 44000
  tasks: 5
  commits: 5
plan_head_before: d7c65ea4d3b93d32ef82bffa0bea799298550136
plan_head_after: afcec5917d27c85f05bac9748a96d8b1b9509834

tech-stack:
  added: []
  patterns:
    - "Declarative navigator key table consulted before the unchanged classic drawer match; every row is an Action::DrawerNav run through WindowManager::apply"
    - "Exhaustive drawer_nav_colon_form helper in the tests so a new DrawerNavOp variant fails to compile until it has a colon twin"

key-files:
  created:
    - src/input/drawer_navigator.rs
  modified:
    - src/input/keymap.rs
    - src/input/mod.rs
    - src/input/modal.rs
    - src/wm/drawer/nav.rs
    - src/wm/agent_state.rs
    - src/colon_command/parse_visual.rs
    - src/pane/completion.rs
    - src/ui/help_catalog.rs
    - src/command_registry.rs
    - src/action.rs
    - docs/user-guide/06-keyboard-mouse-and-command-reference.md
    - tests/phase50_navigator_drawer.rs

key-decisions:
  - "Operations live in WindowManager::apply_drawer_nav (nav.rs); the input module only resolves scopes, looks up the table and calls apply"
  - "CollapseAll is a toggle on an already fully folded tree (so key C needs no second op); documented in help and docs"
  - "AddWindow/AddPane reuse split_focused; a session other than the active one is switched to first, unknown names are refused with a status message"
  - ":drawer expand without all keeps its older meaning (maximize), like :drawer close keeps minimize"
  - "Navigator keys a and A shadow the classic arrange grid / stack-grid keys in navigator style only; :arrange stays available"

requirements-completed: [FLT-01]

coverage:
  - id: D1
    description: "Navigator drawer keys are rows of NAVIGATOR_DRAWER_BINDINGS, lower to Action::DrawerNav and run through WindowManager::apply; unclaimed keys fall through to the classic drawer handler"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_navigator_drawer.rs#keys_tracer_fold_by_key_and_command, keys_jk_and_arrows_move_selection, keys_enter_and_space_focus_selected_pane, keys_bang_jumps_to_attention_in_drawer, keys_capital_n_creates_session_and_selects_it, keys_n_c_plus_w_create_pane, keys_r_opens_rename_prompt_and_rename_applies, keys_x_and_delete_close_selected_pane, keys_pageup_pagedown_scroll_and_braces_resize, keys_unclaimed_keys_fall_through_to_classic, keys_e_and_bang_unclaimed_by_classic"
        status: pass
    human_judgment: false
  - id: D2
    description: "Every DrawerNavOp has a :drawer colon twin, completion verb and help/palette row; table has no duplicate chord and does not shadow a global binding; key and colon give identical state"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_navigator_drawer.rs#keys_colon_equivalents_match_keys, keymap_navigator_drawer_keys_unique, keymap_navigator_drawer_keys_do_not_shadow_globals, drawer_nav_ops_have_colon_completion_and_help, keys_colon_close_keeps_minimize_meaning"
        status: pass
    human_judgment: false
  - id: D3
    description: "Pinned navigator drawer stays open and reflows panes, GUI scale applies; working animation redraws at most every 120 ms and only for a visible navigator drawer"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_navigator_drawer.rs#pin_scale_fixed_keeps_navigator_visible_and_reflows, pin_scale_gui_scale_enlarges_navigator_glyphs; src/wm/agent_state.rs#anim_working_requests_throttled_redraw"
        status: pass
    human_judgment: false
  - id: D4
    description: "Expand all / collapse all and add window/pane by key, :drawer and :navigator, with completion, help, palette and docs"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_navigator_drawer.rs#expand_all_unfolds_every_workspace_and_session, collapse_all_folds_every_tree_to_workspace_rows, collapse_all_on_single_workspace_hides_panes_and_toggles_back, keys_e_upper_expands_all_and_c_upper_collapses_all, colon_navigator_and_drawer_expand_collapse_all_match, expand_collapse_all_have_completion_and_help, classic_unaffected_by_expand_collapse_all, add_window_targets_selected_session, add_pane_splits_selected_row, key_a_and_colon_twins_match, add_classic_unaffected"
        status: pass
    human_judgment: false
  - id: D5
    description: "Feel of the animation cadence and key ergonomics in a real terminal"
    requirement: FLT-01
    verification: []
    human_judgment: true
    rationale: "Tests assert throttle timing and key effects; perceived smoothness is only visually checkable"
---

# Phase 50 Plan 06: Keyboard-First Navigator Drawer Summary

**The focused Navigator drawer is now driven by a declarative key table in keymap.rs (j/k, Enter, `!`, `e`, `E`, `C`, `N`, `a`, `A`, `n`, `r`, `x`, PgUp/PgDn, `{`/`}`), every row lowering to `Action::DrawerNav` with a `:drawer` and `:navigator` command twin, plus a throttled working animation.**

## Performance

- Tasks: 5 (1 tracer, 4 auto); 1 file created, 12 modified; 1182 insertions, 10 deletions
- Commits: 5 (`62236f3` tracer, `2d7e390` key table, `faedcea` animation and pin/scale, `9067717` expand/collapse all, `afcec59` add window/pane)

## Accomplishments

- Tracer: `NAVIGATOR_DRAWER_BINDINGS` / `navigator_drawer_action` in keymap.rs, `handle_navigator_drawer_key` in the new private `drawer_navigator.rs`, one delegation block in `modal.rs`. `e` and `:drawer fold` fold a session, `:drawer enter` focuses a row.
- Full table: 25 rows covering movement, focus, attention, fold, new session/pane, rename prompt, close, scroll and width; `:drawer next|prev|row|scroll|widen|new-session|new-pane|rename|close-pane`. `:drawer close` still means minimize (`DrawerSize(0)`).
- Pin/scale: `*`, `[`, `]`, `d`, `q` fall through to the classic handler and behave identically in navigator style (proved by tests); `tick_agent_ui` additionally requests a redraw every `ANIM_INTERVAL` while a pane is Working and the navigator is visible.
- Expand/collapse all (`E`, `C`, `:navigator|:drawer expand all|collapse all`) with selection kept on the logical row or moved to its workspace.
- Add window/pane to a chosen session (`a`, `A`, `:navigator|:drawer add window [session]|add pane [h|v]`).
- `:navigator` alias of `:drawer` with completion row, registry alias, help/palette rows; docs section "Navigator drawer keys" in chapter 6.

## Deviations from Plan

### Auto-fixed / Harmony Decisions

**1. [Rule 1 - Bug avoided] `:drawer expand` without `all` keeps the maximize alias**
- **Found during:** Task 4
- **Issue:** The plan asks for a usage error for `:drawer expand` without `all`, but `expand` is an existing alias of `maximize` (`DrawerSize(u16::MAX)`); an error would break a working classic command (same harmony rule the plan applies to `:drawer close`).
- **Fix:** Only `expand all` is claimed; bare `expand` still maximizes. `:drawer collapse` without `all` returns the usage error as planned. Test asserts both.
- **Files modified:** src/colon_command/parse_visual.rs, tests/phase50_navigator_drawer.rs

**2. [Plan interpretation] `C` toggles inside `CollapseAll`**
- The plan requires `C` to be one table row lowering to `CollapseAll` and a second `C` on a fully folded tree to expand. The toggle therefore lives in the op, so `:drawer collapse all` repeated on a fully folded tree also expands. Stated in the help row and docs.

**3. [Plan interpretation] TDD commit granularity**
- Tests and implementation for Tasks 2 to 5 were written test-first but committed together per task (no separate RED commits), so each commit is green.

**4. In-crate animation test**
- `anim_working_requests_throttled_redraw` lives in `src/wm/agent_state.rs` (not the integration file) because `tick_agent_ui` is `pub(crate)`, same precedent as 50-04.

## Known Stubs

None. Mouse hit-testing for navigator rows (`DrawerNavOp::Row` is the operation) and footer buttons remain in plan 50-15.

## Threat Flags

None. T-50-16 accepted as planned; T-50-17 mitigated (rename goes through `SessionTreeCommand("rename ...")`, extra whitespace in a name is refused); T-50-SC: `git diff --exit-code Cargo.toml Cargo.lock` clean.

## Verification

- `scripts/verify.sh` (full): `PASS 55/55`
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `cargo test --test drawer_classic_parity`, `--test drawer_behaviour_coverage` (XDG isolated), `--test api_conformance`: pass
- `drawer_navigator.rs` `wm.` allow-list gate: prints nothing
- Acceptance: `Rename(String)` 1, `"new-session"` 1, `"close-pane"` 1 in completion.rs, minimize alias arm unchanged (1), `ExpandAll` in nav.rs 2, `ANIM_INTERVAL` 3, keymap.rs 782 lines, modal.rs 930 lines, `handle_wm_key_event` not edited

## Next Phase Readiness

50-15 can reuse `DrawerNavOp::Row`, `Fold`, `Scroll`, `Widen`, `NewSession`, `NewPane`, `Close`, `AddWindow` and `AddPane` for the mouse and footer buttons; 50-12 can add a `DrawerKeyScope::NavigatorComms` scope to `handle_navigator_drawer_key`.

## Self-Check: PASSED

`src/input/drawer_navigator.rs` exists; commits 62236f3, 2d7e390, faedcea, 9067717, afcec59 are ancestors of HEAD.
