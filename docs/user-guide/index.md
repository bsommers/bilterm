# Bilterm User Manual

Welcome to the official User Manual for **Bilterm** — the Rust + Ratatui terminal multiplexer, shell wrapper, and terminal emulator inspired by **IBM 3270 Mainframe Terminals** and **DOS Borland Turbo Vision (Turbo C / Turbo Pascal IDE)**.

---

## Table of Contents

1. **[Chapter 1: Getting Started & Core Concepts](01-getting-started.md)**
   - Installation, Building from Source, and Launching Bilterm
   - Understanding the **Composite Window Pane** (`OutputCanvas` + `FloatingCommandPrompt` + `OIA Status Bar`)
   - Automatic Alternate Screen (`vim`, `htop`) and `!ECHO LOCK` (`sudo`, `ssh`) Protection
2. **[Chapter 2: The Floating Command Prompt & Command Blocks](02-floating-command-prompt.md)**
   - Pinning Top, Bottom, or Free-Floating via Keyboard, Mouse Drag, or `:prompt`
   - Multi-Line Editing, Live Shell Syntax Highlighting, and Undo/Redo
   - Interactive Fuzzy History Search (`Ctrl+R`)
   - `OSC 133` Command Block Scrollback Jumping (`Alt+PageUp` / `Alt+PageDown`) & `OSC 52` Clipboard Yank
3. **[Chapter 3: Multi-Pane Window Manager, Z-Order Stacking & Guake Quick Terminal](03-window-manager-and-z-stack.md)**
   - Horizontal & Vertical Binary Splits (`Alt+H`, `Alt+V`, `:split`)
   - Z-Order Window Stacking, Deck Shuffling (`Alt+Tab` / `F6`), and Selective Raise (`Alt+1..9`, Mouse Click)
   - Persistent Guake/Quake-Style Drop-Down Quick Terminal (`Alt+\`` / `Ctrl+Space` / `:quake`)
4. **[Chapter 4: Retro Mainframe & DOS Aesthetics, Themes, Glyphs & Configuration](04-themes-glyphs-and-config.md)**
   - Borland Turbo Vision Chrome (`╔═╗`) & Translucent Character-Cell Drop Shadows (`:shadows on`)
   - Color Theme Presets (`turbo-blue`, `turbo-cyan`, `ibm-3278-green`, `ibm-3279-amber`, `ibm-mono`)
   - IBM 3270 Full-Pane Cross-Hair Cursor Ruler (`:crosshair on`) & Custom Carets
   - Unicode, CP437, and Nerd Font Glyph Packs (`:glyphs`)
   - Declarative `~/.config/bilterm/config.toml` & `.ini` Theme Import
5. **[Chapter 5: First-Class `tmux` Integration](05-tmux-integration.md)**
   - Running Bilterm Inside Outer `$TMUX` Sessions (`DCS tmux;` Passthrough)
   - Controlling `tmux` via `tmux -CC` Control Mode
6. **[Chapter 6: Complete Keyboard, Mouse & `:command` Reference](06-keyboard-mouse-and-command-reference.md)**
   - Full Shortcut Tables, Mouse Gestures, and Colon Commands
