# Bilterm

**Bilterm** is a terminal user interface (TUI) multiplexer, shell wrapper, and terminal emulator written in Rust using [Ratatui](https://ratatui.rs/). Inspired by classic **IBM 3270 Mainframe Terminals** (where the command entry prompt sits at the top of the screen over a scrolling output canvas) and **DOS Borland Turbo Vision (Turbo C / Turbo Pascal IDE)** windowing systems (`╔═╗` double-line borders, translucent character-cell drop shadows, Royal Blue / Cyan / 3278 Phosphor Green / 3279 Amber palettes, window number badges, and DOS Function-Key status bars), Bilterm treats the **floating command prompt + underlying PTY output canvas as a single composite terminal pane**.

---

## Key Capabilities

- **Composite Window Pane (`OutputCanvas` + `FloatingCommandPrompt` + `OIA Status Bar`)**:
  - Full-height VT100 / ANSI / 24-bit TrueColor PTY output canvas (`portable-pty` + `vt100-ctt`) that scrolls smoothly underneath an overlaid multi-line command prompt with **zero `SIGWINCH` geometry thrashing** when the prompt moves or hides.
  - Automatic `DECSET 47/1047/1049` alternate-screen auto-hide (`vim`, `nvim`, `htop`, `less`, `fzf`) and three-signal line-discipline detection (`OSC 133` + `tcgetpgrp` + `termios !ECHO`) so `sudo`/`ssh` passwords are never echoed in cleartext.
- **Triple-Modality Command Prompt (Floating or Non-Overlapping Split)**:
  - Pin to **Top** (IBM 3270 default), **Bottom**, **Free-Float** (`Alt+T`, `Alt+B`, `Alt+Up`/`Down`), or partition into a **Non-Overlapping Split Command & Output Pane** (`Alt+S` / `:prompt split-top | split-bottom`).
  - **Resizable Width & Height + Scrollable Content**: Resize height (`Alt+Plus`/`Alt+Minus`, bottom border drag, `:prompt height <3..12>`) and width (`Alt+]`/`Alt+[`, left/right/corner border drag, `:prompt width <cols|full>`), with vertical/horizontal scrolling (`Shift+PageUp`/`Shift+PageDown`, mouse wheel) and a live **`⟳ RUNNING:` / `● LAST CMD:` banner** showing the active command (such as `tail -f /dev/null`) after pressing `Enter`.
  - **Tab Completion, History Pop-up & Session Persistence**: Context-aware `Tab`/`Shift+Tab` completion, `Ctrl+R` fuzzy history search (`nucleo-matcher`), a Borland **Command History Pop-up Window** (`Alt+P` / `:history`), automatic per-session/per-pane history file persistence with rotation (`:history on|off`, `:history max-files <N>`), and temporary script export (`:history save`).
  - **`<F1>` Interactive Help & Full-Screen Cross-Hairs**: Press **`F1`** (or `:help`) for the scrollable Borland Turbo Vision Help Modal, and **`Alt+X`** (or `:crosshair full | pane | off`) to cycle between Pane-Local and Full-Screen IBM 3270 vertical/horizontal cross-hair cursors.
- **Multi-Pane Window Manager, Z-Order Stacking & Guake Drop-Down Terminal**:
  - Split any composite pane horizontally (`Alt+H` / `:split h`) or vertically (`Alt+V` / `:split v`).
  - Stack composite panes in z-order (`Alt+M` / `:layout stack`), shuffle the deck top-to-bottom (`Alt+Tab` / `F6`) or bottom-to-top (`Alt+Shift+Tab`), and selectively raise any pane via `Alt+1..9` or mouse click.
  - Toggle a **persistent Guake/Quake-style drop-down quick terminal** (`Alt+\`` / `Ctrl+Space` / `:quake`) for rapid commands.
  - 2-column wide Unicode / CJK / Emoji / Nerd Font boundary sanitization (`occlude_rect`) prevents border corruption.
- **Retro Mainframe & Borland Turbo Vision Aesthetics**:
  - 5 built-in themes (`turbo-blue`, `turbo-cyan`, `ibm-3278-green`, `ibm-3279-amber`, `ibm-mono`), 4 glyph packs (`unicode`, `cp437`, `nerd-font`, `ascii`), IBM 3270 full-pane cross-hair cursor ruler (`:crosshair on`), translucent character-cell drop shadows (`:shadows on`), and declarative `~/.config/bilterm/config.toml` + `.ini` theme import.
- **First-Class `tmux` Integration**:
  - Outer `$TMUX` coexistence with `DCS tmux;` clipboard passthrough and `tmux -CC` Control Mode synchronization (`src/tmux/mod.rs`).

---

## Quickstart

```bash
cargo build --release
./target/release/bilterm
```

Run the full test and verification suite:

```bash
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

---

## Documentation

- **[User Manual (`docs/user-guide/index.md`)](docs/user-guide/index.md)** — Complete 6-chapter guide covering getting started, the floating command prompt, multi-pane window management & Z-order stacking, themes/glyphs/config, `tmux` integration, and the full shortcut/`:command` reference.
- **[Architecture Overview & ADRs (`docs/architecture/OVERVIEW.md`)](docs/architecture/OVERVIEW.md)** — System architecture diagrams, module boundaries, and Architecture Decision Records (`ADR-001` through `ADR-005`).
