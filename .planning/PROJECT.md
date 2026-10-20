# Bilterm

## What This Is

Bilterm is a terminal user interface (TUI) multiplexer, shell wrapper, and terminal emulator written in Rust using Ratatui. Inspired by classic **IBM mainframe terminals** (such as the IBM 3270 where the command prompt sits at the top of the screen) and **classic DOS Borland Turbo Vision / Turbo C / Turbo Pascal windowing systems** (double-line box-drawing borders, drop shadows, classic blue/cyan/yellow/white CGA/VGA palettes, window numbers, and top/bottom menu/hotkey bars), Bilterm treats the **floating command prompt + underlying output canvas as a single composite terminal pane**. These composite terminal panes can be split horizontally and vertically, or stacked in z-order (shuffled top-to-bottom or raised selectively via keyboard or mouse), with full `tmux` integration/support.

## Core Value

A responsive composite terminal pane (movable/resizable floating command prompt over a full-PTY output canvas that scrolls underneath) that can be split horizontally/vertically, stacked/shuffled in z-order via keyboard or mouse, and integrated seamlessly with `tmux`.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] **Composite Terminal Pane Architecture**: Each terminal unit is a self-contained composite pane consisting of its own PTY output canvas + its own movable/resizable floating command prompt (pinned top, pinned bottom, or free-floating at any vertical offset within that pane's bounds).
- [ ] **Horizontal & Vertical Pane Splitting**: Composite panes can be split horizontally and vertically into tiled layouts (each split spawns a new composite pane with its own independent floating command prompt and underlying PTY output canvas).
- [ ] **Z-Order Pane Stacking & Shuffling**: Composite panes can be stacked on top of one another, shuffled top-to-bottom (cycling z-order), or raised selectively to the front via keyboard shortcuts or mouse clicks/selection.
- [ ] **Tmux Support & Integration**: Seamless operation alongside or integrated with `tmux` (e.g., running smoothly inside `tmux` with proper escape sequence/passthrough handling, `tmux` control mode `-CC` backend integration or session/pane synchronization, and conflict-free prefix/keybinding ergonomics).
- [ ] **Full PTY Shell Wrapper & VT100 Emulation**: Standard shells (`bash`, `zsh`, `fish`, PowerShell/cmd on Windows) with full VT100/ANSI escape sequence parsing and color rendering per composite pane.
- [ ] **Resizable Multi-Line Command Editor**: Each composite pane's command input box is resizable in height and supports multi-line editing, syntax highlighting, and command history search.
- [ ] **Transparent/Occluding Overlay Rendering**: Within each composite pane, terminal scrollback and live output span the full pane height and scroll directly underneath that pane's floating prompt window.
- [ ] **Triple-Modality Controls**: Keyboard shortcuts, mouse drag-and-drop (border/titlebar move, resize, focus/raise), and in-prompt commands / configuration settings for both the floating prompt inside a pane and the composite panes themselves.
- [ ] **Alternate Screen Auto-Hide**: Per-pane DECSET 1049 detection that auto-hides that composite pane's floating command prompt when full-screen TUI apps (`vim`, `htop`, `less`) run inside it, restoring the floating prompt on exit, plus a manual toggle hotkey.
- [ ] **Retro IBM 3270 Mainframe & Borland Turbo Vision (Turbo C / Turbo Pascal) Aesthetics & Cursors**:
  - **Borland Turbo Vision Windowing Chrome**: Double-line box-drawing borders (`╔═╗`) for active panes/floating prompts vs single-line (`┌─┐`) for inactive panes, classic DOS 1-cell/2-cell half-block/shaded drop shadows (`▓`/`▒`/dimmed background), window title badges with window index numbers (`[1]`, `[2]`), clickable border controls (`[▪]` close, `[↕]` zoom/maximize, `◢` resize corner), and classic Borland Turbo C/Pascal color themes (Royal Blue/Cyan/Yellow/White CGA/EGA/VGA palettes) alongside IBM 3270 themes (Phosphor Green, Amber, Monochrome).
  - **Rich Unicode Glyphs, Dingbats & Nerd Font Icons**: Full Unicode glyph, CP437 retro symbol, dingbat, braille/block-element, Powerline, and Nerd Font icon support (`unicode-width` / `unicode-segmentation` aware so wide icons never corrupt border alignment or z-order occlusion).
  - **Highly Customizable Status Bars (OIA + Function-Key Bar)**: Configurable top/bottom/per-pane status bars combining IBM 3270 Operator Information Area (OIA) indicators, classic Borland DOS function-key hint bars (`F1 Help  F5 Zoom  F6 Next...`), custom segments, Unicode/Nerd Font icons, separators, and dynamic shell/git/tmux/pane state widgets.
  - **Cursors & Caret Styles**: Configurable cursor/caret styles including IBM 3270 cross-hair cursor spanning rows/columns, solid block, hollow/blink block, underscore, and vertical bar.
- [ ] **Configuration File & Live Theme Engine**: Declarative configuration file (TOML primary — Rust standard via `serde`/`toml` — with optional INI/theme file import, following XDG `~/.config/bilterm/config.toml`) for customizing keybindings, floating prompt default position/size, custom status bar segments/icons, cursor styles, border glyph sets, and user-defined color schemes.
- [ ] **Command Block Separation**: Jump between previous command invocations and their outputs in scrollback within each composite pane.
- [ ] **Cross-Platform Support**: Linux first (primary target), macOS second, Windows third (ConPTY).

### Out of Scope

- Custom GPU-accelerated windowing / font rasterizer (e.g., WGPU/OpenGL GUI window) — v1 runs inside standard host terminals via Ratatui + Crossterm so it works everywhere locally, over SSH, and inside `tmux`.
- Built-in web browser or non-terminal rich media rendering — keep focus on crisp TUI mainframe ergonomics, composite pane tiling/stacking, and fast PTY execution.

## Context

- **Inspiration**: Classic IBM mainframe terminals (3270 / TSO / ISPF / VM/CMS) where the command entry area is decoupled from scrolling output and commonly positioned at the top of the screen, combined with modern tiling/stacking window manager and multiplexer (`tmux` / Zellij) capabilities.
- **Composite Pane Model**: Unlike standard multiplexers where a pane is just a raw TTY grid, a Bilterm pane is a composite unit (`CompositePane = OutputCanvas + FloatingCommandPrompt + Status/OIA`). Splitting a pane creates a new `CompositePane`. Stacking panes allows overlapping or full-area layers that can be shuffled (rotated in z-order) or selectively raised via keyboard or mouse.
- **Technical Ecosystem**: Rust + `ratatui` + `crossterm` for TUI rendering and input handling; `portable-pty` for cross-platform PTY management; `vt100` or `alacritty_terminal` (`vte`) for virtual terminal state machine and screen buffer emulation per pane; `tmux` integration (both running inside `tmux` and interacting with `tmux` sessions/panes or `tmux -CC` control mode).
- **Platform Priority**: Linux is the primary development and verification target (Phase 1+), followed by macOS (POSIX PTY parity), and Windows (ConPTY compatibility).

## Constraints

- **Tech Stack**: Rust + Ratatui — Required by project vision for fast, zero-cost TUI rendering and cross-platform terminal manipulation.
- **Platform Priority**: Linux first, macOS second, Windows third — Ensures rapid iteration on POSIX PTY semantics while keeping cross-platform abstractions (`portable-pty`, `crossterm`) clean from day one.
- **Compatibility**: Must cleanly handle interactive TUI programs (`vim`, `htop`, `ssh`) per composite pane via Alternate Screen detection, and coexist/integrate cleanly with `tmux`.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Composite Window Pane (`OutputCanvas` + `FloatingPrompt`) as the core layout unit | Allows every split or stacked pane to retain its own independent floating command prompt and scrolling output canvas | — Pending |
| Tiled (H/V splits) + Stacked (z-order shuffle & selective raise via keyboard/mouse) window management | Gives users both side-by-side comparison and overlapping/deck-style window management in the terminal | — Pending |
| First-class `tmux` support | Users rely on `tmux` for session persistence and workflows; Bilterm must work seamlessly with `tmux` | — Pending |
| Full PTY wrapper + VT100 parser (`portable-pty` + `vt100`/`vte`) per composite pane | Needed to render each underlying shell output inside its Ratatui pane rect while compositing the floating command widget on top | — Pending |
| Auto-hide floating prompt on Alternate Screen (DECSET 1049) + manual toggle per pane | Allows seamless use of `vim`, `htop`, and `less` inside any split/stacked pane without keystroke interception conflicts | — Pending |
| Support cross-hair cursor + multiple caret styles (block, blink hollow block, underscore) | Authentic IBM 3270 / mainframe terminal feel alongside customizable retro aesthetics | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-10-19 after initialization*
