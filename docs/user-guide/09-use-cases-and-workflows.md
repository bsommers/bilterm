# Chapter 9: Real-World Use Cases, Agent Workflows & Starter Packs

[← Chapter 8: Extended CLI & GUI Examples](08-extended-cli-and-gui-examples.md) | [User Guide Home](README.md) | [Chapter 10: Security & Best Practices →](10-security-and-best-practices.md)

Bilterm combines **IBM 3270 block-mode command isolation**, **Borland Turbo Vision tiling + Z-stacking**, **per-pane typography scaling**, and a **JSON-RPC 2.0 + MCP automation control plane** into a single binary. Below are six production workflows showing how these capabilities fit together.

![4-Pane Multi-Agent Coding Swarm](../assets/screenshot-multi-agent-swarm.svg)

---

## Table of Contents

1. [Multi-Agent Systems & Autonomous Coding Swarms](#1-multi-agent-systems--autonomous-coding-swarms)
2. [Remote Agents & Distributed SSH / `tmux -CC` Fleet Operations](#2-remote-agents--distributed-ssh--tmux--cc-fleet-operations)
3. [Robot Scripting, Deterministic QA & CI/CD Testing](#3-robot-scripting-deterministic-qa--cicd-testing)
4. [Regular Terminal Operations & Power-User Shell Ergonomics](#4-regular-terminal-operations--power-user-shell-ergonomics)
5. [Developer & Live Terminal Debugging Workflows](#5-developer--live-terminal-debugging-workflows)
6. [Custom Layouts, Incident War Rooms & Starter Packs](#6-custom-layouts-incident-war-rooms--starter-packs)

---

## 1. Multi-Agent Systems & Autonomous Coding Swarms

Modern AI engineering often runs multiple specialized agents in parallel (e.g., an **Architect/Planner**, an **Implementer**, a **Test/Verification Runner**, and a **Security/Telemetry Auditor**).

### Why Bilterm Excels for Multi-Agent Swarms
- **Native MCP (`--mcp`) & JSON-RPC (`--headless`) Control**: An orchestrator agent can spawn panes (`wm.command` `:vsplit`/`:split`), dispatch shell commands (`pane.exec`), read structured OSC 133 command blocks (`pane.read_screen`), and wait for completion events without fragile screen-scraping.
- **Human-in-the-Loop Intervention without `SIGWINCH` Disruption**: Each pane has its own floating 3270 command prompt. A human operator can inspect scrollback or stage a command in the floating prompt while an agent streams build output underneath—without sending `SIGWINCH` or corrupting the agent's stdout stream.
- **Per-Pane Font Scaling**: Zoom the primary Architect/Diff pane to `120%` (`Ctrl++`) while shrinking high-density compiler/test log panes to `80%` (`Ctrl+-`) so 4+ agent panes fit comfortably on a single monitor.

### Quick Start (`starter-packs/01-multi-agent-swarm/`)
```bash
# Launch the 4-pane Cyberpunk Neon Multi-Agent Swarm GUI
bilterm --gui -c starter-packs/01-multi-agent-swarm/config.toml --gui-font "JetBrains Mono" --gui-font-size 15

# Or run the automated swarm verification playbook headlessly
bilterm run starter-packs/01-multi-agent-swarm/playbook.toml
```

---

## 2. Remote Agents & Distributed SSH / `tmux -CC` Fleet Operations

When managing a fleet of cloud instances, Kubernetes nodes, or remote build servers over SSH:

- **Persistent `tmux -CC` Control Mode (`:tmux attach <session>`)**: Attach a remote or local `tmux` session in control mode so remote panes survive laptop sleep or VPN drops while rendering inside Bilterm's local Borland/3270 frames with local floating prompts.
- **Tagged Fleet Registry (`:remote ssh user@host:2222 prod,db`)**: Register SSH endpoints with tags (`@prod`, `@db`, `@canary`) and view live connection health in the slide-out Fleet Drawer (`F7` / `Alt+D` / `:drawer open`).
- **Tag-Filtered & Synchronized Execution (`:remote exec @prod <cmd>` & `:broadcast`)**: Run health checks across all `@prod` nodes or toggle `:broadcast on` to type commands into all unlocked panes simultaneously (while automatically skipping `:lock on` read-only panes and `!ECHO` password prompts).

### Quick Start (`starter-packs/02-remote-dev-and-fleet/`)
```bash
bilterm --gui -c starter-packs/02-remote-dev-and-fleet/config.toml
```

---

## 3. Robot Scripting, Deterministic QA & CI/CD Testing

Testing interactive CLI tools, installers, and TUI applications in CI usually suffers from flaky `sleep` timers and missing display servers.

- **Declarative TOML Playbooks (`bilterm run <playbook.toml>`)**: Script multi-step terminal workflows (`colon`, `exec`, `send_keys`, `feed_output`, `assert_screen_contains`, `assert_trigger_fired`, `assert_exit_code`, `export_ppm`) that execute deterministically in milliseconds without an X11/Wayland display.
- **Visual Regression Framebuffers (`--gui-dump` / `export_ppm`)**: Export pixel-exact Netpbm `.ppm` or vector `.svg` snapshots of any workspace state for CI artifact inspection.
- **Whole-Workspace Asciicast v2 Recording (`:record workspace start <file.cast>`)**: Record either a single pane's PTY stream or the entire composited multi-pane workspace (`Ctrl+X R`) into a standard `.cast` file replayable with `asciinema play`.

### Quick Start (`starter-packs/03-robot-scripting-and-qa/`)
```bash
./starter-packs/03-robot-scripting-and-qa/ci-harness.sh
```

---

## 4. Regular Terminal Operations & Power-User Shell Ergonomics

For everyday shell work, Bilterm eliminates the "scrolling output buried my prompt" problem:

- **Top-Docked, Bottom-Docked, Floating, or Split Prompt (`F4` / `Alt+E`)**: Keep your command prompt pinned at the top of the screen like an IBM 3270 mainframe terminal while `cargo build`, `docker compose logs -f`, or `find /` scrolls underneath.
- **Multi-Line Editing & Live Syntax Highlighting**: Write multi-line `for` loops, `jq` pipelines, or SQL queries with `Shift+Enter`, `Ctrl+Z` undo, `Ctrl+R` fuzzy history search, and `<Tab>` filesystem/`$ENV`/`:colon` completion before submitting to the PTY.
- **Command Block Navigation (`PageUp` / `PageDown` / `:block prev|next`)**: Jump directly between prior commands and their exit codes using OSC 133 semantic prompt markers.
- **Guake Drop-Down Quick Terminal (`F10` / `Alt+\`` / `Ctrl+Space`)**: Pull down a persistent scratchpad shell over your workspace at any time for quick `git status` or calculator checks, then hide it with one keystroke.

---

## 5. Developer & Live Terminal Debugging Workflows

When building or debugging CLI tools, ANSI escape emitters, curses/Ratatui apps, or shell integrations:

- **Live `<F9>` 3270 Terminal Inspector Modal (`:inspector`)**: Inspect real-time PTY line-discipline flags (`ECHO`, `ICANON`, `ISIG`), foreground `tcgetpgrp` vs shell PID, active `DECSET` modes (`?1049` alt screen, `?1006` SGR mouse, `?2004` bracketed paste, `?2026` synchronized output), cursor coordinates, and scrollback memory usage.
- **Zero-Mouse Copy Mode & QuickSelect Hints (`F3` / `Alt+F` / `Alt+C`)**: Press `F3` to overlay 2-letter home-row jump labels (`[aa]`, `[ab]`) over every URL, file path, Git SHA, and IPv4 address on screen—press the 2 letters to yank to system clipboard (`OSC 52`) or insert directly into the floating prompt.
- **Inline High-Resolution Canvas & Plots (`:canvas plot` / `bilterm plot`)**: Render live latency histograms, memory sparklines, or heatmaps directly inside any pane using Kitty graphics, Sixel, iTerm2 `OSC 1337`, or Unicode Sextant/Braille sub-cell rasterization.

---

## 6. Custom Layouts, Incident War Rooms & Starter Packs

During a production incident, screen real estate and accidental-command prevention are critical:

1. **Left Pane (`#1`, `120%` Zoomed Font)**: Primary diagnostic command console in readable 19pt font.
2. **Top-Right Pane (`#2`, `80%` Compact Font)**: `nmon` or `htop` running in `DECSET 1049` alternate screen mode, scaled down with `Ctrl+-` so more CPU cores, disks, and network interfaces fit inside the top-right quadrant.
3. **Bottom-Right Pane (`#3`, `100%` Font, `:lock on`)**: Live `journalctl -fu` or production replica psql console locked read-only (`🔒`) with `Ctrl+X l` (`:lock on`) so accidental keystrokes or `:broadcast` commands cannot mutate production state.

### Quick Start (`starter-packs/04-sysadmin-incident-war-room/`)
```bash
bilterm --gui -c starter-packs/04-sysadmin-incident-war-room/config.toml --gui-font "JetBrains Mono" --gui-font-size 16
```
