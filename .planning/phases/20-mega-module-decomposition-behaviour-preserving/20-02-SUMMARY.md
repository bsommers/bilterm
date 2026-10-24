# Phase 20 Plan 02: Pane Decomposition & ColonCommand Relocation Summary

**Decomposed `src/pane/mod.rs` (1,665 lines) into `prompt_editor.rs` and `modes.rs`, extracted `history_store_tests.rs`, and moved `colon_command` out of `src/pane/` into top-level `src/colon_command/{mod,parse,execute}.rs` to break the `pane ↔ wm` circular dependency.**

## Accomplishments

- Extracted `src/pane/prompt_editor.rs` (711 lines) containing `PromptAnchor`, `PromptMode`, `EditorSnapshot`, `FloatingPromptState`, and `PromptMouseDrag`.
- Extracted `src/pane/modes.rs` (326 lines) containing `CanvasSearchState`, `CopySelectionKind`, `CopyModeState`, `HintsModeState`, and the `CompositePane` search/copy/hints/read-only mode methods.
- Reduced `src/pane/mod.rs` from 1,665 lines to 640 lines while preserving public re-exports.
- Extracted `src/pane/history_store_tests.rs` (264 lines) from `src/pane/history_store.rs` (now 586 lines).
- Moved `src/pane/colon_command.rs` (1,501 lines) out of `src/pane/` into top-level `src/colon_command/`:
  - `src/colon_command/mod.rs` (262 lines): `ColonCommand` enum, module re-exports, and unit tests.
  - `src/colon_command/parse.rs` (556 lines): `parse_colon_command`.
  - `src/colon_command/execute.rs` (704 lines): `execute_colon_command` and `execute_wm_colon_command`.
- Eliminated the `pane ↔ wm` dependency cycle (`grep -rn "crate::wm\|crate::colon_command" src/pane/` returns 0 matches).

## Files Created/Modified

- `src/pane/mod.rs`
- `src/pane/prompt_editor.rs`
- `src/pane/modes.rs`
- `src/pane/history_store.rs`
- `src/pane/history_store_tests.rs`
- `src/colon_command/mod.rs`
- `src/colon_command/parse.rs`
- `src/colon_command/execute.rs`
- `src/lib.rs`
- `src/input/mod.rs`
- `src/api/mod.rs`
- `src/wm/mod.rs`
- `tests/v020_features.rs`
- `tests/v030_features.rs`
- `tests/full_suite_battle_and_perf.rs`

## Verification

- `cargo test --lib pane::` (19 passed)
- `cargo test --lib colon_command::` (5 passed)
- `cargo test --test v020_features --test v030_features --test full_suite_battle_and_perf` (all passed)
- `cargo clippy --all-targets -- -D warnings` (0 warnings)
- `awk 'ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' src/pane/*.rs src/colon_command/*.rs` (exited 0)
