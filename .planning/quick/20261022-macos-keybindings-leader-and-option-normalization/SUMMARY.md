---
id: 20261022-macos-keybindings-leader-and-option-normalization
type: quick
status: complete
completed_at: 2026-10-22
---

# Quick Task Summary: macOS Key Sequences, Leader Prefix (`Ctrl+X`), Option-Key Unicode Normalization, Readline Word Navigation & Raw PTY Passthrough

## Delivered Changes

1. **Universal Prefix / Leader Key (`Ctrl+X` default, configurable via `leader_key` in `config.toml`) ([`src/config/mod.rs`](file:///Users/bill/src/tools/bilterm/src/config/mod.rs), [`src/input/mod.rs`](file:///Users/bill/src/tools/bilterm/src/input/mod.rs), [`src/wm/mod.rs`](file:///Users/bill/src/tools/bilterm/src/wm/mod.rs), [`src/pane/mod.rs`](file:///Users/bill/src/tools/bilterm/src/pane/mod.rs))**:
   - Added `leader_key: String` (default `"Ctrl+X"`) to `BiltermConfig` and `leader_pending: bool` to `WindowManager` and `CompositePane`.
   - Pressing `Ctrl+X` displays a live OIA status prompt (`⌨ LEADER [Ctrl+X]: -/h=SplitH |/v=SplitV z=Zoom o/n/p=Deck 1-9=Pane q=Quake e/t/b/s=Prompt [=Copy f=Hints ?=Help`) and executes the next key chord without requiring `Option`/`Alt` or `Fn+F-keys`:
     - `Ctrl+X -` or `Ctrl+X h`: Split focused pane Horizontally
     - `Ctrl+X |` or `Ctrl+X \` or `Ctrl+X v`: Split focused pane Vertically
     - `Ctrl+X z`: Toggle Zoom / Maximize
     - `Ctrl+X o` or `Ctrl+X Tab`: Shuffle Z-order pane deck top-to-bottom
     - `Ctrl+X Shift+Tab`: Shuffle Z-order pane deck bottom-to-top
     - `Ctrl+X 1`..`9`: Selectively focus & raise Pane `#1`..`#9`
     - `Ctrl+X w`: Close focused pane
     - `Ctrl+X M`: Toggle Tiled Splits vs. Z-Order Stacked Windows
     - `Ctrl+X q` or ``Ctrl+X ` ``: Toggle Guake Quick Terminal drop-down
     - `Ctrl+X Space` or `Ctrl+X :`: Open Fuzzy Command Palette
     - `Ctrl+X ?` or `Ctrl+X F1`: Toggle `<F1>` Help modal
     - `Ctrl+X i` or `Ctrl+X F9`: Toggle `<F9>` Live 3270 Terminal Inspector
     - `Ctrl+X e` or `Ctrl+X m`: Cycle Prompt Mode (`Floating` -> `Fixed` -> `RegularTerminal`)
     - `Ctrl+X g`: Toggle `RawPtyPassthrough`
     - `Ctrl+X t` / `Ctrl+X b` / `Ctrl+X s`: Pin prompt Top / Bottom / Toggle Fixed Split
     - `Ctrl+X [` or `Ctrl+X c`: Toggle Vi Copy Mode
     - `Ctrl+X f`: Activate QuickSelect 3270 Field Hints
     - `Ctrl+X l`: Toggle Read-Only Protected 3270 Field Lock (`🔒 PROT`)
     - `Ctrl+X /`: Start OutputCanvas search (`:search `)
     - `Ctrl+X p` / `Ctrl+X H`: Open Command History Pop-up Window
     - `Ctrl+X x`: Cycle IBM 3270 Cross-Hair Cursor (`Pane` -> `Full` -> `Off`)
     - `Ctrl+X Ctrl+X`: Send literal `^X` (`0x18`) to the child PTY in `RawPtyPassthrough` mode

2. **macOS `Option`-Key Unicode Symbol Normalization ([`src/input/mod.rs`](file:///Users/bill/src/tools/bilterm/src/input/mod.rs), [`src/config/mod.rs`](file:///Users/bill/src/tools/bilterm/src/config/mod.rs))**:
   - Added `macos_option_as_alt: bool` (default `true`) and `normalize_macos_option_key()`, translating macOS `Option+<key>` and `Option+Shift+<key>` Unicode dead-keys/symbols (`å`, `∫`, `ç`, `∂`, `´`, `ƒ`, `©`, `˙`, `ˆ`, `∆`, `˚`, `¬`, `µ`, `˜`, `ø`, `π`, `œ`, `®`, `ß`, `†`, `¨`, `√`, `∑`, `≈`, `¥`, `Ω`, `¡`..`ª`, `\u{00a0}`, `÷`, `≠`, `±`, `–`, `“`, `‘`, `¿`) into canonical `KeyModifiers::ALT` (and `KeyModifiers::SHIFT`) `KeyEvent`s automatically across Terminal.app, iTerm2, Ghostty, WezTerm, Alacritty, and Kitty.

3. **Readline Word Navigation (`Alt+B`, `Alt+F`, `Alt+D`, `Alt+Backspace`) & macOS-Safe Shortcuts ([`src/pane/mod.rs`](file:///Users/bill/src/tools/bilterm/src/pane/mod.rs), [`src/input/mod.rs`](file:///Users/bill/src/tools/bilterm/src/input/mod.rs))**:
   - Added `FloatingPromptState::kill_next_word()` (`Alt+D` / `Alt+Delete`) and bound `Alt+Backspace` to `kill_prev_word()`.
   - When editing text in the command prompt (`!pane.prompt.input.is_empty()`), `Alt+b` (`Option+Left` in macOS terminals) moves cursor word-left and `Alt+f` (`Option+Right` in macOS terminals) moves cursor word-right without moving the prompt box or triggering hints.
   - Added `Ctrl+\` (`Ctrl+Backslash`) as a macOS-safe Quake Quick Terminal toggle (avoiding macOS `Ctrl+Space` input switcher collision) and `F3` / `Alt+Shift+F` / `Ctrl+X f` for 3270 Field Hints.

4. **`RawPtyPassthrough` / `ALT_SCREEN` Key Forwarding ([`src/input/mod.rs`](file:///Users/bill/src/tools/bilterm/src/input/mod.rs))**:
   - When a pane is in `RawPtyPassthrough` mode (such as `vim`, `htop`, `emacs`, `mc` in `DECSET 1049`), `F1..F10` and `Alt+<key>` now pass directly through to the child application instead of being intercepted by the Window Manager, while `Ctrl+X <key>` (Leader), `F12` / `Ctrl+G` (exit raw mode), and `Ctrl+Q` (quit) remain available at all times.
