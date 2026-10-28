# Bilterm Documentation Hub (`v1.12.0`)

[← Back to Project README](../README.md) | [Starter Packs (`starter-packs/`)](../starter-packs/README.md)

Welcome to the comprehensive documentation suite for **Bilterm (`v1.12.0`)** — the IBM 3270 Mainframe & DOS Borland Turbo Vision inspired TUI multiplexer, native desktop GUI terminal (`--gui`), composite-pane shell wrapper, remote SSH fleet manager, and AI-automatable terminal emulator written in Rust + Ratatui.

---

## Documentation Index

| Document | Purpose & Scope |
|----------|-----------------|
| **[`docs/user-guide/README.md`](user-guide/README.md)** | **Complete 10-Chapter Interactive User Manual** with embedded workspace screen renders covering every CLI flag, keybinding, mouse gesture, `:colon` command, use case, and security best practice |
| **[`docs/user-guide/08-extended-cli-and-gui-examples.md`](user-guide/08-extended-cli-and-gui-examples.md)** | **Extended CLI, Native `--gui` & Typography Cookbook** — `bilterm --gui --gui-font "..." --gui-font-size 16`, per-pane `Ctrl++`/`Ctrl+-` font scaling, `--headless`, `bilterm ctl`, `--mcp`, `bilterm run`, and `bilterm plot` |
| **[`docs/user-guide/09-use-cases-and-workflows.md`](user-guide/09-use-cases-and-workflows.md)** | **Real-World Use Cases & Workflows** — Multi-Agent AI Coding Swarms, Remote Agents, Remote Development (`:ssh-tmux`), Robot Scripting & QA, Incident War Rooms, and Custom Per-Pane Font Layouts |
| **[`docs/user-guide/10-security-and-best-practices.md`](user-guide/10-security-and-best-practices.md)** | **Security Architecture & Operational Best Practices** — 3-Signal `!ECHO LOCK`, `/dev/random` escape-trap sanitization, paste-injection guards, `0700`/`0600` confinement, Unix socket `SO_PEERCRED`, and bearer-token RPC auth |
| **[`starter-packs/README.md`](../starter-packs/README.md)** | **5 Ready-to-Run Starter Packs** — Multi-Agent Swarm, Remote Dev & Fleet, Robot QA Harness, Sysadmin Incident War Room, and Retro CRT Workstation profiles |
| **[`docs/DEVELOPER_AND_AGENT_GUIDE.md`](DEVELOPER_AND_AGENT_GUIDE.md)** | **Developer, Maintainer & AI Agent Engineering Guide** — complete crate dependency stack, modular source tree (`<= 800` lines gate), `Action` pipeline, encapsulated invariants, live `<F9>` Inspector / `bilterm.log` / `bilterm ctl` debugging playbook |
| **[`docs/AGENT_AUTONOMY_AND_PERMISSIONS.md`](AGENT_AUTONOMY_AND_PERMISSIONS.md)** | **Agent Autonomy & Super-Permissive Configuration Guide (`Linux` & `macOS`)** — drop-in `settings.json` / `.claude/settings.local.json` / `.planning/config.json` templates distilled from `770+` session approvals for zero-prompt autonomous execution across Antigravity (`agy` / 2.0), Claude Code, and OpenGSD |
| **[`docs/LESSONS_LEARNED_COMPOUNDED.md`](LESSONS_LEARNED_COMPOUNDED.md)** | **Compounded Engineering & Architecture Lessons Learned (`macOS × Linux × v2.0`)** — Unified 10-domain synthesis matrix combining the macOS and Linux retrospectives with forward architectural guardrails for Milestone `v2.0` (`Phases 30–34`) |
| **[`docs/LESSONS_LEARNED.md`](LESSONS_LEARNED.md)** | **Engineering & Architecture Lessons Learned (`v0.1` → `v2.0`, Sourced from macOS)** — macOS `Option` dead-key normalization, Powerline/PUA geometric glyphs (`LastResort` prevention), dual-plane selection & occlusion, smooth vs. mainframe `BlockPage` (`MORE...`/`HOLDING`) scrolling, persistent `herdr` drawer, 3-signal `!ECHO LOCK`, and test determinism |
| **[`docs/LESSONS_LEARNED_LINUX.md`](LESSONS_LEARNED_LINUX.md)** | **Engineering Lessons Learned (Sourced from Linux)** — `Ord::clamp` viewport inversion guards, per-pane font scaling + `TIOCSWINSZ`, `RawPtyPassthrough` WM chord interception (`nmon`/`htop`/`vim`), all-border/corner mouse resizing, and zero-header X11/Xft `dlopen` native GUI |
| **[`docs/purpose.md`](purpose.md)** | Problem statement, IBM 3270 + Borland Turbo Vision design philosophy, threat model, and security boundaries (`!ECHO LOCK`, `sanitize_stream_traps`, `occlude_rect`) |
| **[`docs/architecture.md`](architecture.md)** | Full subsystem breakdown, 3-pass Ratatui compositor, 3-signal line-discipline state machine, SSH/Fleet Federation, JSON-RPC 2.0 / MCP Automation Plane, `WorkspaceRecorder`, and Mermaid/ASCII diagrams |
| **[`docs/running.md`](running.md)** | Prerequisites, installation, **CLI & Automation (`--gui`, `--mcp`, `--headless`, `bilterm ctl`, `bilterm run`, `bilterm plot`)**, **shell resolution (`$SHELL` / `%COMSPEC%`)**, **Tab command completion internals**, `config.toml`, and exit codes |
| **[`docs/how-to.md`](how-to.md)** | Step-by-step task recipes (including `--gui` typography, per-pane font scaling, all-border mouse resizing, whole-workspace `.cast` recording, SSH domains, `herdr` fleet management, and JSON-RPC/MCP automation) and troubleshooting matrix |
| **[`docs/diagrams/README.md`](diagrams/README.md)** | Multi-format visual diagram catalog (**ASCII** `.txt`, **Mermaid** `.mmd`, and **native draw.io** `.drawio` XML) |
| **[`docs/architecture/OVERVIEW.md`](architecture/OVERVIEW.md)** | Architecture Decision Records (`ADR-001` through `ADR-007`) |
| **[`docs/TEST_COVERAGE.md`](TEST_COVERAGE.md)** | Test coverage report (**`83.45%` line coverage**, 240+ passing tests) and battle/stress/performance benchmarks |
| **[`docs/launch-presentation-and-pitch-script.md`](launch-presentation-and-pitch-script.md)** | NotebookLM / Gemini prompt script: 12-slide technical deck specification and 5-minute product launch video script |
| **[`CHANGELOG.md`](../CHANGELOG.md)** | Semantic versioning release history (`v0.1.0` through `v1.12.0`) |

---

## Quick Navigation by Topic

- **How do I launch the Native Desktop GUI (`--gui`) with custom fonts or scale fonts per-pane (`Ctrl++`/`Ctrl+-`)?** -> See **[Chapter 8: Extended CLI & GUI Examples](user-guide/08-extended-cli-and-gui-examples.md)**
- **How can I use Bilterm for Multi-Agent Swarms, Remote Development, Robot Scripting, or Incident War Rooms?** -> See **[Chapter 9: Real-World Use Cases & Workflows](user-guide/09-use-cases-and-workflows.md)** and **[`starter-packs/`](../starter-packs/README.md)**
- **How does Bilterm protect passwords (`!ECHO LOCK`), prevent paste injection, and secure its RPC/MCP sockets?** -> See **[Chapter 10: Security Architecture & Operational Best Practices](user-guide/10-security-and-best-practices.md)**
- **Where is the complete table of ALL CLI flags, keyboard shortcuts, mouse gestures, and `:colon` commands?** -> See **[Chapter 6: Complete CLI, Keyboard, Mouse & `:command` Reference](user-guide/06-keyboard-mouse-and-command-reference.md)**
- **How do I see High-Resolution Graphics, Live Plots, or Tektronix 4014 Vectors in SSH & GUI modes (`:canvas hires`, `bilterm plot`)?** -> See **[`docs/how-to.md#recipe-11-high-resolution-graphics-live-plotting--adaptive-sshgui-canvas-v191`](how-to.md#recipe-11-high-resolution-graphics-live-plotting--adaptive-sshgui-canvas-v191)**
- **How do I connect to remote SSH sessions (`:ssh`), persistent remote `tmux -CC` (`:ssh-tmux`), or federated `herdr`-style fleets (`:remote`)?** -> See **[`docs/how-to.md#recipe-8-remote-ssh-domains--herdr-style-fleet-federation-v140`](how-to.md#recipe-8-remote-ssh-domains--herdr-style-fleet-federation-v140)**
- **How do I drive Bilterm via JSON-RPC 2.0 (`bilterm ctl`), MCP (`bilterm --mcp`), or TOML Playbooks (`bilterm run`)?** -> See **[`docs/how-to.md#recipe-9-json-rpc-20-automation-api-mcp-server--playbooks-v130v160`](how-to.md#recipe-9-json-rpc-20-automation-api-mcp-server--playbooks-v130v160)**
