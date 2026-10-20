# Architecture Research

**Domain:** Composite-Pane Terminal Emulator, Shell Wrapper, and TUI Multiplexer (Rust + Ratatui)
**Researched:** 2026-10-19
**Confidence:** HIGH

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                       Host Terminal (Crossterm / Tmux)                       │
├──────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────────┐  ┌──────────────────────┐  ┌────────────────────┐  │
│  │  Input Event Stream  │  │  Async Event Loop    │  │   Tmux Bridge      │  │
│  │  (Key / Mouse / Sig) │─▶│  (Tokio + Tick/Coa.) │◀─│ (Passthrough / CC) │  │
│  └──────────────────────┘  └──────────┬───────────┘  └────────────────────┘  │
│                                       │                                      │
├───────────────────────────────────────┼──────────────────────────────────────┤
│                               Window Manager Layer                           │
├───────────────────────────────────────┼──────────────────────────────────────┤
│  ┌──────────────────────┐  ┌──────────▼───────────┐  ┌────────────────────┐  │
│  │    Input Router      │  │    Layout Engine     │  │  Z-Order Manager   │  │
│  │ (WM / Prompt / Raw)  │─▶│ (Binary Split Tree)  │◀▶│ (Stack / Shuffle)  │  │
│  └──────────┬───────────┘  └──────────┬───────────┘  └─────────┬──────────┘  │
│             │                         │                        │             │
├─────────────┼─────────────────────────┼────────────────────────┼─────────────┤
│             ▼              Composite Pane Layer (Per Pane)     ▼             │
├──────────────────────────────────────────────────────────────────────────────┤
│  ┌────────────────────────────────────────────────────────────────────────┐  │
│  │                          CompositePane Unit                            │  │
│  │  ┌────────────────┐ ┌────────────────┐ ┌────────────┐ ┌─────────────┐  │  │
│  │  │  OutputCanvas  │ │ FloatingPrompt │ │ OIA Status │ │CursorOverlay│  │  │
│  │  │ (VT Grid View) │ │ (Multi-line Ed)│ │ (3270 Bar) │ │ (Crosshair) │  │  │
│  │  └───────▲────────┘ └───────┬────────┘ └─────▲──────┘ └──────▲──────┘  │  │
│  └──────────┼──────────────────┼────────────────┼───────────────┼─────────┘  │
│             │                  │ (Submit/Send)  │ (DECSET/Meta) │            │
├─────────────┼──────────────────┼────────────────┴───────────────┼────────────┤
│             │          Terminal Emulation & PTY Layer           │            │
│  ┌──────────┴───────────┐  ┌───▼──────────────────┐  ┌──────────┴─────────┐  │
│  │  VT State Machine    │  │  PTY Manager/Writer  │  │  Command Block     │  │
│  │ (vte/vt100 + OSC133) │◀─│ (portable-pty / OS)  │─▶│  Tracker (Scroll)  │  │
│  └──────────────────────┘  └──────────────────────┘  └────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| **Async Event Loop** | Multiplexes Crossterm input streams, PTY output channels, SIGWINCH resize signals, and render frame ticks (60–120 FPS coalescing). | `tokio::select!` loop with `crossterm::event::EventStream` and bounded `tokio::sync::mpsc` channels. |
| **PTY Manager & Reader Threads** | Spawns OS shell processes (`bash`, `zsh`, `fish`, `pwsh`), manages master/slave file descriptors, handles `TIOCSWINSZ` resizes, and reads blocking PTY byte streams off the async runtime. | `portable-pty` (`MasterPty`, `Child`) with dedicated OS reader thread (`std::thread::spawn`) per pane sending byte chunks (`Bytes`) to the main loop. |
| **Virtual Terminal State Machine** | Parses ANSI/VT100/xterm escape sequences, maintains primary & alternate screen grids, tracks scrollback ring buffer, detects DECSET 1049 (Alternate Screen), and parses OSC 133 shell integration markers. | `vte` parser paired with custom/extended `vt100::Parser` or `alacritty_terminal::Term` state grid with semantic block offsets. |
| **CompositePane Widget** | Encapsulates a single terminal unit: renders `OutputCanvas` across full pane bounds, overlays `FloatingPrompt` (with `Clear` occlusion), draws `OiaStatusBar` (IBM 3270 status line), and applies `CursorOverlay`. | Stateful Ratatui `Widget` / `StatefulWidget` composing sub-widgets into a target `ratatui::layout::Rect`. |
| **Layout Engine (Split + Z-Order)** | Manages spatial geometry via a Binary Split Tree (Horizontal/Vertical ratios) and overlapping/deck layers via a Z-Order Stack Manager (raise, lower, shuffle/rotate, floating rect coordinates). | Arena-allocated binary tree (`SlotMap<PaneId, Node>`) for splits + `Vec<PaneId>` z-index ordering vector for painter's algorithm rendering and inverse hit-testing. |
| **Input Router** | Routes keyboard and mouse events based on active mode and pane state: Window Manager prefix/hotkeys, Mouse Hit-Tester (top-most z-order pane/prompt drag/resize), Floating Prompt text editor, or Raw PTY pass-through (when in Alternate Screen or passthrough mode). | Modal state machine (`InputMode::{NormalPrompt, RawPty, WmCommand, MouseDrag}`) inspecting active `PaneId` and its DECSET 1049 flag. |
| **Tmux Bridge** | Detects nested execution (`$TMUX`), wraps outbound escape sequences in DCS passthrough (`\ePtmux;...\e\\`), negotiates non-conflicting prefix keys, and optionally synchronizes windows/panes via `tmux -CC` control mode. | Environment inspector + escape sequence formatter + optional async `tmux -CC` stdout/stdin parser task. |

---

## Recommended Project Structure

```
src/
├── main.rs                 # CLI entry, panic hook (terminal restore), Tokio runtime init
├── app.rs                  # Top-level App state, main select! event loop, frame tick coalescer
├── config/                 # Configuration, keybindings, IBM 3270 color themes
│   ├── mod.rs
│   ├── keymap.rs           # Prefix keys, WM shortcuts, prompt editor bindings
│   └── theme.rs            # Phosphor green, amber, monochrome, cursor style configs
├── wm/                     # Window Manager: spatial layout & z-order stacking
│   ├── mod.rs
│   ├── workspace.rs        # Combines split tree + floating/stacked z-order list
│   ├── split_tree.rs       # Binary tree for H/V tiled splits and rect calculation
│   ├── z_stack.rs          # Z-order stack, shuffle (rotate), selective raise, hit-testing
│   └── hit_test.rs         # Maps (x, y) mouse coords to top-most PaneId & sub-region
├── pane/                   # CompositePane state & sub-component models
│   ├── mod.rs              # CompositePane struct owning VT state, Prompt state, PTY handle
│   ├── prompt.rs           # Multi-line editor buffer, history, drag/pin offset state
│   ├── oia.rs              # IBM 3270 Operator Information Area indicators & metrics
│   └── blocks.rs           # Command block separation & scrollback jump index (OSC 133)
├── vt/                     # Virtual Terminal emulation & ANSI parsing
│   ├── mod.rs
│   ├── screen.rs           # Grid storage, scrollback ring buffer, dirty cell tracking
│   ├── parser.rs           # VTE Perform implementation, DECSET 1049, OSC 133 hooks
│   └── cell.rs             # Cell attributes, colors, graphemes
├── pty/                    # Cross-platform PTY management
│   ├── mod.rs
│   ├── manager.rs          # Spawns portable-pty processes, tracks PIDs & resize calls
│   └── worker.rs           # Blocking OS reader thread & async writer channel per pane
├── input/                  # Modal input routing & mouse interaction state machine
│   ├── mod.rs
│   ├── router.rs           # Dispatches Crossterm events to WM, Prompt, or Raw PTY
│   └── mouse.rs            # Tracks active drag/resize operations on panes & prompts
├── ui/                     # Ratatui rendering widgets & buffer compositors
│   ├── mod.rs              # Root draw() orchestrating painter's algorithm across z-order
│   ├── composite_pane.rs   # Renders OutputCanvas + Clear + FloatingPrompt + OIA
│   ├── output_canvas.rs    # Blits VT screen grid into Ratatui Buffer
│   ├── floating_prompt.rs  # Renders movable/resizable input box with border/title handles
│   ├── oia_bar.rs          # Renders IBM 3270 status line (X SYSTEM, insert mode, cursor pos)
│   └── cursor_overlay.rs   # Post-pass buffer modifier for 3270 cross-hair & custom carets
└── tmux/                   # Tmux coexistence & integration
    ├── mod.rs
    ├── passthrough.rs      # $TMUX detection, DCS passthrough wrapping, terminfo checks
    └── control_mode.rs     # Optional `tmux -CC` protocol parser and session bridge
```

### Structure Rationale

- **`wm/` vs `pane/` separation:** Decouples *where* panes live on screen (`wm/` split tree and z-order stack) from *what* happens inside a pane (`pane/` composite prompt + terminal canvas). This allows any `CompositePane` to transition seamlessly between tiled splits and overlapping z-order stacks without mutating internal pane state.
- **`vt/` and `pty/` isolation:** Keeps low-level OS pseudo-terminal I/O (`pty/`) and stateful ANSI escape sequence parsing (`vt/`) completely independent of Ratatui (`ui/`). This makes VT100 emulation and OSC 133 command block tracking unit-testable via headless byte streams.
- **`ui/` post-processing pipeline:** Isolates Ratatui `Buffer` manipulation—specifically `Clear` widget occlusion for floating prompts, z-indexed overlapping panes, and full-pane/full-screen IBM 3270 cross-hair cursor overlays—into composable rendering passes.

---

## Architectural Patterns

### Pattern 1: Painter's Algorithm with `Clear` Occlusion for Composite & Stacked Panes

**What:** Ratatui renders into a flat 2D `Buffer` of cells (`x, y`) with no built-in z-index or DOM hierarchy. To support both **stacked/overlapping composite panes** and **floating command prompts over scrolling PTY output**, rendering executes in strict back-to-front z-order using `ratatui::widgets::Clear` before drawing occluding layers.

**When to use:** Every render frame when compositing `CompositePane` layers and internal `FloatingPrompt` overlays.

**Trade-offs:**
- *Pros:* Zero custom compositing engine needed; works natively with all Ratatui widgets (`Block`, `Paragraph`, borders); deterministic occlusion.
- *Cons:* Overdraw cost if dozens of full-screen panes are stacked (mitigated because terminal grids are small, e.g., 200x60 = 12,000 cells, taking microseconds in Rust).

**Example:**
```rust
pub fn render_workspace(frame: &mut Frame, workspace: &mut Workspace) {
    // 1. Iterate panes in bottom-to-top Z-order
    for pane_id in workspace.z_order_bottom_to_top() {
        let pane_rect = workspace.rect_for(pane_id);
        let pane = workspace.pane_mut(pane_id);

        // Clear underlying buffer region if this pane overlaps lower panes
        frame.render_widget(ratatui::widgets::Clear, pane_rect);

        // 2. Render full-height PTY OutputCanvas + OIA Status Bar
        let (canvas_rect, oia_rect) = pane.layout_base_rects(pane_rect);
        frame.render_widget(OutputCanvas::new(&pane.vt), canvas_rect);
        frame.render_widget(OiaStatusBar::new(pane), oia_rect);

        // 3. Render FloatingPrompt on top (unless auto-hidden by DECSET 1049)
        if pane.is_prompt_visible() {
            let prompt_rect = pane.compute_prompt_rect(canvas_rect);
            frame.render_widget(ratatui::widgets::Clear, prompt_rect); // Occlude PTY text underneath
            frame.render_widget(FloatingPrompt::new(&pane.prompt), prompt_rect);
        }

        // 4. Render 3270 Cross-Hair / Custom Cursor Overlay directly on Frame Buffer
        if workspace.is_focused(pane_id) {
            CursorOverlay::apply(frame.buffer_mut(), pane, canvas_rect, prompt_rect);
        }
    }
}
```

### Pattern 2: Dedicated PTY Reader Threads + Frame-Coalesced Async Loop

**What:** Cross-platform PTY master file descriptors (`portable-pty`) use blocking OS reads (especially on Windows ConPTY and certain POSIX configurations). Spawn one lightweight OS thread per pane to perform `read(&mut buf)` into reusable byte buffers, forwarding chunks over a bounded `tokio::sync::mpsc` channel (for backpressure) to the central Tokio event loop, which coalesces redraws to a 8.3ms (120 FPS) or 16.6ms (60 FPS) render ticker.

**When to use:** Always, for all PTY I/O management.

**Trade-offs:**
- *Pros:* Prevents `cat /dev/urandom` or massive build logs in one pane from starving input responsiveness or freezing other panes; avoids `tokio::fs` async FD incompatibilities across Linux/macOS/Windows.
- *Cons:* Requires clean thread teardown when child shell exits (`EOF` / `SIGHUP`).

**Example:**
```rust
// Inside PTY worker setup
let (tx, rx) = tokio::sync::mpsc::channel::< (PaneId, Bytes) >(256);
std::thread::spawn(move || {
    let mut buf = [0u8; 16384];
    while let Ok(n) = pty_reader.read(&mut buf) {
        if n == 0 { break; } // EOF
        if tx.blocking_send((pane_id, Bytes::copy_from_slice(&buf[..n]))).is_err() {
            break;
        }
    }
});
```

### Pattern 3: Dual-Mode Input Routing with DECSET 1049 Auto-Switching

**What:** Each `CompositePane` operates in one of two primary interaction modes:
1. **Command Entry Mode (Default / Primary Screen):** Keystrokes route to the `FloatingPrompt` multi-line editor. Pressing `Enter` writes the composed command line (`+ "\n"`) to the PTY master writer. Control signals (`Ctrl+C`, `Ctrl+D`, `Ctrl+Z`) or interactive prompts can be forwarded when the prompt is empty or via passthrough toggle.
2. **Raw PTY Passthrough Mode (Alternate Screen / DECSET 1049 Active):** When the VT parser detects `\x1b[?1049h` (or `\x1b[?47h` / `\x1b[?1047h`), `pane.in_alternate_screen` flips to `true`. The `FloatingPrompt` auto-hides, and the `InputRouter` bypasses the prompt editor, encoding Crossterm key/mouse events directly into VT100 byte sequences written immediately to the PTY master. When `\x1b[?1049l` arrives (`vim` or `htop` exits), the floating prompt restores automatically.

**When to use:** Per-pane input dispatch on every keyboard or mouse event.

**Trade-offs:**
- *Pros:* Solves the classic shell-wrapper dilemma where TUI apps (`vim`, `less`, `htop`, `fzf`) break inside line-buffered command editors.
- *Cons:* Programs that prompt interactively *without* entering Alternate Screen (e.g., `sudo` password prompt, `ssh` passphrase, `python` REPL) require either heuristic detection (PTY `tcgetattr` `ECHO` off / canonical mode inspection on POSIX, or cursor-at-prompt heuristics) plus an explicit manual toggle hotkey (e.g., `Ctrl+Space` or `Alt+P`) to toggle focus between `FloatingPrompt` and direct PTY canvas input.

### Pattern 4: IBM 3270 Cross-Hair Cursor via Post-Render Buffer Mutation

**What:** The hardware terminals use a solid block cursor at `(cursor_x, cursor_y)`. To render an IBM 3270 cross-hair cursor (a horizontal rule across the active row and a vertical rule down the active column within the active composite pane) plus custom caret styles (hollow block, blinking block, underscore), hide the host terminal's hardware cursor during custom rendering and mutate cell attributes (`Modifier::REVERSED`, `Modifier::UNDERLINED`, or background tinting) across row `cy` and column `cx` in Ratatui's `Buffer`.

**When to use:** Final stage of `CompositePane` rendering for the focused pane.

**Trade-offs:**
- *Pros:* Authentic 3270 cross-hair spanning the pane regardless of host terminal emulator capabilities; works identically over SSH and inside `tmux`.
- *Cons:* Requires modifying background/border styles along row `cy` and col `cx` without destroying underlying foreground text readability.

---

## Data Flow

### Request Flow (Keyboard & Mouse Input to PTY / UI)

```
[Host Terminal Input (Crossterm EventStream)]
    │
    ▼
[InputRouter] ──(Is WM Prefix / Global Shortcut?)──▶ [Workspace / Z-Order / Split Tree]
    │                                                       │ (Split, Shuffle, Raise, Resize)
    │ (No)                                                  ▼
    ▼                                                [Mark Dirty -> Render Frame]
[Hit-Test / Active Pane Lookup]
    │
    ├──(Mouse Click on Unfocused Pane)──▶ [Z-Order Manager: Raise Pane to Top]
    ├──(Mouse Drag on Prompt/Pane Border)─▶ [Update Floating Offset / Split Ratio]
    │
    ▼ (Keyboard Event for Focused Pane)
[Check Pane Mode: in_alternate_screen || raw_passthrough_override?]
    │
    ├──(TRUE: Raw PTY Mode)───▶ [Encode Key to ANSI Bytes] ──▶ [PTY Writer Channel]
    │
    └──(FALSE: Prompt Mode)───▶ [FloatingPrompt Multi-Line Editor]
                                     │
                                     ├──(Edit / History / Move)──▶ [Mark Dirty]
                                     └──(Submit / Enter)─────────▶ [PTY Writer Channel]
```

### PTY Output & Compositing Flow

```
[Child Shell Process (bash/zsh/vim)]
    │ (writes stdout/stderr to Slave PTY)
    ▼
[OS Reader Thread (portable-pty Master)]
    │ (sends Bytes via bounded mpsc channel)
    ▼
[Tokio Main Loop] ──▶ [VT State Machine (vte + Screen Grid)]
                           │
                           ├── Updates Primary/Alt Cell Grid & Scrollback
                           ├── Toggles `in_alternate_screen` on DECSET 1049
                           └── Records OSC 133 Command Block Start/End Offsets
                           │
                           ▼ (Coalesced on 60/120Hz Frame Timer)
                      [Ratatui Terminal::draw()]
                           │
                           ├── 1. Compute Split Tree Rects
                           ├── 2. Iterate Z-Order Stack (Back-to-Front):
                           │      ├── Clear Pane Rect (if overlapping)
                           │      ├── Blit VT Grid -> OutputCanvas (Full Pane Height)
                           │      ├── Render OIA Status Bar (Bottom of Pane)
                           │      ├── If !in_alternate_screen:
                           │      │     ├── Clear Prompt Rect (Occlude underlying PTY output)
                           │      │     └── Render FloatingPrompt at (pinned/free offset)
                           │      └── Apply 3270 Cross-Hair & Caret Overlay to Buffer
                           └── 3. Diff Buffer & Flush ANSI to Host Terminal (Crossterm)
```

### Key Data Flows

1. **Full-Height Under-Scrolling (`OutputCanvas` under `FloatingPrompt`):** Crucially, the PTY window size (`TIOCSWINSZ` rows/cols) is set to the **entire `OutputCanvas` dimensions** (`pane_rect.height - oia_height`), *not* reduced by the `FloatingPrompt` height. This guarantees that shell output occupies the full pane height and scrolls directly underneath the floating prompt overlay. Optionally, when the prompt is pinned to the top (3270 style), scrollback offset or top-margin padding can be adjusted by the user so the latest output line isn't hidden behind a bottom-pinned prompt.
2. **Z-Order Shuffling & Selective Raising:** `Workspace` maintains `split_tree: SplitTree` (defining tiled slots) and `z_stack: Vec<PaneId>` (defining render and hit-test priority for floating/stacked panes, or full-workspace deck layers).
   - **Shuffle:** Rotates `z_stack` (`rotate_right(1)` or `rotate_left(1)`), cycling which stacked pane is at the top of the deck.
   - **Selective Raise:** Moves a target `PaneId` (via mouse click hit-test in reverse `z_stack` order or keyboard pane-picker) to the end of `z_stack` (top-most layer).
3. **OSC 133 + Prompt Submission Command Block Tracking:** When the user submits a command from `FloatingPrompt`, `CompositePane` records a `CommandBlock { id, command_text, start_scrollback_row, end_scrollback_row, exit_code }` at the current VT scrollback position. If shell integration (`OSC 133;A/B/C/D`) is also active, exact start/finish markers and exit statuses enrich the block metadata, enabling instant `[` / `]` jumping between prior command invocations in scrollback.

---

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| **1–4 Panes (Typical)** | Standard single-threaded rendering in main Tokio loop; 16KB read buffers per PTY thread; 10,000-line scrollback ring buffer per pane. |
| **5–20 Stacked/Split Panes** | Skip VT grid-to-cell formatting for completely occluded panes in the z-stack; bound PTY channel reads per tick (e.g., max 64KB processed per pane per 8ms frame budget) so a noisy background pane cannot delay UI frame delivery. |
| **100k+ Scrollback Lines / High-Throughput Output** | Use fixed-size circular ring buffers (`VecDeque<Row>` or flat slab storage) with copy-on-write cell attributes to cap memory per pane at ~15–25 MB and keep scrollback allocation $O(1)$. |

### Scaling Priorities

1. **First bottleneck: High-throughput PTY output (`yes`, `cat large.log`, `cargo build`) flooding the main event loop.**
   - *Fix:* Bounded `mpsc::channel(64)` between the PTY reader thread and Tokio loop provides automatic OS-level PTY backpressure, combined with **frame tick coalescing**: process incoming PTY bytes immediately in memory, set a `dirty = true` flag, and only call `terminal.draw()` on the 120Hz `tokio::time::Interval` tick.
2. **Second bottleneck: POSIX `tcgetpgrp` / Foreground Process Inspection when Prompt is decoupled.**
   - *Fix:* When a user runs a command that prompts for interactive input *without* entering DECSET 1049 (e.g., `ssh`, `sudo`, `git commit` using line editor, `read -p`), check the PTY master FD's foreground process group (`tcgetpgrp(master_fd)` != shell PGID on Linux/macOS) or `termios` (`!ICANON` or `!ECHO`) on a lightweight poll/event basis to show an OIA indicator (`INPUT REQUIRED / RAW`) and optionally auto-focus or prompt the user to send input directly.

---

## Anti-Patterns

### Anti-Pattern 1: Shrinking the PTY `TIOCSWINSZ` When Moving/Resizing the Floating Prompt

**What people do:** Treat the `FloatingPrompt` and `OutputCanvas` as two adjacent layout splits, resizing the child PTY (`TIOCSWINSZ`) every time the user drags or resizes the floating command prompt.
**Why it's wrong:** Sending `SIGWINCH` on every mouse drag frame causes the underlying shell (`bash`/`zsh`) to violently reflow text, corrupt multi-line output, and defeats the requirement that terminal output scrolls *underneath* the floating prompt.
**Do this instead:** Size the PTY to the full `OutputCanvas` area (`pane_rect` minus the 1-row OIA bar). Render the `FloatingPrompt` strictly as an **overlay layer** using `ratatui::widgets::Clear` + `FloatingPrompt` widget at its `(offset_x, offset_y, height)` coordinates. Only send `TIOCSWINSZ` when the outer `CompositePane` itself is split or resized.

### Anti-Pattern 2: Calling `terminal.draw()` on Every PTY Read Event

**What people do:** Trigger a full Ratatui `terminal.draw()` inside the `tokio::select!` branch that receives a byte chunk from a PTY reader channel.
**Why it's wrong:** A fast producer emits thousands of small chunks per second, causing 100% CPU utilization in Ratatui diffing and flooding the host terminal (or `tmux`) with redundant ANSI escape sequences.
**Do this instead:** Decouple state mutation from rendering. In the PTY channel branch, feed bytes into `pane.vt.process(&bytes)` and set `app.needs_redraw = true`. Run a dedicated `render_tick = tokio::time::interval(Duration::from_millis(8))` branch that draws only when `app.needs_redraw` is true.

### Anti-Pattern 3: Reverse-Order Mismatch Between Rendering and Mouse Hit-Testing

**What people do:** Render stacked panes or floating prompts in one order, but iterate panes in creation order (or split-tree order) when handling mouse clicks/drags.
**Why it's wrong:** Clicking on an overlapping foreground pane or floating prompt accidentally routes the mouse click to the background pane underneath it.
**Do this instead:** Maintain a single authoritative `z_order: Vec<PaneId>` list. **Render** by iterating `0..len` (bottom-to-top), and **hit-test mouse events** by iterating `(0..len).rev()` (top-to-bottom), checking the floating prompt rect first before the underlying canvas rect within each pane.

---

## Integration Points

### External Services & Host Environment

| System | Integration Pattern | Notes |
|--------|---------------------|-------|
| **Host Terminal (Linux / macOS / Windows)** | `crossterm` raw mode, alternate screen, bracketed paste, mouse capture (`EnableMouseCapture`), kitty keyboard protocol (optional). | Always install a panic hook before entering raw mode so crashes restore terminal state cleanly. |
| **Tmux (Running Inside `$TMUX`)** | Inspect `std::env::var("TMUX")`. Wrap passthrough escape sequences (OSC 52 clipboard, cursor shape changes) in `\x1bPtmux;\x1b...\x1b\\`. | Default Bilterm WM prefix must avoid `Ctrl+B` (or support seamless prefix-free Alt/Mouse bindings) to prevent double-prefix friction inside `tmux`. |
| **Tmux Control Mode (`tmux -CC`) / Session Sync** | Optional backend mode where Bilterm spawns or attaches to `tmux -CC`, mapping tmux `%pane_id` output streams (`%output %1 ...`) to `CompositePane` instances. | Allows Bilterm composite panes to persist across disconnects using `tmux` as the session daemon. |
| **OS Shells (`bash`, `zsh`, `fish`, `pwsh`)** | Spawned via `portable-pty` with injected shell-integration env/rc hooks (emitting OSC 133 prompt/command boundaries). | Keep OSC 133 optional/graceful: if a remote SSH session doesn't emit OSC 133, command block separation still works via `FloatingPrompt` submission timestamps/row markers. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| **`PTY Worker` ↔ `App Event Loop`** | `tokio::sync::mpsc::channel<(PaneId, PtyEvent)>` (Output bytes, ChildExited) and `mpsc::UnboundedSender<Bytes>` for PTY writes. | Non-blocking from UI thread; bounded on read path for backpressure. |
| **`InputRouter` ↔ `Workspace / CompositePane`** | Synchronous method dispatch (`Action` enum) on `&mut AppState`. | Keeps state transitions deterministic and easy to replay in tests. |
| **`VT State Machine` ↔ `CompositePane`** | Synchronous event callbacks (`VtEvent::{AltScreenChanged(bool), Bell, Osc133(BlockMarker), TitleChanged(String)}`). | Immediately updates `CompositePane::in_alternate_screen` so `FloatingPrompt` auto-hides before the next frame draw. |

---

## Suggested Build Order (Roadmap Phase Dependencies)

Because every feature in Bilterm builds around the **`CompositePane`** abstraction, the architecture dictates a strict bottom-up dependency order:

1. **Phase 1: Core PTY + VT100 Engine & Single CompositePane Prototype**
   - *Components:* `pty/` (`portable-pty` + reader thread), `vt/` (`vte`/`vt100` screen grid + DECSET 1049 detection), basic `app.rs` Tokio coalesced event loop, and a single `CompositePane` (`OutputCanvas` + `Clear` + `FloatingPrompt` + basic `InputRouter`).
   - *Why first:* Validates the core value proposition—running a live shell where output scrolls full-height underneath a floating command input box, and `vim`/`htop` auto-hide the floating prompt via DECSET 1049.
2. **Phase 2: Floating Prompt Ergonomics, Triple-Modality Controls & Command Blocks**
   - *Components:* `pane/prompt.rs` (multi-line resize, history, syntax highlight, pinned top/bottom/free-floating vertical offset via keyboard, mouse drag, and in-prompt commands) + `pane/blocks.rs` (scrollback + command block separation/jumping).
   - *Why second:* Completes the intra-pane UX before multiplying panes across a window manager.
3. **Phase 3: Window Manager — H/V Binary Splits & Z-Order Stacking/Shuffling**
   - *Components:* `wm/split_tree.rs`, `wm/z_stack.rs`, `wm/hit_test.rs`, multi-pane `InputRouter` focus management.
   - *Why third:* Once `CompositePane` is a self-contained stateful widget that renders into any `Rect`, the Layout Engine can instantiate $N$ composite panes inside split rects or overlapping z-order stacks with keyboard/mouse shuffle and selective raise.
4. **Phase 4: IBM 3270 Mainframe Aesthetics, OIA Status Bar & Cross-Hair Cursor Overlay**
   - *Components:* `ui/oia_bar.rs`, `ui/cursor_overlay.rs`, `config/theme.rs` (phosphor green/amber/monochrome palettes, cross-hair row/column buffer post-processor, block/hollow/underscore carets, foreground process `X SYSTEM` / `RAW` OIA indicators).
   - *Why fourth:* Builds cleanly on top of the compositor pipeline and multi-pane focus rules established in Phases 1–3.
5. **Phase 5: Tmux Integration & Cross-Platform Hardening (Linux -> macOS -> Windows)**
   - *Components:* `tmux/passthrough.rs`, `tmux/control_mode.rs`, platform-specific PTY/termios polish (`ConPTY` on Windows, POSIX `tcgetpgrp` on Linux/macOS).
   - *Why fifth:* Integrates the completed multiplexer with outer/inner `tmux` workflows and verifies cross-platform parity.

---

## Sources

- Ratatui Architecture & Custom Widget Compositing (`ratatui::widgets::Clear`, direct `Buffer` manipulation, async Tokio event loop with frame coalescing)
- `portable-pty` (WezTerm PTY crate) cross-platform master/slave PTY lifecycle and ConPTY threading semantics
- `vt100` / `alacritty_terminal` (`vte`) state machine specifications, DECSET 1049 (`\x1b[?1049h/l`), and FinalTerm/OSC 133 semantic prompt markers
- IBM 3270 Display System Reference (Operator Information Area status indicators, rule/cross-hair cursor geometry, decoupled command entry conventions)
- Tmux Control Mode (`tmux -CC`) and DCS Passthrough (`\ePtmux;...\e\\`) specifications

---
*Architecture research for: Bilterm (Composite-Pane Rust + Ratatui Terminal Multiplexer)*
*Researched: 2026-10-19*
