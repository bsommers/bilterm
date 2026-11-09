---
phase: 63-keyboard-first-navigation-and-hotkeys
plan: 03
subsystem: input
tags: [keys, key-conflicts, reserved-chords, rpc, mcp, write-gate, parity]
requires: [63-01]
provides:
  - "keyconflict::{check, check_config, validate_user_bindings, enforce_load_policy, load_time_status, explain}"
  - "RESERVED_HOST / RESERVED_CONSUMERS / RESERVED_INTERNAL / ACCEPTED_DEFAULT_SHADOWS data tables"
  - ":keys check and :keys explain backed by the engine"
  - "keys.list|check|explain|profile|window_mode and menu.open RPC; bilterm_keys MCP tool"
affects: [63-02]
key-files:
  created:
    - src/input/keyconflict.rs
    - src/input/keyconflict_data.rs
    - src/colon_command/execute_keys.rs
    - src/api/rpc_keys.rs
    - tests/phase63_keys_surface_parity.rs
  modified:
    - src/input/mod.rs
    - src/input/keyprofile_ops.rs
    - src/colon_command/mod.rs
    - src/api/{mod,rpc,rpc_dispatch,mcp}.rs
    - src/main.rs
    - src/config/dotted.rs
    - src/wm/mod.rs
    - docs/API.md
    - docs/user-guide/10-security-and-best-practices.md
    - docs/user-guide/07-troubleshooting-and-faq.md
    - tests/{api_mcp_resources,phase50_1_api,phase51_1_api,phase51_api_parity,v110_v120_v130_automation_api_mcp_playbooks,api_error_paths,phase63_key_profiles}.rs
decisions:
  - "Finding/Severity ordering is Block < Warn < Info so a plain sort puts blockers first; output sorted by (severity, chord, class, consumer, source)"
  - "Reserved matching keeps Shift significant (Ctrl+Shift+W is not Ctrl+W) because Primary on Windows/Linux is Ctrl+Shift; letters are case-folded only"
  - "Shipped default chords that clash with a consumer are demoted to Info with 'accepted: <reason>' from ACCEPTED_DEFAULT_SHADOWS; an unlisted default Warn or any default Block fails the unit test"
  - "Linux Block list is limited to chords every desktop consumes (Alt+F4, Alt+Tab, Alt+F2, Ctrl+Alt+F1-F7, Ctrl+Alt+Delete, Super+*); Alt+F7/F8, Ctrl+Alt+T/L/D/Arrows are Warn because Ctrl+Alt+L / Ctrl+Alt+Arrows are shipped launcher/resize rows and desktop-dependent"
  - "Write gate = WindowManager::config_write_denied extended to Keys(Profile(Some)), WindowMode, MenuOpen; applies to wm.command too"
  - "Load failure resets [keys] and [keybindings] to defaults (leader_key and the rest of the config kept)"
metrics:
  completed: 2026-11-08
status: complete
commits: 3
plan_head_before: 428e1d6e82dba2e8a35fd37156de73455eaa2276
plan_head_after: 4ee8908331282d2a578f0f8edf69d7157fdf1d01
---

# Phase 63 Plan 03: Key-conflict engine and keyboard-surface parity

One engine (`keyconflict`) now answers `:keys check`, `keys.check`, `bilterm_keys check` and the config-load check, over three classes: internal (duplicates, user bind shadowing the leader key or `Ctrl+Q`, overrides), host (OS/desktop/terminal-host Block and Warn per `HostOs` and `Surface`) and consumer (readline, emacs, vim, tmux, screen, zellij, herdr, Claude Code, IME, tty).

## What shipped

- **Data** (`keyconflict_data.rs`, 295 lines): host rows with per-row consumer and note; consumer sets with CITED/ASSUMED source tags mirroring 63-RESEARCH; `Pattern::{Chord, AnySuper, CtrlAltLetter, AltPrintable}` wildcards.
- **Engine** (`keyconflict.rs`, 583 lines): `check(effective, env, &KeysConfig)` per plan; `check_config(.., &BiltermConfig)` adds `[keybindings]` and `leader_key`; `validate_user_bindings(&BiltermConfig, &KeyEnv)` (error text `key binding 'Alt+F4' is reserved by Windows (close window)`); `enforce_load_policy`, `load_time_status` (`N key conflicts: :keys check`), `revalidate_after_set`, `explain`.
- **Surfaces**: `execute_keys.rs` holds the `Check`/`Explain` bodies, `keyprofile_ops::run_keys_op` delegates to it (replacing the 63-01 internal-only `Check`); `rpc_keys.rs` lowers each method to the same `Action` and returns the JSON the colon path stored in `command_data`; `bilterm_keys` maps `list|check|explain|profile|window_mode|menu_open`.
- **Load hook** (`main.rs`): Block-class user binding -> `config error: ... (using default keys)`; Warn-class -> status `N key conflicts: :keys check` plus `tracing::warn!` per finding. `:set` re-validates through one line in `dotted::try_apply`.
- **Tests** (14, `phase63_keys_surface_parity`): window-mode / menu-open parity across leader, Primary, colon, palette, RPC, MCP; `:keys check` equals RPC equals MCP equals engine; per-OS Block table; load policy; write gate; caps; copy-mode contract; shipped defaults clean on 3 OS x 2 surfaces x kitty on/off.
- **Docs**: API reference (`keys.*`, `menu.open`, `bilterm_keys`, gate), security chapter section 7, troubleshooting entries (Option symbols on macOS, AltGr on Windows, Ctrl+Shift+letter).

## Deviations from Plan

1. **[Rule 3] Gate implementation.** The plan said "existing write gate"; the only gate is `config_write_denied` (config writes only). Extended it in `src/wm/mod.rs` (file outside the plan list) so `keys.profile <name>`, `keys.window_mode`, `menu.open` and their `wm.command` forms honour `[api] allow_config_write`. Name-less `keys.profile` stays read-only.
2. **[Rule 1] Existing assertions updated.** Five tests hard-coded 24 MCP tools (`api_error_paths`, `phase50_1_api`, `phase51_1_api`, `phase51_api_parity`, `v110_v120_v130_...`) and `api_mcp_resources` listed tool names: now 25 with `bilterm_keys` appended last. `phase63_key_profiles::keys_ops_list_check_explain_and_profile_switch` asserted the 63-01 stub text `no internal conflicts`; it now asserts `no conflicts`.
3. **Linux Block list narrowed** (see decisions): the plan listed `Ctrl+Alt+T/L/D` and `Alt+F7/F8` as Block, but `Ctrl+Alt+L` (PaneMove) is a shipped row and the plan also requires zero Block findings on shipped profiles. They are Warn (accepted under `Ctrl+Alt+*`).
4. **Bodies live in two places**: `Keys` arm stays `keyprofile_ops::run_keys_op` (frozen apply seam from 63-01); only `Check`/`Explain` moved to `colon_command/execute_keys.rs`.
5. **`keys.window_mode`/`menu.open` return** `{ok, key_mode, menu_open}` (no colon-path JSON exists for them).
6. **Menu / Window-mode parts of parity are partly vacuous on this branch**: the menu-source scan (`menu_items_for_the_keyboard_surface_lower_to_known_actions`) reads `src/ui/modals.rs` and `src/ui/menu_model.rs` if present and finds no keyboard-surface items until 63-02 adds "Help -> Keyboard Conflicts" and a Window-mode item; the Window-mode `[`/`c` assertion in `copy_mode_entry_contract_for_phase_62` is conditional on `window_mode_sequence` returning `Some` (63-02). Re-run after merge.
7. Verification environment: the `/tmp` per-user quota was exhausted mid-run and the target dir was wiped; rebuilt with `CARGO_PROFILE_{DEV,TEST}_DEBUG=0` and `CARGO_INCREMENTAL=0` (results unaffected).

## Verification

- `cargo test -q --test phase63_keys_surface_parity --test phase63_key_profiles`: `ok. 16 passed` (63-01) and `ok. 14 passed` (63-03)
- `scripts/verify.sh --quick`: `PASS 2/2 2s`
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `cargo test -q --tests --no-fail-fast`: exit 0 (every integration test); `--lib`: 375 passed
- `cargo clippy -q --all-targets -- -D warnings`: clean
- Line gates: max `src/**/*.rs` = `src/action.rs` 1158; new files 72-583 lines.

## Known Stubs

None added. (63-01 stubs `window_mode_sequence`, `which_key_rows` remain 63-02's.)

## Threat Flags

None new. New surface is read-only RPC plus three gated mutating RPCs; config caps unchanged (512 entries / 64 chars) and covered by `chord_caps`.

## Self-Check: PASSED

- Created files exist; commits `c08be03`, `2f70ab9`, `4ee8908` are ancestors of HEAD.
