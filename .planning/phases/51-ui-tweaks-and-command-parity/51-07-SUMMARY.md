---
phase: 51-ui-tweaks-and-command-parity
plan: 07
subsystem: input
tags: [keys, mouse, agents-list, leader-chords, gui-wheel, UIT-03, UIT-01, UNI-R01]
requires: ["51-02"]
provides:
  - "agents_list_keys.rs: AGENTS_LIST_BINDINGS, agents_list_action, handle_agents_list_key"
  - "agents_list.rs: ListMousePlan, plan_agents_list_mouse (pure), handle_agents_list_mouse"
  - "Ctrl+Alt+N/n global rows, leader A / y / Y, reserved_global_action extension"
  - "GUI Ctrl+wheel over the list forwards to the panel handler"
affects: [51-08, 51-12]
key-files:
  created:
    - src/input/agents_list_keys.rs
    - src/input/agents_list.rs
    - tests/phase51_agents_list_input.rs
  modified:
    - src/input/keymap.rs
    - src/input/mod.rs
    - src/input/modal.rs
    - src/input/mouse.rs
    - src/input/gui_route.rs
decisions:
  - "Ctrl+Alt+N confirmed free (no conflict), no fallback to Ctrl+Alt+G"
  - "toggle chord is handled before the table inside handle_agents_list_key and is not a row of AGENTS_LIST_BINDINGS (keeps the table free of global shadows)"
  - "unfocused click on a row plans [Open, SelectPane(id)] even if already selected (never jumps); Close and Minimize badges skip the leading Open"
status: complete
plan_head_before: 0d52cae2f4fc6ee98d42a2ff788b91c9621eed06
plan_head_after: 2edcfbc754fac7a0e5bfada00cd25a7b6199affc
commits: 2
actuals:
  tokens: 14000
  tasks: 2
  commits: 2
---

# Phase 51 Plan 07: Agents list keys and mouse Summary

Every key and mouse gesture of the AGENTS list lowers to `Action::AgentsList(..)` through `WindowManager::apply`, with a pure mouse planner (event + `&WindowManager` -> plan) and a thin applying wrapper; the window is non-modal and top-most.

Commits: `7248582` (Task 1 keys), `2edcfbc` (Task 2 mouse + wheel).

## Gates

- `cargo test -q --test phase51_agents_list_input`: 16 passed (8 `key_`, 8 mouse/gui).
- Regression, all pass: `phase50_agents_window` 9, `phase50_agents_nonmodal` 17, `agents_launch_affordances` 7, `phase50_1_find_seam` 12, `phase50_1_find_input` 13, `phase50_navigator_drawer`, `panel_pin_scale`, `linux_session_regression_and_gui_scaling` 93, `phase50_gui_runtime`, `phase51_seams` 10, `phase51_tracer` 10, `drawer_classic_parity` 11, `--lib` 294.
- `cargo clippy -q --all-targets -- -D warnings`: clean.
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
- `git diff --exit-code HEAD -- Cargo.toml Cargo.lock`: clean.
- Acceptance: `AgentsListOp::Toggle` in keymap.rs = 4; `Action::Agents(AgentsOp::DemoStop)` = 1; `pub const AGENTS_LIST_BINDINGS` = 1; `handle_agents_list_mouse` in mouse.rs = 1; `PanelTarget::AgentsList` in gui_route.rs = 1; `fn plan_agents_list_mouse` = 1.
- Lines: keymap.rs 966 (+8, soft 1000), agents_list.rs 164, agents_list_keys.rs 92, mouse.rs 827 (+5), modal.rs 949 (+8). `handle_wm_key_event` body: 53 lines (unchanged, limit 80).

## GUI and TUI share one path (UNI-R01)

`App::handle_crossterm_event` (src/app.rs:131) is the single entry: the TUI loop (app.rs:83) and every GUI runner (`ui/gui_window.rs:535`, `ui/gui_macos.rs:484/591/630`, `ui/gui_font/linux_x11.rs:688/701/712`) feed it crossterm events, and it calls `handle_wm_key_event` / `handle_wm_mouse_event`. The key gate (modal.rs) and the mouse hook (mouse.rs) therefore apply to both.

## Deviations from Plan

- **keymap.rs line cap:** acceptance says at most 960; actual 966 (the lead's note: soft goal 1000). Kept to the minimum (use line + 2 global rows + 3 leader rows + 2 comments + the reserved chord inline).
- **`pub mod` instead of `mod`:** `agents_list` and `agents_list_keys` are `pub mod` so integration tests can assert the table and the pure planner (the plan's tests need them).
- **Test-first caveat:** tests and implementation were written before the first run, so there was no separate RED run (the key test file did not compile before the module existed).
- **`key_chord_works_in_raw_passthrough`** asserts the list toggles and the pane prompt is unchanged; the PTY write path cannot be observed headless.
- No edits to `src/input/tests.rs` were needed.

## Known Stubs

None. `ScaleStep`, `Scroll`, `SetRect`, `Pin`, `Dock` etc. are applied by `agents_list_apply` whose semantics are 51-05's (still the grouped no-op arm in this base); this plan asserts the produced Action plans only, as specified.

## Threat Flags

None. T-51-17 (leader never consumed; toggle chord first), T-51-18 (outside press plans Blur and passes through; unfocused list intercepts nothing) and T-51-19 (saturating drag math; test at `u16::MAX`) are covered by `key_focused_swallows_and_leader_passes`, `mouse_outside_blurs_and_passes`, `mouse_drag_title_and_border`.

## Self-Check: PASSED

Created files exist; commits `7248582` and `2edcfbc` are ancestors of HEAD.
