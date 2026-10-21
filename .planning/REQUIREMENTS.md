# Requirements: Bilterm

**Defined:** 2026-10-19
**Core Value:** A responsive composite terminal pane (movable/resizable floating command prompt over a full-PTY output canvas that scrolls underneath) that can be split horizontally/vertically, stacked/shuffled in z-order via keyboard or mouse, styled with classic IBM 3270 mainframe & DOS Borland Turbo Vision aesthetics, and integrated seamlessly with `tmux`.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### PTY & Virtual Terminal Engine (`PTY`)

- [x] **PTY-01**: User can spawn and interact with standard shells (`bash`, `zsh`, `fish`, PowerShell/cmd) inside a full PTY with VT100/ANSI/TrueColor rendering and scrollback buffer.
- [x] **PTY-02**: User can run full-screen Alternate Screen TUI programs (`vim`, `nvim`, `htop`, `less`, `fzf`) via `DECSET 47/1047/1049` detection, automatically hiding the pane's floating prompt and routing raw keystrokes directly to the PTY until exit (plus a manual toggle hotkey).
- [x] **PTY-03**: User can safely answer interactive password prompts (`sudo`, `ssh`) and sub-REPLs (`python`, `node`, `git add -p`) via three-signal line-discipline/foreground-process detection (`OSC 133` + `tcgetpgrp` + `termios !ECHO`) without cleartext echo leaks or input deadlocks.
- [x] **PTY-04**: User can copy and paste text seamlessly with bracketed paste (`DECSET 2004`) and `OSC 52` clipboard integration.

### Composite Window Pane & Floating Command Prompt (`PANE`)

- [x] **PANE-01**: Each terminal pane operates as a self-contained **Composite Window Pane** (`OutputCanvas` + `FloatingCommandPrompt` + `StatusBar`), where live PTY output and scrollback span 100% of the pane's canvas height and scroll directly underneath the floating command prompt without triggering `SIGWINCH` reflow storms when moving/hiding the prompt.
- [x] **PANE-02**: User can pin a composite pane's floating command prompt to the top of the pane (IBM 3270 mainframe default), pin it to the bottom, or float it freely at any vertical offset within the pane.
- [x] **PANE-03**: User can resize the floating command prompt's height and edit multi-line commands with live shell syntax highlighting (`tree-sitter-bash`) and fuzzy command history search (`nucleo-matcher`).
- [x] **PANE-04**: User can move, pin (top/bottom), and resize the floating command prompt using all three modalities: keyboard shortcuts, mouse drag-and-drop on the prompt border/titlebar, and `:command` / configuration settings.
- [x] **PANE-05**: User can jump between previous command invocations and their corresponding output blocks in scrollback using `OSC 133` semantic prompt markers and submission indexing.

### Window Management: H/V Splits, Z-Order Stacking & Guake Drop-Down (`WM`)

- [x] **WM-01**: User can split any composite pane horizontally or vertically, spawning a new independent `CompositePane` (each with its own PTY `OutputCanvas` and its own `FloatingCommandPrompt`) in a Binary Split Tree layout.
- [x] **WM-02**: User can stack composite panes in z-order (`Vec<PaneId>`), shuffle/rotate the pane stack top-to-bottom or bottom-to-top, and selectively raise any pane to the front via keyboard shortcuts (including window number hotkeys `Alt+1`..`Alt+9` / picker) or mouse clicks.
- [x] **WM-03**: User can move, resize, focus/raise, zoom/maximize, and close composite panes via keyboard chords, mouse dragging/clicking on window borders, or in-prompt `:command` execution, with accurate reverse-z-order mouse hit-testing and pane-local SGR (1006) mouse coordinate translation.
- [x] **WM-04**: Overlapping floating prompts, stacked composite panes, and drop shadows sanitize 2-column wide Unicode/CJK/Emoji graphemes at occlusion boundaries (`occlude_rect`) so underlying multi-column characters never corrupt border alignment.
- [x] **WM-05**: Persistent Guake/Quake-Style Drop-Down Quick Terminal: User can press a dedicated hotkey to toggle a persistent pop-over standard terminal overlay (Guake style) for quick/btw commands that preserves its live PTY session when hidden.

### Retro Aesthetics, Unicode/Glyphs, Status Bars & Cursors (`UI`)

- [x] **UI-01**: Composite panes and floating prompts render classic **DOS Borland Turbo Vision (Turbo C / Turbo Pascal) windowing chrome**, including double-line active borders (`╔═╗`) vs single-line inactive borders (`┌─┐`), 2-col right / 1-row bottom translucent/shaded character-cell drop shadows, window number badges (`[1]`, `[2]`), and clickable border widgets (`[▪]` close, `[↕]` zoom/maximize, `◢` resize corner).
- [x] **UI-02**: User can switch between classic **Borland Turbo C / Turbo Pascal** color schemes (Royal Blue, Cyan, Yellow, White CGA/EGA/VGA palettes), **IBM 3270 Mainframe** themes (3278 Phosphor Green, 3279 Amber, Monochrome), and custom user-defined TrueColor/256-color palettes.
- [x] **UI-03**: User can configure rich **Unicode glyphs, dingbats, CP437 retro symbols, Braille/block elements, Powerline dividers, and Nerd Font icons** across window borders, titles, and status bars with strict `unicode-width` alignment.
- [x] **UI-04**: User can customize the **Status Bars** (combining the **IBM 3270 Operator Information Area [OIA]** state indicators [`READY`, `X SYSTEM`, `INPUT: CMD BOX` vs `RAW PTY`, row/col coordinates, `[Stack: 1/N]`] and the **Borland DOS Function-Key Bar** [`F1 Help  F5 Zoom  F6 Next...`]) with modular segments, custom icons, and dynamic shell/git/tmux telemetry.
- [x] **UI-05**: User can select and customize cursor/caret styles, including the **IBM 3270 full-pane cross-hair cursor** (horizontal row + vertical column ruler assist via non-destructive cell tinting), **solid block**, **blinking hollow block**, **underscore**, and **vertical bar** carets.

### Configuration & Theme Engine (`CFG`)

- [x] **CFG-01**: User can configure Bilterm via a human-friendly configuration file (`~/.config/bilterm/config.toml` primary via `serde`/`toml`, plus support for `.ini`-style theme/config files) covering default prompt position/height, keybindings, status bar segments, Unicode/Nerd Font glyph sets, cursor/cross-hair styles, and custom color themes.

### Tmux Integration & Cross-Platform Support (`TMUX` & `PLAT`)

- [ ] **TMUX-01**: Bilterm detects when running inside an outer `tmux` session (`$TMUX`), automatically avoids prefix key collisions, wraps `OSC 52` clipboard sequences in `DCS tmux;` passthrough, and displays outer `tmux` context in the status bar.
- [ ] **TMUX-02**: User can attach Bilterm to `tmux` sessions/panes via `tmux` CLI / Control Mode (`tmux -CC`) integration so composite panes can be backed by persistent `tmux` sessions.
- [ ] **PLAT-01**: Full support on **Linux** (primary target, Phase 1+ baseline) and **macOS** (POSIX PTY + slave FD/sysctl termios parity), followed by **Windows** (`ConPTY` pseudo-console support).

### Quality, Architecture Documentation & Extensive User Manual (`QUAL`)

- [ ] **QUAL-01**: Comprehensive automated test suite (unit tests, Ratatui `TestBackend` + `insta` visual snapshot tests for overlays/borders/wide-chars, and headless PTY integration tests) audited with `test-coverage` and `test-driven-development` skills.
- [ ] **QUAL-02**: Complete technical architecture documentation, ADRs, and Mermaid/ASCII component & data-flow diagrams in `docs/architecture/` built using the `docs` and `documentation-and-adrs` skills.
- [ ] **QUAL-03**: Extensive multi-chapter User Manual in `docs/user-guide/` (covering quickstart, composite panes, floating prompt controls, H/V splits, z-order stacking/shuffling, Guake drop-down terminal, IBM 3270 & Borland Turbo Vision themes/cursors/status bars, `config.toml` reference, and `tmux` workflows) built using the `writing-user-guides` skill.

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Workspace & Multiplayer Extensions

- **WKSP-01**: Declarative saved workspace layout templates (auto-spawning pre-split and pre-stacked composite panes per project directory).
- **WKSP-02**: Synchronized broadcast input mode (type once in a floating prompt to execute across multiple selected composite panes simultaneously).
- **DAEM-01**: Standalone native background session daemon (independent of `tmux`) for headless detach/reattach.

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Custom GPU/OpenGL/WGPU windowing rasterizer | Breaks SSH, nested `tmux` portability, and lightweight TUI deployment; `Ratatui` + `Crossterm` runs inside any host terminal everywhere. |
| Built-in web browser or rich media viewer | Distracts from fast mainframe/Turbo Vision TUI ergonomics and composite pane multiplexing. |
| Sixel / Kitty pixel-image clipping across overlapping z-stacked panes | Character-cell overlays cannot cleanly clip half of a hardware Sixel bitmap across overlapping z-order layers in v1. |
| Mandatory cloud account or AI telemetry | Bilterm is a local-first, zero-latency systems tool. |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| PTY-01 | Phase 1 | Complete |
| PTY-02 | Phase 1 | Complete |
| PTY-03 | Phase 1 | Complete |
| PTY-04 | Phase 2 | Complete |
| PANE-01 | Phase 1 | Complete |
| PANE-02 | Phase 2 | Complete |
| PANE-03 | Phase 2 | Complete |
| PANE-04 | Phase 2 | Complete |
| PANE-05 | Phase 2 | Complete |
| WM-01 | Phase 3 | Complete |
| WM-02 | Phase 3 | Complete |
| WM-03 | Phase 3 | Complete |
| WM-04 | Phase 3 | Complete |
| WM-05 | Phase 3 | Complete |
| UI-01 | Phase 4 | Complete |
| UI-02 | Phase 4 | Complete |
| UI-03 | Phase 4 | Complete |
| UI-04 | Phase 4 | Complete |
| UI-05 | Phase 4 | Complete |
| CFG-01 | Phase 4 | Complete |
| TMUX-01 | Phase 5 | Pending |
| TMUX-02 | Phase 5 | Pending |
| PLAT-01 | Phase 5 | Pending |
| QUAL-01 | Phase 5 | Pending |
| QUAL-02 | Phase 5 | Pending |
| QUAL-03 | Phase 5 | Pending |

**Coverage:**

- v1 requirements: 26 total
- Mapped to phases: 26
- Unmapped: 0 ✓

---
*Requirements defined: 2026-10-19*
*Last updated: 2026-10-19 after roadmap creation*
