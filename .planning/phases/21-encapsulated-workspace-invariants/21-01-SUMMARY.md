---
phase: 21-encapsulated-workspace-invariants
plan: 01
subsystem: pane
tags: [encapsulation, composite-pane, submit-command, invariants]
provides:
  - "CompositePane with all 17 fields private (leader_pending removed)"
  - "Unified CompositePane::submit_command and CompositePane::submit_prompt"
  - "Method-only access across app, api, colon_command, input, ui, wm, tmux, and tests"
affects: [pane, input, colon_command, api, ui, wm, tmux]
key-files:
  modified:
    - src/pane/mod.rs
    - src/pane/modes.rs
    - src/input/mod.rs
    - src/api/rpc.rs
    - src/wm/session.rs
key-decisions:
  - "Deduplicated CompositePane::new_headless and CompositePane::spawn via private CompositePane::new_base"
  - "Removed duplicate CompositePane::leader_pending field; WindowManager owns leader state"
  - "Centralized command execution in CompositePane::submit_command so prompt Enter, broadcast Enter, pane.exec, and exec_fleet_command share identical read-only, history, running-banner, OSC 133, and PTY/VT invariants"
requirements-completed: [ARCH-R02]
completed: 2026-10-24
---

# Phase 21 Plan 01: Encapsulate CompositePane Summary

**Encapsulated all `CompositePane` struct fields behind private visibility, removed duplicate `leader_pending`, and unified all 4 command submission paths into `CompositePane::submit_command`.**

## Accomplishments
- Removed `pub` from all fields of `CompositePane` in `src/pane/mod.rs` and deleted the dead `leader_pending` field.
- Added `CompositePane::submit_command(&mut self, cmd: &str)` and `CompositePane::submit_prompt(&mut self)` in `src/pane/mod.rs`, wiring prompt `Enter` (`src/input/mod.rs`), broadcast `Enter` (`src/input/mod.rs`), `pane.exec` (`src/api/rpc.rs`), and `exec_fleet_command` (`src/wm/session.rs`) through the single method.
- Added clean accessors and mutators in `src/pane/modes.rs` (`export_history_to_temp_file`, `take_all_outbound_osc52`, `flush_query_responses`, etc.) and migrated all callers across `src/` and `tests/`.
