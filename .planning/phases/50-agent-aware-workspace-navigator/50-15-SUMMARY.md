---
phase: 50-agent-aware-workspace-navigator
plan: 15
subsystem: input
tags: [navigator, mouse, hit-test, context-menu, footer-buttons, colon-command]
status: complete

requires:
  - phase: 50-06
    provides: "DrawerNavOp, NAVIGATOR_DRAWER_BINDINGS, handle_navigator_drawer_key, wm.-allow-list gate"
  - phase: 50-08
    provides: "AgentsOp::Zoom"
provides:
  - "src/input/drawer_navigator.rs: handle_navigator_drawer_mouse, navigator_drag_resize, navigator_button_action, navigator_menu_click_away"
  - "src/ui/drawer/navigator.rs: navigator_row_at, navigator_fold_marker_cells, navigator_footer_buttons, navigator_plus_cells, navigator_header_plus, navigator_menu_layout, navigator_menu_entry_at (renderer and hit-test share one layout)"
  - "src/wm/drawer/menu.rs: NavigatorMenu, NavigatorMenuEntry, WindowManager::navigator_menu"
  - "DrawerNavOp::MenuOpen / MenuClose / MenuPick with :drawer menu [<x> <y> | close | pick <n>] twins"
affects: [50-12, 50-14]

actuals:
  tokens: 60000
  tasks: 4
  commits: 4
plan_head_before: b70aea23b4df70c654e7d54e09791b1bc7ad50c8
plan_head_after: 3fcbb616ebc6f64182b97e4ec440e6927337dac1

tech-stack:
  added: []
  patterns:
    - "Single private layout routine per drawer (navigator_layout) feeds renderer and every hit-test"
    - "Mouse gestures lower to Action::DrawerNav / DrawerFocus / DrawerSize / DrawerVisibility / Agents applied with wm.apply; the only direct write is the resizing_drag bookkeeping flag"
    - "Key/mouse parity helper (open fixture twice, compare observe() strings)"

key-files:
  created:
    - src/wm/drawer/menu.rs
  modified:
    - src/input/drawer_navigator.rs
    - src/input/mouse_wm.rs
    - src/ui/drawer/navigator.rs
    - src/wm/drawer.rs
    - src/wm/drawer/nav.rs
    - src/colon_command/parse_visual.rs
    - src/pane/completion.rs
    - src/ui/help_catalog.rs
    - docs/user-guide/06-keyboard-mouse-and-command-reference.md
    - tests/phase50_navigator_drawer.rs

key-decisions:
  - "Row, fold-marker and [+] clicks apply DrawerFocus first (like the classic click that focuses the drawer), then Row(i), then Activate/Fold/AddWindow; footer buttons do not focus the drawer"
  - "Footer labels are the key characters; button i sits at inner.x + 1 + 2*i, width 2, dropped from the right when they do not fit"
  - "Right-click menu state is a private WorkspaceDrawer field changed only through DrawerNavOp::Menu*; the menu has a title line (sanitized selected row name) plus six entries and is clamped inside the drawer inner rect"
  - "[+] sits one column after the row label and is shown only where it fits before the right-aligned rollup or counts, so 50-02/50-04 right-alignment assertions stay valid"
  - "Any key dismisses an open menu; Esc is consumed by the dismissal"

requirements-completed: [FLT-01, FLT-02]

coverage:
  - id: D1
    description: "Row click focuses the pane, fold-marker click folds/expands, footer buttons and [+] buttons lower to the same Actions as their keys; non-button cells create nothing"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_navigator_drawer.rs#mouse_tracer_click_row_focuses_pane, mouse_fold_marker_click_folds_and_expands, mouse_footer_*, mouse_non_button_cells_create_nothing, plus_*"
        status: pass
    human_judgment: false
  - id: D2
    description: "Title badges, wheel, edge drag and Ctrl+wheel work in navigator style through Actions; key and mouse paths agree"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_navigator_drawer.rs#mouse_title_badges_work_in_navigator, mouse_wheel_scrolls_navigator_selection, mouse_edge_drag_resizes_navigator_width, mouse_ctrl_wheel_scales_drawer_on_gui_surface, parity_navigator_key_and_mouse_paths_match"
        status: pass
    human_judgment: false
  - id: D3
    description: "Right-click context menu: opens at the pointer, entries equal their key twins, Esc/any key/click-away dismiss, clipped and sanitized, right-click elsewhere still pastes, classic unaffected"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_navigator_drawer.rs#rclick_row_opens_menu_and_entries_match_key_twins, rclick_menu_colon_twins_match_mouse, rclick_esc_and_click_away_close_menu, rclick_elsewhere_still_pastes, rclick_menu_text_is_sanitized_and_clipped, rclick_classic_unaffected"
        status: pass
    human_judgment: false
  - id: D4
    description: "Look and feel of the menu, [+] buttons and footer bar in a real terminal and on the GUI surface"
    requirement: FLT-01
    verification: []
    human_judgment: true
    rationale: "Tests prove placement and state; legibility and ergonomics need eyes (real GUI runners per 50-13)"
---

# Phase 50 Plan 15: Navigator Mouse Parity Summary

**The Navigator sidebar is fully mouse-driven: row and fold-marker clicks, a labelled footer button bar, `[+]` add buttons, title badges, wheel, edge drag and a right-click context menu, every gesture lowering to the same `Action` as its key or `:drawer` twin.**

## Performance

- Tasks: 4 (1 tracer, 3 auto); 1 file created, 10 modified
- Commits: 4 (`a5d347e` tracer, `cc78897` fold markers + footer bar, `6001630` badges/wheel/drag + parity, `3fcbb61` context menu + `[+]`)

## Accomplishments

- Tracer: `navigator_row_at` shares the renderer layout; `handle_navigator_drawer_mouse` is reached through one delegation branch in `handle_drawer_mouse_event`, so the classic hard-coded header/footer column zones (Pitfall 3) are never hit in navigator style.
- Footer bar `* ! N + r x @` replaces the dim hint; fold-marker cells and buttons come from the same layout as the renderer.
- Title badges (`DrawerSize(0)` / `DrawerSize(MAX)` / `DrawerVisibility(OnDemand)` + `DrawerToggle`), wheel (`Scroll(+-3)`), edge drag (`DrawerSize` through `navigator_drag_resize`) and the shared Ctrl+wheel scale.
- Right-click on a row selects it and opens a menu (New session, New window here, Split horizontal, Split vertical, Rename, Close); `:drawer menu`, `:drawer menu close`, `:drawer menu pick <n>` are the colon twins.
- `[+]` after workspace/session names adds a window (`a`), `[+]` beside the title creates a session (`N`).

## Deviations from Plan

### Auto-fixed / Harmony Decisions

**1. [Rule 2 - Missing critical] Menu open/close/pick lowered to new `DrawerNavOp` variants**
- **Why:** ARCH-R01 (all user-triggerable gestures lower to an Action) and the exhaustive colon-twin helper require a twin per op; the plan only said "a transient menu state owned by WindowManager". Added `MenuOpen(Option<(u16,u16)>)`, `MenuClose`, `MenuPick(usize)` with `:drawer menu ...` parse, completion, help and palette rows, and updated the exhaustive test helpers.

**2. [Plan interpretation] `[+]` placement**
- Reserving right-hand columns would break the 50-02/50-04 right-aligned rollup/counts assertions (three existing tests). `[+]` therefore sits after the label and is omitted on rows where it does not fit before the right-aligned content; the header `[+]` is omitted when it would collide with the title-bar badges.

**3. [Plan interpretation] Row, marker and `[+]` clicks focus the drawer**
- Applied `Action::DrawerFocus` before `Row(i)` (the classic click sets `drawer.focused = true`); footer buttons do not, so `r` leaves the prompt focused as its key does.

**4. [Plan interpretation] TDD commit granularity**
- Tests and implementation were written test-first but committed together per task so each commit is green.

**5. Task-1 acceptance grep**
- `grep -c "handle_navigator_drawer_mouse" src/input/mouse_wm.rs` prints 1; `navigator_menu_click_away` adds a second delegating line in mouse_wm.rs (outside-the-drawer press closes an open menu). mouse_wm.rs is 894 lines.

## Known Stubs

None.

## Threat Flags

None. T-50-15 mitigated (`mouse_non_button_cells_create_nothing`); T-50-35 accepted as planned; T-50-SC: `git diff --exit-code Cargo.toml Cargo.lock` clean.

## Verification

- `scripts/verify.sh` (full): `PASS 56/56`
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `drawer_navigator.rs` `wm.` allow-list gate: prints nothing; `grep -c "drawer_mut()" src/input/drawer_navigator.rs` prints 1 (resize bookkeeping flag); `navigator_drag_resize` appears once in mouse_wm.rs
- tests/phase50_navigator_drawer.rs: 79 passed (mouse_/parity_/rclick_/plus_)

## Next Phase Readiness

50-12 can add its comms-glyph click branch to `handle_navigator_drawer_mouse` after the footer/marker checks; 50-14 can fold the new mouse table rows into the final docs pass.

## Self-Check: PASSED

Files exist: `src/wm/drawer/menu.rs`, `src/input/drawer_navigator.rs`; commits a5d347e, cc78897, 6001630, 3fcbb61 are ancestors of HEAD.
