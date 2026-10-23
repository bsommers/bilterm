# Bilterm Documentation Hub

[← Back to Project README](../README.md)

Welcome to the comprehensive documentation suite for **Bilterm** — the IBM 3270 Mainframe & DOS Borland Turbo Vision inspired TUI multiplexer, composite-pane shell wrapper, and terminal emulator written in Rust + Ratatui.

---

## Documentation Index

| Document | Purpose & Scope |
|----------|-----------------|
| **[`docs/purpose.md`](purpose.md)** | Problem statement, IBM 3270 + Borland Turbo Vision design philosophy, threat model, and security boundaries (`!ECHO LOCK`, `sanitize_stream_traps`, `occlude_rect`) |
| **[`docs/architecture.md`](architecture.md)** | Full subsystem breakdown (`v1.4.0`), 3-pass Ratatui compositor, 3-signal line-discipline state machine, SSH/Fleet Federation, JSON-RPC 2.0 / MCP Automation Plane, and Mermaid/ASCII diagrams |
| **[`docs/running.md`](running.md)** | Prerequisites, installation, **CLI & Automation (`--mcp`, `--headless`, `bilterm ctl`, `--playbook`)**, **shell resolution (`$SHELL` / `%COMSPEC%`)**, **Tab command completion internals**, `config.toml` (`[remotes.<name>]`), and exit codes |
| **[`docs/how-to.md`](how-to.md)** | Step-by-step task recipes (including SSH domains, `herdr` fleet management, and JSON-RPC/MCP automation), CI/CD verification pipeline, and troubleshooting matrix |
| **[`docs/user-guide/README.md`](user-guide/README.md)** | Complete 7-chapter interactive User Manual with embedded workspace screenshots covering every keybinding, mouse gesture, and `:colon` command |
| **[`docs/diagrams/README.md`](diagrams/README.md)** | Multi-format visual diagram catalog (**ASCII** `.txt`, **Mermaid** `.mmd`, and **native draw.io** `.drawio` XML) |
| **[`docs/architecture/OVERVIEW.md`](architecture/OVERVIEW.md)** | Architecture Decision Records (`ADR-001` through `ADR-005`) |
| **[`docs/TEST_COVERAGE.md`](TEST_COVERAGE.md)** | Test coverage report (**`83.45%` line coverage**, 86 passing tests across 14 suites) and battle/stress/performance benchmarks |
| **[`docs/launch-presentation-and-pitch-script.md`](launch-presentation-and-pitch-script.md)** | NotebookLM / Gemini prompt script: 12-slide technical deck specification and 5-minute product launch video script |
| **[`CHANGELOG.md`](../CHANGELOG.md)** | Semantic versioning release history (`v0.1.0` through `v1.4.0`) |

---

## Quick Navigation by Topic

- **How do I connect to remote SSH sessions (`:ssh`), persistent remote `tmux -CC` (`:ssh-tmux`), or federated `herdr`-style fleets (`:remote`)?** -> See **[`docs/how-to.md#recipe-8-remote-ssh-domains--herdr-style-fleet-federation-v140`](how-to.md#recipe-8-remote-ssh-domains--herdr-style-fleet-federation-v140)**
- **How do I drive Bilterm via JSON-RPC 2.0 (`bilterm ctl`), MCP (`bilterm --mcp`), or TOML Playbooks (`--playbook`)?** -> See **[`docs/how-to.md#recipe-9-json-rpc-20-automation-api-mcp-server--playbooks-v130`](how-to.md#recipe-9-json-rpc-20-automation-api-mcp-server--playbooks-v130)**
- **Which shell does Bilterm run, and how does `<Tab>` completion work?** -> See **[`docs/running.md#2-shells--tab-command-completions`](running.md#2-shells--tab-command-completions)**
- **How do I switch between a Floating Command Prompt and a Non-Overlapping Split Command/Output Pane?** -> See **[`docs/how-to.md#recipe-2-non-overlapping-split-command--output-panes`](how-to.md#recipe-2-non-overlapping-split-command--output-panes)**
- **How do I enable Full-Screen IBM 3270 Cross-Hairs or customize DOS Drop Shadows & Border Outlines?** -> See **[`docs/user-guide/04-themes-glyphs-and-config.md`](user-guide/04-themes-glyphs-and-config.md)**
- **Where are visual architecture diagrams (`ASCII`, `Mermaid`, `drawio`)?** -> See **[`docs/diagrams/README.md`](diagrams/README.md)**
