# Chapter 6: Complete Keyboard, Mouse & `:command` Reference

## 1. Global & Window Manager Shortcuts

| Keybinding | `:command` | Description |
|------------|------------|-------------|
| `Alt+\`` / `Ctrl+Space` | `:quake` | Toggle persistent Guake/Quake-style drop-down quick terminal (`WM-05`) |
| `Alt+H` | `:split h` / `:sp` | Split focused composite pane horizontally (Top / Bottom) (`WM-01`) |
| `Alt+V` | `:split v` / `:vs` | Split focused composite pane vertically (Left / Right) (`WM-01`) |
| `Alt+Tab` / `F6` | `:deck next` | Shuffle Z-order pane deck top-to-bottom (`WM-02`) |
| `Alt+Shift+Tab` | `:deck prev` | Shuffle Z-order pane deck bottom-to-top (`WM-02`) |
| `Alt+1` .. `Alt+9` | `:focus <1..9>` | Selectively raise and focus pane `#1..#9` (`WM-02`) |
| `Alt+M` | `:layout stack\|tile` | Toggle between Tiled Splits and Overlapping Z-Order Cascade (`WM-02`) |
| `F5` / `Alt+Z` | `:zoom` | Toggle zoom/maximize on the focused composite pane (`WM-03`) |
| `Alt+W` | `:close` | Close focused composite pane (`WM-03`) |
| `Ctrl+G` / `F12` | `:raw` | Toggle between `INPUT: CMD BOX` and `INPUT: RAW PTY` (`PTY-02`) |
| `Ctrl+Q` | `:quit` / `:q` | Cleanly exit Bilterm |

---

## 2. Intra-Pane Floating Command Prompt Shortcuts

| Keybinding | `:command` | Description |
|------------|------------|-------------|
| `Alt+T` | `:prompt top` | Pin floating command prompt to Top (IBM 3270 default) (`PANE-02`) |
| `Alt+B` | `:prompt bottom` | Pin floating command prompt to Bottom (`PANE-02`) |
| `Alt+Up` / `Alt+Down` | `:prompt float <row>` | Move floating command prompt vertically row-by-row (`PANE-02`) |
| `Alt+Plus` / `Alt+Minus` | `:prompt height <3..12>` | Increase / decrease floating prompt height (`PANE-03`) |
| `Shift+Enter` / `Alt+Enter` | — | Insert newline inside multi-line command buffer (`PANE-04`) |
| `Enter` | — | Submit multi-line command (or execute `:command`) |
| `Ctrl+R` | — | Open interactive `nucleo-matcher` fuzzy history search (`PANE-04`) |
| `Alt+PageUp` / `Alt+PageDown` | `:block prev\|next` | Jump between `OSC 133` command blocks in scrollback (`PANE-05`) |
| `Ctrl+Z` | — | Undo last edit in floating prompt (`PANE-04`) |
| `Ctrl+K` / `Ctrl+W` / `Ctrl+Y` | — | Kill to end of line / Kill previous word / Yank killed text |
| — | `:yank` | Copy focused command block or screen via `OSC 52` clipboard (`PTY-04`) |

---

## 3. Retro Aesthetics & Theme `:command`s

| `:command` | Description |
|------------|-------------|
| `:theme turbo-blue \| turbo-cyan \| 3278 \| 3279 \| mono` | Switch color theme preset (`UI-02`) |
| `:glyphs unicode \| cp437 \| nerd-font \| ascii` | Switch glyph pack (`UI-04`) |
| `:cursor block \| hollow \| underscore \| bar` | Switch software caret shape (`UI-05`) |
| `:crosshair [on\|off]` | Toggle IBM 3270 full-pane cross-hair cursor ruler (`UI-05`) |
| `:shadows [on\|off]` | Toggle Borland 2-col/1-row translucent drop shadows (`UI-01`) |
| `:fkeybar [on\|off]` | Toggle bottom Borland DOS Function-Key bar (`UI-03`) |
