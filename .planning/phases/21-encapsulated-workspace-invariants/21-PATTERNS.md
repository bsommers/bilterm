# Phase 21: Encapsulated Workspace Invariants - Pattern Map

**Mapped:** 2026-10-23
**Files analyzed:** `src/pane/{mod,modes,prompt_editor}.rs`, `src/wm/{mod,session}.rs`, `src/app.rs`, `src/input/{mod,keymap,modal,mouse}.rs`, `src/colon_command/execute.rs`, `src/api/{rpc,playbook}.rs`, `src/ui/{mod,modals,status_bar,renderer}.rs`

## Encapsulation Pattern Rules

1. **Zero Public Fields on Domain Aggregates (`CompositePane` & `WindowManager`):**
   - `CompositePane` (`src/pane/mod.rs`) and `WindowManager` (`src/wm/mod.rs`) declare all struct fields without `pub` or `pub(crate)`.
   - Child submodules (`src/pane/modes.rs`, `src/wm/session.rs`) are descendants of their parent `mod.rs` and therefore retain direct access to private fields via Rust's module visibility rules without exposing fields to sibling top-level modules (`app`, `api`, `input`, `colon_command`, `ui`).

2. **Borrow-Splitting Accessors Where Needed:**
   - When callers outside `pane/` or `wm/` read one sub-component while mutating another, provide targeted methods on the aggregate (e.g., `pane.export_history_to_temp_file(custom_path)`, `pane.submit_prompt()`, `wm.broadcast_command(&submitted)`, `wm.poll_all_ptys()`, `wm.drain_outbound_osc52()`) rather than exposing raw fields.

3. **800-Line Ceiling Preservation:**
   - Place `CompositePane` mode/field accessors in `src/pane/modes.rs` (currently 326 lines) and core lifecycle/submission methods in `src/pane/mod.rs` (currently 640 lines, reduced by deduplicating `new_headless`/`spawn` via `Self::new_base`).
   - Keep `src/wm/mod.rs` and `src/wm/session.rs` well under 800 lines by moving or placing `WindowManager` unit tests in `src/wm/tests.rs` (`#[cfg(test)] mod tests;`).
