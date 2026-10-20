# Project Research Summary

**Project:** Bilterm
**Domain:** Composite-Pane Terminal Multiplexer, Floating-Prompt Shell Wrapper & IBM 3270 / Borland Turbo Vision TUI Window Manager (Rust + Ratatui)
**Researched:** 2026-10-20
**Confidence:** HIGH

## Executive Summary

Bilterm is a pure-TUI terminal multiplexer, shell wrapper, and window manager written in Rust that introduces the **Composite Window Pane** (`CompositePane = OutputCanvas + FloatingCommandPrompt + OIA / Function-Key Status Bar`). Unlike traditional terminal multiplexers (`tmux`, `Zellij`) where panes are raw character grids, or GUI-bound shells (`Warp`, `Wave`) that break over SSH, every Bilterm pane renders a full-height virtual terminal output canvas with an overlaid, movable, resizable floating command prompt and a bottom status bar. Panes can be split horizontally/vertically as complete composite units and stacked in z-order with top-to-bottom deck shuffling and selective raising via keyboard, mouse, or command palette, combining **IBM 3270 mainframe ergonomics** with **classic DOS Borland Turbo C / Turbo Pascal (Turbo Vision) windowing aesthetics**.

Research confirms the optimal architecture pairs **`ratatui 0.30.2`** and **`crossterm 0.29.0`** for immediate-mode cell-buffer compositing with **`portable-pty 0.9.0`** for cross-platform PTY lifecycle (Linux, macOS, Windows ConPTY) and **`alacritty_terminal 0.26.0` / `vte 0.15.0`** (or `vt100-ctt 0.17.1` for Phase 1 prototyping) for virtual terminal state and `DECSET 1049` alternate screen tracking. Because Ratatui renders into an intermediate 2D `Buffer`, Bilterm can composite overlapping z-stacked panes and floating prompts using the Painter's Algorithm with boundary-sanitized `Clear` occlusion, render classic Borland Turbo Vision window chrome (double-line active borders `╔═╗`, single-line inactive borders `┌─┐`, translucent/dimmed character-cell drop shadows, window numbers `[1]`/`[2]`, clickable `[▪]` close, `[↕]` zoom, and `◢` resize corner widgets), and apply IBM 3270 cross-hair cursors and custom carets via post-render buffer cell mutations.

The primary technical risks center on **PTY line-discipline/echo desynchronization** (e.g., cleartext password leaks during `sudo`/`ssh`, or interactive REPLs locking up when typing is trapped in the floating editor), **`SIGWINCH` geometry thrashing** if the PTY size changes when hiding/moving the floating prompt, and **wide-character (CJK/Emoji) border corruption** during z-order occlusion and drop-shadow rendering. Mitigating these from Phase 1 requires sizing the PTY strictly to the full `OutputCanvas`, implementing a Three-Signal Input Routing State Machine (`OSC 133` semantic prompts + `tcgetpgrp`/`termios` `!ECHO` inspection + `DECSET 1049` auto-hide), and sanitizing 2-cell wide graphemes at overlay edges.

## Key Findings

### Recommended Stack

See [`STACK.md`](file:///home/bill/src/ai/bilterm/.planning/research/STACK.md) for full version matrices and `Cargo.toml` configuration.

The stack is anchored on **Rust 1.86+ (Edition 2024)** using **`tokio 1.44+`** to multiplex dedicated blocking PTY reader threads (`portable-pty`), `crossterm` async event streams, and 60–120 FPS coalesced render ticks without frame starvation. `reedline` and `rustyline` are explicitly rejected because they seize exclusive ownership of host stdout/raw mode; instead, each pane embeds a bounded **`tui-textarea-2`** widget paired with **`tree-sitter-bash`** and **`nucleo-matcher`**.

**Core technologies:**
- **`ratatui 0.30.2` + `crossterm 0.29.0`:** Immediate-mode TUI cell-buffer compositor and cross-platform terminal I/O (`event-stream`, SGR mouse, Kitty keyboard flags, built-in `border::DOUBLE` / `border::PLAIN` sets for Turbo Vision window frames).
- **`portable-pty 0.9.0`:** Cross-platform PTY allocation and process management abstracting POSIX `openpty`/`forkpty` (Linux/macOS) and Windows `ConPTY`.
- **`alacritty_terminal 0.26.0` + `vte 0.15.0` (with `vt100-ctt 0.17.1` / `tui-term 0.3.4` for rapid prototyping):** Per-pane VT100/ANSI state machine, scrollback ring buffer with reflow, `DECSET 47/1047/1049` alternate screen detection, and `OSC 133` hooks.
- **`tui-textarea-2 0.10` + `tree-sitter 0.24` + `nucleo-matcher 0.3`:** Multi-line floating command editor with real-time shell syntax highlighting and fuzzy history search.
- **`tmux_interface 0.4` + `nom 8.0`:** Outer `$TMUX` coexistence/passthrough and `tmux -CC` control mode protocol parsing.

### Expected Features

See [`FEATURES.md`](file:///home/bill/src/ai/bilterm/.planning/research/FEATURES.md) for dependency graphs and competitor comparisons.

**Must have (table stakes):**
- **Per-Pane PTY Lifecycle & VT100/TrueColor Emulation** — Spawn/reap shells (`bash`, `zsh`, `fish`, `pwsh`) with accurate ANSI state and scrollback buffers.
- **Alternate Screen (`DECSET 47/1047/1049`) Auto-Hide & Raw Passthrough** — Automatically hide the floating prompt and forward raw keystrokes when running `vim`, `nvim`, `htop`, `less`, or `fzf`.
- **Dynamic Resize (`SIGWINCH`), Copy/Paste (`DECSET 2004` / `OSC 52`), and Shell Integration (`OSC 133` / `OSC 7`)** — Seamless daily-driver terminal ergonomics.

**Should have (competitive differentiators for v1):**
- **Composite Window Pane (`OutputCanvas` + `FloatingPrompt` + `OIA / Function-Key Bar`)** — Terminal output spans the full pane height and scrolls continuously underneath a movable (top-pinned 3270 style, bottom-pinned, or free-floating), resizable multi-line command prompt.
- **H/V Composite Pane Splitting** — Every horizontal or vertical split creates a complete new `CompositePane` with its own floating prompt and PTY.
- **Z-Order Pane Stacking, Top-to-Bottom Shuffling & Selective Raise** — Overlap composite panes in a z-ordered deck, rotate/shuffle the stack, or raise any pane via keyboard picker (`Alt+1`..`Alt+9` window numbers) or mouse click.
- **Classic DOS Borland Turbo Vision Windowing & IBM 3270 Mainframe Aesthetics** —
  - *Borland Turbo C / Turbo Pascal (Turbo Vision) Chrome:* Double-line active borders (`╔═╗`) vs single-line inactive borders (`┌─┐`), classic DOS 2-col right / 1-row bottom character-cell drop shadows (dimming underlying cell fg/bg), window numbers (`[1]`, `[2]`), clickable border widgets (`[▪]` close, `[↕]` zoom/maximize, `◢` resize corner), Borland Blue/Cyan/Yellow/White CGA/VGA color themes, and bottom function-key (`F1 Help  F2 Zoom  F3 Split...`) status bars.
  - *IBM 3270 Mainframe Ergonomics:* Full-pane horizontal/vertical cross-hair cursor ruler, selectable caret styles (solid block, blinking hollow block, underscore, bar), 3278 Green / 3279 Amber / Monochrome phosphor themes, and Operator Information Area (OIA) status indicators (`X SYSTEM`, `INPUT: CMD BOX` vs `RAW PTY`, `ROW/COL` coordinates).
- **Triple-Modality Controls & First-Class `tmux` Coexistence** — Keyboard shortcuts, mouse drag/resize/click-to-raise (including Turbo Vision border controls), and `:command` control, plus conflict-free execution inside `tmux` and `tmux -CC` control mode integration.
- **Command Block Separation & Scrollback Jumping** — Navigate directly between prior commands and outputs per pane.

**Defer (v1.x / v2+):**
- **Saved Workspace Templates & Synchronized Broadcast Input (v1.x)** — Declarative layout persistence and multi-pane typing broadcast.
- **Standalone Background Daemon/Detach Server & External IPC Socket (v2+)** — Defer native daemon persistence in v1 by leveraging `tmux` coexistence/control mode first.
- **Anti-Features (Do Not Build):** Custom GPU/OpenGL window rasterizer, cloud/AI login dependencies, custom non-POSIX shell syntax, or Sixel pixel occlusion across overlapping character cells.

### Architecture Approach

See [`ARCHITECTURE.md`](file:///home/bill/src/ai/bilterm/.planning/research/ARCHITECTURE.md) for component diagrams and data flows.

Bilterm uses a layered reactive architecture driven by a central `tokio::select!` loop with **frame-tick coalescing (60–120 FPS)**. Window geometry (`wm/`) is strictly decoupled from internal pane state (`pane/`), enabling any `CompositePane` to live in a binary split tree (`SplitTree`), an overlapping z-order deck (`z_stack: Vec<PaneId>`), or both without state mutation. Rendering traverses `z_stack` bottom-to-top (Painter's Algorithm), whereas mouse hit-testing traverses `z_stack` top-to-bottom (`rev()`), testing the floating prompt rect and Turbo Vision border widgets (`[▪]`, `[↕]`, `◢`, titlebar drag) before the underlying `OutputCanvas`.

**Major components:**
1. **`pty/` (PTY Manager & Reader Workers)** — Spawns `portable-pty` child shells and runs one dedicated OS blocking reader thread per pane sending `Bytes` over a bounded `mpsc::channel(256)` for natural backpressure.
2. **`vt/` (Virtual Terminal State Machine)** — Processes byte streams via `vte`/`alacritty_terminal`, tracks primary/alternate grids, scrollback, `DECSET 1049`, and `OSC 133` block markers.
3. **`pane/` (`CompositePane` Model)** — Owns per-pane `vt` state, `FloatingPrompt` multi-line editor state/offset, `OiaStatusBar` telemetry, window number (`[1]`), and `CommandBlock` scrollback indices.
4. **`wm/` (Layout Engine: Split Tree + Z-Order Manager)** — Computes tiled H/V split rects, maintains the authoritative `Vec<PaneId>` z-order stack (shuffle/rotate/raise/zoom), and performs inverse mouse hit-testing across window frames, border controls, and floating prompts.
5. **`input/` (Modal Input Router)** — Routes events between Window Manager chords, `FloatingPrompt` editing, mouse drag/resize/border-widget actions, and direct `RawPty` passthrough.
6. **`ui/` (Ratatui Compositor & Post-Processors)** — Renders `OutputCanvas`, boundary-safe `Clear` occlusion, Turbo Vision frames (`╔═╗` active / `┌─┐` inactive, `[▪]`, `[↕]`, `◢`, and character-cell drop shadows), `FloatingPrompt`, `OiaStatusBar` + DOS function-key bar, and post-pass `CursorOverlay` (3270 cross-hair and custom carets).
7. **`tmux/` (Tmux Bridge)** — Handles `$TMUX` environment detection, DCS passthrough (`\ePtmux;...\e\\`), and `tmux -CC` control mode synchronization.

### Critical Pitfalls

See [`PITFALLS.md`](file:///home/bill/src/ai/bilterm/.planning/research/PITFALLS.md) for full root-cause analysis and recovery checklists.

1. **Echo & Line-Discipline Desync (`sudo` Password Leaks, REPL Lockout, Double Echo)** — Modern shells (`zsh`/`bash`/`fish`) keep the PTY in `!ICANON && !ECHO` mode at the prompt. Avoid cleartext password leaks and REPL lockouts by combining `OSC 133` prompt markers with a `PtyIntrospector` trait checking `tcgetpgrp(master_fd) != shell_pgid` and `termios` `(ICANON && !ECHO)` to immediately switch to masked/direct PTY passthrough.
2. **Alternate Screen (`DECSET 1049/1047/47`) & `SIGWINCH` Geometry Thrashing** — Never resize the child PTY (`TIOCSWINSZ`) when moving, resizing, or auto-hiding the `FloatingPrompt`. Keep PTY dimensions fixed to the full `OutputCanvas` rect and use virtual scroll padding so output scrolls smoothly underneath without triggering `SIGWINCH` reflow storms.
3. **Wide-Character (CJK/Emoji) & Style Bleed-Through on Z-Order Borders and Drop Shadows** — Ratatui's `Clear` widget ignores 2-column glyphs straddling `rect.left() - 1` or `rect.right() - 1`, causing 1-column row shifts on host terminals. Implement a custom `occlude_rect` compositor pass that sanitizes split wide characters at left/right boundaries, resets `Modifier` flags, and applies Turbo Vision drop shadows by dimming `(fg: DarkGray, bg: Black)` on the 2-column right and 1-row bottom shadow strip without splitting 2-cell wide glyphs.
4. **Tmux Prefix Collisions & Untranslated SGR (1006) Mouse Coordinates** — Avoid `Ctrl+B` default prefix collisions; translate host mouse coordinates to pane-local 1-based `(col - canvas.x + 1, row - canvas.y + 1)` before forwarding SGR 1006 events to child TUIs, and block mouse leak-through under the `FloatingPrompt`, Turbo Vision border widgets, or higher z-order panes.
5. **Cross-Platform PTY Divergences (macOS `ENOTTY` & Windows ConPTY Screen Synthesis)** — Isolate OS introspection behind `PtyIntrospector` (Linux `/proc`/master FD `tcgetattr`, macOS slave FD/`sysctl`, Windows process-tree/ConPTY handling) and always feed ConPTY output through the full VT state machine rather than naive line splitting.
6. **Hardware Cursor Fighting 3270 Cross-Hair & Hollow-Block Carets** — Hide the host hardware cursor (`terminal.hide_cursor()`) when rendering custom software carets (`HollowBlock`, `BlinkingHollowBlock`), and apply the 3270 cross-hair via non-destructive background tinting/underlining clipped to the focused pane so syntax highlighting and Borland CGA/VGA or 3270 phosphor palettes remain legible.

## Implications for Roadmap

Based on component dependencies and pitfall prevention, the project should be structured into 5 sequential phases:

### Phase 1: Core PTY, VT100 Engine & Single CompositePane Prototype
**Rationale:** Validates Bilterm's foundational mechanic—full-height terminal output scrolling underneath an overlaid floating command prompt with zero `SIGWINCH` thrashing—before introducing multi-pane window management.
**Delivers:** Working single-pane TUI spawning a real shell via `portable-pty`, frame-coalesced Tokio loop, full-height `OutputCanvas`, movable `FloatingPrompt` overlay with wide-char-safe `occlude_rect`, panic-hook terminal restoration, and `DECSET 47/1047/1049` + `PtyIntrospector` (`!ECHO` / `tcgetpgrp`) auto-switching into raw PTY mode.
**Addresses:** PTY lifecycle, VT100 fidelity, Composite Window Pane, Alternate Screen auto-hide.
**Avoids:** Pitfall 1 (`sudo` password leaks / REPL lockout), Pitfall 2 (`SIGWINCH` thrashing), and unbounded PTY channel freezes.

### Phase 2: Floating Prompt Ergonomics, Triple-Modality Intra-Pane Controls & Command Blocks
**Rationale:** Completes the intra-pane shell wrapper experience so every `CompositePane` is a self-contained, feature-complete unit before multiplying panes in the Window Manager.
**Delivers:** Multi-line resizable `FloatingPrompt` (`tui-textarea-2`), live bash syntax highlighting (`tree-sitter-bash`), fuzzy history (`nucleo-matcher`), top-pinned (3270) / bottom-pinned / free-floating vertical positioning via keyboard, mouse drag/resize, and `:command` palette, bracketed-paste submission, plus `OSC 133` + submission-indexed Command Block scrollback navigation.
**Uses:** `tui-textarea-2`, `tree-sitter-bash`, `nucleo-matcher`, `vte` `OSC 133` hooks.
**Implements:** `pane/prompt.rs`, `pane/blocks.rs`, intra-pane `input/mouse.rs`.

### Phase 3: Window Manager — H/V Composite Splits & Z-Order Stacking/Shuffling
**Rationale:** With `CompositePane` encapsulated as a stateful widget rendering into any `Rect`, the spatial layout engine and z-order deck manager can instantiate and composite $N$ panes cleanly.
**Delivers:** Binary Space Partitioning (`SplitTree`) for H/V composite pane splits (each spawning an independent `CompositePane`), authoritative `z_stack: Vec<PaneId>` for overlapping/stacked panes, top-to-bottom rotational deck shuffle, selective pane raise (keyboard picker `[1]`..`[9]` + mouse click-to-raise), zoom/maximize toggle (`[↕]`), reverse-z-order mouse hit-testing with pane-local SGR 1006 coordinate translation, and `occlude_rect` boundary sanitization across stacked panes.
**Addresses:** H/V splitting of composite panes, z-order pane stacking/shuffling/selective raise, multi-pane triple-modality controls.
**Avoids:** Pitfall 3 (wide-grapheme border corruption across overlapping panes) and Anti-Pattern 3 (render vs hit-test z-order mismatch).

### Phase 4: Retro Mainframe & DOS Aesthetics — IBM 3270 + Borland Turbo Vision Chrome, OIA & Cursors
**Rationale:** Layers Bilterm's dual retro visual identity (IBM 3270 mainframe ergonomics + DOS Borland Turbo Vision windowing chrome) onto the multi-pane compositor and focus manager built in Phases 1–3.
**Delivers:**
- **Borland Turbo Vision Windowing Chrome:** Double-line active borders (`╔═╗`) vs single-line inactive borders (`┌─┐`), classic DOS character-cell drop shadows on floating prompts and stacked panes, window numbers (`[1]`, `[2]`), clickable border widgets (`[▪]` close, `[↕]` zoom, `◢` resize corner), Borland Blue/Cyan/Yellow/White CGA/VGA color themes, and bottom function-key bar.
- **IBM 3270 Mainframe Ergonomics:** Per-pane/workspace Operator Information Area (`OiaStatusBar`) showing system lock/input routing state (`READY`, `X SYSTEM`, `INPUT: CMD BOX` vs `RAW PTY`, z-stack deck index `[Stack: 1/N]`, row/col coordinates, exit codes), post-render `CursorOverlay` for full-pane cross-hair cursor rulers, selectable carets (solid block, blinking hollow block, underscore, vertical bar) with hardware-cursor suppression when needed, and 3278 Green / 3279 Amber / Monochrome phosphor themes.
**Addresses:** Borland Turbo Vision window chrome & themes, IBM 3270 aesthetics, OIA / function-key status bar, cross-hair cursor & retro caret styles.
**Avoids:** Pitfall 6 (cross-hair destroying text legibility or clashing with the host hardware cursor) and Pitfall 3 (drop-shadow wide-character clipping).

### Phase 5: First-Class `tmux` Integration & Cross-Platform Hardening (Linux -> macOS -> Windows)
**Rationale:** Integrates the completed multiplexer with outer/inner `tmux` workflows and hardens OS-specific PTY edge cases across all target platforms.
**Delivers:** `$TMUX` environment detection, conflict-free prefix/keybinding profiles, DCS escape passthrough (`OSC 52` clipboard write forwarding with read-query blocking), `tmux -CC` control mode session backend (`tmux_interface` + `nom`), and cross-platform CI matrix verification for Linux, macOS (`sysctl`/slave FD `termios`), and Windows (`ConPTY` teardown and process-tree inspection).
**Addresses:** First-class `tmux` coexistence/control mode, cross-platform terminal I/O parity.
**Avoids:** Pitfall 4 (`tmux` prefix deadlocks & swallowed passthrough sequences) and Pitfall 5 (macOS `ENOTTY` and Windows `ConPTY` hang/redraw quirks).

### Phase Ordering Rationale

- **Bottom-Up Composability:** `CompositePane` depends on `pty/` and `vt/` (Phase 1) and internal prompt/block ergonomics (Phase 2). Building `wm/` H/V splitting and z-order stacking in Phase 3 treats `CompositePane` as a black-box `Rect` target, preventing layout refactors from breaking terminal emulation.
- **Early Risk Retirement:** The two highest-risk architectural hazards—line-discipline/password-prompt routing and full-height PTY sizing under a floating overlay—are solved and snapshot-tested in Phase 1 rather than discovered after multi-pane window management is built.
- **Clean Post-Processing Separation:** IBM 3270 cross-hair cursors, Borland Turbo Vision active/inactive frame chrome (`╔═╗` vs `┌─┐`), drop shadows, clickable border controls (`[▪]`, `[↕]`, `◢`), and OIA/function-key status bars (Phase 4) build directly on the z-order pane bounds and hit-testing engine established in Phase 3.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1 & Phase 5 (OS-Specific `PtyIntrospector` & `tmux -CC` Protocol):** Needs targeted verification of macOS slave FD `tcgetattr` vs `sysctl` foreground process group checks, Windows `ConPTY` child process detection, and `tmux -CC` octal-escaped `%output` stream edge cases.

Phases with standard patterns (skip research-phase):
- **Phase 2 (Floating Editor & Block Indexing):** Well-documented `tui-textarea-2`, `tree-sitter-bash`, and `OSC 133` patterns.
- **Phase 3 & Phase 4 (BSP Split Tree, Painter's Algorithm Z-Stack, Turbo Vision Borders/Shadows & 3270 Buffer Styling):** Deterministic data structures and standard Ratatui `Buffer` cell manipulation fully specified in [`ARCHITECTURE.md`](file:///home/bill/src/ai/bilterm/.planning/research/ARCHITECTURE.md).

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All crate versions (`ratatui 0.30.2`, `crossterm 0.29.0`, `portable-pty 0.9.0`, `alacritty_terminal 0.26.0`) and compatibility constraints verified. |
| Features | HIGH | Directly aligned with `PROJECT.md` requirements, IBM 3270 OIA/cursor specifications, Borland Turbo Vision TUI conventions, and modern shell-wrapper ergonomics. |
| Architecture | HIGH | Ratatui `Buffer` compositing, bounded PTY reader threads, and split-tree + z-stack window management follow proven Rust TUI patterns. |
| Pitfalls | HIGH | POSIX `termios`/`tcgetpgrp`, `DECSET 1049`, Unicode East Asian Width occlusion, and `tmux` passthrough failure modes are well-characterized with concrete mitigations. |

**Overall confidence:** HIGH

### Gaps to Address

- **`alacritty_terminal` vs `vt100-ctt` Adapter Effort in Phase 1:** Define an internal `TerminalBackend` trait in Phase 1 so prototyping can start with `vt100-ctt` (or directly with `alacritty_terminal::Term`) without locking out line-reflow on horizontal split resize.
- **Interactive Prompts Without `OSC 133` or `!ECHO` (e.g., Remote SSH Sessions or `git add -p`):** Combine foreground process group inspection (`tcgetpgrp != shell_pgid`) with an unmistakable OIA mode indicator and a single-keystroke manual toggle (`Ctrl+Space` or `Alt+P`) between `FloatingPrompt` and direct PTY input.
- **Top-Pinned Prompt Viewport Padding:** Validate user preference between virtual top scroll padding vs translucent/auto-peek behavior when the floating prompt is pinned to row 0 (IBM 3270 mode) during short command output.

## Sources

### Primary (HIGH confidence)
- [`PROJECT.md`](file:///home/bill/src/ai/bilterm/.planning/PROJECT.md) — Core Bilterm vision, composite pane definition, z-order stacking/shuffling, IBM 3270 & Borland Turbo Vision aesthetics, and `tmux` coexistence requirements.
- `crates.io` (`ratatui 0.30.2`, `crossterm 0.29.0`, `portable-pty 0.9.0`, `alacritty_terminal 0.26.0`, `vt100-ctt 0.17.1`, `tui-term 0.3.4`, `tui-textarea-2 0.10`, `tmux_interface 0.4`) — Crate capabilities and MSRV (`1.86+`).
- XTerm / DECSET & FinalTerm Specifications — `DECSET 47/1047/1049` (Alternate Screen), `DECSET 1000/1002/1003/1006` (SGR Mouse), `DECSET 2004` (Bracketed Paste), `DECSCUSR` (Cursor Shapes), `OSC 52` (Clipboard), and `OSC 133` (Semantic Prompts).
- POSIX `termios(3)` / `tty_ioctl(4)` & Microsoft Windows `ConPTY` (`CreatePseudoConsole`) documentation — Line-discipline inspection, `TIOCSWINSZ`, `tcgetpgrp`, and pseudo-console buffer rendering behavior.
- IBM 3270 Display System Reference & Borland Turbo Vision (Turbo C / Turbo Pascal) UI Guide — Operator Information Area (OIA) indicators, cross-hair cursor ruler assist, 3278/3279 phosphor palettes, and classic DOS double/single-line box-drawing chrome (`╔═╗`/`┌─┐`, `[▪]`, `[↕]`, `◢`, drop shadows, and CGA/VGA blue/cyan/yellow/white palettes).

---
*Research completed: 2026-10-20*
*Ready for roadmap: yes*
