# Chapter 6: Complete Keyboard, Mouse & `:command` Reference

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [← Chapter 5: tmux Integration](05-tmux-integration.md) | [Chapter 7: Troubleshooting & FAQ →](07-troubleshooting-and-faq.md)

## Table of Contents

- [1. Function Keys & Global Window Manager Shortcuts](#1-function-keys--global-window-manager-shortcuts)
- [2. Intra-Pane Command Prompt & History Shortcuts](#2-intra-pane-command-prompt--history-shortcuts)
- [3. Mouse Gestures](#3-mouse-gestures)
- [4. Retro Aesthetics, Outlines & Theme `:command`s](#4-retro-aesthetics-outlines--theme-commands)

---

## 1. Function Keys & Global Window Manager Shortcuts

| Keybinding | `:command` | Description |
|------------|------------|-------------|
| **`F1`** | `:help` / `:?` | Toggle interactive scrollable Borland Turbo Vision Help Modal |
| **`F2`** / `Alt+H` | `:split h` / `:sp` | Split focused composite pane horizontally (Top / Bottom) (`WM-01`) |
| `Alt+V` | `:split v` / `:vs` | Split focused composite pane vertically (Left / Right) (`WM-01`) |
| **`F5`** / `Alt+Z` | `:zoom` | Toggle zoom/maximize on the focused composite pane (`WM-03`) |
| **`F6`** / `Alt+Tab` | `:deck next` | Shuffle Z-order pane deck top-to-bottom (`WM-02`) |
| `Alt+Shift+Tab` | `:deck prev` | Shuffle Z-order pane deck bottom-to-top (`WM-02`) |
| `Alt+1` .. `Alt+9` | `:focus <1..9>` | Selectively raise and focus pane `#1..#9` (`WM-02`) |
| `Alt+M` | `:layout stack\|tile` | Toggle between Tiled Splits and Overlapping Z-Order Cascade (`WM-02`) |
| **`F10`** / `Alt+\`` / `Ctrl+Space` | `:quake` / `:guake` | Toggle persistent Guake/Quake-style drop-down quick terminal (`WM-05`) |
| **`F12`** / `Ctrl+G` | `:raw` | Toggle between `INPUT: CMD BOX` and `INPUT: RAW PTY` (`PTY-02`) |
| `Alt+X` | `:crosshair [full\|pane\|off]` | Cycle IBM 3270 Cross-Hair Cursor (Pane-Local -> Full-Screen -> Off) |
| `Alt+W` | `:close` | Close focused composite pane (`WM-03`) |
| `Ctrl+Q` | `:quit` / `:q` | Cleanly exit Bilterm |

---

## 2. Intra-Pane Command Prompt & History Shortcuts

| Keybinding | `:command` | Description |
|------------|------------|-------------|
| `Alt+T` | `:prompt top` | Pin floating command prompt to Top (IBM 3270 default) (`PANE-02`) |
| `Alt+B` | `:prompt bottom` | Pin floating command prompt to Bottom (`PANE-02`) |
| `Alt+S` | `:prompt split` / `split-top` / `split-bottom` | Toggle non-overlapping Split vs. Floating command prompt |
| `Alt+Up` / `Alt+Down` | `:prompt float <row>` | Move floating command prompt vertically row-by-row (`PANE-02`) |
| `Alt+Plus` / `Alt+Minus` | `:prompt height <3..12>` | Increase / decrease command prompt height (`PANE-03`) |
| `Alt+]` / `Alt+[` | `:prompt width <cols\|full>` | Increase / decrease command prompt width (or reset to full width) |
| `Shift+PageUp` / `Shift+PageDown` | — | Scroll multi-line command prompt content up / down |
| `Tab` / `Shift+Tab` | — | Interactive Tab completion for `:commands`, paths, binaries, and `$ENV` |
| `Alt+P` | `:history` / `:hist` | Open Borland Command History Pop-up Window (`Enter`=select, `s`=save temp file) |
| — | `:history save [path]` | Save session command history to a temporary `.sh` file (or custom path) |
| — | `:history on \| off` | Enable or disable automatic session history file persistence |
| — | `:history max-files <N>` | Set max retained session history files before automatic rotation |
| `Ctrl+R` | — | Open interactive `nucleo-matcher` fuzzy history search (`PANE-04`) |
| `Shift+Enter` / `Alt+Enter` | — | Insert newline inside multi-line command buffer (`PANE-04`) |
| `Enter` | — | Submit command (keeping it visible in the `RUNNING` / `LAST CMD` banner) |
| `Alt+PageUp` / `Alt+PageDown` | `:block prev\|next` | Jump between `OSC 133` command blocks in scrollback (`PANE-05`) |
| `Ctrl+Z` | — | Undo last edit in command prompt (`PANE-04`) |
| `Ctrl+K` / `Ctrl+W` / `Ctrl+Y` | — | Kill to end of line / Kill previous word / Yank killed text |
| — | `:yank` | Copy focused command block or screen via `OSC 52` clipboard (`PTY-04`) |
| — | `:clear` | Clear output canvas and reset scrollback |

---

## 3. Mouse Gestures

| Mouse Gesture | Action |
|---------------|--------|
| **Drag prompt top border** | Move floating command prompt vertically (and horizontally when custom width is set) |
| **Click `[▲]` / `[▼]` on prompt top-right** | Pin floating command prompt to Top or Bottom |
| **Drag prompt bottom border** | Resize command prompt height (`3..12` rows) |
| **Drag prompt left or right border** | Resize command prompt width (`16..canvas_width` columns) |
| **Drag prompt bottom-right corner (`◢`)** | Resize both command prompt width and height simultaneously |
| **Mouse Wheel inside multi-line prompt** | Scroll multi-line command prompt lines up or down |
| **Mouse Wheel over output canvas** | Scroll output canvas VT100 scrollback up or down |
| **Click any background pane** | Focus and selectively raise that pane to top of Z-order stack |

---

## 4. Retro Aesthetics, Outlines & Theme `:command`s

| `:command` | Description |
|------------|-------------|
| `:theme turbo-blue \| turbo-cyan \| 3278 \| 3279 \| mono` | Switch color theme preset (`UI-02`) |
| `:outline [window\|prompt] <solid\|double\|rounded\|thick\|dotted\|dashed\|quadruple\|ascii>` | Switch window and/or prompt border outline style |
| `:border <style>` | Alias for `:outline <style>` |
| `:shadows <on\|off\|none\|translucent\|tinted\|solid>` | Switch or toggle DOS drop-shadow style (`UI-01`) |
| `:glyphs unicode \| cp437 \| nerd-font \| ascii` | Switch glyph pack (`UI-04`) |
| `:cursor block \| hollow \| underscore \| bar` | Switch software caret shape (`UI-05`) |
| `:crosshair [on\|off\|full\|pane]` | Toggle or set pane-local vs. full-screen IBM 3270 cross-hairs (`UI-05`) |
| `:fkeybar [on\|off]` | Toggle bottom Borland DOS Function-Key bar (`UI-03`) |

---

[← Chapter 5: tmux Integration](05-tmux-integration.md) | [User Guide Home](README.md) | [Chapter 7: Troubleshooting & FAQ →](07-troubleshooting-and-faq.md)
