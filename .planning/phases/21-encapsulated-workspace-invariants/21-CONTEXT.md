# Phase 21 — Context (User Decisions & Architectural Scope)

**Captured:** 2026-10-23 (from `/gsd-plan-phase 21` session)
**Sources:** `.planning/ROADMAP.md` Phase 21, `.planning/reviews/ARCHITECTURE_REVIEW_PLAN.md` §4 P1.2, `21-RESEARCH.md`

## Requirement ID mapping

- **ARCH-R02** — Encapsulated `WindowManager` and `CompositePane` invariants behind private fields, single `WindowManager::remove_pane` and `CompositePane::submit_command` methods, `enum Modal`, `enum Focus { Quick, Pane(PaneId) }` (no `PaneId 0` sentinel), and `leader_pending` in one place (`WindowManager`).

## Locked Decisions

- **D-01 Private struct fields on `CompositePane` and `WindowManager`:** Every field on `CompositePane` (`src/pane/mod.rs`) and `WindowManager` (`src/wm/mod.rs`) is private (zero `pub` fields). External modules (`app`, `api`, `input`, `colon_command`, `ui`, `main`, and `tests/`) interact with `CompositePane` and `WindowManager` strictly through methods.
- **D-02 Single `CompositePane::submit_command` and `submit_prompt`:** `CompositePane::submit_command(&mut self, cmd: &str) -> Result<()>` is the single source of truth for command execution (checking `read_only`, updating `prompt.running_command` and deduplicated `prompt.history`, resetting `vt` scrollback, recording OSC 133 command block submission, persisting to `history_store`, and writing `cmd + "\n"` to `pty`). `CompositePane::submit_prompt(&mut self) -> Result<String>` takes the editor buffer via `prompt.take_submission()` and delegates to `submit_command`. Prompt `Enter`, broadcast `Enter`, `pane.exec` JSON-RPC, and `exec_fleet_command` all route through `submit_command`.
- **D-03 Single `WindowManager::remove_pane(id)`:** `WindowManager::remove_pane(&mut self, id: PaneId) -> bool` is the single source of truth for removing a workspace pane (refusing if `panes.len() <= 1`, removing the leaf from `split_tree`, removing from `panes` and `z_stack`, clearing `zoomed_pane` if `Some(id)`, cleaning up fleet pane references in `fleet`, and rebalancing scrollback). Used by `close_focused_pane`, exit-reaping in `poll_all_ptys`, and `disconnect_fleet_node`.
- **D-04 `enum Modal` on `WindowManager`:** Replace the 9 independent modal fields (`help_visible`, `help_scroll`, `inspector_visible`, `inspector_scroll`, `palette_visible`, `palette_query`, `palette_selected`, `menubar_open_category`, `menubar_selected_item`) with a single mutually-exclusive `pub enum Modal { None, Help { scroll: usize }, Inspector { scroll: usize }, Palette { query: String, selected: usize }, Menubar { category: usize, item: usize } }`.
- **D-05 `enum Focus { Quick, Pane(PaneId) }` (no `PaneId 0` sentinel):** Introduce `pub enum Focus { Quick, Pane(PaneId) }`. `WindowManager::focus(&self) -> Focus` returns `Focus::Quick` when the Quake quick terminal is visible and `Focus::Pane(id)` otherwise. `WindowManager::active_pane_id(&self) -> PaneId` always returns the active workspace `PaneId` (`self.z_stack.top().unwrap_or(1)`), never `0`. `WindowManager::hit_test_pane` returns `Option<(Focus, Rect, Rect)>`.
- **D-06 Single `leader_pending` on `WindowManager`:** Remove the duplicate `leader_pending` field from `CompositePane` so `leader_pending` exists only on `WindowManager`.

## Carried constraints

- All `.rs` files in `src/` must remain `<= 800` lines (`ARCH-R04`).
- `src/pane/` must contain zero imports from `crate::wm` or `crate::colon_command`.
- Gate: `cargo test` + `cargo clippy --all-targets -- -D warnings`.
- Tests rewrite `docs/assets/screenshot-3278-inspector.svg` — revert before committing.
