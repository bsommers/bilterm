---
id: 20261022-macos-keybindings-leader-and-option-normalization
type: quick
status: in_progress
created_at: 2026-10-22
---

# Quick Plan: macOS Key Sequences, Leader Prefix (`Ctrl+X`), Option-Key Unicode Normalization, Readline Word Navigation & Raw PTY Passthrough

## Problem Summary

1. **macOS `Option` (`Alt`) Key Dead-Key / Unicode Collisions**:
   - On macOS (Terminal.app, iTerm2, Ghostty, WezTerm, Kitty), pressing `Option+<key>` without configuring "Option as Meta" emits macOS Unicode characters (`˙`, `√`, `∫`, `ƒ`, `†`, `ß`, `ç`, `¬`, `π`, `∑`, `Ω`, `µ`, `≈`, `÷`, `–`, `≠`, `±`, `“`, `‘`, `\u{00a0}`, `¡`..`ª`) with `KeyModifiers::NONE`, causing hotkeys to fail and inserting stray symbols into the command prompt.
2. **Readline / macOS `Option+Left` (`ESC b`) & `Option+Right` (`ESC f`) Collision**:
   - macOS terminals map `Option+Left` to `\x1bb` (`Alt+b`) and `Option+Right` to `\x1bf` (`Alt+f`).
   - Bilterm previously bound `Alt+b` to `PromptAnchor::Bottom` and `Alt+f` to `enter_hints_mode()`, causing word-left/word-right navigation in the command prompt to move the prompt box or trigger hints instead of moving the cursor by word.
3. **`RawPtyPassthrough` / `ALT_SCREEN` Key Theft (`vim`, `htop`, `emacs`, `mc`)**:
   - `handle_wm_key_event` and `handle_key_event` previously intercepted `F1..F10` and `Alt+<key>` globally before checking `effective_routing_mode()`, stealing `F1..F10` from `htop`/`mc` and `Alt+<key>` (`M-x`, `M-f`, `M-b`) from `emacs`/`vim`/raw shells.
4. **macOS System Shortcut Collisions (`Ctrl+Space`, `Option+Tab`, `Fn+F-keys`)**:
   - `Ctrl+Space` conflicts with macOS Input Source switching, `Option+Tab` conflicts with window switchers, and `F1..F12` require `Fn` on MacBooks.

## Tasks

1. **Configurable Leader/Prefix Key (`Ctrl+X` default) & `macos_option_as_alt` in `BiltermConfig` ([`src/config/mod.rs`](file:///Users/bill/src/tools/bilterm/src/config/mod.rs), [`src/wm/mod.rs`](file:///Users/bill/src/tools/bilterm/src/wm/mod.rs), [`src/pane/mod.rs`](file:///Users/bill/src/tools/bilterm/src/pane/mod.rs))**:
   - Add `leader_key: String` (default `"Ctrl+X"`) and `macos_option_as_alt: bool` (default `true`) to `BiltermConfig`.
   - Track `leader_pending: bool` on `WindowManager` and `CompositePane` with visual status feedback in the OIA status bar / status message.
   - Support complete home-row `Ctrl+X <key>` leader sequences across both `CommandPrompt` and `RawPtyPassthrough` modes (including `Ctrl+X [` / `Ctrl+X c` for Copy mode, `Ctrl+X -` / `Ctrl+X |` for H/V splits, `Ctrl+X z` for zoom, `Ctrl+X o` / `Ctrl+X n` / `Ctrl+X p` for pane cycling, `Ctrl+X 1..9` for pane focus, `Ctrl+X q` for Quake terminal, `Ctrl+X f` for 3270 Field Hints, `Ctrl+X t`/`b`/`s`/`e` for prompt docking/mode, `Ctrl+X ?` for Help, `Ctrl+X i` for Inspector, `Ctrl+X Space` for Command Palette, and `Ctrl+X Ctrl+X` to send literal `^X` to the PTY).

2. **macOS `Option`-Key Unicode Normalization ([`src/input/mod.rs`](file:///Users/bill/src/tools/bilterm/src/input/mod.rs))**:
   - Implement `normalize_macos_option_key(key: KeyEvent, enabled: bool) -> KeyEvent` mapping macOS US/International `Option+<key>` and `Option+Shift+<key>` Unicode symbols (`å`, `∫`, `ç`, `∂`, `´`, `ƒ`, `©`, `˙`, `ˆ`, `∆`, `˚`, `¬`, `µ`, `˜`, `ø`, `π`, `œ`, `®`, `ß`, `†`, `¨`, `√`, `∑`, `≈`, `¥`, `Ω`, `¡`..`ª`, `÷`, `≠`, `±`, `–`, `“`, `‘`, `¿`, `\u{00a0}`) to their canonical `KeyModifiers::ALT` (and `KeyModifiers::SHIFT`) `KeyEvent`s.

3. **Readline Word Navigation (`Alt+B`, `Alt+F`, `Alt+D`, `Alt+Backspace`) & Mac-Friendly Direct Chords ([`src/pane/mod.rs`](file:///Users/bill/src/tools/bilterm/src/pane/mod.rs), [`src/input/mod.rs`](file:///Users/bill/src/tools/bilterm/src/input/mod.rs))**:
   - Add `FloatingPromptState::kill_next_word()` for `Alt+D` / `Alt+Delete` and bind `Alt+Backspace` to `kill_prev_word()`.
   - When editing text in `CommandPrompt` (`!pane.prompt.input.is_empty()`), route `Alt+b` (`Option+Left`) to `move_word_left()` and `Alt+f` (`Option+Right`) to `move_word_right()`.
   - Support `Alt+Shift+B` / `Ctrl+X b` for bottom prompt anchor, `Alt+Shift+F` / `F3` / `Ctrl+X f` for 3270 Field Hints, and `Ctrl+\` (`Ctrl+Backslash`) / `Ctrl+X q` for Quake Quick Terminal (avoiding macOS `Ctrl+Space` input switcher collision).

4. **`RawPtyPassthrough` / `ALT_SCREEN` Key Forwarding ([`src/input/mod.rs`](file:///Users/bill/src/tools/bilterm/src/input/mod.rs))**:
   - In `RawPtyPassthrough` mode (including `DECSET 1049` alternate screen programs like `vim`, `htop`, `emacs`, `mc`), forward `F1..F10` and `Alt+<key>` directly to the child PTY instead of intercepting them globally, while keeping `Ctrl+X <key>` (Leader), `F12` / `Ctrl+G` (exit raw mode), and `Ctrl+Q` (quit) active at all times.

5. **Update UI Help Modal, Command Palette, Tests & Documentation ([`src/ui/mod.rs`](file:///Users/bill/src/tools/bilterm/src/ui/mod.rs), [`src/wm/mod.rs`](file:///Users/bill/src/tools/bilterm/src/wm/mod.rs), [`docs/`](file:///Users/bill/src/tools/bilterm/docs/user-guide/index.md))**:
   - Update `<F1>` Help Modal and Command Palette shortcut labels to showcase `Ctrl+X` leader chords and macOS-friendly bindings.
   - Add unit and integration tests covering macOS Option-key Unicode normalization, `Ctrl+X` leader chords, `Alt+B`/`Alt+F`/`Alt+D`/`Alt+Backspace` Readline word editing, and `F1..F10` + `Alt+<key>` passthrough in `RawPtyPassthrough` mode.
