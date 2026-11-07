# Session Checkpoint — 2026-11-06

## Completed Work
1. **Milestone Completion & Release (`v3.0.0`)**:
   - Completed Phases 51.1, 51.2, 52, 53, 54, and 55 (`1 -> 55` all complete).
   - Built, verified, and published GitHub Release `v3.0.0` with all 14 cross-platform archives + `SHA256SUMS`.
2. **Linux `<F7>` & Function/Special Key Routing Fixes**:
   - Fixed `<F7>` asymmetry in `ALT_SCREEN` / `RawPtyPassthrough` (`src/input/keymap.rs`) so plain `<F7>` both opens and closes the drawer under `RawFkeyMode::Auto` and `Bilterm`, and `Alt+F7` always toggles the drawer even during `BlockPage` hold.
   - Fixed `WindowManager::toggle_drawer()` (`src/wm/drawer.rs`) and `apply_config` (`src/wm/session.rs`) so `<F7>` closes `DrawerVisibilityMode::Fixed` and `AutoHide` hovered drawers on the first press.
   - Added `WindowManager::blur_utility_windows()` and bypassed `<F7>` / `Alt+F7` in `src/input/modal.rs` so focused utility windows (`Fonts`, `Commands`, `ScriptStudio`, `AgentsList`, `SessionMap`, `Agents`, `McpConsole`) never swallow `<F7>`.
   - Fixed focused drawer (`Classic` and `Navigator`) in `src/input/modal.rs` so `F1..F12` and `Ctrl+` / `Alt+` global chords (`F1` Help, `F2` Split, `F3` Hints, `F4` Mode, `F6` Deck, `F8` Copy, `F9` Inspector, `F10` Quake, `F12` Raw, `Ctrl+P` Palette, `Alt+M` Layout, `Alt+1..9`, etc.) pass through instead of being swallowed or shadowed by single-letter drawer keys.
   - Fixed Modern bottom F-key bar mouse click hit-testing (`src/input/mouse.rs`) for `F7 Drawer` (`34..=46`) and `F12 Raw` (`60..=68`), and ensured clicking top-bar pane pills blurs drawer focus.
   - Fixed Linux X11 GUI key/mouse translation (`src/ui/gui_font/linux_x11.rs` & `src/app.rs`): preserved uppercase on `Alt+Shift+<letter>`, added keypad navigation keysyms (`0xFF80`, `0xFF89`, `0xFF95..=0xFF9F`), ignored `CapsLock`/`NumLock` on `F1..F12`, accepted `KeyEventKind::Repeat`, and deduplicated `MOTION_NOTIFY` per cell when not dragging.
3. **Script Studio (`:script` / `:studio`) Window Controls Fixes**:
   - Fixed title-bar badge rendering & hit-testing (`src/ui/script_studio_ui.rs`, `src/input/script_studio.rs`): moved `[•]` Pin badge to `r_edge - 11..-8` so it no longer overwrites `[□]` Dock (`r_edge - 8..-5`), and aligned `[X]` Close (`r_edge - 5..-1`) so clicking `X` closes instead of docking.
   - Compacted `TOOLBAR_TEXT` (`63` cols, 1-cell glyphs) so all 6 buttons (`Run`, `Step`, `Reset`, `Rec`, `Edit`, `Save`) fit inside the default 66-col window and match exact click hitboxes and `[● Rec]` / `[✎ Edit]` highlight offsets.
   - Fixed `FOOTER_TEXT` (`"F5:Run F10/n:Step r:Reset R:Rec i:Edit x:Del o:New s:Save *:Pin"`) and made footer items clickable.
   - Made clicking an editor line position `cursor_col` and enter `[EDIT]` mode on second click, cleared `docked = false` on `ScriptStudioOp::SetRect` so dragging/resizing a docked Script Studio window works, and stripped `SHIFT` on `KeyCode::Char(_)` in nav mode (`src/input/script_studio_keys.rs`).
4. **Verification**:
   - Added `tests/linux_fkeys_and_key_routing.rs` (9 comprehensive integration tests).
   - `bash scripts/verify.sh` -> `PASS 119/119`.
   - `bash scripts/check_classic_goldens.sh` -> `GOLDENS OK 11`.
