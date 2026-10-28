<!-- GSD:project-start source:PROJECT.md -->

## Project

**Bilterm**

Bilterm is a terminal user interface (TUI) multiplexer, shell wrapper, and terminal emulator written in Rust using Ratatui. Inspired by classic **IBM mainframe terminals** (such as the IBM 3270 where the command prompt sits at the top of the screen) and **classic DOS Borland Turbo Vision / Turbo C / Turbo Pascal windowing systems** (double-line box-drawing borders, drop shadows, classic blue/cyan/yellow/white CGA/VGA palettes, window numbers, and top/bottom menu/hotkey bars), Bilterm treats the **floating command prompt + underlying output canvas as a single composite terminal pane**. These composite terminal panes can be split horizontally and vertically, or stacked in z-order (shuffled top-to-bottom or raised selectively via keyboard or mouse), with full `tmux` integration/support.

**Core Value:** A responsive composite terminal pane (movable/resizable floating command prompt over a full-PTY output canvas that scrolls underneath) that can be split horizontally/vertically, stacked/shuffled in z-order via keyboard or mouse, and integrated seamlessly with `tmux`.

### Constraints

- **Tech Stack**: Rust + Ratatui — Required by project vision for fast, zero-cost TUI rendering and cross-platform terminal manipulation.
- **Platform Priority**: Linux first, macOS second, Windows third — Ensures rapid iteration on POSIX PTY semantics while keeping cross-platform abstractions (`portable-pty`, `crossterm`) clean from day one.
- **Compatibility**: Must cleanly handle interactive TUI programs (`vim`, `htop`, `ssh`) per composite pane via Alternate Screen detection, and coexist/integrate cleanly with `tmux`.

<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->

## Technology Stack

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **Rust** | `1.88+` (Edition 2024) | Systems language & memory-safe concurrency | Required by `tui-term 0.3.4+` and modern `ratatui 0.30` modular workspace crates; zero-cost abstractions keep multi-pane 60–120 FPS cell buffer compositing under <1ms per frame. |
| **`ratatui`** | `0.30.2` | Immediate-mode TUI rendering engine & cell buffer compositor | Standard Rust TUI framework. Its immediate-mode intermediate `Buffer` architecture (`frame.buffer_mut()`) allows rendering a pane's PTY canvas first, then rendering `widgets::Clear` + the floating command prompt directly on top, and finally applying post-processing shaders/cell mutations for IBM 3270 cross-hair cursors and z-order window stacking. |
| **`crossterm`** | `0.29.0` (with `event-stream`) | Cross-platform host terminal manipulation & input events | Pairs directly with `ratatui 0.30`. Provides unified raw mode, SGR mouse drag/click/scroll capture (`EnableMouseCapture`), Kitty keyboard protocol flags (`PushKeyboardEnhancementFlags`), bracketed paste, and Windows Console API / ConPTY host compatibility. |
| **`portable-pty`** | `0.9.0` | Cross-platform pseudoterminal (PTY) allocation & process management | Maintained by the WezTerm project. Abstracts POSIX `openpty`/`forkpty` on Linux and macOS alongside Windows `ConPTY` behind a single `PtySystem` trait (`MasterPty`, `Child`, `CommandBuilder`), satisfying Bilterm's Linux -> macOS -> Windows priority without platform-ifdef spaghetti. |
| **`alacritty_terminal`** + **`vte`** | `0.26.0` / `0.15.0` | Primary virtual terminal state machine, ANSI/VT100 parser, scrollback grid & DECSET tracking | Battle-tested terminal emulation core. Unlike simpler screen scrapers, `alacritty_terminal::Term` natively tracks `TermMode::ALT_screen` (DECSET 1049 for auto-hiding the floating prompt during `vim`/`htop`), `TermMode::BRACKETED_PASTE`, mouse reporting modes, semantic OSC 133 shell prompt/command-block markers, and fast ring-buffer scrollback reflow on H/V pane resize. *(See `vt100-ctt` + `tui-term` below for rapid Phase 1 prototyping).* |
| **`tokio`** | `1.44+` (`full` / `rt-multi-thread`, `sync`, `process`, `io-util`, `time`) | Asynchronous event loop, multi-PTY stream multiplexing & timers | Multiplexes N concurrent PTY reader threads/streams, `crossterm::event::EventStream`, `tmux -CC` control mode stdout streams, and cursor blink ticks (`tokio::select!`) into a single lock-free `mpsc` render loop without frame starvation. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **`vt100-ctt`** + **`tui-term`** | `0.17.1` / `0.3.4` | Drop-in `PseudoTerminal` Ratatui widget & lightweight VT100 screen parser | Use if you want immediate out-of-the-box `PseudoTerminal` widget rendering in Phase 1 before writing a custom `alacritty_terminal::grid::Grid` -> `ratatui::buffer::Buffer` adapter, or as a lightweight reference implementation. (`vt100-ctt` is the active 2025/2026 continuation of the unmaintained upstream `vt100 0.16.2` crate; `atuin-vt100 0.19.1` is also viable). |
| **`tui-textarea`** (`tui-textarea-2` / `edtui`) | `0.10.x` (for `ratatui 0.30`) | Multi-line command input widget for each pane's floating prompt | Provides multi-line text editing, cursor movement, yank/paste, undo/redo, and line-wrapping inside a bounded `ratatui::layout::Rect` so the floating prompt can dynamically resize vertically without owning the whole terminal screen. |
| **`tree-sitter`** + **`tree-sitter-bash`** | `0.24+` / `0.23+` | Incremental AST parsing & live shell syntax highlighting | Highlights commands, flags, pipes, strings, and syntax errors in real time inside each pane's floating command editor with sub-millisecond incremental re-parsing on every keystroke. |
| **`nucleo-matcher`** | `0.3.x` | Lock-free/high-speed fuzzy matching (from Helix editor) | Powers interactive command history search (`Ctrl+R`) inside the floating prompt and quick pane/window switcher overlays. |
| **`tmux_interface`** + **`nom`** | `0.4.0` (or `1.0`) / `8.0.0` | Tmux CLI command builder & `tmux -CC` control mode protocol parser | `tmux_interface` wraps standard `tmux` CLI invocations (`list-panes`, `split-window`, `send-keys`), while `nom` parses asynchronous `%output %pane_id`, `%layout-change`, and `%window-add` notifications emitted by `tmux -CC` control mode. |
| **`serde`** + **`toml`** | `1.0+` / `0.8+` | Configuration & layout serialization | Parses `~/.config/bilterm/config.toml` for IBM 3270 phosphor themes (green/amber/monochrome), cursor style defaults, keybindings, and saved composite pane layouts. |
| **`cassowary`** / `ratatui::layout` | Built-in (`0.30`) | Constraint-based H/V split tree & floating overlay geometry | Built into `ratatui::layout`. Use binary space partitioning (BSP tree of `Direction::Horizontal` / `Direction::Vertical` splits) for tiled panes, and explicit `Rect` intersection/clamping for stacked z-order panes and per-pane floating prompts. |
| **`color-eyre`** + **`tracing`** + **`tracing-appender`** | `0.6+` / `0.1+` / `0.2+` | Panic hooks (restoring raw mode) & non-blocking file logging | Critical for TUI development: `tracing` must write to a background file (`bilterm.log`) because `stdout`/`stderr` are owned by the alternate screen; `color-eyre` restores terminal state (`DisableMouseCapture`, leave raw mode) before printing crash backtraces. |
| **`insta`** | `1.42+` | Snapshot testing for `ratatui::backend::TestBackend` | Captures deterministic ASCII/ANSI snapshots of composite panes, floating prompt occlusion, z-order stacking, and IBM 3270 OIA status bars in CI without needing a real TTY. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| **`cargo-nextest`** | Fast parallel test runner | Essential for running PTY integration tests (`portable-pty` shell spawning + `TestBackend` snapshot assertions) in isolated processes. |
| **`tokio-console`** | Async task & channel inspector | Diagnoses backpressure or stalled PTY reader tasks when high-throughput commands (`cat hugefile`, `yes`) flood a pane. |
| **`vttest`** / **`esctest`** | VT100/ANSI escape sequence compliance suite | Run inside a Bilterm composite pane to verify cursor addressing, alternate screen switching (`DECSET 1049`), and color rendering. |

## Installation

# Cargo.toml (Rust 2024 Edition, MSRV 1.88+)

# Core TUI & Terminal Manipulation

# Cross-Platform PTY & Terminal Emulation State Machine

# Optional rapid-prototyping widget layer:

# Floating Command Prompt Editor, Syntax Highlighting & Fuzzy History

# Async Runtime & Channels

# Tmux Integration & Control Mode Parsing

# Configuration, Error Handling & Diagnostics

## Architecture & Rationale for Bilterm's Core Mechanics

### 1. Composite Pane Rendering & Floating Overlays (`ratatui 0.30`)

- **Why `ratatui` works best here:** Ratatui renders into an in-memory 2D grid of `Cell` structs (`ratatui::buffer::Buffer`) before diffing and flushing to `crossterm`.
- **Layering within a Composite Pane:**

### 2. IBM 3270 Mainframe Aesthetics & Cross-Hair Cursors

- Hardware terminal cursors only support a single coordinate and cannot draw a full-row + full-column cross-hair confined to an individual split pane.
- Using `ratatui::buffer::Buffer`, Bilterm can render a **software cross-hair cursor** per active pane by iterating over all cells in `pane_rect` sharing `cursor_x` (vertical hair) or `cursor_y` (horizontal hair) and mutating their `Style` (e.g., phosphor green underline/dim background tint `#003300` or amber `#331900`), while applying `Modifier::REVERSED` or custom block glyphs (`█`, `▯`, `_`, `▏`) at `(cursor_x, cursor_y)`.

### 3. Virtual Terminal Emulation (`alacritty_terminal` vs `vt100-ctt`)

- **`alacritty_terminal 0.26` (Recommended Primary):** Gives direct programmatic access to `TermMode` bitflags (`ALT_SCREEN`, `SHOW_CURSOR`, `APP_CURSOR`, mouse modes), damage tracking (only re-rendering changed cells), large scrollback history with line-wrapping reflow on resize, and custom `vte::Perform` hooks to intercept OSC 133 semantic prompt sequences (`\x1b]133;A\x07` through `D`) for **Command Block Separation** (jumping between prior commands in scrollback).
- **`vt100-ctt 0.17.1` + `tui-term 0.3.4` (Prototyping Alternative):** Extremely simple API (`parser.process(&bytes)`, `parser.screen().alternate_screen()`). Great for a Day-1 spike, but lacks `alacritty_terminal`'s text reflow on horizontal split resize and OSC 133 extensibility. Wrap the terminal emulator behind an internal `TerminalBackend` trait so either can be swapped cleanly.

### 4. Cross-Platform PTY + Async Event Loop (`portable-pty` + `tokio`)

- `portable-pty` readers (`MasterPty::try_clone_reader()`) use blocking OS reads (particularly on Windows ConPTY pipes and macOS kqueue/dev-ptmx).
- **Standard Pattern:** Spawn a dedicated `tokio::task::spawn_blocking` reader thread per composite pane that reads 4KB–64KB chunks from `portable-pty` and forwards them over a bounded `tokio::sync::mpsc::channel` to the state coordinator, ensuring bursts of PTY output (`find /`) never block UI input events or panic async runtimes on Windows/macOS.

### 5. Dual-Mode `tmux` Integration

- **Mode A — Running *inside* `tmux` (`$TMUX` detected):** Query and enable `allow-passthrough on` via `tmux_interface` where needed, forward outer resize/focus events, and default Bilterm's prefix key (e.g., `Ctrl+A` or `Alt+Space` or 3270 `Attn`/`PA1` mapped keys) so `Ctrl+B` passes cleanly to `tmux` (or vice versa).
- **Mode B — Controlling `tmux` as a backend (`tmux -CC` control mode):** Spawn `tmux -CC new-session` or `attach` via `tokio::process::Command`, parse line-oriented `%output %pane_id <octal-escaped-bytes>` and `%layout-change` events using `nom 8.0`, and pipe each `%pane_id` byte stream into its corresponding Bilterm `CompositePane` VT parser. This gives Bilterm instant `tmux` session persistence across SSH disconnects while retaining Bilterm's local floating command prompt per pane!

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| **`alacritty_terminal 0.26`** | **`vt100-ctt 0.17.1`** + **`tui-term 0.3.4`** | Use `vt100-ctt` + `tui-term` in Phase 1 if you want a 50-line working PTY canvas before implementing custom scrollback reflow and OSC 133 command-block tracking. |
| **`tui-textarea-2 0.10`** | **`edtui`** (`0.9+`) | Use `edtui` if users specifically want modal Vim-first keybindings inside the floating command prompt out of the box rather than Emacs/Readline-style shell bindings. |
| **`tree-sitter 0.24`** | **`syntect 5.2+`** | Use `syntect` if you want TextMate grammar highlighting for dozens of languages inside scrollback previews; `tree-sitter-bash` is faster and more accurate for live shell command input. |
| **`portable-pty 0.9.0`** | **`pty-process 0.5`** / **`rustix-openpty`** | Only if Windows support were dropped permanently. Since Windows (`ConPTY`) is a stated Phase 3 target, `portable-pty` is mandatory. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **`reedline`** or **`rustyline`** | They assume exclusive ownership of the host terminal's raw mode, cursor position, and `stdout` stream. Calling `reedline.read_line()` inside a `ratatui` draw loop corrupts the screen buffer and blocks multi-pane rendering. | **`tui-textarea` (`tui-textarea-2`)** rendered into a `ratatui::layout::Rect` buffer widget, paired with **`tree-sitter-bash`** and **`nucleo-matcher`**. |
| **Unmaintained `tui-rs`** | Archived in 2023; lacks `Buffer` compositing ergonomics, `Clear` widget fixes, and modern `crossterm` support. | **`ratatui 0.30.2`** |
| **Unmaintained upstream `vt100` (`0.16.2`)** | Upstream lags on modern CSI/OSC sequences and `ratatui 0.30` trait bounds, causing panics on malformed UTF-8 or modern TUI sequences inside sub-panes. | **`alacritty_terminal 0.26.0`** (or **`vt100-ctt 0.17.1`** if using `tui-term`). |
| **`termion` or `termwiz` as host backend** | `termion` is Unix-only (breaks Windows target). `termwiz` has weaker integration with third-party `ratatui` widget ecosystems than `crossterm`. | **`crossterm 0.29.0`** |
| **Direct async `tokio::fs::File` wraps on PTY master FDs** | Fails or deadlocks on macOS (`kqueue` does not poll `/dev/ptmx` identically to Linux `epoll`) and Windows (`ConPTY` uses anonymous/named pipes). | **`portable-pty 0.9.0`** wrapped in dedicated `tokio::task::spawn_blocking` reader loops feeding a bounded `tokio::sync::mpsc` channel. |

## Stack Patterns by Variant

- Spawn local shells (`$SHELL` / `pwsh.exe`) via `portable-pty` per `CompositePane`.
- When user submits a command in the Floating Command Prompt, write `command + "\n"` to that pane's `MasterPty` writer; when the pane enters `ALT_SCREEN` (DECSET 1049) or Raw Passthrough mode, forward keystrokes directly to `MasterPty` and auto-hide the floating prompt.
- Replace local `portable-pty` spawning with a single `tokio::process::Command::new("tmux").arg("-CC")` bridge.
- Route floating command prompt submissions via `tmux send-keys -t %pane_id` and feed `%output %pane_id` streams into the pane's `alacritty_terminal::Term` state machine.
- Emit `\x1bPtmux;\x1b...\x1b\\` passthrough sequences when necessary, enable `focus-events` and SGR mouse passthrough, and expose a status indicator in the IBM 3270 Operator Information Area (OIA) bottom bar showing outer `tmux` coexistence state.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `ratatui@0.30.2` | `crossterm@0.29.0` | `ratatui 0.30` re-exports compatible backend traits for `crossterm 0.29`. Pin both to avoid duplicate `crossterm` versions in `Cargo.lock`. |
| `tui-term@0.3.4` | `ratatui@0.30.x`, `vt100-ctt@0.17.1` | Requires Rust `1.86.0+` MSRV. |
| `tui-textarea-2@0.10.x` | `ratatui@0.30.x` | Use `tui-textarea-2` package alias in `Cargo.toml` if upstream `tui-textarea` has not bumped its `ratatui-core 0.30` peer dependency. |
| `alacritty_terminal@0.26.0` | `vte@0.15.0` | Always use the exact `vte` version re-exported by `alacritty_terminal::vte` to avoid trait version mismatches on `vte::Perform`. |

## Sources

- `crates.io/crates/ratatui` — Verified `0.30.2` modular release and `crossterm 0.29.0` pairing (HIGH confidence)
- `crates.io/crates/portable-pty` — Verified `0.9.0` WezTerm cross-platform PTY abstraction for Linux/macOS/Windows ConPTY (HIGH confidence)
- `crates.io/crates/alacritty_terminal` — Verified `0.26.0` release (April 2026) and `TermMode::ALT_SCREEN` / `Grid` capabilities (HIGH confidence)
- `crates.io/crates/tui-term` & `vt100-ctt` — Verified `tui-term 0.3.4` (MSRV 1.86) and `vt100-ctt 0.17.1` Ratatui widget ecosystem compatibility (HIGH confidence)
- `crates.io/crates/tui-textarea-2` & `tmux_interface` — Verified `0.10.x` and `0.4.0` versions for multi-line Ratatui input and tmux CLI control (HIGH confidence)

<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->

## Conventions

> **Full Engineering & Agent Reference:** Always consult [`docs/DEVELOPER_AND_AGENT_GUIDE.md`](docs/DEVELOPER_AND_AGENT_GUIDE.md), [`docs/LESSONS_LEARNED_COMPOUNDED.md`](docs/LESSONS_LEARNED_COMPOUNDED.md), and [`docs/AGENT_AUTONOMY_AND_PERMISSIONS.md`](docs/AGENT_AUTONOMY_AND_PERMISSIONS.md) before modifying state, keybindings, commands, security boundaries, or agent execution permissions.

1. **Module & Function Size Ceilings (`ARCH-R04`, `ARCH-R01`)**:
   - Every Rust source file under `src/**/*.rs` MUST remain **`<= 800` lines**. Split cohesive submodules (`foo/mod.rs` + children) rather than growing monolithic files.
   - `src/input/mod.rs::handle_wm_key_event` MUST remain **`<= 80` lines**; all keyboard shortcuts live in declarative `KeyBinding` tables in `src/input/keymap.rs`.
2. **Unified `Action` Dispatch (`ARCH-R01`)**:
   - Every user-triggerable operation (leader chord, global/pane shortcut, `:colon` command, Fuzzy Command Palette item, top menu bar action, JSON-RPC `"wm.command"`/`"command.execute"`, MCP tool, or TOML Playbook `colon` step) MUST lower to `crate::action::Action` (`pub type ColonCommand = crate::action::Action;`) and execute through `WindowManager::apply(&mut self, action: Action) -> ActionResult`.
   - Every new `Action` variant MUST be covered in `src/action.rs::tests::every_action_is_reachable_from_keymap_colon_or_rpc`.
3. **Encapsulated State Invariants (`ARCH-R02`)**:
   - Never make `WindowManager` topology fields (`panes`, `layout`, `z_order`, `focused_pane_id`, `zoomed_pane_id`, `modal`, `workspace_recorder`) or `CompositePane` internal fields (`prompt`, `vt_worker`, `line_discipline`, `mode`, `focus`) `pub`.
   - Pane removal MUST go exclusively through `WindowManager::remove_pane(id)` (`debug_assert!` verifies post-conditions across `panes`, `layout`, `z_order`, `focused_pane_id`, and `zoomed_pane_id`).
   - Command submission MUST go exclusively through `CompositePane::submit_command(&mut self) -> Result<Option<SubmittedCommand>>`.
   - Overlays are mutually exclusive via `Modal` (`None`, `Help`, `CommandPalette`, `Inspector`, `TopMenu`); intra-pane focus is mutually exclusive via `Focus` (`FloatingPrompt`, `RawPassthrough`, `HistoryPopup`, `CopyMode`).
4. **Filesystem & Credential Confinement (`SEC-R01..04`)**:
   - All runtime state, session TOMLs, history exports, `.cast` recordings,Unix RPC sockets, and snapshots MUST use `src/util/secure_fs.rs` (`ensure_private_dir` `0700`, `write_private_file_atomic` `0600`, `validate_safe_name` rejecting `..`, `/`, `\`, NUL).
   - Never pass secrets on CLI argv; use `--token-file <path>` (`0600`).
5. **Context Window & Token-Efficiency Discipline (`PERF-CTX-01..03`)**:
   - **Compact Between Tasks**: Recommend or run `/compact` after updating `.planning/CHECKPOINT.md` when switching between major tasks so tool turns do not replay large historical contexts.
   - **Quiet Cargo Flags (`-q` / `--quiet`)**: Always pass `-q` (`cargo build -q`, `cargo clippy -q --all-targets -- -D warnings`, `cargo test -q`) to suppress routine `Compiling ...` and `test ... ok` lines (~90% token reduction).
   - **Delegate Build/Test Loops to `flash` Subagent**: Run multi-step `cargo build -q` / `cargo clippy -q` / `cargo test -q` verification loops inside an isolated `flash` subagent (`invoke_subagent` with `Model: "flash"`) so compiler/test logs never bloat the main conversation history.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->

## Architecture

Bilterm is organized as a cycle-free 48-file modular DAG under `src/` (`vt` / `config` / `theme` / `tmux` / `util` → `pane` → `wm` → `action` / `colon_command` / `input` → `api` / `app` → `lib` / `main`):

- **`src/action.rs` & `src/colon_command/{mod,parse,execute,complete}.rs`**: Canonical `Action` enum (`60+` variants), `:colon` parser, `WindowManager::apply(Action)` executor (`execute_colon_command`), and `<Tab>` completion.
- **`src/input/{mod,keymap,modal,mouse,normalize}.rs`**: Declarative keymap tables (`DEFAULT_LEADER_BINDINGS`, `DEFAULT_GLOBAL_BINDINGS`, `DEFAULT_PANE_BINDINGS`), dual `Ctrl+B`/`Ctrl+X` leader with `$TMUX` fallback and held-`Ctrl` normalization, modal key handlers, SGR mouse routing, and macOS `Option`-as-`Alt` normalization.
- **`src/pane/{mod,prompt,prompt_editor,render,overlays,discipline,completion,selection,hints}.rs`**: `CompositePane` (combining `OutputCanvas`, `FloatingCommandPrompt`, and 3270 `OiaStatusBar`), 3-signal `!ECHO LOCK` + `asciinema` `force_command_prompt` override, `DECSET 1049` alternate screen auto-hide, Vi copy mode, and QuickSelect field hints.
- **`src/wm/{mod,layout_tree,render,shaders,palette,session,fleet}.rs`**: Multi-pane BSP `SplitNode` tiling, Z-order `z_order: Vec<PaneId>` cascade, Guake drop-down pane, 3-pass Ratatui compositor + CRT/cross-hair/spotlight/focal-zoom shaders, and `WorkspaceRecorder` whole-workspace `.cast` recording.
- **`src/vt/{mod,worker,snapshot,parser,graphics,recording}.rs`**: Dedicated `portable-pty` reader + `alacritty_terminal 0.26` worker thread publishing immutable `Arc<GridSnapshot>` frames via `tokio::sync::watch` (60 FPS flood immunity), Kitty/Sixel graphics, and Asciicast v2 `.cast` recorders (`SessionRecorder` & `WorkspaceRecorder`).
- **`src/api/{mod,rpc,mcp,playbook,gui_renderer}.rs`**: JSON-RPC 2.0 server/client, native MCP `2024-11-05` stdio server, TOML automation playbooks, and headless `.ppm` renderer.

See [`docs/DEVELOPER_AND_AGENT_GUIDE.md`](docs/DEVELOPER_AND_AGENT_GUIDE.md) and [`docs/architecture.md`](docs/architecture.md) for full details, Mermaid diagrams, and live debugging workflows (`<F9>` 3270 Inspector, `RUST_LOG=bilterm=debug` -> `bilterm.log`, `bilterm ctl`).
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->

## Project Skills & Superpowers Integration

Bilterm uses **OpenGSD** (`.planning/`) for milestone/phase orchestration paired with **Superpowers** (`~/.gemini/config/plugins/superpowers/skills/`) and **SWE-Skills** (`~/.gemini/config/plugins/swe-skills/skills/`) for execution discipline:

| Workflow Stage | Primary Skill | Purpose |
|----------------|---------------|---------|
| **Ideation & Design** | `brainstorming` + `/gsd-discuss-phase` | Explore user intent, visual layout trade-offs, and ADRs before writing code. |
| **Planning** | `writing-plans` + `/gsd-plan-phase` | Decompose phases into bite-sized, contract-driven tasks with explicit verification commands. |
| **Implementation** | `test-driven-development` + `subagent-driven-development` | Write failing unit/integration/snapshot tests (`insta` + `TestBackend`) first, then implement minimal passing Rust code. |
| **Debugging** | `systematic-debugging` + `/gsd-debug` | Isolate PTY/VT100/termios/layout bugs with deterministic reproduction tests before proposing fixes. |
| **Quality & Completion** | `verification-before-completion` + `requesting-code-review` | Run `cargo test` and `cargo clippy --all-targets -- -D warnings` with fresh evidence before any commit or release claim. |
| **Context Compaction** | `compacting-context-safely` | Update `.planning/CHECKPOINT.md` and `.planning/STATE.md` with verified working-tree state before resetting context or ending a session. |
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->

## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:

- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->

## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
