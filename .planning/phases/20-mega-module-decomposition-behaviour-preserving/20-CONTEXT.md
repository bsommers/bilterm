# Phase 20 — Context (User Decisions & Architectural Scope)

**Captured:** 2026-10-23 (from `/gsd-plan-phase 20` session)
**Sources:** `.planning/ROADMAP.md` Phase 20, `.planning/reviews/ARCHITECTURE_REVIEW_PLAN.md` §4 P1.4, `20-RESEARCH.md`

## Requirement ID mapping

- **ARCH-R04** — Mega-module decomposition along existing seams (`vt/`, `pane/`, `input/`, `api/`, plus moving `colon_command` out of `pane/` to eliminate the `pane ↔ wm` cycle, moving `cell_to_ratatui_style` to `ui/`, and ensuring no source file in `src/` exceeds ~800 lines).

## Locked Decisions

- **D-01 Behaviour-preserving mechanical split:** All splits preserve existing function signatures, behavior, and public re-exports (`pub use ...` at each module root `mod.rs`) except for the two explicit architectural relocations mandated by Success Criteria 1 & 2 (`cell_to_ratatui_style` moved from `crate::vt` to `crate::ui`, and `colon_command` moved from `crate::pane::colon_command` to top-level `crate::colon_command`).
- **D-02 Eliminate `pane ↔ wm` circular dependency (without re-introducing a transitive cycle):** `colon_command` moves to `src/colon_command/` (`pub mod colon_command;` in `src/lib.rs`) and is NOT re-exported from `src/pane/mod.rs`. All callers in `src/` and `tests/` import `crate::colon_command` / `bilterm::colon_command`. `src/pane/` must contain zero imports from `crate::wm` or `crate::colon_command`.
- **D-03 Decouple `src/vt/mod.rs` from `ratatui`:** `cell_to_ratatui_style` and `vt_color_to_ratatui` move to `src/ui/` (exported at `crate::ui::cell_to_ratatui_style`), removing `use ratatui::...` from `src/vt/mod.rs`.
- **D-04 Strict 800-line ceiling across `src/**/*.rs`:** Every `.rs` file under `src/` must be `<= 800` lines (`wc -l`). In addition to decomposing `vt/mod.rs`, `pane/mod.rs`, `pane/colon_command.rs`, `input/mod.rs`, and `api/mod.rs`, extract `ui/modals.rs` + `ui/status_bar.rs` from `src/ui/mod.rs` (2,026 lines), `wm/session.rs` from `src/wm/mod.rs` (1,173 lines), and split inline test modules where needed (`vt/scrollback_tests.rs`, `pane/history_store_tests.rs`) so zero files in `src/` exceed 800 lines.
- **D-05 Zero snapshot or test regressions:** All 138 unit tests, all 10 integration test targets, and all `insta` snapshots must pass unchanged with `cargo clippy --all-targets -- -D warnings` clean.

## Carried constraints

- Gate: `cargo test` + `cargo clippy --all-targets -- -D warnings`.
- Tests rewrite `docs/assets/screenshot-3278-inspector.svg` — revert before committing.
