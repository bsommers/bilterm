# Feature Research

**Domain:** Terminal Multiplexer, Floating-Prompt Shell Wrapper & TUI Window Manager (IBM 3270 / Mainframe-inspired)
**Researched:** 2026-10-20
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist in any terminal emulator or multiplexer. Missing these makes the product unusable for daily shell workflows.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **PTY Lifecycle Management** | Every pane must spawn, monitor, and cleanly reap interactive login/non-login shells (`bash`, `zsh`, `fish`, `pwsh`/`cmd`). | MEDIUM | Use `portable-pty` for cross-platform POSIX PTY + Windows ConPTY lifecycle and child exit handling. |
| **VT100 / ANSI Escape Sequence Fidelity** | Programs emit 256-color/TrueColor SGR, cursor movement, erase-in-line/display, and scrolling regions. | MEDIUM | Requires per-pane virtual terminal state machine (`vt100` / `vte` / `alacritty_terminal`) mapped to Ratatui cells. |
| **Per-Pane Scrollback Buffer** | Users expect to scroll up through thousands of lines of historical output independently in each pane. | MEDIUM | Ring buffer of lines per composite pane with viewport offset decoupled from live PTY cursor when scrolled back. |
| **Dynamic Resize & `SIGWINCH` Propagation** | Resizing the host terminal or splitting/resizing a pane must immediately reflow/resize the underlying PTY grid. | MEDIUM | Must call `MasterPty::resize(PtySize)` per pane whenever layout rects change. |
| **Alternate Screen Detection (`DECSET 1049`)** | Full-screen TUI apps (`vim`, `nvim`, `htop`, `less`, `fzf`) switch to the alternate screen buffer and expect raw input. | MEDIUM | Crucial for Bilterm: entering `DECSET 1049` (or `47`/`1047`) must auto-hide the floating prompt and forward raw keystrokes directly to the PTY. |
| **Copy / Paste & Bracketed Paste (`DECSET 2004`)** | Users need text selection, system clipboard integration (OSC 52 / native), and safe multi-line paste. | MEDIUM | Must support both copying from the output canvas and pasting into the floating command editor or raw PTY. |
| **Shell Integration & Prompt/Working-Dir Tracking** | Knowing when a command starts/ends, its exit code, and CWD (via OSC 133 / FinalTerm semantic prompts & OSC 7). | MEDIUM | Enables reliable command block boundaries, status indicators, and CWD inheritance when splitting a pane. |
| **Cross-Platform Terminal I/O** | Must work cleanly on Linux, macOS, and Windows without garbled input or broken modifiers. | MEDIUM | `crossterm` event stream + kitty keyboard protocol / modifyOtherKeys support where available. |

### Differentiators (Competitive Advantage)

Features that define Bilterm's unique identity: combining IBM 3270 mainframe ergonomics and Warp-style decoupled command editing with tiling + stacking window management inside a TUI.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Composite Window Pane (`OutputCanvas` + `FloatingPrompt`)** | Unlike standard multiplexers where a pane is just a raw TTY grid, every Bilterm pane bundles a full-height scrolling PTY output canvas with an overlaid, movable, resizable floating command prompt. | HIGH | Output scrolls continuously underneath the floating prompt (`ratatui::widgets::Clear` + overlay composition). Prompt can be pinned top (IBM 3270 style), pinned bottom, or free-floating at any vertical offset. |
| **H/V Splitting of Composite Panes** | Splitting horizontally or vertically spawns a complete new `CompositePane` with its own independent floating command prompt, PTY session, and OIA status bar. | MEDIUM | Binary space partitioning (BSP) or flex layout tree where each leaf (or stacked group) renders a full composite pane. |
| **Z-Order Pane Stacking, Shuffling & Selective Raise** | Panes can overlap or occupy full-workspace layers in a z-ordered deck. Users can shuffle the stack top-to-bottom (rotational cycle) or selectively raise any pane to the front via keyboard picker or mouse click. | HIGH | Bridges tiling (tmux/i3) and stacking/deck window managers (dwm deck, Floating WMs). Requires strict z-index render ordering and hit-testing for mouse focus/raise. |
| **First-Class `tmux` Integration & Support** | Coexists and integrates with `tmux` rather than fighting it: runs cleanly inside `tmux` (outer/inner prefix disambiguation, DCS passthrough) and supports `tmux` session/pane synchronization or `tmux -CC` control mode backend. | HIGH | Solves the #1 adoption blocker for TUI multiplexers (Zellij/byobu users still needing `tmux` persistence on remote servers). |
| **IBM 3270 OIA (Operator Information Area) Status Bar** | Authentic mainframe bottom status line per pane or workspace displaying system lock state (`X SYSTEM` / `READY`), cursor coordinates (`ROW/COL`), active mode, shell exit code, and `tmux` indicators. | LOW | High aesthetic and functional value; gives immediate diagnostic feedback on whether input is routed to the floating prompt or raw PTY. |
| **Cross-Hair Cursor & Retro Mainframe Caret Styles** | Full-pane or screen-spanning horizontal/vertical cross-hair ruler intersecting the active cursor, plus selectable solid block, blinking hollow block, underscore, and bar carets with classic phosphor palettes (3278 Green, 3279 Amber, Monochrome). | MEDIUM | Implemented via cell background/border styling across the active row and column of the Ratatui buffer without mutating underlying text. |
| **Resizable Multi-Line Floating Command Editor** | Compose complex multi-line shell commands, pipes, and loops in a dedicated editor box with syntax highlighting and history search before sending to the PTY. | MEDIUM | Eliminates awkward line-wrapping in standard prompts while keeping full live output visible above and below the floating box. |
| **Command Block Navigation in Scrollback** | Jump directly between previous command invocations and their outputs within each composite pane's scrollback buffer. | MEDIUM | Powered by floating prompt submission hooks + OSC 133 markers; allows quick inspection or copying of prior command outputs. |
| **Triple-Modality Controls** | Every pane and floating prompt action works via keyboard shortcuts, mouse drag-and-drop (titlebar move, border resize, click-to-raise), and `:command` / CLI configuration. | MEDIUM | Ensures fast keyboard-driven workflows over SSH while feeling like a desktop window manager with mouse support enabled. |

### Anti-Features (Commonly Requested, Often Problematic)

Features to deliberately **NOT** build in v1 to prevent scope creep and architectural bloat.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Custom GPU Window / Font Rasterizer (WGPU/OpenGL)** | GUI terminals like Warp, Alacritty, and WezTerm render custom pixel shaders and smooth animations. | Breaks execution over SSH, inside `tmux`, and on headless Linux servers; turns a nimble TUI multiplexer into a heavyweight desktop GUI app. | Render strictly via `ratatui` + `crossterm` inside any host terminal emulator. |
| **Built-In AI Chat / Cloud Account Requirement** | Modern shells (Warp, Fig, Wave) bundle cloud logins and LLM chat sidebars. | Alienates privacy-conscious systems programmers, adds network/auth dependencies, and bloats binary size and startup time. | Keep Bilterm 100% local, offline, and zero-telemetry; users can run CLI AI tools inside any composite pane. |
| **Custom Non-POSIX Shell Language** | Projects like Nushell or Oil invent new shell syntax for structured data. | Breaks existing `.bashrc`/`.zshrc` scripts and muscle memory; huge maintenance burden. | Wrap the user's existing POSIX/Windows shell via PTY and use OSC 133 / input hooks for command block tracking. |
| **Full Pixel Graphics Compositing Across Overlapping Panes (Sixel/Kitty Graphics)** | Users sometimes ask for inline image protocols inside TUI multiplexers. | Character-cell z-order occlusion (`FloatingPrompt` over `OutputCanvas` or stacked panes) cannot cleanly clip half-occluded Sixel pixel bitmaps in host terminals. | Stick to high-precision Unicode/Braille/half-block character rendering and standard ANSI/TrueColor cells in v1. |
| **Complex WASM Plugin Ecosystem in v1** | Zellij provides a full WASM plugin runtime. | Massive API surface and stabilization cost before core composite-pane + z-order UX is proven. | Provide clean TOML/keybinding configuration and `:command` palette controls first. |

## Feature Dependencies

```
[PTY Lifecycle & VT100 Parser]
    └──requires──> [Composite Pane (OutputCanvas + FloatingPrompt)]
                       ├──requires──> [Alternate Screen Auto-Hide (DECSET 1049)]
                       ├──requires──> [Multi-Line Command Editor]
                       ├──requires──> [Command Block Navigation]
                       └──requires──> [H/V Composite Pane Splitting]
                                          └──requires──> [Z-Order Pane Stacking & Shuffling]
                                                             └──enhances──> [Triple-Modality Controls (Mouse Hit-Test / Raise)]

[Composite Pane (OutputCanvas + FloatingPrompt)]
    └──enhances──> [IBM 3270 OIA Status Bar & Cross-Hair Cursor]

[H/V Splitting & Z-Order Stacking]
    └──enhances──> [Tmux Integration (Passthrough / Sync / Control Mode)]
```

### Dependency Notes

- **Composite Pane requires PTY Lifecycle & VT100 Parser:** The floating prompt cannot execute commands or render scrolling output underneath without a working per-pane PTY and virtual screen buffer.
- **Alternate Screen Auto-Hide requires Composite Pane + VT100 Parser:** The parser must detect `CSI ? 1049 h` / `CSI ? 1049 l` state transitions and signal the owning `CompositePane` to hide/restore its `FloatingPrompt` and switch input routing from editor-buffered to direct-PTY mode.
- **Z-Order Pane Stacking & Shuffling requires Composite Pane Layout Engine:** Stacking, cycling z-order top-to-bottom, and raising panes require a window manager tree/deck model that tracks bounding rects and z-indices for each `CompositePane`.
- **Triple-Modality Mouse Controls require Z-Order Hit-Testing:** Clicking to focus/raise a stacked pane or dragging a floating prompt within a pane requires resolving mouse `(col, row)` coordinates against top-most z-order layers first.
- **Command Block Navigation requires Floating Prompt Submission + Shell Integration:** Recording scrollback line offsets when commands are dispatched from the floating prompt (augmented by OSC 133 when available) enables accurate block jumping.

## MVP Definition

### Launch With (v1)

Minimum viable product to validate Bilterm's core value proposition:

- [ ] **Composite Terminal Pane (`OutputCanvas` + `FloatingPrompt`)** — Movable/resizable floating command prompt (top, bottom, or arbitrary vertical offset) over a full-PTY VT100 scrolling output canvas.
- [ ] **Alternate Screen (`DECSET 1049`) Auto-Hide + Manual Toggle** — Seamless transition into raw PTY mode for `vim`, `htop`, and `less`, restoring the floating prompt on exit.
- [ ] **Horizontal & Vertical Splitting of Composite Panes** — Split workspace into multiple side-by-side or top/bottom composite panes, each with its own independent floating prompt and PTY.
- [ ] **Z-Order Pane Stacking, Shuffling & Selective Raise** — Stack composite panes in z-order, shuffle top-to-bottom (cycle deck), and selectively raise any pane to the front via keyboard or mouse.
- [ ] **Triple-Modality Controls (Keyboard, Mouse Drag/Resize/Raise, In-App Commands)** — Complete control over both the inner floating prompt and outer composite panes.
- [ ] **IBM 3270 Mainframe Aesthetics, OIA Status Bar & Cross-Hair Cursor** — OIA indicator bar, classic phosphor themes, cross-hair cursor ruler, and block/hollow-blink/underscore carets.
- [ ] **Command Block Separation & Scrollback Jumping** — Navigate between previous commands and outputs per pane.
- [ ] **`tmux` Coexistence & Integration** — Clean execution inside `tmux` (conflict-free prefix/keybindings, passthrough sequences, status integration) and `tmux` session/control interoperability.

### Add After Validation (v1.x)

Features to add once core composite pane tiling/stacking is validated:

- [ ] **Saved Workspace Layouts & Session Templates** — Persist and restore H/V split + z-order stack arrangements and per-pane prompt positions from declarative config files.
- [ ] **Synchronized Broadcast Input Across Selected Panes** — Type once in a master floating prompt (or broadcast mode) and dispatch simultaneously to multiple split/stacked panes.
- [ ] **Regex Search & Filter Overlay in Scrollback** — Live incremental search across a pane's scrollback buffer and command blocks with match highlighting.

### Future Consideration (v2+)

Features to defer until after v1 maturity:

- [ ] **Native Daemon/Server Detach Mode (Standalone Persistence without `tmux`)** — Background client/server socket architecture for users on machines without `tmux`.
- [ ] **Extensible Scripting / IPC Socket API** — Allow external CLI scripts to open, split, stack, or send commands to Bilterm composite panes programmatically.

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| PTY Lifecycle & VT100 Emulation per Pane | HIGH | MEDIUM | P1 |
| Composite Pane (`OutputCanvas` + `FloatingPrompt` Overlay) | HIGH | HIGH | P1 |
| Alternate Screen (`DECSET 1049`) Auto-Hide & Raw Routing | HIGH | MEDIUM | P1 |
| H/V Composite Pane Splitting | HIGH | MEDIUM | P1 |
| Z-Order Pane Stacking, Top-to-Bottom Shuffle & Selective Raise | HIGH | HIGH | P1 |
| Multi-Line Resizable Floating Command Editor | HIGH | MEDIUM | P1 |
| Triple-Modality Controls (Keyboard + Mouse Drag/Resize/Raise) | HIGH | MEDIUM | P1 |
| IBM 3270 OIA Status Bar, Cross-Hair Cursor & Caret Styles | HIGH | LOW | P1 |
| `tmux` Coexistence, Keybinding Ergonomics & Control Integration | HIGH | HIGH | P1 |
| Command Block Separation & Navigation | MEDIUM | MEDIUM | P1 |
| Saved Layout Templates & Broadcast Input | MEDIUM | MEDIUM | P2 |
| Standalone Daemon/Detach Server | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Must have for v1 launch (aligned with `PROJECT.md` Active requirements)
- P2: Should have in v1.x once core is validated
- P3: Future consideration (v2+)

## Competitor Feature Analysis

| Feature | `tmux` / `Zellij` (TUI Multiplexers) | `Warp` / `Wave` (Modern GUI Shells) | `x3270` / IBM 3270 (Mainframe Terminals) | **Bilterm (Our Approach)** |
|---------|--------------------------------------|-------------------------------------|------------------------------------------|----------------------------|
| **Command Input Model** | Raw PTY line discipline at active cursor row | Decoupled multi-line input editor pinned at bottom/top of GUI window | Dedicated command entry field (often top or bottom) + OIA status line | **Per-pane movable/resizable floating command prompt over scrolling PTY canvas** |
| **Pane Splitting (H/V)** | Splits raw TTY grids | Splits GUI tabs/panes | Single session screen (or MVS split screen) | **Splits full `CompositePane` units (each gets its own floating prompt + PTY canvas)** |
| **Z-Order Stacking & Shuffling** | Basic single floating pane toggle (`tmux popup`, Zellij floating layer) | None (strictly tiled panes + tabs) | None | **Full z-order pane stacking, top-to-bottom deck shuffling, and selective raise via keyboard/mouse** |
| **Output Visibility Under Prompt** | Prompt consumes bottom line; no overlay | Prompt occupies dedicated non-overlapping region | Fixed field layout | **Output spans full pane height and scrolls continuously underneath the floating prompt** |
| **Cursors & Aesthetics** | Host terminal cursor only | GUI block/bar cursor | Cross-hair cursor ruler, blinking/solid block, underscore, OIA bar | **TUI-rendered cross-hair cursor ruler, block/hollow-blink/underscore carets, and IBM 3270 OIA bar** |
| **Deployment Environment** | Runs anywhere in terminal / over SSH | Requires local GPU desktop GUI | Dedicated X11/GUI emulator (`x3270`/`c3270`) | **Pure Rust + Ratatui TUI: runs locally, over SSH, and seamlessly inside/with `tmux`** |

## Sources

- Project specification and constraints in [`PROJECT.md`](file:///home/bill/src/ai/bilterm/.planning/PROJECT.md)
- Terminal multiplexer domain architecture (`tmux` control mode `-CC` specification, DCS passthrough sequences, `Zellij` tiled vs floating pane model)
- Modern block-based shell wrappers (`Warp` input editor decoupling, FinalTerm / OSC 133 semantic prompt integration specification)
- IBM 3270 Display System Reference (Operator Information Area [OIA] indicators, cross-hair cursor assist, 3278/3279 phosphor ergonomics, ISPF top-command-line conventions)

---
*Feature research for: Bilterm (TUI Multiplexer, Floating-Prompt Shell Wrapper & IBM 3270-inspired Composite Window Manager)*
*Researched: 2026-10-20*
