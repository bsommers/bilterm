# Phase 61 Plan 01: Classic Drawer Two-Line Rows Summary

**One-liner:** Added `classic_row_lines` configuration (`0` auto, `1` single-line, `2` two-line) to Classic drawer layout metrics, unified `:drawer rows 1|2` across Navigator and Classic styles, and exposed `[R:n]` header chip, focused drawer `1`/`2` keys, and View menu entries.

## Tasks Completed

| Task | Description | Files Modified |
|------|-------------|----------------|
| 1 | Add `classic_row_lines` config field and style-aware `set_drawer_row_lines` | `src/wm/drawer.rs`, `src/wm/drawer/nav.rs` |
| 2 | Honor `classic_row_lines_mode()` in `drawer_vertical_layout_metrics` | `src/ui/drawer.rs` |
| 3 | Add `ClassicHeaderChipKind` / `classic_header_chips` `[R:n]` chip, focused drawer keys `1`/`2`, and View menu items | `src/ui/drawer.rs`, `src/input/mouse_wm.rs`, `src/input/modal.rs`, `src/ui/modals.rs` |
| 4 | Update `:drawer` usage, help/palette strings, and user-guide documentation | `src/colon_command/parse_visual.rs`, `src/ui/help_catalog_data.rs`, `src/ui/help_catalog.rs`, `docs/user-guide/04-themes-glyphs-and-config.md`, `docs/user-guide/11-navigator-sidebar-and-agent-traffic.md` |
| 5 | Add `tests/phase61_drawer_rows.rs` covering config, setter, layout step, rendering, mouse hit-testing/wheel, and parity | `tests/phase61_drawer_rows.rs` |

## Verification

- `CARGO_TARGET_DIR=/tmp/bilterm-target-61-01 cargo clippy -q --all-targets -- -D warnings`: PASS
- `CARGO_TARGET_DIR=/tmp/bilterm-target-61-01 cargo test -q --test phase61_drawer_rows`: PASS (8/8 tests)
- `CARGO_TARGET_DIR=/tmp/bilterm-target-61-01 bash scripts/check_classic_goldens.sh`: PASS (11/11 golden snapshots preserved)
