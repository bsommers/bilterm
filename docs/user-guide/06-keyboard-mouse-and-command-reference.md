# Chapter 6: Complete Keyboard, Mouse & `:command` Reference

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [← Chapter 5: tmux Integration](05-tmux-integration.md) | [Chapter 7: Troubleshooting & FAQ →](07-troubleshooting-and-faq.md)

## Table of Contents

- [1. Function Keys & Global Window Manager Shortcuts](#1-function-keys--global-window-manager-shortcuts)
- [2. Intra-Pane Command Prompt & History Shortcuts](#2-intra-pane-command-prompt--history-shortcuts)
- [3. Mouse Gestures](#3-mouse-gestures)
- [4. Retro Aesthetics, Outlines & Theme `:command`s](#4-retro-aesthetics-outlines--theme-commands)

---

## 1. Function Keys, `Ctrl+X` Leader Prefix & Global Window Manager Shortcuts

> **macOS Compatibility:** Bilterm provides a universal **`Ctrl+X` Leader Prefix** (`keybindings.leader_key = "Ctrl+X"`) that avoids macOS Spotlight (`Ctrl+Space`), Mission Control, and Touch Bar `Fn` key collisions. In addition, when `keybindings.macos_option_as_alt = true` (default), macOS US-layout `Option` Unicode glyphs (`˙`, `√`, `∫`, `ƒ`, `†`, `ß`, `¬`, `µ`, `≈`, `∑`, `π`, `÷`, `¡`..`ª`, etc.) are automatically normalized back to `Alt+<key>`.

| Keybinding | `:command` | Description |
|------------|------------|-------------|
| **`F1`** / `Ctrl+X ?` | `:help` / `:?` | Toggle interactive scrollable Borland Turbo Vision Help Modal |
| **`F2`** / `Ctrl+X -` / `Alt+H` | `:split h` / `:sp` | Split focused composite pane horizontally (Top / Bottom) (`WM-01`) |
| `Ctrl+X \|` / `Ctrl+X v` / `Alt+V` | `:split v` / `:vs` | Split focused composite pane vertically (Left / Right) (`WM-01`) |
| **`F4`** / `Ctrl+X e` / `Alt+E` | `:mode [float\|fixed\|terminal\|cycle]` | Cycle between **Floating Prompt**, **Fixed (Docked) Prompt**, and **Regular Terminal** |
| **`F5`** / `Ctrl+X z` / `Alt+Z` | `:zoom` | Toggle zoom/maximize on the focused composite pane (`WM-03`) |
| **`F6`** / `Ctrl+X Tab` / `Alt+Tab` | `:deck next` | Shuffle Z-order pane deck top-to-bottom (`WM-02`) |
| `Alt+Shift+Tab` | `:deck prev` | Shuffle Z-order pane deck bottom-to-top (`WM-02`) |
| `Ctrl+X 1..9` / `Alt+1` .. `Alt+9` | `:focus <1..9>` | Selectively raise and focus pane `#1..#9` (`WM-02`) |
| `Ctrl+X m` / `Alt+M` | `:layout stack\|tile` | Toggle between Tiled Splits and Overlapping Z-Order Cascade (`WM-02`) |
| **`F9`** / `Ctrl+X i` | `:inspector` / `:inspect` | Toggle live 3270 Terminal Inspector modal (active DECSET modes, Kitty flags, cursor cell `U+XXXX`, CSI/OSC log) (`BEL-01`) |
| **`F10`** / `Ctrl+\` / `Alt+\`` | `:quake` / `:guake` | Toggle persistent Guake/Quake-style drop-down quick terminal (`WM-05`) |
| **`F12`** / `Ctrl+G` | `:raw` | Toggle between `INPUT: CMD BOX` and `INPUT: RAW PTY` (`PTY-02`) |
| **`Ctrl+P`** / `Alt+Space` | `:palette` / `:cmd` | Open interactive Fuzzy Command Palette (`nucleo-matcher`) (`CMD-01`) |
| `Ctrl+X f` / `Alt+Shift+F` | `:hints` / `:hint` | Toggle QuickSelect 3270 Field Hints (`[a]..[z]`) for URLs, paths, IPv4s, and Git SHAs (`ECO-01`) |
| `Ctrl+X l` / `Alt+L` | `:lock [on\|off]` | Toggle Read-Only Protected 3270 Field Lock (`🔒 PROT`) on focused pane (`ECO-04`) |
| — | `:broadcast [on\|off]` | Toggle Synchronized Multi-Pane Input Broadcast (`📡 BCAST`) across unlocked panes (`ECO-04`) |
| — | `:menubar [on\|off]` | Toggle interactive Top Borland Turbo Vision Menu Bar (`ECO-03`) |
| — | `:crt [on\|off]` | Toggle retro CRT phosphor scanline shader (`CRT-01`) |
| — | `:notifications clear` | Clear `OSC 9` / `OSC 777` desktop notification & bell badge (`🔔`) (`NTF-01`) |
| — | `:trigger add <pat> \| clear` | Add or clear live output highlight triggers on the `OutputCanvas` (`TRG-01`) |
| — | `:record start [path] \| stop` | Start or stop Asciicast v2 (`.cast`) terminal session recording (`⏺ REC`) (`REC-01`) |
| — | `:session save <name>` | Save multi-pane workspace topology and pane metadata to `$XDG_STATE_HOME/bilterm/sessions/<name>.toml` (`ECO-02`) |
| — | `:session restore <name>` | Restore saved multi-pane workspace session (`ECO-02`) |
| — | `:session list` | List saved workspace sessions (`ECO-02`) |
| — | `:ssh [split h\|v] <user@host[:port]> [cmd]` | Connect focused or newly split pane to remote SSH host (or `[remotes.<name>]` profile) (`RMT-01`) |
| — | `:ssh-tmux <user@host[:port]> [session]` | Attach/spawn persistent remote `tmux -CC` control session over SSH (`RMT-02`) |
| — | `:remote add <name> <dest> [rpc] [tmux]` | Register a `herdr`-style federated remote node in `FleetRegistry` (`RMT-03`) |
| — | `:remote list` | List all registered local and remote fleet nodes and connection status (`RMT-03`) |
| — | `:remote connect <name>` | Connect to a registered remote fleet node (`RMT-03`) |
| — | `:remote exec <name> <method> [json]` | Execute a JSON-RPC 2.0 method on a remote fleet node (`RMT-03`) |
| — | `:remote disconnect <name>` | Disconnect a remote fleet node (`RMT-03`) |
| `Ctrl+X x` / `Alt+X` | `:crosshair [full\|pane\|off]` | Cycle IBM 3270 Cross-Hair Cursor (Pane-Local -> Full-Screen -> Off) |
| `Ctrl+X w` / `Alt+W` | `:close` | Close focused composite pane (`WM-03`) |
| `Ctrl+Q` | `:quit` / `:q` | Cleanly exit Bilterm |

---

## 2. Intra-Pane Command Prompt & History Shortcuts

| Keybinding | `:command` | Description |
|------------|------------|-------------|
| `Ctrl+X t` / `Alt+T` | `:prompt top` | Pin floating command prompt to Top (IBM 3270 default) (`PANE-02`) |
| `Ctrl+X b` / `Alt+Shift+B` | `:prompt bottom` | Pin floating command prompt to Bottom (`PANE-02`) |
| `Ctrl+X s` / `Alt+S` | `:prompt split` / `split-top` / `split-bottom` | Toggle non-overlapping Split vs. Floating command prompt |
| `Alt+b` / `Alt+f` | — | Readline move backward / forward by word inside the command prompt |
| `Alt+d` / `Alt+Backspace` | — | Readline kill next word / kill previous word inside the command prompt |
| `Alt+Up` / `Alt+Down` | `:prompt float <row>` | Move floating command prompt vertically row-by-row (`PANE-02`) |
| `Alt+Plus` / `Alt+Minus` | `:prompt height <3..12>` | Increase / decrease command prompt height (`PANE-03`) |
| `Alt+]` / `Alt+[` | `:prompt width <cols\|full>` | Increase / decrease command prompt width (or reset to full width) |
| `Shift+PageUp` / `Shift+PageDown` | — | Scroll multi-line command prompt content up / down |
| `Tab` / `Shift+Tab` | — | Interactive Tab completion for `:commands`, paths, binaries, and `$ENV` |
| `Ctrl+X p` / `Alt+P` | `:history` / `:hist` | Open Borland Command History Pop-up Window (`Enter`=select, `s`=save temp file) |
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
| `Ctrl+X /` / `Alt+/` | `:search <query>` | Search `OutputCanvas` grid and highlight matches (`VAL-02`) |
| `Alt+n` / `Alt+N` | `:search next\|prev\|clear` | Jump to next/previous canvas search match or clear search (`VAL-02`) |
| `Ctrl+X [` / `Alt+C` | `:copy` | Enter keyboard-driven Vi Copy Mode (`h/j/k/l/w/b/0/$`, `v` char, `V` line, `b`/`Ctrl+V` block, `y`/`Enter` yank) (`VAL-04`) |
| `Ctrl+LeftClick` | `:links [index]` | Yank explicit `OSC 8` hyperlink or auto-detected URL via `OSC 52` (`VAL-03`) |
| — | `:clear` | Clear output canvas and reset scrollback |
| — | `:scrollback` / `:sb` | Show scrollback lines used/capacity, memory, and the global budget |
| — | `:scrollback clear` | Drop the active pane's scrollback history (keeps the screen) |

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
