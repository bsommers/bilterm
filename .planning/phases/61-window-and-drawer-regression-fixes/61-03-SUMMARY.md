# Plan 61-03 Summary: Per-Window State Preservation Across Focus and Reflow (`FIX-03`)

## Status
Completed.

## Root Causes Fixed (`FIX-03`)
1. **Focusing click executed OIA & prompt-chrome actions (`src/input/mouse.rs`)**:
   - Computed `was_active` before `wm.focus_and_raise(hit_id)` in `handle_wm_mouse_event`.
   - When `!was_active` and a `LeftDown` lands on the pane's OIA status bar (`on_pane_status_bar`) or prompt top border (`on_prompt_chrome_row`), the click now focuses/raises the window and returns `Ok(AppAction::Handled)` without cycling prompt mode/anchor or starting a prompt drag.
   - Canvas clicks, title-bar badges, right-click context menu, and scroll wheel remain pass-through.
2. **Copy mode cleared on focus change (`src/wm/z_stack.rs`, `src/wm/mod.rs`)**:
   - Removed `clear_inactive_transient_copy_cursors` and its call sites in `focus_and_raise`, `shuffle_deck_next`, and `shuffle_deck_prev` so inactive panes retain their Vi copy mode state across focus changes.
3. **Regular Terminal (raw passthrough) mode lost across snapshot/restore (`src/wm/session.rs`, `src/wm/layout_profile.rs`, `src/wm/window_manage_bulk.rs`)**:
   - Added `#[serde(default, skip_serializing_if = "std::ops::Not::not")] pub raw_passthrough: bool` to `SavedPaneState`.
   - Populated `raw_passthrough: p.prompt().manual_raw_override` in `snapshot_session` and `capture_topology_undo`, and restored `manual_raw_override` / `force_command_prompt` in `apply_saved_session_snapshot`.
4. **Scrollback offset reset on column reflow (`src/vt/mod.rs`)**:
   - Captured `prev_offset = self.scrollback_offset()` before reflow/rebuild in `VtParser::set_size` (when not on alternate screen) and restored via `self.set_scrollback_offset(prev_offset)`.

## Files Modified
- `src/input/mouse.rs`
- `src/wm/mod.rs`
- `src/wm/z_stack.rs`
- `src/wm/session.rs`
- `src/wm/layout_profile.rs`
- `src/wm/window_manage_bulk.rs`
- `src/vt/mod.rs`
- `tests/phase61_window_state_preserved.rs`
