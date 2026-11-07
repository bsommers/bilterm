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
| `--version` | `-v`, `-V` | *(none)* | Print version string (`bilterm 3.0.4`) |
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
| **`F7`** / `Alt+Shift+D` | `<L> d` / `<L> D` | `:drawer [toggle]` | Toggle Navigator-style Slide-Out Sessions/Windows/Panes & Fleet Drawer |
| — | `<L> !` | `:drawer attention` | Focus the next pane needing attention (blocked, then errored, then done) |
| `Ctrl+Alt+A` | — | `:agents toggle` | Open the Agent Traffic window, or close it when open (also menu View > Agent Traffic and the Navigator footer `@ agents` button; every `F` key is already taken) |
| `Ctrl+Alt+N` | — | `:agents list toggle` | Open the AGENTS list window (one row per pane, with agent state), or close it when open. Also menu View > Agents List; works while a full-screen program owns the pane. See [Chapter 11](11-navigator-sidebar-and-agent-traffic.md#agents-list-window) |
| — | `<L> A` | `:agents list toggle` | The same toggle as a leader chord, for hosts that swallow `Ctrl+Alt+N` |
| — | `<L> y` | `:agents demo` | Start the agent traffic demo (memory only; re-running replaces the earlier demo). Also menu View > Agent Demo |
| — | `<L> Y` | `:agents demo stop` | Stop the demo: close only the panes it created and remove its traffic. Also menu View > Stop Agent Demo |
| `Ctrl+Alt+F` | — | `:find agent` | Open the Find window (agents, sessions and command history; `Tab` switches kind), or close it when open. Also View > Find Agents. No leader chord ([Chapter 12](12-find-registry-and-history.md)) |
| — | `<L> a` | `:agents zoom` | Open the Agent Traffic window (graph + conversation); see [Chapter 11](11-navigator-sidebar-and-agent-traffic.md). `Ctrl+O` inside it cycles graph / conversation / compose; `F6` keeps its global `DeckNext` meaning |
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
| `Ctrl+Alt+R` | `<L> U` | `:ssh-tmux reconnect` | Reconnect persistent remote tmux link now (skips retry timer; reserved in raw passthrough; menu Tmux > Reconnect link) |
| — | `<L> u` | `:ssh-tmux prefill` | Open prompt prefilled with `:ssh-tmux ` ready to type `<user@host> [session]` (menu Tmux > SSH tmux link...) |
| — | `<L> Q` | `:ssh-tmux disconnect` | Stop persistent remote tmux link; remote tmux session keeps running (menu Tmux > Disconnect link) |
| `Alt+X` | `<L> x` | `:crosshair` | Cycle IBM 3270 Cross-Hair Cursor (`Pane-Local` -> `Full-Screen` -> `Off`) |
| `Alt+O` | `<L> L` | `:spotlight` | Cycle Focal Spotlight shader around active cursor (`Circle` -> `Ellipse` -> `Pixel` -> `Off`) |
| `Alt+Shift+O` | — | `:focalzoom` | Cycle Focal Zoom Magnifying Lens (`1.25x` -> `1.5x` -> `2.0x` -> `Off`) |
| `Alt+}` / `Alt+{` | `<L> }` / `<L> {` | `:spotlight +\|-` | Expand or shrink Focal Spotlight / Focal Zoom lens radius |
| — | `Ctrl+X D` | `:detach [name]` | Detach client & spawn background daemon keeping full multi-pane layout intact |
| — | — | `:attach [name]` | Re-attach & restore saved/detached multi-pane window layout in-place |
| — | — | `:mcp [status\|tree\|...]` | Open interactive Master Control Program Console (`↑`/`↓`/`j`/`k`/`Enter`/Click, `r`, `f`, `n`, `m`, `d`) |
| **`Ctrl+Q`** | `<L> Ctrl+Q` | `:quit` / `:q` / `:quit!` | Exit Bilterm (`:quit` honors Exit Guard; `:quit!` forces immediate exit) |

### AGENTS list window keys (while the list has the keyboard)

The list takes the keyboard when it opens and when you click inside it. Table `AGENTS_LIST_BINDINGS` in `src/input/agents_list_keys.rs`; the leader key is never consumed, and other keys are swallowed while the list has focus. Full detail: [Chapter 11](11-navigator-sidebar-and-agent-traffic.md#agents-list-window).

| Key | Action (colon twin) |
|-----|---------------------|
| `Up` / `k`, `Down` / `j` | Select previous / next row (`:agents list select up` / `down`) |
| `PageUp` / `PageDown` | Select a page up / down (`:agents list select page-up` / `page-down`) |
| `Home` / `End` | First / last row (`:agents list select first` / `last`) |
| `Enter` | Focus the selected pane and leave the list (`:agents list jump`) |
| `z` | Open the traffic window on the selected pane (`:agents list zoom`) |
| `f` | Toggle the agents filter (`:agents list filter toggle`) |
| `*` | Pin / unpin (`:agents list pin toggle`) |
| `[` / `]` | Panel scale -10 / +10 (`:agents list scale -10` / `+10`) |
| `Shift`+arrows | Move the window one cell (`:agents list move <dx> <dy>`) |
| `Alt`+arrows | Resize the window one cell (`:agents list resize <dw> <dh>`) |
| `d` | Dock into the tiling strip, or float (`:agents list dock toggle`) |
| `Esc` | Close, or release the keyboard when pinned (`:agents list dismiss`) |
| `Ctrl+Alt+N` | Close the list (`:agents list toggle`) |

### Interactive Link Banner & Held Pane Keys

When an active pane shows an interactive link banner (`Backoff`, `Failed`, `Detached`, or `Ended` state):

| Key | `:command` twin | Effect |
|-----|-----------------|--------|
| `Enter` / `r` / `R` | `:ssh-tmux reconnect` | Reconnect persistent remote tmux link immediately (skips backoff timer) |
| `x` / `X` | `:ssh-tmux disconnect` | Stop remote tmux link; remote tmux session keeps running |
| `d` / `D` | `:detach` | Detach Bilterm client |

Non-interactive banners (`Connecting`) pass keystrokes directly to the PTY raw so password and SSH prompts are never swallowed.

### Menu bar entries added in Phase 51 & Phase 51.1

| Menu entry | Runs |
|------------|------|
| View > Agents List (Ctrl+Alt+N) | `:agents list toggle` |
| View > Agent Demo (Ctrl+X y) | `:agents demo` |
| View > Stop Agent Demo (Ctrl+X Y) | `:agents demo stop` |
| Window > Tile Windows & Panels | `:arrange grid` |
| Tmux > SSH tmux link... (Ctrl+X u) | `:ssh-tmux prefill` |
| Tmux > Reconnect link (Ctrl+Alt+R) | `:ssh-tmux reconnect` |
| Tmux > Disconnect link (Ctrl+X Q) | `:ssh-tmux disconnect` |
| Tmux > Link info | `:ssh-tmux info` |

Every colon verb has a command-palette entry (`Ctrl+P`, then type) or a recorded exclusion with a reason. `:arrange` has no key chord: use the palette (`Ctrl+P`, then `arrange`) or the Window menu. The full mapping is in [Chapter 11, Surface uniformity](11-navigator-sidebar-and-agent-traffic.md#surface-uniformity).

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
| `Alt+C` / `<L> [` / `<L> c` | `:copy` / `:vi` | Enter keyboard-driven Vi Copy Mode (`h/j/k/l/w/b/0/$`, `g`/`G`, `v` char, `V` line, `r`/`R`/`Ctrl+V` rectangular block, `y`/`Enter` yank via `OSC 52`, `q`/`Esc` exit) |
| `Alt+R` | `:rect` / `:select` / `:copy rect` | Enter 2D Rectangular Block Selection Mode (`INPUT: RECT [F8/Enter/Esc]`) at the current cursor |

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
| **`Alt + LeftClick + Drag` over `OutputCanvas`** | **2D Rectangular Block Selection**: Drag-select a rectangular region on the canvas (even inside `RawPtyPassthrough` apps); keeps highlight visible on release and yanks the block to the system/OSC 52 clipboard (or plain `LeftDrag` when `:rect mode on` is active) |
| **`Ctrl + LeftClick` on `OutputCanvas` URL / `OSC 8` link** | Copy URL to clipboard via `OSC 52` (`:links`) |
| **Click Bottom DOS Function-Key Bar (`F1 Help` .. `F12 Raw`)** | Execute the clicked function-key action immediately |
| **Navigator drawer: click a row** | Select and focus that row's pane (`:drawer row <n>` then `:drawer enter`) |
| **Navigator drawer: click a fold marker (`▾` / `▸`, ASCII `-` / `+`)** | Fold or unfold that workspace or session (`:drawer fold`) |
| **Navigator drawer: click `[+]` after a session or workspace name** | Add a window to that session (key `a`); the `[+]` beside the `WORKSPACES` title creates a session (key `N`) |
| **Navigator drawer: right-click a row** | Select the row and open a menu at the pointer: New session, New window here, Split horizontal, Split vertical, Rename, Close (`:drawer menu`, `:drawer menu pick <n>`); `Esc`, any key or a click elsewhere dismisses it. Right-click outside the drawer still pastes |
| **Navigator drawer: footer buttons `* ! N + r x @ agents`** | Pin, jump to attention, new session, new pane, rename, close the selected pane, toggle the agent traffic window (opens on the selected pane); each is the key of the same character (buttons that do not fit are dropped from the right) |
| **Navigator drawer: title badges, wheel, edge drag** | Minimize / maximize / close; wheel moves the selection by 3 rows; dragging the edge resizes `navigator_width_cols`; `Ctrl + wheel` scales the drawer on the GUI surface |
| **Navigator drawer: click the comms glyph (`⇄you`, `⋔3`)** | Open the Agent Traffic window on that conversation (`:drawer comms`) |
| **Agent Traffic window: drag title bar / edge / corner** | Move / resize the window (`:agents move`, `:agents resize`); the top edge is the title bar, its corners resize |
| **Agent Traffic window: title badges `[□]` `[X]` and pin `◇`/`◆`** | Maximize / close / pin or unpin (`:agents max`, `close`, `pin`, `unpin`) |
| **Agent Traffic window: drag the splitter** | Resize graph vs conversation, 20 to 80 percent (`:agents split`) |
| **Agent Traffic window: click / drag / right-click a node** | Select (second click focuses the pane) / move and pin / unpin (`:agents select`, `enter`, `node-pin`, `node-unpin`) |
| **Agent Traffic window: click an edge** | Select it; `Enter` or a second click filters the conversation to that pair (`:agents edge`, `:agents filter pair`) |
| **Agent Traffic window: conversation line, wheel, `[copy]`, filter buttons, chips, follow marker** | Select / scroll / copy / filter / follow (`:agents line`, `scroll`, `copy`, `filter ...`, `follow`); `[send]` is disabled |
| **`Ctrl` + wheel over the drawer, `:mcp` console or Agent Traffic window** | Scale that panel on the GUI surface (`:drawer scale`, `:agents scale`). Real GUI windows: Linux/X11 only; on Windows and macOS `Ctrl` + wheel over a panel still zooms the font (use `[` `]` or the scale commands) |
| **AGENTS list: click a row** | Select it; click the selected row again (while the list has the keyboard) to jump to its pane (`:agents list select <id>`, `:agents list jump`). An unfocused list takes the keyboard and selects; it does not jump |
| **AGENTS list: right-click a row** | Open the Agent Traffic window on that pane (`:agents list zoom <id>`) |
| **AGENTS list: title badges** | Pin / unpin, minimize (dismiss), dock / float, close (`:agents list pin toggle`, `dismiss`, `dock toggle`, `close`) |
| **AGENTS list: footer chips `[filter]` `[dock]` `[zoom]`** | Toggle the filter; dock / float; open the traffic window on the selection (`:agents list filter toggle`, `dock toggle`, `zoom`) |
| **AGENTS list: drag the title bar, an edge or a corner** | Move / resize the window (the drag sends `:agents list rect` as it moves; the keys and colon use `move` and `resize`) |
| **AGENTS list: wheel / `Ctrl` + wheel** | Scroll three rows / step the window scale by 10 percent on the GUI surface (`:agents list scroll`, `scale +n`/`-n`) |
| **Open menu-bar dropdown** | Takes the click before the AGENTS list and Agent Traffic windows (a click on a dropdown item runs that item) |
| **Click on interactive link banner row** | Reconnect persistent remote tmux link immediately (`RemoteLink(Reconnect)` / `:ssh-tmux reconnect`) |
| **Click on held remote exit banner row** | Respawn exited shell in-place inside held pane (`RespawnPaneInPlace` / `:respawn-pane`) |

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
| `:search` | `:find` | `<query> \| next \| prev \| clear \| off` | Search `OutputCanvas` text, jump between matches, or clear search highlights. `:find <text>` does the same, except that `:find agent`, `:find session` and `:find history` open the Find window instead ([Chapter 12](12-find-registry-and-history.md)); use `:search <word>` to search for one of those words |
| `:find` | — | `agent \| session \| history [text]` | Open the Find window: fuzzy, ranked search of agents and panes, live and saved sessions, or command history. Every row shows its source (`[live]`, `[saved:<name>]`, `[log:<name>]`); secrets are masked and counted, never shown ([Chapter 12](12-find-registry-and-history.md)) |
| `:finder` | — | `kind <agent\|session\|history\|next\|prev> \| query [text] \| move <+n\|-n\|top\|bottom> \| select <n> \| enter \| prefill \| close` | Drive the open Find window from the keyboard or a script. No palette entry: the window must be open |
| `:copy` | `:vi` | `[rect \| block \| line \| char \| all \| yank \| cut \| clear \| mode [on\|off\|toggle]]` | Toggle keyboard-driven Vi Copy Mode or execute rectangular/character/line copy actions |
| `:rect` | `:select`, `:rect-select`, `:box-select` | `[mode [on\|off\|toggle] \| <r1> <c1> <r2> <c2> [copy\|highlight] \| all [copy] \| copy \| yank \| cut \| clear \| off \| status]` | Enter 2D rectangular block selection mode, toggle persistent mouse rectangular drag mode, highlight/copy an exact 1-based `(r1, c1)..(r2, c2)` rectangle, or yank/cut/clear the rectangular selection |
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
| `:theme` | — | `<preset\|file.itermcolors> \| window <spec> \| list \| browse [dir] \| import [file\|dir\|glob] \| export [file.toml] \| config` | Switch color theme preset, apply per-window theme, import `.itermcolors`/TOML/YAML files/directories/globs, open interactive browser, or inspect `~/.config/bilterm/` (`UI-02`) |
| `:window` | `:win`, `:pane-style` | `[menu \| show \| reset \| theme [name\|list\|browse\|import\|next\|prev\|reset] \| outline <style\|next\|reset> \| bold [on\|off\|reset] \| <font-color\|bg\|prompt-bg\|prompt-color\|cursor-color\|frame-color\|inactive-border\|title-color\|controls-color\|status-color\|badge-color> [color\|reset]]` | Per-window theme browser, individual theme element color overrides (`#RRGGBB` or named colors), per-window outline style, and bold border |
| `:browse` | `:files`, `:filebrowser` | `[dir \| file]` | Open interactive Borland Text File & Theme Directory Browser (`Up`/`Down`/`Enter`/`Backspace`/type-to-filter) |
| `:outline` | `:border` | `[window\|prompt] <double\|solid\|rounded\|thick\|dotted\|dashed\|quadruple\|ascii>` | Set workspace window and/or prompt border outline style |
| `:shadows` | `:shadow` | `[on \| off \| none \| translucent \| tinted \| solid]` | Set or toggle 2-col right / 1-row bottom DOS drop-shadow mode (`UI-01`) |
| `:glyphs` | — | `unicode \| cp437 \| nerd-font \| ascii` | Switch status bar and window badge glyph pack (`UI-04`) |
| `:cursor` | — | `[list \| reset \| block \| hollow \| underline \| underscore \| bar \| custom <char> \| blink \| solid \| rapid \| color <#hex\|name\|theme>]` | Open interactive cursor selector or set cursor shape, custom character, blink mode (`blink`/`solid`/`rapid`), and color (`#RRGGBB` or named preset) (`UI-05`) |
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
| `:drawer` | `:sidebar`, `:panels` | `[toggle \| focus \| left \| right \| top \| bottom \| edge <edge> \| fixed \| autohide \| ondemand \| float \| push \| mode <float\|push> \| size <N> \| style <classic\|navigator\|toggle>]` | Toggle, focus, dock (`Left`/`Right`/`Top`/`Bottom`), pin (`Fixed`/`AutoHide`/`OnDemand`), or set layout (`Overlay`/`Push`) of the Navigator-style Slide-Out Sessions/Windows/Panes & Fleet Drawer; `style navigator` selects the compact agent-aware Navigator tree (`herdr` is a deprecated alias; config key `navigator_width_cols`) |

**Navigator drawer keys.** With `drawer.style = navigator` and the drawer focused, these keys are consulted first (declarative table `NAVIGATOR_DRAWER_BINDINGS` in `src/input/keymap.rs`); every other key keeps its classic drawer meaning (`q`, `*`, `[`, `]`, `d`, `m`, `p`, `J`/`K`, Shift/Alt+arrows, ...). Each key lowers to an `Action` and has a `:drawer` command twin.

| Key | `:drawer` twin | Effect |
|-----|----------------|--------|
| `j` / `Down`, `k` / `Up` | `:drawer next`, `:drawer prev` | Move the selection |
| `Enter` / `Space` | `:drawer enter` | Focus the selected row |
| `!` | `:drawer attention` | Jump to the next pane needing attention |
| `e` | `:drawer fold` | Fold or unfold the selected session (a pane row folds its session) |
| `E` | `:drawer expand all`, `:navigator expand all` | Unfold every workspace and session |
| `C` | `:drawer collapse all`, `:navigator collapse all` | Fold everything to workspace rows; pressing it again on a fully folded tree unfolds all |
| `N` | `:drawer new-session` | Create a new session and switch to it |
| `a` | `:drawer add window [session]`, `:navigator add window [session]` | Add a window to the selected (or named) session and focus it; a selected pane row is the split target. In navigator style this replaces the classic `a` (arrange grid), which stays available as `:arrange grid` |
| `A` | `:drawer add pane [h\|v]`, `:navigator add pane [h\|v]` | Split the selected pane (horizontal by default) and focus the new pane. In navigator style this replaces the classic `A` (arrange stack-grid), available as `:arrange stack-grid` |
| `n` / `c` / `+` / `w` | `:drawer new-pane` (`new-window`) | Split the focused pane into a new window |
| `r` / `R` | `:drawer rename [name]` | Rename the selected session (without a name the prompt opens prefilled) |
| `x` / `Delete` | `:drawer close-pane` | Close the selected pane (`:drawer close` still minimizes the drawer) |
| `PageUp` / `PageDown` | `:drawer scroll -5`, `:drawer scroll 5` | Move the selection by five rows |
| `{` / `}` | `:drawer widen -2`, `:drawer widen 2` | Narrow or widen the drawer (`navigator_width_cols`) |
| (mouse click) | `:drawer row <n>` | Select row `n` (1-based) |
| (mouse right-click) | `:drawer menu`, `:drawer menu close`, `:drawer menu pick <n>` | Open, dismiss, or run an entry of the row context menu (entries apply `N`, `a`, `A` (horizontal and vertical), `r` and `x`) |
| `Right` / `o` / `Enter` on the glyph | `:drawer comms-focus on\|off`, `:drawer comms` | Focus the comms glyph of the selected row; open the Agent Traffic window on its conversation |
| (none) | `:drawer style [classic\|navigator\|toggle]` | Switch drawer style; see [Chapter 11](11-navigator-sidebar-and-agent-traffic.md) |
| (none) | `:drawer rows 1\|2` | Lines per Navigator pane row (`drawer.row_lines`); see [Chapter 11](11-navigator-sidebar-and-agent-traffic.md) |

`:navigator` is an alias of `:drawer` for the Navigator tree (`:navigator expand all`, `:navigator collapse all`, `:navigator style classic`). `:drawer expand` without `all` keeps its older meaning (maximize the drawer).

### 6.7 Remote SSH Domains, Remote `tmux -CC` & `herdr` Fleet Federation

| `:command` | Aliases | Arguments | Description |
|------------|---------|-----------|-------------|
| `:ssh` | — | `<user@host \| profile> [port]` | Connect active pane to remote SSH host or named `[remotes.<profile>]` (`SSH:<host>` badge in OIA) |
| `:ssh-tmux` | `:sshtmux` | `<user@host \| profile> [session]` | Attach or spawn persistent remote `tmux -CC` session over SSH (`ssh -tt`) |
| `:ssh-tmux reconnect` | — | *(none)* | Reconnect the persistent remote tmux link immediately (skips backoff timer; `Ctrl+Alt+R` / `Ctrl+X U`) |
| `:ssh-tmux disconnect` | — | *(none)* | Stop the remote tmux link; remote tmux session keeps running on the host (`Ctrl+X Q`) |
| `:ssh-tmux info` | — | *(none)* | Show tmux link state, retry timer, last exit reason and mapped panes |
| `:ssh-tmux prefill` | — | *(none)* | Open prompt prefilled with `:ssh-tmux ` ready to type `<user@host> [session]` (`Ctrl+X u`) |
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
| `:session` | `:workspace` | `new [name] \| blank [name] \| rename [old] <new> \| switch <name> \| delete <name> \| move [name] <dir> \| reorder <s1..> \| swap <a> <b> \| save [name] \| restore [name] \| processes [on\|off\|toggle] \| cmd [<command>\|clear] \| shell [<prog> [args..]] \| cwd [<path>] \| relaunch \| info \| list \| tree \| branch <parent/child> \| graft <src> <dst> \| detach-subtree <node> \| checkpoint [slot]` | Create, rename, reorder, save, and restore multi-pane workspace sessions (including per-pane `shell`, `shell_args`, `running_command`, and `cwd`), configure auto-relaunch (`session_restore_processes`), and manage hierarchical `SessionTree` (`$XDG_STATE_HOME/bilterm/sessions/<name>.toml`) |
| `:record` | `:rec`, `:asciinema` | `[start [path.cast] \| workspace [path.cast] \| pane [path.cast] \| stop]` | Start/stop whole-workspace (`WorkspaceRecorder`) or single-pane (`SessionRecorder`) Asciicast v2 `.cast` recording |
| `:trigger` | `:triggers` | `add <pattern> \| clear` | Highlight matching case-insensitive tokens on the `OutputCanvas` and increment OIA trigger counter |
| `:notifications` | `:notify`, `:bell` | `clear` | Clear `OSC 9` / `OSC 777` desktop notification and bell counter (`🔔`) |
| `:inspector` | `:inspect` | `[on \| off]` | Toggle live `<F9>` 3270 Terminal Inspector modal |
| `:palette` | `:commands`, `:cmd` | *(none)* | Toggle `<Ctrl+P>` Fuzzy Command Palette modal |
| `:help` | `:h`, `:?` | *(none)* | Toggle `<F1>` Borland Turbo Vision Help modal |

**`:agents` (Agent state and the Agent Traffic window, Phase 50).** Every verb lowers to an `Action` and has a key and/or mouse twin; the full table is in [Chapter 11](11-navigator-sidebar-and-agent-traffic.md#commands).

| `:command` | Arguments | Description |
|------------|-----------|-------------|
| `:agents` | `[list]` | Agent state per pane (working / blocked / idle / done / errored) with counts; structured data over RPC |
| `:agents` | `zoom [pane-id] \| toggle [pane-id] \| close \| dismiss` | Open / toggle / close the Agent Traffic window; `dismiss` is `Esc` (releases the keyboard when pinned) |
| `:agents` | `select <left\|right\|up\|down\|node-key> \| edge <next\|prev\|a b> \| enter` | Node and edge cursor |
| `:agents` | `move <dx> <dy> \| resize <dw> <dh> \| rect <x> <y> <w> <h> \| size <cols>x<rows>\|auto \| max \| split <20..80\|+n\|-n>` | Window geometry and splitter |
| `:agents` | `grab [on\|off] \| nudge <dir> \| node-pin <key> <x> <y> \| node-unpin [key]` | Node placement |
| `:agents` | `pin [toggle] \| unpin \| scale <75..200\|+n\|-n>` | Pin the window open; own panel scale (GUI surface) |
| `:agents` | `focus [graph\|snoop\|compose] \| line <+n\|-n\|seq> \| copy \| scroll <±n\|top> \| follow [on\|off]` | Conversation navigation |
| `:agents` | `filter agent [label] \| pair [a b] \| text <kw> \| clear \| remove <agent\|pair\|text>` | Conversation filters (AND-combined, no regex) |

---

[← Chapter 5: tmux Integration](05-tmux-integration.md) | [User Guide Home](README.md) | [Chapter 7: Troubleshooting & FAQ →](07-troubleshooting-and-faq.md) | [Chapter 8: Extended CLI & GUI Examples →](08-extended-cli-and-gui-examples.md)
