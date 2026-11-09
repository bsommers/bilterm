---
phase: 63-keyboard-first-navigation-and-hotkeys
plan: 02
subsystem: input-ui
tags: [keys, window-mode, which-key, menu-bar, mnemonics, accelerators, alt-m-purge]
requires: [63-01]
provides:
  - WINDOW_MODE_BINDINGS / WinScope / WINDOW_MODE_FOOTER (src/input/window_mode_tables.rs)
  - window_mode_action(scope, key) and the Window-mode router (src/input/window_mode.rs)
  - which_key_rows / rows_for / footer_line / render_which_key_panel (src/ui/which_key_ui.rs)
  - MenuCategory / MenuItem model with mnemonics and computed hints (src/ui/menu_model.rs)
  - handle_menubar_key (src/input/menu_keys.rs)
  - WindowManager::{pin_which_key, which_key_pinned, which_key_visible}
affects: [63-03]
key-files:
  created:
    - src/input/window_mode.rs
    - src/input/window_mode_tables.rs
    - src/input/menu_keys.rs
    - src/ui/menu_model.rs
    - src/ui/which_key_ui.rs
    - src/wm/which_key_state.rs
    - tests/phase63_window_mode_and_menu.rs
    - tests/snapshots/phase63_window_mode_and_menu__*.snap (7)
  modified:
    - src/input/{mod,modal,modal_palette,keymap,keyprofile,keyhint_window}.rs
    - src/wm/{mod,key_mode}.rs
    - src/ui/{mod,modals,help_catalog,help_catalog_data}.rs
    - README.md, docs/README.md, docs/purpose.md, docs/launch-presentation-and-pitch-script.md
    - docs/architecture/adr/ADR-003-hybrid-split-tree-and-z-stack.md
    - docs/user-guide/{index,README,03,06,08,11}
    - tests/{phase51_agents_list_cmd,phase51_1_catalog,input_modal_guards_coverage}.rs
decisions:
  - "`?` in Window mode pins the which-key panel and is handled directly (view state, not a command); it has no table row"
  - "Window-mode mode-control rows (Esc/Enter/q exit, r/m/f sub-modes) live in the Focus scope and apply in every scope"
  - "Menu letters: item mnemonic first, then category mnemonic as fallback; Alt+letter or Shift+letter always address categories"
  - "New menu items are appended (existing tests pin indexes); dropdown width 30 -> 40 so the right-aligned accelerator fits"
  - "The which-key popup is a ratatui widget in the WorkspaceWidget buffer, which the GUI rasterises; no separate texture id"
metrics:
  completed: 2026-11-08
status: complete
commits: 3
plan_head_before: 428e1d6
plan_head_after: 382c7c16dda9f5a808a0c650c573f62a5a97e8e3
---

# Phase 63 Plan 02: Window mode, which-key overlay, keyboard menu bar

Sticky Window mode (every key lowers to an `Action` run through `WindowManager::apply`, raw-PTY safe), a which-key footer and popup generated from the live tables, and a fully keyboard-operable menu bar whose accelerators are computed per OS/surface/profile.

## What shipped

- **Window mode** (`window_mode_tables.rs`, `window_mode.rs`): `WINDOW_MODE_BINDINGS: &[(WinScope, KeyModifiers, KeyCode, Action)]` modelled on `NAVIGATOR_DRAWER_BINDINGS`. Focus scope: `h j k l`/arrows, `1..9`, `Tab/n`, `BackTab/p`, `z`, `s` (Stacked), `t` (Tiled), `Space` (cycle), `- "` / `| %` splits, `x`, `d/D`, `c/[`, `a`, `/`, `:`, `< > J K`; Resize and Move scopes map `h j k l`/arrows to `PaneResize`/`PaneMove` (leader step deltas); `r/m/f` switch sub-mode; `Esc/Enter/q` exit. Wired as one early branch in `modal.rs::handle_wm_modal_key` (before raw passthrough and the utility-window guards), so nothing reaches the child PTY; unknown keys are swallowed with `Window mode: unknown key, ? for help`. Actions that hand control away (`CopyModeToggle`, `PaletteToggle`, `Find`, `DrawerFocus`, `AgentsList`) leave the mode first. `[keys.window_mode]` additions are honoured (63-01 parsed them). Repeat: `is_wm_toggle_chord` treats only focus/resize/move steps as repeatable in Window mode. `handle_wm_key_event` is unchanged (59 lines).
- **Which-key** (`ui/which_key_ui.rs`, `wm/which_key_state.rs`): one generator (`rows_for`) reads the live leader rows or the Window-mode rows (aliases collapsed, shortest first, 4 max); `footer_line` builds the leader hint and the Window footer (`WINDOW [focus]  hjkl move  1-9 jump  z zoom  s stack  t tile  x close  ? help  Esc exit`) from `WINDOW_MODE_FOOTER`; the hand-written `LEADER_FOOTER` string of 63-01 is gone (`EffectiveKeymap::leader_status_line/leader_footer_rows` and `keyprofile::which_key_rows` now delegate). Popup shows after `keys.which_key_delay_ms` of idle, at once on `?`, never before when `keys.which_key = false`; it ends with the key mode. `window_mode_sequence` fills the 63-01 stub from `LEADER_OVERRIDES` + the table (`Stack Windows` hints `Ctrl+X w s`; the legacy profile, where leader `w` closes panes, does not advertise it).
- **Menu bar** (`ui/menu_model.rs`, `input/menu_keys.rs`, `ui/modals.rs`): data moved out of `modals.rs` into `MenuCategory { title, mnemonic, items: &[MenuItem { label, command, mnemonic }] }` with mnemonics `S W P V T X H D E`; labels carry no key text; helper signatures in `modals.rs` are unchanged and read the model. Keys: `Left/Right/Tab/BackTab` wrap over `MENUBAR_CATEGORIES.len()` (literal `7` removed), `Up/Down/Home/End`, `Enter`, `Esc`, letter = item mnemonic then category fallback, `Alt/Shift+letter` = category. The renderer underlines mnemonics and right-aligns `item_hint` (`⌘L`, `Ctrl+Shift+L`, `Ctrl+X M`, `Ctrl+X w s`). New items (appended): Window `Tile Windows`, `Toggle Stack/Tile`, `Window Mode`; Help `Keyboard Conflicts...` (`:keys check`).
- **Alt+M purge**: `help_catalog.rs`, `help_catalog_data.rs`, `README.md`, `docs/README.md`, `docs/purpose.md`, launch script, ADR-003, user-guide `index/README/03/06/08/11` no longer name `Alt+M` outside the new `#### Legacy (Alt) chords` subsection of chapter 6, which also holds the new "Keyboard-first navigation" section (Window mode table, menu keys, profiles, `[keys]`, Option-as-Meta).

## Verification

- `cargo test -q --test phase63_window_mode_and_menu --test phase63_key_profiles`: `test result: ok. 16 passed` (key profiles) and `test result: ok. 30 passed` (this plan)
- `scripts/verify.sh --quick`: `PASS 2/2 4s` (clippy `-D warnings` + unit; first cold run `PASS 2/2 29s`)
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `cargo test -q --tests --no-fail-fast`: exit 0, 137 `test result: ok`, none failed
- Gates: largest touched file `src/ui/mod.rs` 883 lines (all `src/**/*.rs` < 1180); `handle_wm_key_event` 59 lines, no growth; new `WindowManager` field private.

## Deviations from Plan

1. **[Rule 3 - environment]** The shared `/tmp` user quota was exhausted by concurrent sessions (EDQUOT even for tool output). Builds used `CARGO_TARGET_DIR=/home/bill/.cache/gsd-63-02/target` (a private directory outside `/tmp`) instead of `/tmp/bilterm-target-63-02`, plus `CARGO_INCREMENTAL=0`. The partial `/tmp/bilterm-target-63-02` was deleted first. No effect on results.
2. **[Rule 3 - existing tests pin old behaviour]** Three tests outside the plan list were updated, each for an intended consequence of this plan: `tests/phase51_agents_list_cmd.rs` (`appended` counts: Window 1 -> 4, Help 0 -> 1; first-N items and order are unchanged), `tests/phase51_1_catalog.rs` (dropdown width bound 30 -> `MENUBAR_DROPDOWN_WIDTH` = 40), `tests/input_modal_guards_coverage.rs` (Left wraps to the last of 9 categories not index 6; `x` is now the Tmux mnemonic so the "unmapped key" probe is `!`).
3. **[Rule 2 - new private state]** The which-key idle timer and pin need state: new `src/wm/which_key_state.rs` plus one field/init line/`mod` line in `src/wm/mod.rs`, and `reset_which_key`/`touch_which_key` calls in the 63-01 setters of `src/wm/key_mode.rs` (no signature change).
4. **[Scope]** Small 63-01-file edits: `keyprofile.rs` (delegation of the footer/status/rows to `which_key_ui`, and the Window-mode hint gated on the profile having a leader row for `WindowMode(Enter)`), `keymap.rs` (Window-mode early return in `is_wm_toggle_chord`), `keyhint_window.rs` (stub filled), `input/mod.rs` (3 one-line `mod` additions: `menu_keys`, `window_mode`, `window_mode_tables`).
5. **[Not done as written]** (a) No separate GUI card texture / texture-id constant: the GUI rasterises the `WorkspaceWidget` cell buffer, and `session_map_gui.rs` (the suggested pattern) is not wired to any renderer, so the ratatui popup is the single TUI+GUI path. `src/ui/renderer.rs` is untouched. (b) The utility-window `captures_input()` guards in `modal.rs` were not switched to `key_prefix_active()`: the Window-mode branch returns before them, so they can never see Window mode, and they keep `is_leader_pending()`.
6. **[Docs beyond the list]** `docs/user-guide/index.md`, `docs/user-guide/README.md` and `docs/purpose.md` also named `Alt+M` and were updated so the scan test holds for all of `docs/**/*.md`.
7. **[Design note]** `?` is handled in the router (pins the popup) instead of lowering to an `Action`: it is ephemeral view state with no colon/RPC twin. All table keys lower to `Action`s and are parity-tested against `apply`.
8. Commits group Tasks 1 and 2 (the router calls the footer builder, so they do not compile apart); Task 3 and a follow-up test fix are separate.

## Known Stubs

None. (`KeysOp::Check` host/consumer findings remain 63-03's.)

## Threat Flags

None. No new input surface beyond key routing; `[keys.window_mode]` entries come from the already-capped (512 entries / 64 chars) `KeysConfig`.

## Self-Check: PASSED

- Created files exist; commits `0cb8fe3`, `589c7cb`, `382c7c1` are ancestors of HEAD.
