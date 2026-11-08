---
phase: 63-keyboard-first-navigation-and-hotkeys
plan: 01
subsystem: input
tags: [keys, keymap, profiles, primary-modifier, macos-option, altgr, kitty-protocol, window-mode-state]
requires: []
provides:
  - KeyEnv / HostOs / Surface / primary_mods (platform-aware Primary modifier)
  - KeyChord parse/format/display/from_event (one chord grammar)
  - KeyProfile / EffectiveKeymap / keys_for_action / hint_for_action (per-OS profiles)
  - Action::WindowMode(WindowModeOp), Action::MenuOpen{category}, Action::Keys(KeysOp) + colon forms
  - KeyMode state machine (Normal / LeaderPending / Window(sub)) on WindowManager
  - "[keys] config table (KeysConfig)"
affects: [63-02, 63-03, 62]
tech-stack:
  added: []
  patterns:
    - "effective keymap is a cheap value that filters the static legacy tables (no table copies)"
    - "KeyEnv injected, never read from cfg! in routing"
key-files:
  created:
    - src/input/keyenv.rs
    - src/input/chord.rs
    - src/input/keyprofile.rs
    - src/input/keyprofile_tables.rs
    - src/input/keyprofile_ops.rs
    - src/input/keyhint_window.rs
    - src/config/keys.rs
    - src/action_keys.rs
    - src/wm/key_mode.rs
    - src/colon_command/parse_keys.rs
    - tests/phase63_key_profiles.rs
  modified:
    - src/action.rs
    - src/input/mod.rs
    - src/input/keymap.rs
    - src/pane/mod.rs
    - src/wm/mod.rs
    - src/wm/drawer.rs
    - src/error.rs
    - src/main.rs
    - src/ui/gui_macos.rs
    - src/ui/gui_window.rs
    - src/ui/gui_font/linux_x11.rs
    - src/config/mod.rs
    - src/pane/completion.rs
    - src/ui/help_catalog.rs
    - src/ui/help_catalog_data.rs
    - src/colon_command/{mod,parse,execute_wm}.rs
decisions:
  - "leader w = Window mode in non-legacy profiles (W still ClosePane; legacy keeps w = ClosePane, m = PromptModeSet)"
  - "legacy_alt = auto keeps the Alt+<printable> layer on Windows/Linux for one release; mac and portable never have it"
  - "Ctrl+G raw toggle restricted to the command prompt outside legacy; Ctrl+_ / Ctrl+\\x1f FontZoomOut dropped outside legacy"
  - "headless WindowManager defaults to the legacy profile and KeyEnv::headless() (Linux, TUI, no kitty)"
  - "Ctrl+Alt+<letter> launcher rows stay on every profile except portable (they are the only keyboard path for several windows and are not compose chords)"
  - "bare :menu / :menu on|off remain the pre-existing menu-bar visibility toggle; only :menu open [category] is new"
metrics:
  duration: single session
  completed: 2026-11-08
status: complete
commits: 3
plan_head_before: 17a0ae370797a82d18cbf3840716645a68f4152d
plan_head_after: 9ebea211d3ea18b4b93cf54a9f51c6a3be839665
---

# Phase 63 Plan 01: Platform-aware key model, per-OS profiles, frozen Action surface

Platform-aware `Primary` modifier (`Cmd` on macOS GUI, `Ctrl+Shift` on Windows/Linux GUI and kitty TUIs, none on legacy TUIs), five key profiles (`mac`, `windows`, `linux`, `portable`, bit-for-bit `legacy`), the Alt+M replacement (`Primary+L`, leader `M`, Window mode), the macOS-Option / AltGr / normalizer fixes, and the frozen `Action::{WindowMode, MenuOpen, Keys}` surface for plans 63-02 and 63-03.

## What shipped

- **KeyEnv / Primary** (`src/input/keyenv.rs`): `primary_mods` returns `SUPER` (macOS GUI), `CONTROL|SHIFT` (Win/Linux GUI, kitty TUI), `None` (legacy TUI). The kitty flag is OR-ed in from a process flag set by `TerminalRestoreGuard` when it pushed `DISAMBIGUATE_ESCAPE_CODES`.
- **KeyChord** (`src/input/chord.rs`): one parser/formatter; `keymap::format_key_chord` delegates to it, so `[keybindings]` can now bind arrows, Home/End, PageUp/Down, Insert/Delete, Backspace.
- **Profiles** (`src/input/keyprofile*.rs`): `EffectiveKeymap` filters the unchanged `DEFAULT_*_BINDINGS` tables per profile: `mac` drops every `Alt+<printable>` row (global and pane), adds `Cmd+Option+Arrows`; `windows`/`linux` keep the Alt layer while `legacy_alt != off`; `portable` drops all Alt rows and has no Primary; all non-legacy profiles drop `NON_LEGACY_REMOVED` (Alt+Shift focus aliases, Alt+Tab, Ctrl+_, Ctrl+Space, Alt+Space, Alt+`). Leader `w` -> Window mode, leader `m` -> menu bar outside legacy.
- **Router** (`src/input/mod.rs`, `keymap.rs`): `handle_wm_key_event` is 59 lines (limit 80). One new call resolves `reserved_global_action(...).or_else(primary_action)` before the raw-passthrough branch; the leader hint string is generated from the live leader table (`leader_status_line`).
- **Fixes**: normalizer gated by `KeyEnv` (identity off macOS TUI); Ctrl+G raw toggle limited to CommandPrompt outside legacy; macOS GUI delivers composed text for Option+key unless prompt-editing or bound (`OptionPolicy`) and delivers unbound Cmd chords as SUPER events; Win32 `is_altgr` keeps AltGr as text; X11 maps `Ctrl+Shift+!..(`, `{ } :` to base keys and drops Super chords; kitty keyboard protocol pushed/popped by `TerminalRestoreGuard::enter_host_tui_with`.
- **Surface**: `:wmode [on|off|toggle|focus|resize|move]`, `:menu open [category]`, `:keys [list|check|explain <chord>|profile [name]]`; palette, F1 help, completion rows; `every_action_is_reachable_from_keymap_colon_or_rpc` samples added.

## API FROZEN (63-02 and 63-03 may not change these signatures)

All paths are `bilterm::...` (crate `bilterm`).

### Actions and operands (`src/action_keys.rs`, re-exported from `bilterm::action`)

```rust
// Action (src/action.rs) - three new variants, colon_form_or_summary() handles all
Action::WindowMode(WindowModeOp)
Action::MenuOpen { category: Option<usize> }
Action::Keys(KeysOp)

pub enum WinSub { Focus /*default*/, Resize, Move }          // Copy, Default, Hash, Serialize; fn name(self)->&'static str
pub enum WindowModeOp { Enter, Exit, Toggle, Sub(WinSub) }   // Copy; fn colon_form(&self)->String
pub enum KeysOp { List, Check, Explain(String), Profile(Option<String>) }  // fn colon_form(&self)->String
```
Colon forms: `:wmode` (Toggle), `:wmode on|off|focus|resize|move`, `:menu open [N]`, `:keys list|check|explain <chord>|profile [name]` (parser: `bilterm::colon_command::parse_keys::parse_keys_command`). `apply` arms: `WindowMode` mutates `KeyMode` and sets a status line; `MenuOpen` sets `show_top_menubar = true` and calls `set_menubar_open_category(Some(c.unwrap_or(0)))` (category is stored unclamped; the menu renderer/handler clamp); `Keys` runs `bilterm::input::keyprofile_ops::run_keys_op` (pub(crate)), writes JSON via `set_command_data` and the status line. `KeysOp::Check` is internal-duplicates only: 63-03 replaces the `Check` arm body in `keyprofile_ops.rs`, signature unchanged.

### Key mode state (`src/wm/key_mode.rs`; field private on `WindowManager`)

```rust
pub enum KeyMode { Normal, LeaderPending, Window(WinSub) }   // Copy, Default=Normal; re-exported as bilterm::wm::KeyMode
impl KeyMode { pub const fn is_window(self)->bool; pub const fn is_leader_pending(self)->bool; pub const fn window_sub(self)->Option<WinSub>; }
impl WindowManager {
    pub fn key_mode(&self) -> KeyMode;
    pub fn key_prefix_active(&self) -> bool;          // LeaderPending || Window
    pub fn enter_window_mode(&mut self);              // Window(Focus)
    pub fn exit_window_mode(&mut self);               // only leaves Window; never clobbers LeaderPending
    pub fn set_window_sub(&mut self, sub: WinSub);    // enters Window mode if inactive
    pub fn is_leader_pending(&self) -> bool;          // kept accessor
    pub fn set_leader_pending(&mut self, pending: bool); // false never clobbers Window mode
    pub fn key_env(&self) -> KeyEnv;                  // surface derived from gui_surface, kitty OR-ed from the terminal guard
    pub fn set_key_env(&mut self, env: KeyEnv);       // takes ONE KeyEnv value (build with KeyEnv::new(os, surface))
    pub fn set_key_profile(&mut self, name: KeyProfileName);   // headless tests / same as :keys profile <name>
    pub fn keymap(&self) -> EffectiveKeymap;
    pub fn hint_for_action(&self, action: &Action) -> Option<String>;   // honours config.leader_key
    pub fn keys_for_action(&self, action: &Action) -> Vec<KeyChord>;
}
```
**Hook for 63-02**: 63-01 does NOT route keys while `KeyMode::Window` is active (that is `handle_window_mode_key` in 63-02, wired in `modal.rs::handle_wm_modal_key`). Until then `:wmode off` is the exit. `wm.key_prefix_active()` is the helper the utility-window guards should use.

### Environment (`src/input/keyenv.rs`)

```rust
pub enum HostOs { MacOs, Windows, Linux }                    // detect() const fn, name()
pub enum Surface { Tui, Gui }
pub struct KeyEnv { pub os: HostOs, pub surface: Surface, pub kitty_kb: bool, pub inside_tmux: bool }  // Copy
impl KeyEnv { pub const fn new(os, surface)->Self; pub const fn with_kitty(self, bool)->Self;
              pub const fn headless()->Self /* Linux, Tui */; pub fn detect()->Self; }
pub fn primary_mods(env: &KeyEnv) -> Option<KeyModifiers>;
pub fn set_kitty_active(on: bool); pub fn kitty_active() -> bool;
```

### Chords (`src/input/chord.rs`)

```rust
pub struct KeyChord { pub mods: KeyModifiers, pub primary: bool, pub code: KeyCode }   // Copy, Eq, Hash
impl KeyChord {
    pub const fn new(mods, code)->Self; pub const fn primary(code)->Self; pub fn raw(KeyEvent)->Self;
    pub fn parse(&str) -> Result<Self, String>;                 // <=64 chars, rejects unknown tokens
    pub fn format(&self, env: &KeyEnv) -> String;               // "Primary+L", "Cmd+Alt+Left" (Primary stays symbolic)
    pub fn display(&self, env: &KeyEnv) -> String;              // "⌘L" (macOS GUI), "Ctrl+Shift+L", "Primary+L" if unavailable
    pub fn from_event(KeyEvent, &KeyEnv) -> Self;               // base-key + Primary folding, Shift dropped on letters
    pub fn matches_primary(&self, env: &KeyEnv) -> bool;
    pub fn folded(&self) -> Self;
}
pub const MAX_CHORD_LEN: usize = 64;  pub fn base_key(c: char) -> char;
```
Grammar: `Ctrl|Control Alt|Option|Opt Shift Cmd|Super Primary` + char, `F1..F24`, `Space Enter Tab BackTab Esc Left Right Up Down Home End PageUp PageDown Insert Delete Backspace`.

### Profiles (`src/input/keyprofile.rs`, `keyprofile_tables.rs`; config in `bilterm::config::keys`)

```rust
pub enum KeyProfileName { Auto, Mac, Windows, Linux, Portable, Legacy }   // ALL, as_str(), parse(&str)
pub enum KittyProtocol { Auto, Off }  pub enum LegacyAlt { Auto, On, Off }
pub struct KeysConfig { profile, kitty_protocol, legacy_alt, which_key: bool /*true*/, which_key_delay_ms: u32 /*400*/,
                        bind: BTreeMap<String,String>, window_mode: BTreeMap<String,String>, unbind: Vec<String> }
impl KeysConfig { pub fn validate(&self) -> Result<(), BiltermError> }    // BiltermConfig.keys; caps 512 entries / 64 chars
pub const MAX_BIND_ENTRIES: usize = 512;

pub struct KeyProfile;  impl KeyProfile {
    pub fn resolve(cfg: &KeysConfig, env: &KeyEnv) -> EffectiveKeymap;
    pub fn concrete(name: KeyProfileName, os: HostOs) -> KeyProfileName;   // Auto -> mac|windows|linux
}
pub struct EffectiveKeymap { /* Clone, Eq */ }
impl EffectiveKeymap {
    pub fn legacy_default() -> Self;  pub fn profile(&self)->KeyProfileName;  pub fn env(&self)->KeyEnv;
    pub fn alt_layer(&self)->bool;  pub fn primary_enabled(&self)->bool;  pub fn focus_arrow_mods(&self)->Option<KeyModifiers>;
    pub fn global_rows(&self) -> impl Iterator<Item=(KeyModifiers, KeyCode, &'static Action)> + Clone + '_;
    pub fn pane_rows(&self)   -> impl Iterator<Item=(KeyModifiers, KeyCode, &'static Action)> + Clone + '_;
    pub fn leader_rows(&self) -> Vec<(KeyCode, Action)>;
    pub fn global_action(&self, key: KeyEvent, fuzzy_search_active: bool) -> Option<Action>;
    pub fn pane_action(&self, key: KeyEvent) -> Option<Action>;
    pub fn leader_action(&self, key: KeyEvent) -> Option<Action>;
    pub fn primary_action(&self, key: KeyEvent) -> Option<Action>;
    pub fn keys_for_action(&self, action: &Action) -> Vec<KeyChord>;          // Primary first
    pub fn leader_keys_for_action(&self, action: &Action) -> Vec<KeyCode>;
    pub fn hint_for_action(&self, action: &Action) -> Option<String>;        // Primary > leader "Ctrl+X M" > Window mode > other chord
    pub fn hint_for_action_with_leader(&self, action: &Action, leader_label: &str) -> Option<String>;
    pub fn unexpected_duplicates(&self) -> Vec<DuplicateChord>;               // internal integrity (63-03 builds on this)
    pub fn option_chord_wanted(&self, key: KeyEvent) -> bool;                 // macOS GUI Option policy
    pub fn leader_footer_rows(&self) -> Vec<WhichKeyRow>;  pub fn leader_status_line(&self, leader_label: &str) -> String;
}
pub fn hint_for_action(action: &Action, env: &KeyEnv) -> Option<String>;      // auto profile, default config, "Ctrl+X" leader
pub fn compact_hint(hint: &str) -> String;                                    // "Ctrl+X M" -> "^X M"
pub fn leader_key_label(code: KeyCode) -> String;
pub struct WhichKeyRow { pub chord: String, pub label: String, pub group: &'static str }
pub fn which_key_rows(mode: &KeyMode, env: &KeyEnv) -> Vec<WhichKeyRow>;      // STUB: leader rows only
// tables: PRIMARY_BINDINGS, PROMPT_EDIT_ALT_KEYS, MAC_EXTRA, NON_LEGACY_REMOVED, LEADER_OVERRIDES, ALLOWED_SHADOWS: &[(&str chord, &str reason)]
```
Extension seam for 63-02: `bilterm::input::keyhint_window::window_mode_sequence(&Action) -> Option<String>` currently returns `None`; 63-02 replaces its body from `WINDOW_MODE_BINDINGS` so `hint_for_action(Stack Windows)` yields `Ctrl+X w s`. `which_key_rows` is a stub with the final row shape; 63-02 supersedes it from `ui/which_key_ui.rs` (and should switch `EffectiveKeymap::leader_status_line` over to it).
Public legacy API unchanged: `default_global_action`, `default_pane_action`, `default_leader_action`, `DEFAULT_*_BINDINGS` (`normalize_macos_option_key` gained a third `&KeyEnv` parameter). `reserved_global_action(&EffectiveKeymap, key)` is `pub(super)`.

### Pane / GUI seams
`CompositePane::keymap()/set_keymap(EffectiveKeymap)` (WM pushes the active keymap in `dispatch_pane_key`); `ui::gui_macos::{OptionPolicy, translate_macos_key_event(.., &OptionPolicy)}`; `ui::gui_window::is_altgr(ctrl, lalt, ralt)`; `TerminalRestoreGuard::enter_host_tui_with(KittyProtocol)`.

## Decisions and recorded open-decision defaults

- Leader `w` = Window mode in non-legacy profiles; `W` stays ClosePane; legacy keeps `w` = ClosePane and `m` = PromptModeSet.
- `legacy_alt = auto` = on (Windows/Linux keep the Alt layer one release); `mac` and `portable` never have it.
- Ctrl+G raw toggle only at the command prompt outside legacy; `Ctrl+_` / `\x1f` FontZoomOut removed outside legacy.
- Headless WM defaults to `legacy` and `KeyEnv::headless()` (Linux, Tui).
- Hard-coded pane Alt hotkeys (`Alt+e` prompt-mode cycle, `Alt+r`/`Alt+c` raw copy, `Alt+p` drawer preview) follow `alt_layer()`; prompt-editing Alt keys (`b f d y a`, Backspace, Delete, arrows, Enter) stay everywhere.

## Deviations from Plan

1. **[Rule 1/3 - test adaptation]** `normalize_macos_option_key` is now identity off a macOS TUI, so the two existing tests that type Option symbols through a headless WM (`src/input/tests.rs::macos_option_unicode_normalization_...`, `tests/session_lessons_and_regression_suite.rs::lesson_1_...`) now call `wm.set_key_env(KeyEnv::new(MacOs, Tui))` first. No other existing test changed.
2. **[Rule 3 - name clash]** `:menu` already existed as an alias of `:menubar` (visibility toggle). `:menu open [category]` is claimed by the new parser only when `parts[1] == "open"`; bare `:menu`/`:menu on|off` are untouched and the existing `:menu` completion row now lists `open`.
3. **[Scope]** Files outside the plan's list were touched where required: `src/pane/mod.rs` (per-pane keymap field), `src/wm/drawer.rs` (one `leader_pending` field reference), `src/lib.rs` (`action_keys` module), `src/main.rs` (one line: `enter_host_tui_with(config.keys.kitty_protocol)`; 63-03 owns the load hook in a different region of the file). `command_registry.rs` needed no edit (palette rows supply the summaries).
4. **`src/action.rs`** grew to 1158 lines (limit 1180): operand types live in the new `src/action_keys.rs`.
5. `[keys.unbind]` is a plain `unbind = [..]` list (as the plan states), not the research's `chords` table. `keys.window_mode` and `keys.which_key*` are parsed and validated but consumed by 63-02.
6. `Ctrl+Alt+<letter>` launcher rows are kept on `mac` (research was ambiguous); `test mac_profile_has_no_alt_printable_wm_chord` defines "Alt+printable" as Alt without Ctrl/Super.
7. Commits are grouped by dependency rather than strictly per plan task because Task 2's tables reference the Task 3 `Action` variants: (1) KeyEnv/KeyChord/config, (2) profiles + fixes + Action/KeyMode surface, (3) colon/catalog/tests. Only the final state was built and tested; intermediate commits were not individually compile-checked.

## Verification

- `cargo test -q --test phase63_key_profiles`: `running 16 tests ... test result: ok. 16 passed; 0 failed`
- `scripts/verify.sh --quick`: `PASS 2/2 18s`
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `--test linux_fkeys_and_key_routing --test terminal_torture_and_stress --test session_lessons_and_regression_suite` (ARCH-R01/R04 gates): all pass (9, 16, 7, 6 tests)
- `cargo test -q --tests` (every integration test file) and `--lib` (367 unit tests): exit 0. `cargo clippy -D warnings` clean (part of `--quick`).
- Line gates: max `src/**/*.rs` = `src/action.rs` 1158; `handle_wm_key_event` = 59 lines.
- Not verifiable on this Linux host: the Win32 (`cfg(windows)`) edits in `gui_window.rs` compile only on Windows (the pure `is_altgr`/VK helpers and their tests do run here); macOS Cocoa runner call site is `cfg(target_os = "macos")` (the pure translator and `OptionPolicy` are tested here); the live kitty push needs a TTY (compile + pure paths covered). Real-device macOS/Windows validation remains UAT.

## Known Stubs

- `src/input/keyhint_window.rs::window_mode_sequence` returns `None` (63-02 fills it).
- `src/input/keyprofile.rs::which_key_rows` returns leader rows only (63-02 supersedes).
- `KeysOp::Check` reports internal duplicates only (63-03 adds host/consumer findings and `keyconflict`).
- Window mode has state and entry points but no key router until 63-02.

## Threat Flags

None. Config caps (512 entries, 64-char chords) are enforced at deserialize and in `KeysConfig::validate`; `:keys profile` is a session-only config mutation (persist via `:config save`).

## Self-Check: PASSED

- All files listed under `key-files.created` exist; commits `0ae5c8b`, `56a0f38`, `9ebea21` are ancestors of HEAD.
