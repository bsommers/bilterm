# Phase 22 Plan 02 Summary — Declarative Keymap Tables, Dual Leader (`Ctrl+B` + `Ctrl+X`), `tmux`/`herdr` Chords, Held-`Ctrl` Normalization & `asciinema` Prompt Override

- **Phase / Plan:** `22-02`
- **Requirements:** `ARCH-R01`, `REC-02`
- **Status:** COMPLETE

## Key Changes

1. **Declarative Keymap Tables (`ARCH-R01.2`):**
   - Replaced procedural `match` trees in [`src/input/keymap.rs`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs) with [`DEFAULT_LEADER_BINDINGS`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs), [`DEFAULT_GLOBAL_BINDINGS`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs), and [`DEFAULT_PANE_BINDINGS`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs), plus lookup functions [`default_leader_action`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs), [`default_global_action`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs), and [`default_pane_action`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs) dispatched via [`WindowManager::apply`](file:///home/bill/src/ai/bilterm/src/wm/mod.rs).
   - Simplified [`handle_wm_key_event`](file:///home/bill/src/ai/bilterm/src/input/mod.rs) in [`src/input/mod.rs`](file:///home/bill/src/ai/bilterm/src/input/mod.rs) to 46 lines (`<= 80` line budget).

2. **Dual-Prefix Leader & `tmux`/`herdr` Chord Parity (`ARCH-R01.2`, `REC-02`):**
   - Added [`is_leader_key_with_tmux`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs) accepting both `Ctrl+B` and `Ctrl+X` when standalone (`!inside_tmux`) and falling back to `Ctrl+X` when `$TMUX` is detected so `Ctrl+B` passes cleanly to outer `tmux`.
   - Added `"` (`SplitHorizontal`), `%` (`SplitVertical`), `n` (`DeckNext`), `p` (`DeckPrev`), `H` (`HistoryPopupToggle`), and `R` (`RecordWorkspaceStart(None)`) to [`DEFAULT_LEADER_BINDINGS`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs).

3. **Held-`Ctrl` Leader Normalization & `asciinema` PTY-Wrapper Override (`REC-02`):**
   - Normalized held-`Ctrl` follow-up chords (`Ctrl+X Ctrl+<key>` / `Ctrl+B Ctrl+<key>`) in [`handle_wm_leader_key`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs) so `Ctrl` is stripped before looking up [`default_leader_action`](file:///home/bill/src/ai/bilterm/src/input/keymap.rs) (while keeping double-tap `Ctrl+X Ctrl+X` / `Ctrl+B Ctrl+B` passthrough in raw mode and `Ctrl+Q` quit).
   - Added `force_command_prompt: bool` to [`FloatingPromptState`](file:///home/bill/src/ai/bilterm/src/pane/prompt_editor.rs) and [`CompositePane::is_active_canvas_password_prompt`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs) so explicit prompt-mode toggles (`F4`, `F12`, `Ctrl+G`, `Ctrl+X e/t/b/s`, `:mode float/fixed`) override false-positive `!ECHO` passthrough from `asciinema rec` / `script` PTY wrappers without weakening `DECSET 1049` `ALT_SCREEN` or active canvas `[sudo] password` auto-hide.
