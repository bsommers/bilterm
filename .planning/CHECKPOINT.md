# Session Checkpoint (2026-10-30)

**Status:** COMPLETE — all requested features & fixes verified (`clippy`, 194 unit tests, 12 integration tests, release binary installed to `~/.local/bin/bilterm`).

## Completed Work
1. **Software Cursor Blinking** ([`src/config/mod.rs`](file:///home/bill/src/ai/bilterm/src/config/mod.rs), [`src/ui/cursor.rs`](file:///home/bill/src/ai/bilterm/src/ui/cursor.rs), [`src/ui/cursor_cmd.rs`](file:///home/bill/src/ai/bilterm/src/ui/cursor_cmd.rs), [`src/wm/mod.rs`](file:///home/bill/src/ai/bilterm/src/wm/mod.rs), [`src/input/mod.rs`](file:///home/bill/src/ai/bilterm/src/input/mod.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs)):
   - Implemented software cursor blink phase tracking (`cursor_blink_hidden`, `last_blink_toggle`, `tick_cursor_blink`, `reset_cursor_blink`) at 500ms (`Blink`) and 250ms (`RapidBlink`) intervals across FloatingPrompt, RawPtyPassthrough, and CopyMode.
2. **Mouse Text Selection Alignment (Normal & Scaled Windows)** ([`src/pane/mod.rs`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs), [`src/wm/mod.rs`](file:///home/bill/src/ai/bilterm/src/wm/mod.rs), [`src/wm/z_stack.rs`](file:///home/bill/src/ai/bilterm/src/wm/z_stack.rs), [`src/ui/typography.rs`](file:///home/bill/src/ai/bilterm/src/ui/typography.rs), [`src/ui/renderer.rs`](file:///home/bill/src/ai/bilterm/src/ui/renderer.rs), [`src/ui/gui_font/linux_x11.rs`](file:///home/bill/src/ai/bilterm/src/ui/gui_font/linux_x11.rs)):
   - Unified Copy-Mode floating-prompt dodge inside `CompositePane::prompt_rect`.
   - Aligned `resize_all` / `current_workspace_area` with top menu bar (`top_off`) and bottom DOS F-key bar (`bot_off`).
   - Added `tui_cell_grid_mode`, `mouse_pixel_hint`, and `translate_pane_mouse_event` for accurate scaled-pane hit-testing in both TUI and X11 GUI modes.
3. **Borland Top Menu Bar Mouse Alignment, Pull-Down Menus & Clickable Help** ([`src/ui/modals.rs`](file:///home/bill/src/ai/bilterm/src/ui/modals.rs), [`src/input/mouse.rs`](file:///home/bill/src/ai/bilterm/src/input/mouse.rs), [`src/input/mouse_wm.rs`](file:///home/bill/src/ai/bilterm/src/input/mouse_wm.rs), [`src/input/modal.rs`](file:///home/bill/src/ai/bilterm/src/input/modal.rs)):
   - Replaced hardcoded menu bar column ranges with exact title-width helpers (`menubar_category_at_col`, `menubar_category_x_offset`, `menubar_dropdown_rect`, `menubar_item_command`).
   - Wired mouse hover and left-click on top menu bar categories (including `" Help "` at category 6), pull-down menu items, `<F1>` Help modal rows, and `<Ctrl+P>` Command Palette rows.
4. **Session Creation, Naming/Renaming, Reordering & CLI Switches** ([`src/wm/session_manager.rs`](file:///home/bill/src/ai/bilterm/src/wm/session_manager.rs), [`src/wm/drawer.rs`](file:///home/bill/src/ai/bilterm/src/wm/drawer.rs), [`src/colon_command/parse.rs`](file:///home/bill/src/ai/bilterm/src/colon_command/parse.rs), [`src/main.rs`](file:///home/bill/src/ai/bilterm/src/main.rs), [`src/ui/help_catalog.rs`](file:///home/bill/src/ai/bilterm/src/ui/help_catalog.rs)):
   - Added `:session new [name]`, `:new-session [name]`, `:session blank [name]`, `:session rename [old] <new>`, `:rename-session <new>`, `:session switch <name>`, `:session move [name] <up|down|top|bottom|1..N>`, `:session reorder <s1> <s2>...`, `:session swap <a > <b>`, and `:session delete <name>`.
   - Added `<F7>` Drawer shortcuts `N` (new session) and `Shift+Up`/`Alt+Up`/`K`, `Shift+Down`/`Alt+Down`/`J` (move selected session up/down).
   - Added CLI subcommands and switches `bilterm new [SESSION]`, `bilterm -n/--new-session [SESSION]`, `bilterm blank [SESSION]`, and `bilterm -B/--blank/--blank-session/--clean`.

## Verification Baseline
- All `src/**/*.rs` files `<= 800` lines (`ARCH-R04`).
- `cargo clippy -q --all-targets -- -D warnings` — 0 warnings.
- `cargo test -q --lib` — 194/194 passed.
- `cargo test -q --test slide_out_drawer_and_telemetry --test full_suite_battle_and_perf` — 12/12 passed.
- `cargo build -q --release && cp target/release/bilterm ~/.local/bin/bilterm` — installed `bilterm 2.2.0`.
