# Phase 21 Plan 03 Summary — Invariant Unit Tests & Phase 21 State Synchronization

- **Plan:** `21-03-PLAN.md` (`ARCH-R02`)
- **Status:** Completed

## Changes Delivered

1. **Workspace & Pane Invariant Unit Tests (`src/wm/tests.rs`, `src/pane/mod.rs`):**
   - `reap_exited_zoomed_pane_clears_zoom_and_updates_tree_and_z_stack`: Verifies that reaping an exited zoomed pane via `WindowManager::poll_all_ptys` -> `remove_pane` clears `zoomed_pane`, removes the leaf from `SplitTree`, removes the ID from `ZStack`, and restores focus to the remaining pane.
   - `disconnect_fleet_node_removes_associated_ssh_pane_via_remove_pane`: Verifies that disconnecting a fleet SSH node removes its pane from `panes`, `split_tree`, `z_stack`, and `zoomed_pane`.
   - `modal_enum_enforces_mutual_exclusion_and_focus_avoids_zero_sentinel`: Verifies that opening `Help`, `Inspector`, `Palette`, or `Menubar` enforces mutual exclusion via `Modal`, and that `wm.focus()` returns `Focus::Quick` while `wm.active_pane_id()` never returns a `0` sentinel.
   - `submit_command_and_submit_prompt_enforce_identical_invariants`: Verifies that `CompositePane::submit_prompt` and `CompositePane::submit_command` both record history, set `running_command`, record OSC 133 command blocks, and enforce the `read_only` guard.

2. **Planning & Traceability Updates (`.planning/{REQUIREMENTS,ROADMAP,STATE}.md`):**
   - Marked `ARCH-R02` and Phase 21 (`21-01-PLAN.md`..`21-03-PLAN.md`) complete.
   - Advanced `.planning/STATE.md` to Phase 22 (`unified-action-command-vocabulary-and-data-driven-keymap`).
