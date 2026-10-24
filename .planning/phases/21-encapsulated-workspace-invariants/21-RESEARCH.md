# Phase 21: Encapsulated Workspace Invariants - Research

**Researched:** 2026-10-23
**Domain:** Domain aggregate encapsulation, invariant enforcement, state-machine sum types (`Modal`, `Focus`)
**Confidence:** HIGH (all struct definitions, field accesses, and test assertions inspected across `src/` and `tests/`)

## Summary

Phase 21 implements **ARCH-R02** (`.planning/reviews/ARCHITECTURE_REVIEW_PLAN.md` §4 P1.2): encapsulating `WindowManager` and `CompositePane` invariants behind private fields, consolidating duplicated pane-removal and command-submission logic into single authoritative methods (`WindowManager::remove_pane` and `CompositePane::submit_command`), replacing boolean modal flags with `enum Modal`, replacing the `PaneId 0` quick-terminal sentinel with `enum Focus { Quick, Pane(PaneId) }`, and removing the duplicate `leader_pending` field from `CompositePane`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ARCH-R02.1 | `WindowManager` and `CompositePane` fields are private; `app`, `api`, and `input` use methods only | Audited all 24 fields of `WindowManager` and 18 fields of `CompositePane` across `src/` and `tests/`; designed complete accessor/mutator surface on both types |
| ARCH-R02.2 | Single `WindowManager::remove_pane` serves close, exit-reaping, and fleet removal (zoom/z-stack/tree cleanup covered by tests, including reaping a zoomed pane) | Identified 3 divergent removal paths (`close_focused_pane`, `App::poll_all_ptys`, `FleetRegistry::disconnect`); unified under `WindowManager::remove_pane(id)` + `FleetRegistry::remove_pane_references(id)` |
| ARCH-R02.3 | Single `CompositePane::submit_command` serves the prompt and `pane.exec` | Identified 4 command submission sites (`handle_command_prompt_key`, broadcast `Enter` in `handle_wm_key_event`, `pane.exec` in `src/api/rpc.rs`, and `exec_fleet_command` in `src/wm/session.rs`); unified under `CompositePane::submit_command` and `CompositePane::submit_prompt` |
| ARCH-R02.4 | Modal state is one `enum Modal`, focus is `enum Focus { Quick, Pane(PaneId) }` (no `PaneId 0` sentinel), and `leader_pending` exists in one place | Verified `pane.leader_pending` has zero external callers; designed `enum Modal` and `enum Focus` replacing all 9 modal fields and `PaneId 0` sentinel checks in `wm`, `input/mouse.rs`, and `vt/scrollback.rs` |
</phase_requirements>

## Invariant Consolidation Analysis

### 1. Pane Removal (`WindowManager::remove_pane`)
Previously:
- `WindowManager::close_focused_pane` (`src/wm/mod.rs:377`) removed from `split_tree`, `panes`, `z_stack`, cleared `zoomed_pane`, and called `rebalance_scrollback`, but did not clean up `fleet`.
- `App::poll_all_ptys` (`src/app.rs:94-98`) mutated `wm.panes`, `wm.split_tree`, and `wm.z_stack` directly, skipping `zoomed_pane` cleanup, `fleet` cleanup, and `rebalance_scrollback` (and calling `refresh_line_discipline()` redundantly after `poll_pty_events()`).
- `FleetRegistry::disconnect` (`src/remote/mod.rs:308`) removed the node from `fleet.nodes` without closing its associated `SshPane` / `SshTmux` workspace pane(s).

Resolution:
- `WindowManager::remove_pane(&mut self, target_id: PaneId) -> bool` performs all 6 steps atomically:
  1. Guard `self.panes.len() > 1`
  2. `self.split_tree.remove_leaf(target_id)`
  3. `self.panes.remove(&target_id)`
  4. `self.z_stack.remove(target_id)`
  5. `if self.zoomed_pane == Some(target_id) { self.zoomed_pane = None; }`
  6. `self.fleet.remove_pane_references(target_id)` + `self.rebalance_scrollback(Instant::now())`
- `WindowManager::close_focused_pane`, `WindowManager::poll_all_ptys` (delegated from `App::poll_all_ptys`), and `WindowManager::disconnect_fleet_node` all call `self.remove_pane(id)`.

### 2. Command Submission (`CompositePane::submit_command`)
Previously:
- Prompt `Enter` (`src/input/mod.rs:248-265`), broadcast `Enter` (`src/input/mod.rs:442-463`), `pane.exec` (`src/api/rpc.rs:308-324`), and `exec_fleet_command` (`src/wm/session.rs:196-209`) each manually mutated `prompt.history`, `history_store`, `prompt.running_command`, `vt.reset_scrollback()`, `vt.record_command_submission()`, and `pty.write_all()`.

Resolution:
- `CompositePane::submit_command(&mut self, cmd: &str) -> Result<()>` checks `self.read_only`, updates `self.prompt.running_command` and deduplicated `self.prompt.history`, calls `self.vt.reset_scrollback()`, `self.vt.record_command_submission(cmd)`, `let _ = self.history_store.record_command(cmd)`, and writes `cmd + "\n"` to `self.pty`.
- `CompositePane::submit_prompt(&mut self) -> Result<String>` extracts the current prompt text via `self.prompt.take_submission()` and delegates to `self.submit_command(&submitted)`.

### 3. `enum Modal` & `enum Focus`
- `Modal` replaces 9 fields on `WindowManager` with a single `modal: Modal` field (`Modal::None`, `Modal::Help { scroll }`, `Modal::Inspector { scroll }`, `Modal::Palette { query, selected }`, `Modal::Menubar { category, item }`), making simultaneous conflicting modals unrepresentable.
- `Focus` (`Focus::Quick` | `Focus::Pane(PaneId)`) replaces the `PaneId 0` sentinel in `WindowManager::focus()`, `WindowManager::hit_test_pane()`, and `handle_wm_mouse_event()`. `ScrollbackGovernor` tracks `quick_seen: bool` separately from `seen: BTreeSet<PaneId>` so `0` is never inserted as a fake `PaneId`.
