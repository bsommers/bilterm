# ADR-006: Unified `Action` Command Vocabulary, Encapsulated Workspace Invariants & Whole-Workspace `.cast` Compositor Recorder

- **Status:** Accepted (`Phases 19–22`, `v1.7.0`)
- **Date:** 2026-10-24
- **Requirements:** `SEC-R01..04`, `ARCH-R01`, `ARCH-R02`, `ARCH-R04`, `REC-02`

## Context

As Bilterm grew from a single-pane IBM 3270 / Borland Turbo Vision prototype into a multi-pane multiplexer with JSON-RPC 2.0 / MCP automation, TOML playbooks, remote SSH/fleet federation, and session recording, four structural challenges emerged:
1. **Multi-Surface Dispatch Duplication (`ARCH-R01`)**: Keyboard chords (`src/input/keymap.rs`), `:colon` commands (`src/colon_command/`), JSON-RPC `"wm.command"` (`src/api/rpc.rs`), and TOML playbooks (`src/api/playbook.rs`) had overlapping dispatch logic.
2. **Exposed Struct Fields & Invariant Drift (`ARCH-R02`)**: Public fields on `WindowManager` and `CompositePane` allowed callers to close/reap panes or submit commands through slightly different paths (e.g., reaping an exited zoomed pane or using `PaneId 0` as a focus sentinel for the Guake quick terminal).
3. **`asciinema` PTY-Wrapper & Dual-Leader Ergonomics (`REC-02`)**:
   - Users coming from `tmux` / `herdr` expect `Ctrl+B` (`"`, `%`, `n`, `p`, `z`), while classic DOS/Borland users and nested `$TMUX` sessions need `Ctrl+X`.
   - Running Bilterm inside or alongside `asciinema rec` can trigger false-positive `!ECHO` line-discipline detection on the outer PTY, and users holding `Ctrl` across a leader sequence (`Ctrl+X Ctrl+<key>`) previously hit the raw-mode double-tap passthrough branch instead of the leader command.
   - Single-pane `.cast` recording (`VtParser`) only captured raw child PTY bytes for one pane rather than the full multi-pane split tree, z-order stack, floating prompts, Borland menus, and 3270 cross-hairs.

## Decisions

1. **Mega-Module Decomposition & `<= 800` Line Ceiling (`ARCH-R04`)**:
   - Decomposed `vt/`, `pane/`, `input/`, `api/`, `ui/`, and `wm/` into focused single-responsibility files (`48` files under `src/`), moving `cell_to_ratatui_style` into `src/ui/mod.rs` and `colon_command` into `src/colon_command/{mod,parse,execute}.rs` to eliminate the `pane ↔ wm` dependency cycle.
2. **Encapsulated `WindowManager` & `CompositePane` Invariants (`ARCH-R02`)**:
   - Made all fields on `WindowManager` and `CompositePane` private.
   - Consolidated all pane removals (`:close`, child process exit reaping, fleet disconnect) into `WindowManager::remove_pane(id)` and all command submissions (`Enter`, broadcast, RPC `pane.exec`) into `CompositePane::submit_command(cmd)`.
   - Replaced boolean modal flags with an exclusive `enum Modal` and replaced `PaneId 0` focus sentinels with `enum Focus { Quick, Pane(PaneId) }`.
3. **Unified `Action` Enum & Declarative Keymap Tables (`ARCH-R01`)**:
   - Created `crate::action::Action` (`src/action.rs`) as the single command vocabulary and `WindowManager::apply(Action)` (`src/wm/mod.rs`) as the single executor.
   - Replaced procedural keymap `match` blocks with `DEFAULT_LEADER_BINDINGS`, `DEFAULT_GLOBAL_BINDINGS`, and `DEFAULT_PANE_BINDINGS` tables in `src/input/keymap.rs`, reducing `handle_wm_key_event` to 46 lines.
   - Implemented `is_leader_key_with_tmux`: accepts both `Ctrl+B` and `Ctrl+X` when standalone, and automatically falls back to `Ctrl+X` when `$TMUX` is detected.
   - Added held-`Ctrl` leader chord normalization (`Ctrl+X Ctrl+<key>` / `Ctrl+B Ctrl+<key>`) and `FloatingPromptState::force_command_prompt` so explicit prompt toggles (`F4`, `F12`, `Ctrl+G`, `Ctrl+X e/t/b/s`) override false-positive `!ECHO` from `asciinema rec` while `CompositePane::is_active_canvas_password_prompt` preserves `[sudo] password` protection.
4. **Whole-Workspace Asciicast v2 (`.cast`) Compositor Frame Recorder (`REC-02`)**:
   - Implemented `WorkspaceRecorder` (`src/vt/recording.rs`) hooked into `WorkspaceWidget::render` (`src/ui/mod.rs`), diffing successive `ratatui::buffer::Buffer` frames into compact `CUP` + SGR ANSI sequences written as standard asciicast v2 `.cast` files (`0600` permissions in the per-user `0700` runtime directory).

## Consequences

- Every `Action` variant is verified by the compile-time exhaustive table test `every_action_is_reachable_from_keymap_colon_or_rpc` in `src/action.rs`.
- Multi-pane splits, z-order stacks, floating command prompts, and modals can be recorded directly inside Bilterm via `:record start [path]` / `:record workspace [path]` / `Ctrl+X R` and replayed with standard `asciinema play` or `agg`.
