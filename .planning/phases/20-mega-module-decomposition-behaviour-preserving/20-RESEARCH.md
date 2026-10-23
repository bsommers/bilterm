# Phase 20: Mega-Module Decomposition (behaviour-preserving) - Research

**Researched:** 2026-10-23
**Domain:** Rust module system, visibility (`pub(super)` / `pub(crate)`), split `impl` blocks, dependency cycle elimination
**Confidence:** HIGH (all line counts, symbol boundaries, and call sites verified directly in `src/` and `tests/`)

## Summary

Phase 20 implements **ARCH-R04** (`.planning/reviews/ARCHITECTURE_REVIEW_PLAN.md` §4 P1.4): splitting the mega-modules (`vt/mod.rs`, `pane/mod.rs`, `pane/colon_command.rs`, `input/mod.rs`, `api/mod.rs`, and the remaining >800-line modules `ui/mod.rs` and `wm/mod.rs`) along their existing functional seams so that every `.rs` file in `src/` has a single clear responsibility and is `<= 800` lines, the `pane ↔ wm` circular dependency is eliminated, and the entire test suite + `insta` snapshots pass without behavioral changes.

Rust's module privacy model makes splitting large structs (`VtParser`, `AutomationControlPlane`, `WindowManager`) across child modules zero-cost:
1. Multiple `impl StructName { ... }` blocks are permitted across submodules within the same crate.
2. Child submodules (`vt::scan`, `vt::graphics`, `vt::detect`, `vt::recording` under `vt/mod.rs`) automatically have access to private fields of `VtParser` defined in `vt/mod.rs` without widening field visibility outside `vt/`.
3. Re-exporting public types and functions (`pub use ...`) from each parent `mod.rs` preserves external caller paths across `src/` and `tests/` except where relocation is an explicit phase requirement (`crate::ui::cell_to_ratatui_style` and `crate::colon_command`).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ARCH-R04.1 | `vt/mod.rs` (2,716 LOC) split into `scan`, `graphics`, `detect`, `codec`, `recording`; `cell_to_ratatui_style` moves to `ui/` | Verified exact symbol boundaries in `src/vt/mod.rs`; `ratatui` import removed from `src/vt/mod.rs`; `scrollback.rs` (834 LOC) test module extracted to `scrollback_tests.rs` |
| ARCH-R04.2 | `pane/mod.rs` (1,665 LOC) split into `prompt_editor` (`FloatingPromptState`) and `modes` (copy/hints/search); `colon_command.rs` (1,501 LOC) moves out of `pane/` to `src/colon_command/` | `src/pane/colon_command.rs` is the sole importer of `crate::wm` inside `src/pane/`; moving it to `src/colon_command/` (`mod.rs`, `parse.rs`, `execute.rs`) eliminates the `pane ↔ wm` cycle |
| ARCH-R04.3 | `input/mod.rs` (2,262 LOC) split into `encode`, `mouse`, `modal`, `keymap`; `api/mod.rs` (3,124 LOC) split into `json`, `rpc`, `mcp`, `playbook`, `server` | All submodule boundaries verified to be `<= 780` LOC |
| ARCH-R04.4 | No source file exceeds ~800 lines, `pane ↔ wm` cycle is gone, full test suite + `insta` snapshots pass unchanged | Also split `src/ui/mod.rs` (2,026 LOC -> `modals.rs` + `status_bar.rs` + `mod.rs`) and `src/wm/mod.rs` (1,173 LOC -> `session.rs` + `mod.rs`) and extract inline tests in `history_store.rs` (850 LOC) so every `.rs` file in `src/` is strictly `<= 800` lines |
</phase_requirements>

## Current Line Counts (> 800 LOC in `src/`)

| File | Current LOC | Target Decomposition | Max Resulting File LOC |
|------|------------:|----------------------|-----------------------:|
| `src/api/mod.rs` | 3,124 | `json.rs` (~285), `playbook.rs` (~390), `mcp.rs` (~215), `rpc.rs` (~750), `server.rs` (~610), `tests.rs` (~650), `mod.rs` (~60) | ~750 |
| `src/vt/mod.rs` | 2,716 | `scan.rs` (~680), `graphics.rs` (~260), `detect.rs` (~280), `codec.rs` (~175), `recording.rs` (~85), `tests.rs` (~330), `mod.rs` (~620) | ~680 |
| `src/input/mod.rs` | 2,262 | `encode.rs` (~190), `mouse.rs` (~280), `modal.rs` (~460), `keymap.rs` (~520), `mod.rs` (~750) | ~750 |
| `src/ui/mod.rs` | 2,026 | `modals.rs` (~710), `status_bar.rs` (~305), `mod.rs` (~760, incl. `cell_to_ratatui_style`) | ~760 |
| `src/pane/mod.rs` | 1,665 | `prompt_editor.rs` (~700), `modes.rs` (~155), `mod.rs` (~750) | ~750 |
| `src/pane/colon_command.rs` | 1,501 | Moved to `src/colon_command/`: `mod.rs` (~265), `parse.rs` (~555), `execute.rs` (~700) | ~700 |
| `src/wm/mod.rs` | 1,173 | `session.rs` (~380, session save/restore + fleet/ssh helpers), `mod.rs` (~750) | ~750 |
| `src/pane/history_store.rs` | 850 | `history_store.rs` (~585) + `history_store_tests.rs` (~265) | ~585 |
| `src/vt/scrollback.rs` | 834 | `scrollback.rs` (~620) + `scrollback_tests.rs` (~215) | ~620 |

## Dependency Cycle Analysis (`pane ↔ wm`)

- Currently:
  - `src/wm/mod.rs`, `src/wm/split_tree.rs`, `src/wm/z_stack.rs`, `src/wm/quick_terminal.rs` import `crate::pane::{CompositePane, PaneId, PromptAnchor, history_store::HistorySettings}`.
  - `src/pane/colon_command.rs` imports `crate::wm::split_tree::SplitDirection` and `crate::wm::{WindowManager, WorkspaceLayoutMode}`.
  - No other file in `src/pane/` imports `crate::wm`.
- Resolution:
  - Move `src/pane/colon_command.rs` to `src/colon_command/` (`mod.rs`, `parse.rs`, `execute.rs`) and register `pub mod colon_command;` in `src/lib.rs`.
  - Update imports in `src/input/`, `src/api/`, `src/wm/`, and `tests/{v020_features,v030_features,full_suite_battle_and_perf}.rs` from `pane::colon_command` to `colon_command`.
  - Verification: `grep -rn "crate::wm\|crate::colon_command" src/pane/` returns 0 lines.
