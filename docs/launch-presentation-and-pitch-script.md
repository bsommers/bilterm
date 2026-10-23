# Bilterm NotebookLM / Gemini Ingest Script: Technical Slide Deck & 5-Minute Launch Video

**Target System:** Google Gemini / NotebookLM Source Document & Prompt Directive  
**Target Deliverables:**
1. **Technical-Focus Slide Deck Specification** (12 Slides, Architectural Anatomy & Performance Proofs)
2. **5-Minute Open-Source Product Launch Video Script** (Timestamped, Direct, Technical Voiceover & Visual Cues)

---

# 1. Core Technical Value Points (Bilterm v1.5.0 Reference Spec)

- **Language & Runtime:** Rust 1.88+ (Edition 2024), modular `ratatui 0.30.2`, `crossterm 0.29.0`, `portable-pty 0.9.0`, `alacritty_terminal 0.26.0`, `tokio 1.44`.
- **Quality & Verification:** 91 passing tests across unit, integration, and torture suites; 83.45% line coverage; ANSI compliance validated via `vttest` and `esctest`.
- **License:** Apache License, Version 2.0 (`Apache-2.0`).

### Architectural Distinctives:
1. **Composite Terminal Pane Pattern:**
   - Command prompt, underlying PTY output canvas, and IBM 3270 Operator Information Area (OIA) status bar form a single coordinated cell buffer.
   - Non-overlapping split mode (`:prompt split-top` / `:prompt split-bottom` or `Alt+S`) eliminates fake resize signals (`SIGWINCH`), preventing shell history redrawing glitches.
   - Floating prompt mode (`Alt+T` / `Alt+B` / drag) auto-relocates and avoids boundary clipping.
2. **Decoupled Concurrency & Wire-Speed Streaming:**
   - Dedicated OS thread per PTY hosting an `alacritty_terminal` state engine.
   - Worker transmits immutable `GridSnapshot` records over bounded channels to Ratatui's immediate-mode compositor (`Buffer`).
   - Flood immunity: Ingests >120 MB/s (`tail -f /dev/random`) while UI renders at 60 FPS under <0.85ms per frame overhead without keyboard latency.
3. **Adaptive Line-Discipline Seams:**
   - Intercepts `DECSET 1049` (Alternate Screen) to automatically collapse the floating prompt for full-screen applications (`vim`, `htop`, `less`).
   - 3-signal password interception detects dropped `ECHO` flags, terminal queries, and PAM password prompts, locking prompt history and signaling `!ECHO LOCK` on the 3270 OIA bar.
   - Raw passthrough mode (`F12` / `:raw`) routes bytes directly to the PTY master.
4. **Windowing & Retro Aesthetics:**
   - Dual topology: Binary Space Partitioned (BSP) tiling (`F2`/`Alt+H`, `Alt+V`, `F5`) + Borland Turbo Vision Z-order cascading window decks (`Alt+M`, `F6`, `Alt+1..9`) + Guake scratchpad dropdown (`F10`).
   - 5 retro palettes (`turbo-blue`, `turbo-cyan`, `3278` phosphor green, `3279` amber, `mono`), CP437 double borders (`╔═╗`), character-cell drop shadows, CRT scanlines (`:crt`), and IBM 3270 cross-hairs (`Alt+X`).
   - Math shaders: In-TUI Focal Spotlight (`Alt+O`) and Focal Zoom Lens (`Alt+Shift+O`) with Hermite `smoothstep` falloff; passthrough for Kitty graphics (`APC G`) and Sixel (`DCS q`).
5. **Infrastructure & Agent Automation:**
   - Direct remote SSH composite panes (`:ssh`) and `tmux -CC` control mode bridge (`:ssh-tmux`) preventing nested multiplexer status bar inception.
   - Federated `herdr`-style fleet fan-out across `@tag` groups (`:remote exec @tag <cmd>`) with multi-pane broadcast and read-only locks.
   - Native Model Context Protocol (MCP 2024-11-05) stdio server (`bilterm --mcp`) exposing 8 tools for AI agents.
   - Headless JSON-RPC 2.0 daemon (`bilterm --headless --listen <addr>`) and declarative TOML playbooks (`bilterm --playbook <file.toml>`) with regex output assertions.

---

# 2. Technical Slide Deck Specification

## Slide 1: Title & Paradigm Shift
- **Header:** Bilterm: The Composite Terminal Pane Multiplexer
- **Subheader:** Fusing IBM 3270 Mainframe Rigor with Borland Turbo Vision Ergonomics in Rust
- **Content:**
  - **The Problem:** Conventional terminal multiplexers treat the command line as a slave to the bottom of the screen; PTY floods freeze UI loops; TUI programs (`vim`) collide with overlays.
  - **The Bilterm Seam:** Floating Command Prompt + Full PTY Canvas + 3270 OIA Bar as a single composite unit. Zero `SIGWINCH` geometry thrashing.
- **Footer:** Rust 2024 | Ratatui 0.30 | 83.45% Coverage | Apache-2.0

## Slide 2: Architectural Anatomy (The 3-Pass Compositor)
- **Header:** The Composite Pane & 3-Pass Render Pipeline
- **Diagram:**
  ```text
  [ Pass 1: OutputCanvas (PTY GridSnapshot) ]
                 ↓
  [ Pass 2: widgets::Clear + FloatingCommandPrompt (Top / Bottom / Split) ]
                 ↓
  [ Pass 3: Post-Processing (3270 Cross-hair, CRT Scanlines, Focal Shaders, OIA Bar) ]
  ```
- **Technical Specs:**
  - Immediate-mode cell buffer mutation via `ratatui::buffer::Buffer`.
  - Non-Overlapping Split Mode (`Alt+S`) mathematically partitions the pane rect; no fake resize signals sent to underlying shell.
  - Floating Mode (`Alt+T` / `Alt+B`): Rendered on top with custom border shadows, auto-relocating on boundary collision.

## Slide 3: Terminal Emulation & Concurrency Seams
- **Header:** Decoupled Concurrency: Zero-Lock PTY Streaming
- **Technical Specs:**
  - Dedicated OS worker thread running `alacritty_terminal` state machine.
  - Immutable `GridSnapshot` transfer over bounded crossbeam/tokio channels.
  - Flood Immunity: Running `cat /dev/urandom` runs at wire speed (>120 MB/s) while Ratatui compositor renders at a locked 60 FPS with zero input lag.
  - ANSI Compliance: Verified against `vttest` and `esctest` test suites for cursor addressing, bracketed paste, SGR mouse tracking, and UTF-8 decodes.

## Slide 4: State Machine & Line Discipline Safety
- **Header:** Adaptive Line Discipline: Alternate Screen & !ECHO LOCK
- **Technical Specs:**
  - `DECSET 1049` Interception: Detects entry into alternate screen buffer (`vim`, `htop`, `less`) and automatically collapses floating prompt to grant 100% PTY canvas visibility.
  - 3-Signal Password Seam: Intercepts `termios` `ECHO` flag drops, terminal query sequences, and PAM password prompts.
  - OIA Security Lockout: Displays `!ECHO LOCK` on the 3270 status bar, isolates key inputs directly to PTY master, and clears prompt buffers to prevent credential leakage.

## Slide 5: Multi-Pane & Windowing Topology
- **Header:** Dual Topology: BSP Tiling + Z-Order Cascade Deck
- **Comparison Matrix:**
  - **Binary Space Partitioning (BSP):** `F2` / `Alt+H` (horizontal split), `Alt+V` (vertical split), `F5` (full-pane zoom). Dynamic ratio rebalancing.
  - **Borland Turbo Vision Z-Deck:** `Alt+M` (toggle tile/stack), `F6` (shuffle deck), `Alt+1..9` (direct index). Character-cell drop shadows and active-frame highlight.
  - **Guake Dropdown:** `F10` persistent scratchpad overlay slides from top of screen without interrupting active workspace.

## Slide 6: Visual Pipeline & Shaders
- **Header:** Retro Phosphor CRT & Focal Post-Processing
- **Features:**
  - 5 Palettes: `turbo-blue` (CGA 0x0000AA), `turbo-cyan`, `3278` (P1 phosphor green `#00FF66`), `3279` (amber `#FFB000`), `mono`.
  - 3270 Cross-Hairs (`Alt+X`): Renders full-row and full-column hardware cross-hair overlays across active pane or entire workspace.
  - Mathematical Focal Shaders:
    - `:spotlight` (`Alt+O`): Hermite polynomial `smoothstep` radial falloff focusing on active cursor.
    - `:focalzoom` (`Alt+Shift+O`): In-TUI software magnifying glass (1.25x–3.0x zoom) for high-density log analysis.
  - Sixel & Kitty Graphics: Inline rendering (`APC G` / `DCS q`) passed through cell coordinate mapper.

## Slide 7: Remote Infrastructure: SSH & tmux -CC Bridge
- **Header:** First-Class Remote Fleets & Control Mode Multiplexing
- **Technical Specs:**
  - Direct Remote Panes: `:ssh user@host [port]` allocates local composite pane bound to encrypted SSH channel.
  - `tmux -CC` Control Mode Driver: Attaches to remote tmux sessions without nested status bar inception. Translates tmux `%output %pane_id` streams into native Bilterm composite panes.
  - Session Persistence: Bilterm handles rendering and floating prompts locally; remote tmux retains session state across disconnects.

## Slide 8: `herdr`-Style Fleet Orchestration
- **Header:** Multi-Node Tagged Fleet Fan-Out
- **Technical Specs:**
  - Node Registry: In-memory and config-persisted node catalog (`:remote add <name> <host> [@tags]`).
  - Broadcast Engine: Fan-out execution engine (`:remote exec <@tag|all> <command>`) streams stdout/stderr back into multiplexed composite panes simultaneously.
  - Operator Safety: Real-time `:lock on` prevents accidental keystrokes to critical production nodes; `:broadcast on` mirrors live interactive typing.

## Slide 9: The Agent Control Plane (MCP & JSON-RPC)
- **Header:** Built for AI Agents: Native MCP 2024-11-05 & JSON-RPC 2.0
- **Technical Specs:**
  - Stdio MCP Server (`bilterm --mcp`): Exposes 8 structured agent tools (`workspace_status`, `pane_exec`, `pane_read`, `pane_split`, `pane_close`, `fleet_exec`, `layout_set`, `prompt_inject`).
  - Headless Automation Engine (`bilterm --headless --listen <addr>`): Fully headless Unix/TCP daemon for external CI/CD pipelines.
  - `bilterm ctl`: CLI client for querying status, sending input, and scraping screen matrices without attached display.

## Slide 10: Declarative Robot Playbooks
- **Header:** Deterministic TUI Testing via TOML Playbooks
- **Example Snippet:**
  ```toml
  [[step]]
  action = "split"
  direction = "horizontal"

  [[step]]
  action = "exec"
  pane = 1
  command = "cargo test"

  [[step]]
  action = "wait_for_output"
  pane = 1
  regex = "test result: ok"
  timeout_ms = 5000
  ```
- **Technical Specs:**
  - Enables headless verification of interactive CLI tools.
  - Netpbm framebuffer exporter (`bilterm --gui-dump frame.ppm`) captures pixel/cell snapshots for regression testing.

## Slide 11: Performance & Test Benchmarks
- **Metrics Table:**
  - Idle CPU Usage: < 0.1% (event-driven `tokio::select!` loop; sleeps on empty channels)
  - Frame Render Time: < 0.85ms on 200x60 grid with 8 active composite splits
  - PTY Throughput: > 120 MB/s ingestion without UI stuttering
  - Line Coverage: 83.45% verified via cargo-tarpaulin across 91 integration & unit tests
  - Memory Footprint: ~14MB base RSS

## Slide 12: Getting Started & Open Source Ecosystem
- **Header:** Open Source Apache-2.0: Install & Contribute
- **Specs & Links:**
  - Install: `cargo install bilterm` or `nix run github:bill/bilterm`
  - GitHub: `https://github.com/bill/bilterm`
  - Complete 7-chapter user guide, `roff(7)` manpage (`bilterm --man`), and full shell completions (`--completions bash|zsh|fish`).

---

# 3. 5-Minute Open-Source Product Launch Video Script

- **Target Duration:** 5:00 (300 seconds)
- **Delivery Rate:** ~140 words/minute (approx. 700 words total spoken dialogue)
- **Tone:** Technical, direct, authoritative, zero conversational filler

---

### [0:00 - 0:40] PART 1: THE TERMINAL CONUNDRUM & THE BILTERM HYPOTHESIS
**[VISUAL]**: High-contrast screen capture of a conventional terminal multiplexer running a fast-scrolling log (`tail -f /dev/urandom | base64`) while typing at the bottom. As the log scrolls, the prompt jumps and text gets lost. Cut to Bilterm launching in `turbo-blue` palette with Borland double-line box-drawing frames and an IBM 3270 status bar.  
**[AUDIO - VO]**:  
"For forty years, terminal emulators have forced a broken compromise: your command prompt is pinned to the very bottom of the screen, drowned out by scrolling stdout, or occluded when you need to inspect previous command output.

Enter Bilterm.

Bilterm is an open-source terminal multiplexer, shell wrapper, and agent execution engine built from the ground up in Rust using Ratatui 0.30 and modern VT-state engines. It resolves this tension through a singular architectural concept: the Composite Terminal Pane.

Bilterm treats your command prompt, your scrolling PTY output canvas, and your 3270 Operator Information Area not as disconnected windows, but as a single, coordinated cell-buffer composite."

---

### [0:40 - 1:30] PART 2: THE COMPOSITE PANE & ZERO-THRASH GEOMETRY
**[VISUAL]**: Screen recording demonstrates:
1. Typing in top-pinned prompt (`Alt+T`). Output streams cleanly underneath without shifting the prompt.
2. User hits `Alt+S` (Split Prompt mode). Canvas split occurs instantly.
3. User runs `top` or `vim`. The prompt smoothly vanishes as alternate screen activates (`DECSET 1049`), then reappears instantly on exit.
4. User runs `sudo ls`. The 3270 OIA bar flashes `!ECHO LOCK`, demonstrating password shielding.  
**[AUDIO - VO]**:  
"Watch what happens when we execute long-running commands. In Bilterm, you can pin your command prompt to the top with Alt+T, the bottom with Alt+B, or float it anywhere with a mouse drag.

Need guaranteed separation? Alt+S activates Non-Overlapping Split mode. Bilterm partitions the pane geometry internally—zero SIGWINCH thrashing, zero cursor dislocation, and zero broken shell redraws.

Under the hood, Bilterm monitors terminal line disciplines. Launch Vim, htop, or an interactive curses app, and Bilterm intercepts DECSET 1049, automatically autohiding the floating prompt to give the application exclusive control.

Run sudo or SSH, and Bilterm catches termios echo drops, instantly engaging the 3270 `!ECHO LOCK` state. Your keystrokes bypass prompt history buffers and pass cleanly to the PTY master."

---

### [1:30 - 2:20] PART 3: CONCURRENCY, FLOOD IMMUNITY & THE VT SEAM
**[VISUAL]**: Screen splits into a 4-pane grid. In Pane 1: `cat /dev/urandom | base64` streams at wire speed. In Pane 2: user types code with responsive sub-millisecond input feedback. In Pane 3: `htop` displays minimal CPU consumption. Architecture diagram overlays the screen showing decoupled threads.  
**[AUDIO - VO]**:  
"Most TUI multiplexers collapse when flooded with raw PTY output because their event loop blocks on terminal writes. Bilterm implements a decoupled concurrency seam.

Each composite pane runs its own dedicated OS thread hosting an alacritty_terminal parser. The worker ingests raw byte streams, maintains the scrollback grid, and transmits immutable GridSnapshots over bounded channels to Ratatui's immediate-mode compositor.

Running `yes` or dumping megabytes of logs cannot starve your keyboard input or stutter the render loop. Bilterm maintains a rock-solid 60 frames per second at under one millisecond per frame compositing overhead."

---

### [2:20 - 3:15] PART 4: MAINFRAME RIGOR & BORLAND TURBO VISION AESTHETICS
**[VISUAL]**: Tour of visual capabilities:
1. `Alt+M` switches from BSP tiled splits to Borland Z-order cascading window decks.
2. `F6` shuffles windows with character-cell drop shadows.
3. `Alt+X` toggles full-screen IBM 3270 cross-hairs tracking the active cursor.
4. `:theme 3278` shifts display to vivid phosphor green CRT with scanlines (`:crt`).
5. `Alt+O` fires the Focal Spotlight shader, dimming background panes with smooth Hermite interpolation.
6. `F10` triggers the Guake dropdown terminal from the top bezel.  
**[AUDIO - VO]**:  
"Bilterm’s interface pays homage to two legendary eras of computing: IBM 3270 mainframe terminals and Borland Turbo Vision.

You get double-line CP437 box-drawing borders, drop shadows, and full Z-order window stacking with Alt+M and F6. Toggle between binary space partitioned splits or shuffle overlapping panes in a true window deck.

Press Alt+X to toggle IBM 3270 cross-hairs—hardware-style row and column guide lines that follow your cursor across the grid.

And Bilterm features in-TUI mathematical shaders: Alt+O casts a Focal Spotlight with Hermite smoothstep falloff onto your active workspace, while Alt+Shift+O activates a software magnifying lens to inspect dense logs. Five historic palettes—including IBM 3278 Phosphor Green and 3279 Amber—are rendered with zero external asset dependencies."

---

### [3:15 - 4:10] PART 5: FLEET ORCHESTRATION & SSH / TMUX-CC BRIDGES
**[VISUAL]**: Terminal demonstration:
1. Command `:ssh prod-db-01` spawning a remote pane.
2. Command `:ssh-tmux web-edge app-session` attaching via control mode.
3. Command `:remote exec @workers uptime` broadcasting to three servers simultaneously, returning aligned outputs in separate composite panes.  
**[AUDIO - VO]**:  
"Bilterm is engineered for production infrastructure.

It bridges directly into tmux using `tmux -CC` control mode over local or SSH pipes. Instead of running a multiplexer inside a multiplexer with ugly nested status bars, Bilterm ingests tmux percent-output notifications and maps each remote pane directly into Bilterm’s native composite panes. Your session stays alive on the remote server; your interface stays fast and local.

With herdr-style fleet orchestration, tag remote servers using `:remote add`, and fan out commands with `:remote exec @tag`. Manage dozens of nodes across your cluster with synchronized execution, read-only locks, and multi-pane broadcast."

---

### [4:10 - 5:00] PART 6: THE AGENT CONTROL PLANE & CALL TO ACTION
**[VISUAL]**: Split screen:
Left: Terminal running `bilterm --mcp` showing live JSON-RPC traffic.
Right: An autonomous LLM agent using Bilterm tools to split panes, inspect output, run tests, and report results.
Final screen displays the Bilterm logo, GitHub repository URL, and installation one-liner: `cargo install bilterm`.  
**[AUDIO - VO]**:  
"Finally, Bilterm was built for the next decade of automated software engineering.

Run `bilterm --mcp` to expose a native Model Context Protocol server directly to AI coding agents. Agents get eight fine-grained tools to split panes, execute shell commands, read terminal grids, and orchestrate tests.

Prefer deterministic automation? Write declarative TOML playbooks with regex assertions and verify TUI workflows headlessly in CI.

Everything you’ve seen is open source, licensed under Apache-2.0, with 91 tests passing at over 83 percent coverage.

Stop fighting your terminal prompt. Take control of your canvas.

`cargo install bilterm` or clone the repository on GitHub. Welcome to Bilterm."
