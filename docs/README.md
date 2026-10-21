# Bilterm Documentation Hub

[← Back to Project README](../README.md)

Welcome to the comprehensive documentation suite for **Bilterm** — the IBM 3270 Mainframe & DOS Borland Turbo Vision inspired TUI multiplexer, composite-pane shell wrapper, and terminal emulator written in Rust + Ratatui.

---

## Documentation Index

| Document | Purpose & Scope |
|----------|-----------------|
| **[`docs/purpose.md`](purpose.md)** | Problem statement, IBM 3270 + Borland Turbo Vision design philosophy, threat model, and security boundaries (`!ECHO LOCK`, `sanitize_stream_traps`, `occlude_rect`) |
| **[`docs/architecture.md`](architecture.md)** | Full subsystem breakdown, 3-pass Ratatui compositor, 3-signal line-discipline state machine, Tab completion engine, storage layer, and embedded Mermaid/ASCII diagrams |
| **[`docs/running.md`](running.md)** | Prerequisites, installation, **shell resolution (`$SHELL` / `%COMSPEC%`)**, **Tab command completion internals**, configuration schema, environment variables, and exit codes |
| **[`docs/how-to.md`](how-to.md)** | Step-by-step task recipes, CI/CD verification pipeline, diagnostic logging (`bilterm.log`), and troubleshooting matrix |
| **[`docs/user-guide/README.md`](user-guide/README.md)** | Complete 7-chapter interactive User Manual covering every keybinding, mouse gesture, and `:colon` command |
| **[`docs/diagrams/README.md`](diagrams/README.md)** | Multi-format visual diagram catalog (**ASCII** `.txt`, **Mermaid** `.mmd`, and **native draw.io** `.drawio` XML) |
| **[`docs/architecture/OVERVIEW.md`](architecture/OVERVIEW.md)** | Architecture Decision Records (`ADR-001` through `ADR-005`) |
| **[`docs/TEST_COVERAGE.md`](TEST_COVERAGE.md)** | Test coverage report (`81.83%` line coverage) and terminal torture/stress test suite documentation |
| **[`CHANGELOG.md`](../CHANGELOG.md)** | Semantic versioning release history |

---

## Quick Navigation by Topic

- **Which shell does Bilterm run, and how does `<Tab>` completion work?** -> See **[`docs/running.md#shells--tab-command-completions`](running.md#shells--tab-command-completions)**
- **How do I switch between a Floating Command Prompt and a Non-Overlapping Split Command/Output Pane?** -> See **[`docs/how-to.md#recipe-2-non-overlapping-split-command--output-panes`](how-to.md#recipe-2-non-overlapping-split-command--output-panes)**
- **How do I enable Full-Screen IBM 3270 Cross-Hairs or customize DOS Drop Shadows & Border Outlines?** -> See **[`docs/user-guide/04-themes-glyphs-and-config.md`](user-guide/04-themes-glyphs-and-config.md)**
- **Where are visual architecture diagrams (`ASCII`, `Mermaid`, `draw.io`)?** -> See **[`docs/diagrams/README.md`](diagrams/README.md)**
