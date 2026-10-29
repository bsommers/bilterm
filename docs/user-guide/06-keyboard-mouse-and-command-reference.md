# Chapter 6: Complete CLI, Keyboard, Mouse & `:command` Reference

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [← Chapter 5: tmux Integration](05-tmux-integration.md) | [Chapter 7: Troubleshooting & FAQ →](07-troubleshooting-and-faq.md)

## Table of Contents

- [1. Complete Command-Line Interface (CLI) Reference](#1-complete-command-line-interface-cli-reference)
- [2. Dual `Ctrl+B` / `Ctrl+X` Leader Prefix, Function Keys & Global Workspace Shortcuts](#2-dual-ctrlb--ctrlx-leader-prefix-function-keys--global-workspace-shortcuts)
- [3. Per-Pane Font Scaling, Pane Resizing & Window Moving Shortcuts](#3-per-pane-font-scaling-pane-resizing--window-moving-shortcuts)
- [4. Intra-Pane Command Prompt, Readline, History & Vi Copy Mode Shortcuts](#4-intra-pane-command-prompt-readline-history--vi-copy-mode-shortcuts)
- [5. Complete Mouse Gestures Reference](#5-complete-mouse-gestures-reference)
- [6. Complete `:colon` Command Reference (All Subsystems)](#6-complete-colon-command-reference-all-subsystems)
  - [6.1 Window Manager, Splits, Z-Stack, Resize & Move](#61-window-manager-splits-z-stack-resize--move)
  - [6.2 Floating Command Prompt, Modes, History & Scrollback](#62-floating-command-prompt-modes-history--scrollback)
  - [6.3 Per-Pane Typography & Dynamic Font Scaling (`:fontsize`, `:font`)](#63-per-pane-typography--dynamic-font-scaling-fontsize-font)
  - [6.4 Retro Themes, Outlines, Drop Shadows, Shaders & Focal Lenses](#64-retro-themes-outlines-drop-shadows-shaders--focal-lenses)
  - [6.5 Tektronix 4010/4014 & High-Res Pixel/Sub-Cell Canvas (`:canvas`, `:tek`)](#65-tektronix-40104014--high-res-pixelsub-cell-canvas-canvas-tek)
  - [6.6 Slide-Out Sessions/Windows/Panes & Fleet Drawer (`:drawer`)](#66-slide-out-sessionswindowspanes--fleet-drawer-drawer)
  - [6.7 Remote SSH Domains, Remote `tmux -CC` & `herdr` Fleet Federation](#67-remote-ssh-domains-remote-tmux--cc--herdr-fleet-federation)
  - [6.8 Workspace Sessions, Recording, Triggers & Inspection](#68-workspace-sessions-recording-triggers--inspection)

---

## 1. Complete Command-Line Interface (CLI) Reference

| CLI Option / Subcommand | Short / Alias | Arguments | Description |
|-------------------------|---------------|-----------|-------------|
| `bilterm` | — | *(none)* | Launch interactive TUI multiplexer inside the current host terminal (auto-resumes detached session) |
| `attach` | `-a`, `--attach` | `[SESSION]` | Re-attach to live background daemon (`daemon-<session>.sock`) or restore saved multi-pane layout |
| `ls` / `sessions` | `--list-sessions` | *(none)* | List live background daemon sockets (`LIVE DAEMON`) & saved `.toml` layouts (`SAVED LAYOUT`) |
| `--gui` | `-G` | *(none)* | Launch standalone native desktop GUI window (X11 on Linux, Win32 GDI on Windows, Cocoa/AppKit on macOS) |
| `--gui-font` | `-F`, `--font` | `<FAMILY>` | Font family or preset for native `--gui` window or `--gui-dump` (e.g. `"JetBrains Mono"`, `"IBM 3270"`, `"Fira Code"`, `"ibm3270"`, `"vga"`, `"tek4014"`, `"iosevka"`, `"cascadia"`, `"hack"`) |
| `--gui-font-size` | `--font-size` | `<PT>` | Base font size in points (`6..=72`, default `16`) for `--gui` and `--gui-dump` |
| `--gui-font-file` | `--font-file` | `<PATH.ttf\|.otf>` | Load a custom TrueType/OpenType font file directly from disk |
| `--theme` | `-T` | `<THEME>` | Override active color palette (`turbo-blue`, `turbo-cyan`, `ibm-3278-green`, `ibm-3279-amber`, `ibm-mono`, `cyberpunk-neon`, `solarized-dark`, `dracula`, `gruvbox-dark`, `tokyo-night`) |
| `--config` | `-c` | `<FILE.toml>` | Load declarative configuration from custom TOML file path |
| `--session` | `-s` | `<NAME>` | Use and restore named persistent command-history session (`[A-Za-z0-9._-]`) |
| `--profile` | `-P` | `<NAME>` | Launch with layout profile (`default`, `single`, `dual-v`, `dual-h`, `main-side`, `quad`, `cascade`, or custom) |
| `--headless` | `-d`, `--daemon` | *(none)* | Run headless JSON-RPC 2.0 workspace daemon (default: private `0600` Unix socket `$XDG_RUNTIME_DIR/bilterm/bilterm.sock`) |
| `--listen` | `-l` | `<ADDR>` | Custom RPC listen address (`127.0.0.1:PORT`, `localhost:PORT`, or `unix:/path/to.sock`) |
| `--allow-remote` | `-r` | *(none)* | Allow binding JSON-RPC daemon to non-loopback TCP interface (requires `--token-file`) |
| `--token-file` | `-t` | `<PATH>` | Path to `0600` file containing bearer token for JSON-RPC auth (or `BILTERM_TOKEN_FILE` env) |
| `--snapshot-dir` | `-S` | `<DIR>` | Directory for confined RPC/MCP `.ppm` snapshots (default: `$XDG_STATE_HOME/bilterm/snapshots`) |
| `--mcp` | `-m` | *(none)* | Run native Model Context Protocol (`2024-11-05`) JSON-RPC 2.0 server over `stdio` |
| `run <PLAYBOOK.toml>` | `-p`, `--playbook` | `<FILE.toml>` | Execute declarative multi-step TOML robot playbook with assertions and exit |
| `ctl` | `--ctl` | `[-t <PATH>] [ENDPOINT] <METHOD> [JSON]` | Send JSON-RPC 2.0 request to a running Bilterm instance and print JSON result |
| `plot` | `--plot` | `[KIND] [CSV] [-o <FILE>] [-b <MODE>] [-r <RES>]` | Render high-res chart (`hires`, `line`, `bar`, `scatter`, `candlestick`, `heatmap`) inline or export `.svg`/`.ppm` |
| `--gui-dump` | `-g` | `<PATH.ppm\|.svg>` | Render workspace via `GuiWorkspaceRenderer` to a standalone `.ppm` or `.svg` file and exit |
| `--completions` | `-C` | `<bash\|zsh\|fish>` | Print shell completion script to `stdout` |
| `--man` | `-M` | *(none)* | Print standalone `roff(7)` manual page (`docs/bilterm.1`) |
| `--version` | `-v`, `-V` | *(none)* | Print version string (`bilterm 2.1.0`) |
| `--help` | `-h` | *(none)* | Print CLI usage summary |

---

## 2. Dual `Ctrl+B` / `Ctrl+X` Leader Prefix, Function Keys & Global Workspace Shortcuts

> **Dual `Ctrl+B` (`tmux`/`herdr`) + `Ctrl+X` (Borland/DOS) Leader Prefix & `$TMUX` Auto-Fallback:**
> By default (`keybindings.leader_key = "Ctrl+B"`), Bilterm accepts **both** **`Ctrl+B`** (`tmux`/`herdr` muscle memory) and **`Ctrl+X`** (Borland Turbo Vision / macOS safe) as leader keys, and automatically normalizes held-`Ctrl` second strokes (`Ctrl+B Ctrl+<key>` / `Ctrl+X Ctrl+<key>`). When running nested inside an outer `$TMUX` session, Bilterm automatically falls back to **`Ctrl+X`** only so `Ctrl+B` passes cleanly to your outer `tmux`. When `keybindings.macos_option_as_alt = true` (default), macOS US-layout `Option` Unicode glyphs (`˙`, `√`, `∫`, `ƒ`, `†`, `ß`, `¬`, `µ`, `≈`, `∑`, `π`, `÷`, `¡`..`ª`, etc.) are also normalized to `Alt+<key>`.

| Direct Shortcut | Leader Chord (`<L>` = `Ctrl+B` or `Ctrl+X`) | `:command` | Description |
|-----------------|---------------------------------------------|------------|-------------|
| **`F1`** | `<L> ?` / `<L> F1` | `:help` / `:?` | Toggle interactive scrollable Borland Turbo Vision Help Modal |
| **`F2`** / `Alt+H` | `<L> "` / `<L> -` / `<L> h` / `<L> F2` | `:split h` / `:sp` | Split focused composite pane horizontally (Top / Bottom) |
| `Alt+V` | `<L> %` / `<L> \|` / `<L> \` / `<L> v` | `:split v` / `:vs` | Split focused composite pane vertically (Left / Right) |
| **`F3`** / `Alt+Shift+F` | `<L> f` / `<L> F` / `<L> F3` | `:hints` / `:fields` | Toggle QuickSelect 3270 Field Hints (`[a]..[z]`) for URLs, paths, IPv4s, Git SHAs |
| **`F4`** / `Alt+E` | `<L> e` / `<L> m` / `<L> F4` | `:mode [float\|fixed\|terminal\|cycle]` | Cycle between **Floating Prompt**, **Fixed (Docked) Prompt**, and **Regular Terminal** |
| **`F5`** / `Alt+Z` | `<L> z` / `<L> F5` | `:zoom` | Toggle zoom/maximize on the focused composite pane |
| **`F6`** / `Alt+Tab` | `<L> o` / `<L> n` / `<L> Tab` / `<L> F6` | `:deck next` | Shuffle Z-order pane deck top-to-bottom (or focus next pane) |
| `Alt+Shift+Tab` | `<L> p` / `<L> Shift+Tab` | `:deck prev` | Shuffle Z-order pane deck bottom-to-top (or focus previous pane) |
| **`F7`** / `Alt+Shift+D` | `<L> d` / `<L> D` | `:drawer [toggle]` | Toggle `herdr`-style Slide-Out Sessions/Windows/Panes & Fleet Drawer |
| **`F9`** | `<L> i` / `<L> F9` | `:inspector` / `:inspect` | Toggle live 3270 Terminal Inspector modal (DECSET modes, cell `U+XXXX`, CSI/OSC log) |
| **`F10`** / `Alt+\`` / `Ctrl+\` / `Ctrl+Space` | `<L> q` / `<L> \`` / `<L> ~` / `<L> F10` | `:quake` / `:guake` | Toggle persistent Guake/Quake-style drop-down quick terminal |
| **`F12`** / `Ctrl+G` | `<L> g` / `<L> F12` | `:raw` | Toggle between `INPUT: CMD BOX` and `INPUT: RAW PTY` |
| **`Ctrl+P`** / `Alt+Space` | `<L> :` / `<L> Space` | `:palette` / `:cmd` | Open interactive Fuzzy Command Palette (`nucleo-matcher`) |
| `Alt+1` .. `Alt+9` | `<L> 1` .. `<L> 9` | `:focus <1..9>` | Selectively raise and focus pane `#1..#9` |
| `Alt+M` | `<L> M` | `:layout stack\|tile` | Toggle between Tiled Splits and Overlapping Z-Order Cascade |
| `Alt+W` | `<L> w` | `:close` | Close focused composite pane |
| `Alt+L` | `<L> l` | `:lock [on\|off]` | Toggle Read-Only Protected 3270 Field Lock (`🔒 PROT`) on focused pane |
| `Alt+P` | `<L> P` / `<L> H` | `:history` / `:hist` | Toggle Borland Command History Pop-up Window |
| — | `<L> R` | `:record workspace` | Start or stop whole-workspace Asciicast v2 (`.cast`) recording (`⏺ REC`) |
| `Alt+X` | `<L> x` | `:crosshair` | Cycle IBM 3270 Cross-Hair Cursor (`Pane-Local` -> `Full-Screen` -> `Off`) |
| `Alt+O` | `<L> L` | `:spotlight` | Cycle Focal Spotlight shader around active cursor (`Circle` -> `Ellipse` -> `Pixel` -> `Off`) |
| `Alt+Shift+O` | — | `:focalzoom` | Cycle Focal Zoom Magnifying Lens (`1.25x` -> `1.5x` -> `2.0x` -> `Off`) |
| `Alt+}` / `Alt+{` | `<L> }` / `<L> {` | `:spotlight +\|-` | Expand or shrink Focal Spotlight / Focal Zoom lens radius |
| — | `Ctrl+X D` | `:detach [name]` | Detach client & spawn background daemon keeping full multi-pane layout intact |
| — | — | `:attach [name]` | Re-attach & restore saved/detached multi-pane window layout in-place |
| — | — | `:mcp [status\|tree\|...]` | Open interactive Master Control Program Console (`↑`/`↓`/`j`/`k`/`Enter`/Click, `r`, `f`, `n`, `m`, `d`) |
| **`Ctrl+Q`** | `<L> Ctrl+Q` | `:quit` / `:q` / `:quit!` | Exit Bilterm (`:quit` honors Exit Guard; `:quit!` forces immediate exit) |

---

## 3. Per-Pane Font Scaling, Pane Resizing & Window Moving Shortcuts

> **Works Even Inside Full-Screen TUIs (`nmon`, `htop`, `vim`):**
> Per-pane font scaling (`Ctrl++`, `Ctrl+-`, `Ctrl+0`, `Ctrl+MouseWheel`) and pane focus/resize chords are intercepted at the Window Manager layer *before* `RawPtyPassthrough`, so you can scale fonts and resize panes even while an interactive TUI owns the alternate screen!

| Direct Shortcut | Leader Chord (`<L>` = `Ctrl+B` or `Ctrl+X`) | `:command` | Description |
|-----------------|---------------------------------------------|------------|-------------|
| **`Ctrl++`** / **`Ctrl+=`** | `<L> Ctrl++` / `<L> Ctrl+=` | `:fontsize +` | **Zoom In Focused Pane Font** (`+10%` scale step up to `300%`; dynamically recomputes that pane's `cols × rows` PTY grid and sends `TIOCSWINSZ`) |
| **`Ctrl+-`** / **`Ctrl+_`** | `<L> Ctrl+-` / `<L> Ctrl+_` | `:fontsize -` | **Zoom Out Focused Pane Font** (`-10%` scale step down to `50%`; dynamically expands that pane's `cols × rows` PTY grid to fit more columns/rows) |
| **`Ctrl+0`** / **`Ctrl+)`** | `<L> 0` / `<L> Ctrl+0` | `:fontsize reset` | **Reset Focused Pane Font Scale** to `100%` (`1.0x`) |
| **`Ctrl + ScrollWheelUp/Down`** | — | `:fontsize +\|-` | **Per-Pane Mouse Wheel Font Zoom** on the pane directly under the mouse cursor |
| `Ctrl+Alt+Left` / `Alt+Shift+Left` | `<L> Left` / `<L> <` | `:resize left [N]` | Shrink focused pane width (`-4` cols) or move split divider left |
| `Ctrl+Alt+Right` / `Alt+Shift+Right` | `<L> Right` / `<L> >` | `:resize right [N]` | Grow focused pane width (`+4` cols) or move split divider right |
| `Ctrl+Alt+Up` / `Alt+Shift+Up` | — | `:resize up [N]` | Shrink focused pane height (`-2` rows) or move horizontal split divider up |
| `Ctrl+Alt+Down` / `Alt+Shift+Down` | — | `:resize down [N]` | Grow focused pane height (`+2` rows) or move horizontal split divider down |
| `Ctrl+Shift+Left` / `Ctrl+Alt+h` | — | `:move left [N]` | Move floating Z-Stack window left (`-4` cols) or swap tiled pane |
| `Ctrl+Shift+Right` / `Ctrl+Alt+l` | — | `:move right [N]` | Move floating Z-Stack window right (`+4` cols) or swap tiled pane |
| `Ctrl+Shift+Up` / `Ctrl+Alt+k` | `<L> K` | `:move up [N]` | Move floating Z-Stack window up (`-2` rows) |
| `Ctrl+Shift+Down` / `Ctrl+Alt+j` | `<L> J` | `:move down [N]` | Move floating Z-Stack window down (`+2` rows) |
| — | `<L> =` | `:equalize` / `:cascade` / `:resize reset` | Reset all tiled split dividers to 50/50 or reset Z-Stack windows to cascade geometry |

---

## 4. Intra-Pane Command Prompt, Readline, History & Vi Copy Mode Shortcuts

| Keybinding | `:command` | Description |
|------------|------------|-------------|
| `Alt+T` / `<L> t` | `:prompt top` | Pin floating command prompt to Top (IBM 3270 default) |
| `Alt+Shift+B` / `<L> b` (or `Alt+b` when prompt empty) | `:prompt bottom` | Pin floating command prompt to Bottom |
| `Alt+S` / `<L> s` | `:prompt split` / `split-top` / `split-bottom` | Toggle non-overlapping Split vs. Floating command prompt |
| `Alt+Up` / `Alt+Down` / `<L> Up` / `<L> Down` | `:prompt float <row>` | Move floating command prompt vertically row-by-row |
| `Alt+Plus` / `Alt+Minus` / `<L> +` | `:prompt height <3..12>` | Increase / decrease command prompt height (`3..12` rows) |
| `Alt+]` / `Alt+[` / `<L> ]` | `:prompt width <cols\|full>` | Increase / decrease command prompt width (`4` cols per step) |
| `Shift+Enter` / `Alt+Enter` | — | Insert newline inside multi-line command buffer (`==> ` / `... `) |
| `Enter` | — | Submit command (keeping it visible in the `⟳ RUNNING:` / `● LAST CMD:` banner) |
| `Shift+PageUp` / `Shift+PageDown` | — | Scroll multi-line command prompt viewport up / down |
| `Tab` / `Shift+Tab` | — | Context-aware Tab completion (`:commands`, `$PATH`, cwd paths, `$ENV`) |
| `Ctrl+R` | — | Open interactive `nucleo-matcher` fuzzy history search (`Ctrl+R`/`Up`/`Down` cycle, `Enter` accept, `Esc` cancel) |
| `Up` / `Down` | — | Navigate multi-line cursor or recall previous/next command from history |
| `Ctrl+A` / `Home` | — | Move cursor to start of line |
| `Ctrl+E` / `End` | — | Move cursor to end of line |
| `Alt+b` / `Alt+Left` | — | Readline move backward by word |
| `Alt+f` / `Alt+Right` | — | Readline move forward by word |
| `Alt+d` / `Alt+Delete` | — | Readline kill next word |
| `Alt+Backspace` / `Ctrl+W` | — | Readline kill previous word (into yank buffer) |
| `Ctrl+K` | — | Kill from cursor to end of line (into yank buffer) |
| `Ctrl+U` | — | Kill entire line (into yank buffer) |
| `Ctrl+Y` | — | Yank (paste) last killed text |
| `Ctrl+Z` | — | Undo last edit in command prompt |
| `Ctrl+C` | — | Clear current prompt input; if foreground job is running, sends `SIGINT` (`0x03`) to PTY |
| `Alt+PageUp` / `Alt+PageDown` / `<L> PageUp`/`PageDown` | `:block prev\|next` | Jump between `OSC 133` semantic command blocks in scrollback |
| `Alt+/` / `<L> /` | `:search <query>` | Search `OutputCanvas` grid and highlight matches |
| `Alt+n` / `Alt+N` / `<L> N` | `:search next\|prev\|clear` | Jump to next/previous canvas search match |
| `Alt+C` / `<L> [` / `<L> c` | `:copy` / `:vi` | Enter keyboard-driven Vi Copy Mode (`h/j/k/l/w/b/0/$`, `g`/`G`, `v` char, `V` line, `Ctrl+V` rectangular block, `y`/`Enter` yank via `OSC 52`, `q`/`Esc` exit) |

---

## 5. Complete Mouse Gestures Reference

| Mouse Target & Gesture | Action Performed |
|------------------------|------------------|
| **Click `[≡]` or Top Menu Bar items** | Open/navigate interactive Borland Turbo Vision pull-down menu bar |
| **Click `[X]` on pane title bar (left corner)** | Close that composite pane (or hide Quake overlay) |
| **Click `[▲]` / `[▼]` on pane title bar (right corner)** | Toggle Zoom / Maximize (`WM-03`) on that composite pane |
| **Drag ANY Pane Border (`Top`, `Bottom`, `Left`, `Right`)** | **Resize Pane Frame**: In `Tiled` mode, drags the corresponding split divider (or opposite divider for outer edges); in `Stacked` mode, resizes that edge while anchoring the opposite edge |
| **Drag ANY Pane Corner (`TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`)** | **2-Axis Corner Resize**: Simultaneously resizes both horizontal and vertical split dividers (in `Tiled` mode) or both window dimensions (in `Stacked` mode) |
| **Drag Pane Title Bar interior (in `Stacked` mode)** | Move floating Z-Stack window freely across the workspace |
| **Click anywhere inside an unfocused pane** | Focus that pane and selectively raise it to the top of the Z-Stack |
| **Drag Floating Prompt Top Border** | Move floating command prompt vertically and horizontally inside the pane |
| **Click `[▲]` / `[▼]` on Floating Prompt top-right** | Pin floating command prompt to Top or Bottom |
| **Drag Floating Prompt Bottom Border** | Resize command prompt height (`3..12` rows) |
| **Drag Floating Prompt Left or Right Border** | Resize command prompt width (`16..canvas_width` columns) |
| **Drag Floating Prompt Bottom-Right Corner (`◢`)** | Resize command prompt width and height simultaneously |
| **Click OIA Status Bar mode badge (`FLOAT` / `FIXED` / `TERM`)** | Cycle pane between Floating Prompt, Fixed (Docked) Prompt, and Regular Terminal |
| **`Ctrl + Mouse Scroll Wheel` over any pane** | **Per-Pane Font Zoom In / Out** (`+10%` / `-10%`) for the pane under the cursor |
| **Mouse Scroll Wheel inside multi-line prompt** | Scroll multi-line command prompt lines up or down |
| **Mouse Scroll Wheel over `OutputCanvas`** | Scroll VT100 scrollback history up or down (`3` lines per tick) |
| **`Ctrl + LeftClick` on `OutputCanvas` URL / `OSC 8` link** | Copy URL to clipboard via `OSC 52` (`:links`) |
| **Click Bottom DOS Function-Key Bar (`F1 Help` .. `F12 Raw`)** | Execute the clicked function-key action immediately |

---

## 6. Complete `:colon` Command Reference (All Subsystems)

### 6.1 Window Manager, Splits, Z-Stack, Resize & Move

| `:command` | Aliases | Arguments | Description |
|------------|---------|-----------|-------------|
| `:split` | `:sp` | `[h\|v\|horizontal\|vertical]` | Split focused pane horizontally (default `h`) or vertically (`v`) |
| `:vsplit` | `:vs` | *(none)* | Split focused pane vertically (Left / Right) |
| `:close` | — | *(none)* | Close focused composite pane |
| `:zoom` | — | *(none)* | Toggle zoom/maximize on focused pane |
| `:layout` | — | `tile \| tiled \| stack \| cascade \| save [name] \| switch <profile> \| default [name] \| autosave [on\|off] \| list \| next \| prev` | Switch workspace between `Tiled`/`Stacked` or manage/switch multi-pane layout profiles |
| `:profile` | `:profiles` | `[list \| switch <name> \| save [name] \| default [name] \| autosave [on\|off\|toggle] \| next \| prev \| delete <name>]` | List, dynamically switch (`single`, `dual-v`, `dual-h`, `main-side`, `quad`, `cascade`, or custom), save, set startup default, or toggle autosave for layout profiles |
| `:autosave` | — | `[on \| off \| toggle]` | Toggle automatic saving of pane/window layout changes (`ON` by default) |
| `:deck` | — | `[next \| prev]` | Shuffle Z-order window deck top-to-bottom (`next`) or bottom-to-top (`prev`) |
| `:focus` | `:raise` | `<pane_id>` | Focus and selectively raise pane `#<pane_id>` to top of Z-Stack |
| `:resize` | `:winresize` | `<left\|right\|up\|down\|+w\|-w\|+h\|-h\|reset\|<dw> [dh]> [amt]` | Resize focused pane dimensions or adjust tiled split ratios |
| `:move` | `:winmove` | `<left\|right\|up\|down\|reset\|<dx> [dy]> [amt]` | Move floating Z-Stack window or swap tiled panes |
| `:equalize` | `:cascade` | *(none)* | Reset all tiled split dividers to equal 50/50 ratios and reset Z-Stack cascade positions |
| `:quake` | `:guake` | *(none)* | Toggle persistent top-anchored Guake/Quake drop-down quick terminal |
| `:quit` | `:q` | *(none)* | Cleanly exit Bilterm |

### 6.2 Floating Command Prompt, Modes, History & Scrollback

| `:command` | Aliases | Arguments | Description |
|------------|---------|-----------|-------------|
| `:prompt` | — | `top \| bottom \| split-top \| split-bottom \| split \| float [row] \| height <3..12> \| width <16..cols\|full> \| fixed \| floating \| terminal \| cycle` | Control command prompt anchor, non-overlapping split mode, floating row, height, width, or display mode |
| `:mode` | — | `[float \| fixed \| terminal \| cycle]` | Switch pane between Floating Prompt, Fixed (Docked) Prompt, and Regular Terminal |
| `:raw` | — | *(none)* | Toggle between `INPUT: CMD BOX` and `INPUT: RAW PTY` |
| `:history` | `:hist` | `[popup \| save [path] \| on \| off \| toggle \| runs \| max-files <N>]` | Open History Pop-up, export commands to `.sh`, toggle persistence, view run segments, or set max retained session dirs |
| `:block` | — | `prev \| next` | Jump to previous or next `OSC 133` semantic command block in scrollback |
| `:search` | `:find` | `<query> \| next \| prev \| clear \| off` | Search `OutputCanvas` text, jump between matches, or clear search highlights |
| `:copy` | `:vi` | *(none)* | Toggle keyboard-driven Vi Copy Mode |
| `:yank` | — | *(none)* | Copy active selection, command block, or screen via `OSC 52` clipboard |
| `:links` | `:url`, `:urls` | *(none)* | Yank explicit `OSC 8` hyperlink or detected URL on the canvas via `OSC 52` |
| `:hints` | `:hint`, `:fields` | *(none)* | Toggle QuickSelect 3270 Field Hints (`[a]..[z]`) |
| `:clear` | — | *(none)* | Clear `OutputCanvas` and reset scrollback |
| `:scrollback` | `:sb` | `[info \| status \| clear]` | Display active pane scrollback line/memory telemetry or clear scrollback history |
| `:lock` | `:readonly`, `:prot` | `[on \| off]` | Toggle Read-Only Protected 3270 Field Lock (`🔒 PROT`) on active pane |
| `:broadcast` | `:bcast` | `[on \| off]` | Toggle synchronized command broadcast (`📡 BCAST`) across all unlocked panes |

### 6.3 Per-Pane Typography & Dynamic Font Scaling (`:fontsize`, `:font`)

| `:command` | Aliases | Arguments | Description |
|------------|---------|-----------|-------------|
| `:fontsize` | `:textsize` | `[+ \| - \| reset \| <6..72>pt \| <50..300>%]` | Scale the **active pane's** font size and dynamically adjust its terminal `cols × rows` grid |
| `:font` | `:panefont`, `:winfont` | `list \| status \| reset \| [pane <id>\|all] [<preset\|family>] [<pt>pt] [<pct>%] [double [on\|off]] [bold [on\|off]] [italic [on\|off]]` | Configure per-pane or workspace-wide font family (`ibm3270`, `vga`, `tek4014`, `jetbrains`, `firacode`, `iosevka`, `cascadia`, `hack`), point size, scale percentage, `2×1` double-width cell mode, bold, or italic |

### 6.4 Retro Themes, Outlines, Drop Shadows, Shaders & Focal Lenses

| `:command` | Aliases | Arguments | Description |
|------------|---------|-----------|-------------|
| `:theme` | — | `turbo-blue \| turbo-cyan \| 3278 \| 3279 \| mono \| cyberpunk \| solarized \| dracula \| gruvbox \| tokyo` | Switch color theme preset (`UI-02`) |
| `:outline` | `:border` | `[window\|prompt] <double\|solid\|rounded\|thick\|dotted\|dashed\|quadruple\|ascii>` | Set workspace window and/or prompt border outline style |
| `:shadows` | `:shadow` | `[on \| off \| none \| translucent \| tinted \| solid]` | Set or toggle 2-col right / 1-row bottom DOS drop-shadow mode (`UI-01`) |
| `:glyphs` | — | `unicode \| cp437 \| nerd-font \| ascii` | Switch status bar and window badge glyph pack (`UI-04`) |
| `:cursor` | — | `block \| hollow \| underscore \| bar` | Switch software cursor caret style (`UI-05`) |
| `:crosshair` | — | `[on \| off \| full \| pane]` | Toggle or set pane-local vs. full-screen IBM 3270 cross-hair cursor ruler |
| `:crt` | `:scanlines` | `[on \| off]` | Toggle retro CRT alternating scanline luminance shader (`CRT-01`) |
| `:spotlight` | `:spot`, `:focus-light` | `[circle\|ellipse\|off] [fuzzy\|pixel] [full\|pane] [+|-] [size <rx> [ry]] [feather <5..100>]` | Configure radial Focal Spotlight shader around the active cursor |
| `:focalzoom` | `:fzoom`, `:lens`, `:magnify` | `[1.25x\|1.5x\|2x\|3x\|subtle\|med\|high\|ultra\|off] [circle\|ellipse] [fuzzy\|pixel] [full\|pane] [+|-] [grow\|shrink] [size <rx> [ry]] [feather <5..100>]` | Configure radial Focal Zoom magnifying lens over the active cursor |
| `:fkeybar` | — | `[on \| off]` | Toggle bottom 1-row Borland DOS Function-Key bar (`UI-03`) |
| `:menubar` | `:menu` | `[on \| off]` | Toggle top 1-row Borland Turbo Vision pull-down menu bar (`ECO-03`) |

### 6.5 Tektronix 4010/4014 & High-Res Pixel/Sub-Cell Canvas (`:canvas`, `:tek`)

| `:command` | Arguments | Description |
|------------|-----------|-------------|
| `:canvas demo` / `:canvas hires` | *(none)* | Render anti-aliased RGBA pixel/sub-cell scientific & financial showcase scene |
| `:canvas tek` / `:tek [demo]` | *(none)* | Render classic 1974 Tektronix 4014 green/amber storage-tube vector wireframe |
| `:canvas mode` | `<hires \| pixel \| octant \| halfblock \| braille \| sextant \| quadrant \| ascii>` | Select canvas rendering backend (`octant` = `2×4` 2-color TrueColor, `halfblock` = `1×2` TrueColor `▀`/`▄`, `braille` = `2×4` dots) |
| `:canvas res` | `<auto \| 1x \| 2x \| 4x \| 720p \| 1080p \| 4k \| <WxH>>` | Set `PixelCanvasBuffer` resolution preset or exact pixel dimensions |
| `:canvas plot` | `<line \| bar \| scatter \| candlestick \| heatmap> <v1,v2,...>` | Render live high-DPI chart on the active pane canvas |
| `:canvas line` | `<x0> <y0> <x1> <y1>` | Draw anti-aliased vector line segment |
| `:canvas circle` | `<cx> <cy> <r> [filled]` | Draw signed-distance-field anti-aliased circle or disc |
| `:canvas rect` | `<x> <y> <w> <h> [filled]` | Draw vector rectangle or filled box |
| `:canvas bezier` | `<x0> <y0> <cx0> <cy0> <cx1> <cy1> <x1> <y1>` | Draw anti-aliased cubic Bezier curve |
| `:canvas sprite` | `<id> <x> <y> [vx] [vy]` | Spawn or update 2D sprite on the retained scene graph (`:canvas step` to advance physics/AABB collisions) |
| `:canvas image` | `[width] [height]` | Load procedural RGBA test image (`:canvas zoom <f>`, `:canvas pan <dx> <dy>`) |
| `:canvas export` | `<path.svg \| path.ppm>` | Export active pane canvas to standalone vector `.svg` or Netpbm `.ppm` |
| `:canvas clear` | *(none)* | Clear graphical canvas and return pane to normal terminal text |

### 6.6 Slide-Out Sessions/Windows/Panes & Fleet Drawer (`:drawer`)

| `:command` | Aliases | Arguments | Description |
|------------|---------|-----------|-------------|
| `:drawer` | `:sidebar`, `:panels` | `[toggle \| focus \| left \| right \| top \| bottom \| edge <edge> \| fixed \| autohide \| ondemand \| float \| push \| mode <float\|push> \| size <N>]` | Toggle, focus, dock (`Left`/`Right`/`Top`/`Bottom`), pin (`Fixed`/`AutoHide`/`OnDemand`), or set layout (`Overlay`/`Push`) of the `herdr`-style Slide-Out Sessions/Windows/Panes & Fleet Drawer |

### 6.7 Remote SSH Domains, Remote `tmux -CC` & `herdr` Fleet Federation

| `:command` | Aliases | Arguments | Description |
|------------|---------|-----------|-------------|
| `:ssh` | — | `<user@host \| profile> [port]` | Connect active pane to remote SSH host or named `[remotes.<profile>]` (`SSH:<host>` badge in OIA) |
| `:ssh-tmux` | `:sshtmux` | `<user@host \| profile> [session]` | Attach or spawn persistent remote `tmux -CC` session over SSH |
| `:remote` | `:fleet`, `:herdr` | `list \| add <name> <user@host> [@tag1,@tag2] \| exec <all\|local\|remote\|@tag\|node> <cmd> \| disconnect <name>` | Register remote fleet nodes, inspect fleet status, fan out commands by `@tag`, or disconnect nodes |

### 6.8 Workspace Sessions, Master Control Program (`:mcp`), Exit Guard, Recording & Inspection

| `:command` | Aliases | Arguments | Description |
|------------|---------|-----------|-------------|
| `:detach` | — | `[name]` | Save full multi-pane layout (`SplitTree` + `ZStack` + prompts + screen lines), spawn background daemon (`daemon-<session>.sock`), and detach |
| `:attach` | — | `[name]` | Re-attach to live background daemon via JSON-RPC or restore saved `.toml` workspace layout |
| `:mcp` | — | `[status \| tree \| float \| push [edge] \| attach <node> \| spawn <parent/child> \| graft <src> <dst> \| detach-subtree <node> \| broadcast <node> <cmd> \| freeze <node> \| thaw <node> \| derez <node> --confirm \| checkpoint \| shutdown --confirm]` | Master Control Program Root Console (`↑`/`↓`/`j`/`k`/`Enter`/Click to activate row, `r`=Refresh, `f`=Freeze/Thaw, `n`=Branch, `m`=Float↔Push, `d`=Edge, `Esc`=Close) |
| `:exit-guard` | `:exitguard` | `[guard \| respawn \| detach \| off]` | Configure final/root shell exit protection (`[r]` Respawn, `[d]` Detach, `[q]` Quit) |
| `:respawn-pane` | `:respawn` | *(none)* | Respawn an exited shell in-place inside a held pane |
| `:scroll` | `:more`, `:pagehold` | `[smooth \| page \| instant \| toggle \| next \| prev \| release \| clear]` | Configure Mainframe Page-Hold (`MORE...`) & Smooth Scroller (`PF8`/`PA2`/`Space`/Click OIA `SCRL`) |
| `:session` | `:workspace` | `save [name] \| restore [name] \| list \| tree \| branch <parent/child> \| graft <src> <dst> \| detach-subtree <node> \| checkpoint [slot]` | Save/restore multi-pane workspace topology and manage hierarchical `SessionTree` (`$XDG_STATE_HOME/bilterm/sessions/<name>.toml`) |
| `:record` | `:rec`, `:asciinema` | `[start [path.cast] \| workspace [path.cast] \| pane [path.cast] \| stop]` | Start/stop whole-workspace (`WorkspaceRecorder`) or single-pane (`SessionRecorder`) Asciicast v2 `.cast` recording |
| `:trigger` | `:triggers` | `add <pattern> \| clear` | Highlight matching case-insensitive tokens on the `OutputCanvas` and increment OIA trigger counter |
| `:notifications` | `:notify`, `:bell` | `clear` | Clear `OSC 9` / `OSC 777` desktop notification and bell counter (`🔔`) |
| `:inspector` | `:inspect` | `[on \| off]` | Toggle live `<F9>` 3270 Terminal Inspector modal |
| `:palette` | `:commands`, `:cmd` | *(none)* | Toggle `<Ctrl+P>` Fuzzy Command Palette modal |
| `:help` | `:h`, `:?` | *(none)* | Toggle `<F1>` Borland Turbo Vision Help modal |

---

[← Chapter 5: tmux Integration](05-tmux-integration.md) | [User Guide Home](README.md) | [Chapter 7: Troubleshooting & FAQ →](07-troubleshooting-and-faq.md) | [Chapter 8: Extended CLI & GUI Examples →](08-extended-cli-and-gui-examples.md)
