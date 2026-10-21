# Bilterm User Manual

Welcome to the official User Manual for **Bilterm** — the Rust + Ratatui terminal multiplexer, shell wrapper, and terminal emulator inspired by **IBM 3270 Mainframe Terminals** and **DOS Borland Turbo Vision (Turbo C / Turbo Pascal IDE)**.

> **Quick Interactive Help (`<F1>`)**: At any time inside Bilterm, press **`F1`** or run **`:help`** to open the scrollable Borland Turbo Vision Help Modal directly on screen (`Up`/`Down`/`PageUp`/`PageDown` to scroll, `F1`/`Esc` to close).

---

## Table of Contents

1. **[Chapter 1: Getting Started & Core Concepts](01-getting-started.md)**
   - Installation, Building from Source, and Launching Bilterm
   - Understanding the **Composite Window Pane** (`OutputCanvas` + `FloatingCommandPrompt` + `OIA Status Bar`)
   - Non-Overlapping **Split Command / Output Mode** (`Alt+S` / `:prompt split-top | split-bottom`) vs. **Floating Prompt Mode**
   - Automatic Alternate Screen (`vim`, `htop`) and `!ECHO LOCK` (`sudo`, `ssh`) Protection
2. **[Chapter 2: The Command Prompt, Tab Completion, History & Command Blocks](02-floating-command-prompt.md)**
   - Pinning Top, Bottom, Free-Floating, or Non-Overlapping Split (`Alt+T`, `Alt+B`, `Alt+S`, Mouse Drag, `:prompt`)
   - Resizable Width (`Alt+[` / `Alt+]`, Mouse Left/Right/Corner Drag, `:prompt width <cols|full>`) & Height (`Alt+Plus`/`Alt+Minus`)
   - Scrollable Multi-Line Content (`Shift+PageUp` / `Shift+PageDown`, Mouse Scrollbar) & Running Command Banner (`⟳ RUNNING:` / `● LAST CMD:`)
   - Interactive Tab Completion (`Tab` / `Shift+Tab`), Fuzzy History Search (`Ctrl+R`), and the Borland **Command History Pop-up Window** (`Alt+P` / `:history`)
   - Persistent Session History Files (`session_id`, `pane_id`, ISO-8601 timestamps, file rotation) & Temp-File Export (`:history save`)
   - `OSC 133` Command Block Scrollback Jumping (`Alt+PageUp` / `Alt+PageDown`) & `OSC 52` Clipboard Yank
3. **[Chapter 3: Multi-Pane Window Manager, Z-Order Stacking & Guake Quick Terminal](03-window-manager-and-z-stack.md)**
   - Horizontal & Vertical Binary Splits (`F2` / `Alt+H`, `Alt+V`, `:split`)
   - Z-Order Window Stacking, Deck Shuffling (`Alt+Tab` / `F6`), and Selective Raise (`Alt+1..9`, Mouse Click)
   - Persistent Guake/Quake-Style Drop-Down Quick Terminal (`F10` / `Alt+\`` / `Ctrl+Space` / `:quake`)
4. **[Chapter 4: Retro Mainframe & DOS Aesthetics, Outlines, Drop Shadows, Cross-Hairs & Config](04-themes-glyphs-and-config.md)**
   - Selectable Window & Prompt Border Outlines (`double`, `solid`, `rounded`, `thick`, `dotted`, `dashed`, `quadruple`, `ascii`)
   - Themeable DOS Drop Shadows (`translucent`, `tinted`, `solid`, `none`)
   - Color Theme Presets (`turbo-blue`, `turbo-cyan`, `ibm-3278-green`, `ibm-3279-amber`, `ibm-mono`)
   - IBM 3270 Cross-Hair Cursor Ruler: **Pane-Local** (`:crosshair on`) vs. **Full-Screen** (`:crosshair full` / `Alt+X`)
   - Status Bar Icons, Live `R:xx C:xxx` Coordinate Counter & `OUT:<lines>L/<bytes>` Output Length Telemetry
   - Declarative `~/.config/bilterm/config.toml` & `.ini` Theme Import
5. **[Chapter 5: First-Class `tmux` Integration](05-tmux-integration.md)**
   - Running Bilterm Inside Outer `$TMUX` Sessions (`DCS tmux;` Passthrough)
   - Controlling `tmux` via `tmux -CC` Control Mode
6. **[Chapter 6: Complete Keyboard, Mouse & `:command` Reference](06-keyboard-mouse-and-command-reference.md)**
   - Full Shortcut Tables, Function Keys (`F1`–`F12`), Mouse Gestures, and Colon Commands
