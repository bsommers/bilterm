# Phase 21 Plan 02 Summary — Encapsulate `WindowManager`, Introduce `Modal` & `Focus`, and Unify `remove_pane`

- **Plan:** `21-02-PLAN.md` (`ARCH-R02`)
- **Status:** Completed

## Changes Delivered

1. **Private `WindowManager` Struct Fields (`ARCH-R02.1`):**
   - Made all fields on `WindowManager` private in `src/wm/mod.rs`.
   - Added typed read/write methods (`panes`, `pane`, `pane_mut`, `pane_count`, `layout_mode`, `set_layout_mode`, `split_tree`, `z_stack`, `zoomed_pane`, `quick_terminal`, `quick_terminal_mut`, `is_quick_terminal_visible`, `config`, `config_mut`, `modal`, `modal_mut`, `close_modal`, `is_help_visible`, `help_scroll`, `is_inspector_visible`, `inspector_scroll`, `set_inspector_visible`, `is_palette_visible`, `palette_state`, `open_palette_with_query`, `menubar_open_category`, `menubar_state`, `set_menubar_open_category`, `is_broadcast_input`, `is_leader_pending`, `set_leader_pending`, `fleet`, `fleet_mut`, `scrollback_mut`).
   - Extracted inline `WindowManager` unit tests into `src/wm/tests.rs` and moved palette/theme/history helpers into `src/wm/session.rs` so every `src/**/*.rs` file stays well below 800 lines.

2. **`Modal` & `Focus` Enums (`ARCH-R02.4`):**
   - Replaced the 9 boolean/scroll/query modal fields on `WindowManager` with a single mutually-exclusive `Modal` enum (`None`, `Help { scroll }`, `Inspector { scroll }`, `Palette { query, selected }`, `Menubar { category, item }`).
   - Introduced `Focus { Quick, Pane(PaneId) }` and removed the `PaneId 0` sentinel from `active_pane_id()`, `hit_test_pane()`, and `ScrollbackGovernor` (`mark_quick_seen()`).

3. **Unified `WindowManager::remove_pane` (`ARCH-R02.2`):**
   - Implemented `WindowManager::remove_pane(&mut self, target_id: PaneId) -> Result<bool>` that removes the pane from `panes`, `split_tree`, `z_stack`, clears `zoomed_pane` if `Some(target_id)`, and removes fleet node references (`FleetRegistry::remove_pane_references`).
   - Routed `close_focused`, `poll_all_ptys` exit-reaping, and `disconnect_fleet_node` (`:remote disconnect` & RPC `"fleet.disconnect"`) through `remove_pane`.

## Verification

- `cargo test`: All unit and integration tests pass.
- `cargo clippy --all-targets -- -D warnings`: Zero warnings.
- `test $(awk '/^pub struct WindowManager/,/^\}/' src/wm/mod.rs | grep -c "pub") -eq 1`: Passed.
- `awk 'ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' $(find src -name '*.rs')`: Passed (`src/wm/mod.rs` is 715 lines, `src/wm/session.rs` is 730 lines).
